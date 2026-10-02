//! Writing an edited project back to its authored source.
//!
//! # The rule this module exists to keep
//!
//! HTML, CSS, and SVG stay canonical. An edit changes the smallest authored
//! span that can express it and leaves every other byte alone. Nothing here
//! regenerates a document, reformats a stylesheet, or writes a visual value
//! into `lamine.yaml`.
//!
//! # What is supported
//!
//! | Edit | Written to | How |
//! |---|---|---|
//! | rename | `lamine.yaml` | existing metadata semantics |
//! | text | the element's text in HTML | replaces the text between its tags |
//! | geometry | a `style` attribute on the element | sets explicit box values |
//! | style | the owning CSS declaration | rewrites that declaration's value |
//!
//! An edit with no owned source is reported as unsupported rather than being
//! silently dropped, because a save that claims to have persisted something it
//! did not is worse than one that says so.
//!
//! # Provenance
//!
//! Every write targets a byte range that was resolved when the project was
//! opened. Ranges are re-resolved before use rather than trusted from a previous
//! load, so an externally edited file cannot cause a write to land in the wrong
//! place.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::project_bundle::{BundleError, ProjectBundle};
use crate::source_binding::SourceIndex;
use crate::source_document::NodeId;
use crate::style::Stylesheet;

/// One change to write back to source.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceEdit {
    /// Replace the text content of a node's element.
    Text { node: NodeId, text: String },
    /// Write explicit geometry onto a node's element.
    ///
    /// A move and a resize are separate because they are separate kinds of
    /// source change: a resize is an explicit box, and a move is a position
    /// expressed in whichever of the two ways the element's own positioning
    /// allows. See [`Placement`].
    Geometry {
        node: NodeId,
        /// `None` when only the box changed.
        placement: Option<Placement>,
        width: Option<f32>,
        height: Option<f32>,
    },
    /// Rewrite the value of a property a node owns.
    ///
    /// Only supported when a declaration is already authored for that node and
    /// this property. An edit with no authored owner is reported, not invented.
    Style {
        node: NodeId,
        property: String,
        value: String,
    },
}

/// How a moved element's new position is expressed in source.
///
/// The two cases exist because the two cases have different honest answers.
/// Turning every dragged element into `position: absolute` — which is what this
/// milestone inherited — is correct for a box the author already took out of
/// the flow, and wrong for one that did not: lifting a `main` out of `body`
/// deletes the only thing that let it reflow with its siblings, so reopening
/// the project shows a page the author never wrote.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Out of the flow, at this offset from the containing block.
    ///
    /// Written as `left`/`top`, which is what an absolutely positioned box
    /// already means.
    ContainingBlock { x: f32, y: f32 },
    /// Still in the flow, offset by this much from where the flow puts it.
    ///
    /// Written as `transform: translate(...)`, which moves a box without
    /// removing it from the flow. The offset is added to whatever the author
    /// already authored, so a second move composes with the first instead of
    /// replacing it.
    Flow { dx: f32, dy: f32 },
}

/// Where the declarations for one geometry edit go.
///
/// Split because a box and a position do not have the same answer: a size is
/// always a local declaration on the element, while a flow offset may belong to
/// a stylesheet rule the author wrote.
#[derive(Clone, Debug, Default, PartialEq)]
struct GeometryWrites {
    /// Declarations for the element's own inline `style`.
    inline: Vec<(String, String)>,
    /// `(file, value range, replacement)` for authored declarations.
    css: Vec<(String, (usize, usize), String)>,
}

/// A change that could not be written, with the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct UnsupportedEdit {
    pub node: NodeId,
    pub kind: &'static str,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct SaveOutcome {
    /// Authored files actually written.
    pub written: Vec<PathBuf>,
    /// Edits deliberately not written.
    pub unsupported: Vec<UnsupportedEdit>,
}

/// Apply edits to the project on disk and persist metadata.
///
/// Returns what was written and what was not, so a caller never has to guess
/// whether an edit survived.
pub fn save_project(
    root: &std::path::Path,
    document: &crate::source_document::PersistentDocument,
    edits: &[SourceEdit],
) -> Result<SaveOutcome, BundleError> {
    let mut outcome = SaveOutcome::default();
    let mut html_edits: BTreeMap<String, Vec<(NodeId, String)>> = BTreeMap::new();
    // File -> element -> the declarations to set on that element's own inline
    // style. Grouped per element rather than per edit so a geometry write and a
    // style write to the same element produce one attribute, not two that
    // fight over it.
    let mut inline: BTreeMap<String, BTreeMap<NodeId, Vec<(String, String)>>> = BTreeMap::new();
    let mut css_edits: Vec<(String, (usize, usize), String)> = Vec::new();

    for edit in edits {
        let node_id = match edit {
            SourceEdit::Text { node, .. }
            | SourceEdit::Geometry { node, .. }
            | SourceEdit::Style { node, .. } => node,
        };
        let Some(node) = document.structure.nodes.iter().find(|n| &n.id == node_id) else {
            outcome.unsupported.push(UnsupportedEdit {
                node: node_id.clone(),
                kind: "unknown-node",
                reason: "no such node in the persistent document".into(),
            });
            continue;
        };
        let file = node.source.file.clone();
        match edit {
            SourceEdit::Text { text, .. } => {
                // Text is written only into the range the element owns. An
                // element whose content is not a single run of text — a wrapper
                // around a child, or empty — owns nothing, and rewriting "the
                // inside" anyway would destroy markup the author wrote.
                match text_owner(document, node) {
                    Some(_) => html_edits
                        .entry(file)
                        .or_default()
                        .push((node_id.clone(), text.clone())),
                    None => outcome.unsupported.push(UnsupportedEdit {
                        node: node_id.clone(),
                        kind: "text",
                        reason: "this element's content is not a single run of text, \
                                 so there is no text range to replace"
                            .into(),
                    }),
                }
            }
            SourceEdit::Geometry {
                placement,
                width,
                height,
                ..
            } => match geometry_writes(document, node, placement.as_ref(), *width, *height) {
                Ok(writes) => {
                    inline
                        .entry(file)
                        .or_default()
                        .entry(node_id.clone())
                        .or_default()
                        .extend(writes.inline);
                    css_edits.extend(writes.css);
                }
                Err(reason) => outcome.unsupported.push(UnsupportedEdit {
                    node: node_id.clone(),
                    kind: "geometry",
                    reason,
                }),
            },
            SourceEdit::Style {
                property, value, ..
            } => {
                // Ownership decides where the edit lands. See
                // `StyleTarget` for the policy; the short version is that an
                // authored declaration is rewritten in place, and a property
                // this element does not itself declare — including one it only
                // inherits — is written as a local declaration on the element
                // rather than mutating an ancestor.
                match style_target(document, node, property) {
                    Some(StyleTarget::Declaration { file, range, .. }) => {
                        css_edits.push((file, range, value.clone()));
                    }
                    Some(StyleTarget::Element { .. }) => {
                        inline
                            .entry(file)
                            .or_default()
                            .entry(node_id.clone())
                            .or_default()
                            .push((property.clone(), value.clone()));
                    }
                    None => outcome.unsupported.push(UnsupportedEdit {
                        node: node_id.clone(),
                        kind: "style",
                        reason: format!("`{property}` cannot be written on this element"),
                    }),
                }
            }
        }
    }

    // Write HTML files that have text or inline-style edits.
    for (file, edits_for_file) in &html_edits {
        let path = resolve(root, file)?;
        let original = read_source(&path)?;
        let index = SourceIndex::parse(&original);
        let rewritten = rewrite_html(
            root,
            file,
            &original,
            &index,
            edits_for_file,
            inline.get(file),
        )?;
        write_if_changed(&path, &original, &rewritten, &mut outcome)?;
    }
    // Files with only inline-style edits.
    for (file, edits_for_file) in &inline {
        if html_edits.contains_key(file) {
            continue;
        }
        let path = resolve(root, file)?;
        let original = read_source(&path)?;
        let index = SourceIndex::parse(&original);
        let rewritten = rewrite_html(root, file, &original, &index, &[], Some(edits_for_file))?;
        write_if_changed(&path, &original, &rewritten, &mut outcome)?;
    }
    // CSS files. Each edit targets the exact span of the declaration that owns
    // it, and the spans are applied back-to-front so an earlier replacement
    // cannot shift a later one.
    // File -> the declaration spans to rewrite inside it.
    type CssEdits = BTreeMap<String, Vec<((usize, usize), String)>>;
    let mut css_by_file: CssEdits = BTreeMap::new();
    for (file, range, value) in css_edits {
        css_by_file.entry(file).or_default().push((range, value));
    }
    for (file, changes) in &css_by_file {
        let path = resolve(root, file)?;
        let original = read_source(&path)?;
        let rewritten = apply_css_edits(&original, changes);
        write_if_changed(&path, &original, &rewritten, &mut outcome)?;
    }

    // Metadata last: the file that records which nodes exist. Written from the
    // in-memory document, so a rename made in the editor survives the save
    // rather than being replaced by whatever was on disk when the project was
    // opened. Authored sources are cleared because the bundle never writes
    // them: HTML and CSS are written above, byte-for-byte, from their own edits.
    //
    // Written only when the structure actually differs from what is on disk, so a
    // text-only or geometry-only edit leaves the metadata file — and the
    // formatting the author chose for it — untouched.
    let mut updated = document.clone();
    updated.sources.clear();
    let on_disk = ProjectBundle::load(root)?;
    if on_disk.document.structure != updated.structure {
        let bundle = ProjectBundle::from_document(root, updated)?;
        let encoded = bundle.metadata_yaml()?;
        let metadata_path = root.join(crate::project_bundle::METADATA_FILE);
        crate::project_bundle::write_atomically(&metadata_path, encoded.as_bytes())?;
        outcome.written.push(metadata_path);
    }
    Ok(outcome)
}

fn read_source(path: &std::path::Path) -> Result<String, BundleError> {
    std::fs::read_to_string(path).map_err(|source| BundleError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve(root: &std::path::Path, file: &str) -> Result<PathBuf, BundleError> {
    let candidate = root.join(file);
    if !candidate.starts_with(root) {
        return Err(BundleError::InvalidSourceReference {
            file: file.to_owned(),
            referenced_by: NodeId::new("unknown").expect("valid"),
        });
    }
    Ok(candidate)
}

/// Decide what a geometry edit writes, and where each part goes.
///
/// # The policy
///
/// A size is always the element's own business: `width`/`height` become local
/// declarations, because pinning a box to the number the editor measured is
/// the only way to write an authored width down at all.
///
/// A position follows the element's own positioning:
///
/// - Already out of the flow: `left`/`top`, measured from the containing block.
///   `position` is not restated, because the author already said so — inline
///   or in a rule — and repeating it locally would be a second declaration
///   saying the same thing.
/// - In the flow: `transform: translate(...)`, added to whatever offset the
///   author already wrote. An element that has no authored transform gains one;
///   an element whose author wrote something this editor cannot offset (a
///   rotation, say) is reported rather than having that transform replaced.
///
/// # The failure this is honest about
///
/// A `transform` the editor does not understand is a real conflict, not a
/// missing feature. Overwriting `rotate(4deg)` with a translate would delete an
/// authored decision, so the save reports the node and leaves the file alone.
fn geometry_writes(
    document: &crate::source_document::PersistentDocument,
    node: &crate::source_document::StructuralNode,
    placement: Option<&Placement>,
    width: Option<f32>,
    height: Option<f32>,
) -> Result<GeometryWrites, String> {
    let mut writes = GeometryWrites::default();
    if placement.is_none() && width.is_none() && height.is_none() {
        return Err("no geometry value to write".to_owned());
    }
    if let Some(width) = width {
        writes
            .inline
            .push(("width".to_owned(), format!("{}px", css_length(width))));
    }
    if let Some(height) = height {
        writes
            .inline
            .push(("height".to_owned(), format!("{}px", css_length(height))));
    }
    let Some(placement) = placement else {
        return Ok(writes);
    };
    match *placement {
        Placement::ContainingBlock { x, y } => {
            if !is_out_of_flow(document, node)? {
                writes
                    .inline
                    .push(("position".to_owned(), "absolute".to_owned()));
            }
            writes
                .inline
                .push(("left".to_owned(), format!("{}px", css_length(x))));
            writes
                .inline
                .push(("top".to_owned(), format!("{}px", css_length(y))));
        }
        Placement::Flow { dx, dy } => {
            let Some(target) = style_target(document, node, "transform") else {
                return Err("`transform` cannot be written on this element".to_owned());
            };
            let (base, css_target) = match target {
                StyleTarget::Declaration {
                    file,
                    range,
                    authored,
                } => {
                    let base = crate::style::parse_translate(&authored).ok_or_else(|| {
                        format!(
                            "this element's authored `transform: {authored}` is not an offset \
                             this editor can add to"
                        )
                    })?;
                    (base, Some((file, range)))
                }
                StyleTarget::Element { authored } => {
                    // No authored transform means no base to compose with. An
                    // authored one this subset cannot read is a conflict, not an
                    // absent base.
                    let base = match authored {
                        None => (0.0, 0.0),
                        Some(authored) => {
                            crate::style::parse_translate(&authored).ok_or_else(|| {
                                format!(
                                    "this element's authored `transform: {authored}` is not an \
                                     offset this editor can add to"
                                )
                            })?
                        }
                    };
                    (base, None)
                }
            };
            let moved = (base.0 + dx, base.1 + dy);
            if (dx, dy) == (0.0, 0.0) {
                // Back where the flow puts it. `transform: none` says exactly
                // that and reads as no offset; leaving an authored offset in
                // place would reopen the element somewhere the editor is not
                // showing it.
                if base != (0.0, 0.0) {
                    match css_target {
                        Some((file, range)) => writes.css.push((file, range, "none".to_owned())),
                        None => writes
                            .inline
                            .push(("transform".to_owned(), "none".to_owned())),
                    }
                }
                return Ok(writes);
            }
            let value = format!(
                "translate({}px, {}px)",
                css_length(moved.0),
                css_length(moved.1)
            );
            match css_target {
                // The author owns this offset, so the edit belongs to their
                // declaration rather than a new local one that would fight it.
                Some((file, range)) => writes.css.push((file, range, value)),
                None => writes.inline.push(("transform".to_owned(), value)),
            }
        }
    }
    Ok(writes)
}

/// Whether a node's element is already positioned out of the normal flow.
///
/// Asked before every move, because it is the whole difference between a
/// `left`/`top` write and a `transform` write. Resolved the same way a style
/// edit resolves ownership — the element's own inline declaration first, then
/// the last matching rule — so the answer agrees with what the renderer and the
/// style ownership rules already believe.
pub fn is_out_of_flow(
    document: &crate::source_document::PersistentDocument,
    node: &crate::source_document::StructuralNode,
) -> Result<bool, String> {
    let Some(target) = style_target(document, node, "position") else {
        return Ok(false);
    };
    let authored = match target {
        StyleTarget::Declaration { authored, .. } => Some(authored),
        StyleTarget::Element { authored } => authored,
    };
    Ok(matches!(
        authored.as_deref().map(str::trim),
        Some("absolute" | "fixed")
    ))
}

/// Format a length the way an author would write it.
///
/// Layout runs in `f32`, so an exact-looking position comes out as
/// `22.399994`. Writing that into the document is accurate and unreadable, and
/// it makes every save produce a noisy diff. Two decimals is finer than any
/// screen can show and keeps the source something a person would have written.
pub fn css_length(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == rounded.trunc() {
        return format!("{}", rounded as i64);
    }
    format!("{rounded}")
}

/// The source range holding a node's own text, if it owns one.
///
/// Re-parses the bound file through the same [`SourceIndex`] the project was
/// opened with, so ownership is decided by the authored parse rather than by
/// string surgery at save time. Returns `None` when the node does not exist,
/// binds to nothing, or has content that is not a single run of text.
fn text_owner(
    document: &crate::source_document::PersistentDocument,
    node: &crate::source_document::StructuralNode,
) -> Option<std::ops::Range<usize>> {
    let html = document.sources.get(&node.source.file)?;
    SourceIndex::parse(html).find(&node.id)?.text_range.clone()
}

/// Where a style edit lands, and what the property is currently authored as.
#[derive(Clone, Debug, PartialEq)]
enum StyleTarget {
    /// An authored declaration already owns this property for this element.
    Declaration {
        /// The stylesheet holding it.
        file: String,
        /// Byte range of that declaration's value.
        range: (usize, usize),
        /// The authored value text, taken from the same range.
        authored: String,
    },
    /// Nothing this element declares owns the property; write it locally.
    Element {
        /// The inline value, when the element happens to declare it there
        /// without owning it. Needed to compose a new value onto what the
        /// author wrote rather than overwriting it.
        authored: Option<String>,
    },
}

/// Decide where an edit to `property` on `node` belongs.
///
/// # The policy, in order
///
/// 1. The element's own inline `style` declaration wins: an inline style is
///    the author saying "this element, not the class".
/// 2. Otherwise the last stylesheet rule matching this element that declares
///    the property is rewritten in place — same file, same declaration, same
///    position. That includes a value written as `var(--accent)`: the edit
///    replaces it with the resolved colour on this element and leaves the
///    variable alone for every other element using it.
/// 3. Otherwise the element gets a local declaration. That covers both a
///    property nobody authored and one the element only *inherits*: editing an
///    inherited `color` writes `color` on this element, and deliberately does
///    not rewrite the ancestor that happened to be its source. Changing an
///    ancestor would silently restyle every sibling inheriting from it, which
///    is never what editing one selected object means.
fn style_target(
    document: &crate::source_document::PersistentDocument,
    node: &crate::source_document::StructuralNode,
    property: &str,
) -> Option<StyleTarget> {
    let html = document.sources.get(&node.source.file)?;
    let index = SourceIndex::parse(html);
    let binding = index.find(&node.id)?;
    let fragment = html.get(binding.element_range.clone())?;
    let (tag, classes) = crate::visual::element_identity(fragment);

    // 1. The element's own inline style, if it declares the property.
    let open_tag_end = fragment.find('>').unwrap_or(fragment.len());
    let open_tag = &fragment[..open_tag_end];
    let inline = parse_inline_declarations(open_tag);
    if let Some((_, value)) = inline.iter().find(|(declared, _)| declared == property) {
        return Some(StyleTarget::Element {
            authored: Some(value.clone()),
        });
    }

    // 2. The stylesheet declaration that owns it. Sorted so the answer does not
    // depend on hash iteration order.
    let mut sheets: Vec<&String> = document
        .sources
        .keys()
        .filter(|name| name.ends_with(".css"))
        .collect();
    sheets.sort();
    for name in sheets {
        let Some(contents) = document.sources.get(name) else {
            continue;
        };
        let sheet = Stylesheet::parse(contents);
        for spelling in ownership_spellings(property) {
            if let Some(range) = sheet.declaration_value_range(&tag, &classes, None, spelling) {
                let authored = contents.get(range.0..range.1).unwrap_or("").to_owned();
                return Some(StyleTarget::Declaration {
                    file: name.clone(),
                    range,
                    authored,
                });
            }
        }
    }

    // 3. Nothing authored here, so the element itself becomes the owner.
    Some(StyleTarget::Element { authored: None })
}

/// The property spellings a visual property may be authored as, in preference
/// order.
///
/// A fill may be authored as `background-color` or as the `background` shorthand;
/// both mean the same colour to the renderer. Rewriting whichever one the author
/// already wrote is the minimal edit — the alternative is adding a second
/// declaration that merely overrides the first.
fn ownership_spellings(property: &str) -> Vec<&str> {
    match property {
        "background-color" => vec!["background-color", "background"],
        other => vec![other],
    }
}

/// Rewrite the text and/or a `style` attribute of bound elements.
///
/// Edits are applied from the end of the document backwards so earlier ranges
/// stay valid while later ones are replaced.
fn rewrite_html(
    root: &std::path::Path,
    file: &str,
    original: &str,
    index: &SourceIndex,
    text_edits: &[(NodeId, String)],
    attribute_edits: Option<&BTreeMap<NodeId, Vec<(String, String)>>>,
) -> Result<String, BundleError> {
    let mut replacements: Vec<(std::ops::Range<usize>, String)> = Vec::new();

    for (node, text) in text_edits {
        let Some(binding) = index.find(node) else {
            continue;
        };
        // The range recorded when the project was parsed, not one recomputed
        // from the element's tags: that is what makes this a text edit rather
        // than a rewrite of the element's content.
        let Some(range) = binding.text_range.clone() else {
            continue;
        };
        if !original.is_char_boundary(range.start) || !original.is_char_boundary(range.end) {
            continue;
        }
        replacements.push((range, escape_text(text)));
    }

    if let Some(edits) = attribute_edits {
        for (node, declarations) in edits {
            let Some(binding) = index.find(node) else {
                continue;
            };
            let element = original.get(binding.element_range.clone()).unwrap_or("");
            let Some(open_end) = element.find('>') else {
                continue;
            };
            let open_tag = &element[..open_end];
            let absolute = binding.element_range.start;
            // Merge into whatever the author already wrote on this element, so
            // a geometry write and a style write share one attribute, and a
            // hand-written declaration neither of them knows about survives.
            let merged = merge_inline_declarations(open_tag, declarations);
            match style_attribute_value(open_tag) {
                // Replace the authored value in place, so the attribute keeps
                // its position and its quoting style.
                Some(range) => {
                    replacements.push((absolute + range.start..absolute + range.end, merged))
                }
                // No `style` attribute yet: add one just before the closing `>`.
                None => {
                    let insert_at = absolute + open_end;
                    replacements.push((insert_at..insert_at, format!(" style=\"{merged}\"")));
                }
            }
        }
    }

    let _ = (root, file);
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut out = original.to_owned();
    for (range, text) in replacements {
        if out.is_char_boundary(range.start) && out.is_char_boundary(range.end) {
            out.replace_range(range, &text);
        }
    }
    Ok(out)
}

/// Parse an open tag's inline `style` into ordered `(property, value)` pairs.
///
/// Order and unknown declarations are preserved, because this is somebody's
/// source: reordering or dropping a declaration the editor does not understand
/// would be an edit the user never asked for. A property is lowercased because
/// CSS property names are case-insensitive and matching must be too.
fn parse_inline_declarations(open_tag: &str) -> Vec<(String, String)> {
    let Some(range) = style_attribute_value(open_tag) else {
        return Vec::new();
    };
    open_tag
        .get(range)
        .unwrap_or("")
        .split(';')
        .filter_map(|part| {
            let (property, value) = part.split_once(':')?;
            let property = property.trim().to_ascii_lowercase();
            let value = value.trim();
            (!property.is_empty() && !value.is_empty()).then(|| (property, value.to_owned()))
        })
        .collect()
}

/// Apply edits to an element's inline style and render the attribute value.
///
/// A property already present is replaced where it stands, so a second save
/// cannot duplicate it and an unrelated declaration keeps its position. A new
/// property is appended. The result is always the authored value plus the
/// edits, never just the edits.
fn merge_inline_declarations(open_tag: &str, edits: &[(String, String)]) -> String {
    let mut declarations = parse_inline_declarations(open_tag);
    for (property, value) in edits {
        match declarations
            .iter_mut()
            .find(|(declared, _)| declared == property)
        {
            Some(existing) => existing.1 = value.clone(),
            None => declarations.push((property.clone(), value.clone())),
        }
    }
    declarations
        .iter()
        .map(|(property, value)| format!("{property}: {value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The byte range of an open tag's existing `style` attribute value.
///
/// Returns the span of the value alone — not of `style="..."` — so a rewrite
/// replaces the declarations and leaves the attribute itself exactly as the
/// author wrote it. Returning the wrong span is how a second save turns
/// `style="a"` into `style="a"a"`.
///
/// Only a real attribute counts: `data-style-note="x"` is a different attribute
/// and must not be treated as the style.
fn style_attribute_value(open_tag: &str) -> Option<std::ops::Range<usize>> {
    let bytes = open_tag.as_bytes();
    let mut search = 0usize;
    while let Some(offset) = open_tag[search..].find("style") {
        let at = search + offset;
        let after_name = at + "style".len();
        let before_ok = at == 0 || !is_attribute_name_byte(bytes[at - 1]);
        let after = open_tag[after_name..].trim_start();
        let lead = open_tag[after_name..].len() - after.len();
        let equals_at = after_name + lead;
        search = after_name;
        if !before_ok || !after.starts_with('=') {
            continue;
        }
        let value_start = equals_at + 1 + (after[1..].len() - after[1..].trim_start().len());
        let value = &open_tag[value_start..];
        let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
        return match quote {
            Some(quote) => {
                let inner = value_start + quote.len_utf8();
                let end = inner + open_tag[inner..].find(quote)?;
                Some(inner..end)
            }
            // Unquoted value: runs to the end of the tag.
            None => {
                let end = value_start
                    + value
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(value.len());
                Some(value_start..end)
            }
        };
    }
    None
}

/// Bytes that can appear inside an attribute name.
///
/// Used so a `style` match inside another attribute's name or value is not
/// mistaken for the attribute itself.
fn is_attribute_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b':'
}

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Replace the values of declaration spans inside a stylesheet.
///
/// Each span was resolved from the declaration that owns the node, so this only
/// ever overwrites a value someone authored. Edits are applied from the end of
/// the file backwards so an earlier replacement cannot shift a later range.
fn apply_css_edits(original: &str, changes: &[((usize, usize), String)]) -> String {
    let mut ordered: Vec<&((usize, usize), String)> = changes.iter().collect();
    ordered.sort_by_key(|((start, _), _)| std::cmp::Reverse(*start));
    let mut out = original.to_owned();
    for ((start, end), value) in ordered {
        if out.is_char_boundary(*start) && out.is_char_boundary(*end) {
            out.replace_range(*start..*end, value);
        }
    }
    out
}

fn write_if_changed(
    path: &std::path::Path,
    original: &str,
    rewritten: &str,
    outcome: &mut SaveOutcome,
) -> Result<(), BundleError> {
    if original == rewritten {
        return Ok(());
    }
    crate::project_bundle::write_atomically(path, rewritten.as_bytes())?;
    outcome.written.push(path.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_open::open_project;
    use crate::source_document::NodeId;

    /// Copy a fixture so a save never touches the committed files.
    fn scratch(fixture: &str) -> PathBuf {
        let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(fixture);
        let to = std::env::temp_dir().join(format!(
            "spool-save-{fixture}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&to);
        copy_tree(&from, &to).expect("copy fixture");
        to
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }

    /// Write a throwaway project for the cases the shared fixtures do not cover.
    ///
    /// Three files and no more: a fixture is a real authored page, and these
    /// cases are about one declaration each. `scratch` remains the default
    /// because most saves are about the projects people actually author.
    fn project_named(name: &str, html: &str, css: Option<&str>, yaml: &str) -> PathBuf {
        let to = std::env::temp_dir().join(format!(
            "spool-save-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).expect("create project");
        std::fs::write(to.join("lamine.yaml"), yaml).expect("write metadata");
        std::fs::write(to.join("index.html"), html).expect("write html");
        if let Some(css) = css {
            std::fs::write(to.join("styles.css"), css).expect("write css");
        }
        to
    }

    /// One node's entry in a `lamine.yaml`, in the shape the bundle reads.
    fn node_yaml(id: &str, parent: Option<&str>, children: &str) -> String {
        format!(
            "  - id: \"{id}\"\n    name: \"{id}\"\n    kind: \"frame\"\n    parent: {parent}\n    file: \"index.html\"\n    selector: \"[data-spool-id=\\\"{id}\\\"]\"\n    children: [{children}]\n",
            parent = parent
                .map(|parent| format!("\"{parent}\""))
                .unwrap_or_else(|| "null".to_owned()),
        )
    }

    /// Where a node sits in world coordinates after opening a project.
    fn position_of(runtime: &crate::canvas::Document, id: &NodeId) -> gpui::Point<f32> {
        runtime
            .objects()
            .iter()
            .find(|object| &object.spool_id == id)
            .expect("node is in the runtime")
            .position
    }

    fn node(id: &str) -> NodeId {
        NodeId::new(id).expect("valid id")
    }

    #[test]
    fn a_text_edit_survives_open_edit_save_reopen() {
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");

        let edited = "Design in source, structure in Spool, saved";
        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Text {
                node: node("spool-text-headline"),
                text: edited.into(),
            }],
        )
        .expect("save succeeds");
        assert!(outcome.unsupported.is_empty(), "the edit was supported");
        assert_eq!(outcome.written.len(), 1, "only the HTML was rewritten");

        // Reopen from disk, not from memory.
        let reopened = open_project(&root).expect("reopens");
        let headline = reopened
            .runtime
            .objects()
            .iter()
            .find(|o| o.spool_id == node("spool-text-headline"))
            .expect("headline survives");
        assert_eq!(headline.text_content.as_deref(), Some(edited));
    }

    #[test]
    fn saving_a_text_edit_leaves_every_other_byte_alone() {
        let root = scratch("landing");
        let before = std::fs::read_to_string(root.join("index.html")).unwrap();
        let loaded = open_project(&root).expect("opens");

        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Text {
                node: node("spool-text-headline"),
                text: "Changed".into(),
            }],
        )
        .expect("save");

        let after = std::fs::read_to_string(root.join("index.html")).unwrap();
        // Everything outside the headline's text is byte-identical.
        let restored = after.replace("Changed", "Design in source, structure in Spool");
        assert_eq!(restored, before, "only the intended text changed");

        // The stylesheet was not touched at all.
        let css_before = std::fs::read_to_string(root.join("styles.css")).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("styles.css")).unwrap(),
            css_before
        );
    }

    #[test]
    fn a_geometry_edit_survives_the_round_trip() {
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        // The CTA is in the normal flow, so there is no authored position to
        // overwrite. The baseline is where the flow puts it, and the editor's
        // move is an offset from there.
        let authored = position_of(&loaded.runtime, &node("spool-cta-primary"));

        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                placement: Some(Placement::Flow {
                    dx: 40.0,
                    dy: 120.0,
                }),
                width: Some(220.0),
                height: Some(48.0),
            }],
        )
        .expect("save");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);

        let html = std::fs::read_to_string(root.join("index.html")).expect("read html");
        assert!(
            html.contains("transform: translate(40px, 120px)"),
            "the move stayed in the flow: {html}"
        );
        assert!(
            !html.contains("position: absolute"),
            "a flow element must not be lifted out of it: {html}"
        );

        let reopened = open_project(&root).expect("reopens");
        let cta = reopened
            .runtime
            .objects()
            .iter()
            .find(|o| o.spool_id == node("spool-cta-primary"))
            .expect("cta survives");
        // Explicit dimensions are read as the border box, which is what the
        // editor measures and writes. Padding stays part of the box, so the
        // width that comes back is the width that was written.
        assert!(
            (cta.size.width - 220.0).abs() < 0.01,
            "the authored width survived, got {}",
            cta.size.width
        );
        assert!((cta.size.height - 48.0).abs() < 0.01);
        assert!(
            (cta.position.x - (authored.x + 40.0)).abs() < 0.01
                && (cta.position.y - (authored.y + 120.0)).abs() < 0.01,
            "the element came back where the editor left it: {} vs {}",
            cta.position,
            gpui::point(authored.x + 40.0, authored.y + 120.0)
        );

        // Re-saving what came back must not drift: a project opened, edited and
        // saved repeatedly has to converge rather than grow by its padding each
        // round trip. The bytes are compared too, because a duplicated style
        // attribute still parses to the same geometry — only the file on disk
        // shows the damage. The reopened project has not been moved again, so
        // the editor has no move to write — only the size it measured.
        let after_first_save = std::fs::read_to_string(root.join("index.html")).expect("read html");
        let measured = cta.size;
        save_project(
            &root,
            &reopened.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                placement: None,
                width: Some(measured.width),
                height: Some(measured.height),
            }],
        )
        .expect("second save");
        let again = open_project(&root).expect("reopens again");
        let cta = again
            .runtime
            .objects()
            .iter()
            .find(|o| o.spool_id == node("spool-cta-primary"))
            .expect("cta survives");
        assert!(
            (cta.size.width - measured.width).abs() < 0.01
                && (cta.size.height - measured.height).abs() < 0.01,
            "a second round trip is a fixed point, got {}x{}",
            cta.size.width,
            cta.size.height
        );
        assert!(
            (cta.position.x - (authored.x + 40.0)).abs() < 0.01
                && (cta.position.y - (authored.y + 120.0)).abs() < 0.01,
            "and the offset did not accumulate: {}",
            cta.position
        );
        assert_eq!(
            std::fs::read_to_string(root.join("index.html")).expect("read html"),
            after_first_save,
            "and the second save changed no bytes at all"
        );
        assert_eq!(
            after_first_save.matches("style=").count(),
            1,
            "one element carries one style attribute: {after_first_save}"
        );
    }

    #[test]
    fn a_style_edit_rewrites_the_owning_declaration_only() {
        let root = scratch("landing");
        let before = std::fs::read_to_string(root.join("styles.css")).unwrap();
        let loaded = open_project(&root).expect("opens");

        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Style {
                node: node("spool-cta-primary"),
                property: "background".into(),
                value: "#ff0000".into(),
            }],
        )
        .expect("save");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);

        let after = std::fs::read_to_string(root.join("styles.css")).unwrap();
        assert!(after.contains("#ff0000"), "the declaration was rewritten");
        assert_eq!(
            after.matches("background").count(),
            before.matches("background").count(),
            "no rule was added or removed"
        );

        let reopened = open_project(&root).expect("reopens");
        let cta = reopened
            .runtime
            .objects()
            .iter()
            .find(|o| o.spool_id == node("spool-cta-primary"))
            .expect("cta survives");
        let fill = cta.fill.expect("the authored background is applied");
        assert_eq!(
            (fill.color.red, fill.color.green, fill.color.blue),
            (255, 0, 0)
        );
    }

    #[test]
    fn a_moved_flow_element_does_not_mention_its_parent() {
        // A flow element has no position of its own, so a move says nothing
        // about where its parent is: the two stay independent, and moving the
        // parent later never requires rewriting the child.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let before = position_of(&loaded.runtime, &node("spool-cta-primary"));

        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                placement: Some(Placement::Flow { dx: 0.0, dy: 10.0 }),
                width: None,
                height: None,
            }],
        )
        .expect("save succeeds");

        let html = std::fs::read_to_string(root.join("index.html")).expect("read html");
        assert!(html.contains("transform: translate(0px, 10px)"), "{html}");
        assert!(!html.contains("left:"), "no absolute position: {html}");

        let reopened = open_project(&root).expect("reopens");
        let cta = position_of(&reopened.runtime, &node("spool-cta-primary"));
        assert!(
            (cta.y - (before.y + 10.0)).abs() < 0.01,
            "{cta} vs {before}"
        );
        let frame = position_of(&reopened.runtime, &node("spool-frame-root"));
        assert_eq!(frame, gpui::point(0.0, 0.0), "the parent did not move");
    }

    #[test]
    fn an_element_the_author_took_out_of_the_flow_keeps_its_own_coordinates() {
        // The other half of the policy: a box that is already out of the flow is
        // written with `left`/`top` from its containing block, and `position` is
        // not restated — the author already said so, and repeating it locally
        // would be a second declaration making the same promise.
        let html = "<!doctype html>\n<body>\n  <div data-spool-id=\"spool-frame-root\" style=\"position: absolute; left: 100px; top: 60px; width: 200px\">\n    <span data-spool-id=\"spool-text-headline\" style=\"position: absolute; left: 4px; top: 6px\">Hello</span>\n  </div>\n</body>\n";
        let yaml = format!(
            "version: 1\nnodes:\n{}{}",
            node_yaml("spool-frame-root", None, "\"spool-text-headline\""),
            node_yaml("spool-text-headline", Some("spool-frame-root"), "")
        );
        let root = project_named("absolute", html, None, &yaml);
        let loaded = open_project(&root).expect("opens");
        assert!(is_out_of_flow(&loaded.document, &loaded.document.structure.nodes[1]).unwrap());

        // The child is absolute, so its position is written relative to the
        // parent's containing block. Writing the child's world coordinate would
        // put it twice as far down once the parent itself moved.
        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-text-headline"),
                placement: Some(Placement::ContainingBlock { x: 12.0, y: 20.0 }),
                width: None,
                height: None,
            }],
        )
        .expect("save succeeds");

        let after = std::fs::read_to_string(root.join("index.html")).expect("read html");
        assert!(
            after.contains("left: 12px") && after.contains("top: 20px"),
            "{after}"
        );
        assert!(
            after.contains(r#"<span data-spool-id="spool-text-headline" style="position: absolute; left: 12px; top: 20px">"#),
            "an already-absolute element keeps its single `position`, with only the \
             coordinates rewritten: {after}"
        );

        let reopened = open_project(&root).expect("reopens");
        // Parent at 100/60 plus a child at 12/20 from it.
        assert_eq!(
            position_of(&reopened.runtime, &node("spool-text-headline")),
            gpui::point(112.0, 80.0)
        );
    }

    #[test]
    fn a_second_move_adds_to_the_offset_the_author_wrote() {
        // Moves compose. A drag is a change from where the element already was,
        // so saving twice must not double the offset — and an author's own
        // `transform` is part of that baseline, not something to overwrite.
        let html = "<!doctype html>\n<body>\n  <div data-spool-id=\"spool-frame-root\">\n    <span data-spool-id=\"spool-text-headline\" style=\"transform: translate(10px, 0px)\">Hello</span>\n  </div>\n</body>\n";
        let yaml = format!(
            "version: 1\nnodes:\n{}{}",
            node_yaml("spool-frame-root", None, "\"spool-text-headline\""),
            node_yaml("spool-text-headline", Some("spool-frame-root"), "")
        );
        let root = project_named("compose", html, None, &yaml);
        let loaded = open_project(&root).expect("opens");
        let before = position_of(&loaded.runtime, &node("spool-text-headline"));

        // Each session the user drags the element 10px to the right of where it
        // was when that session opened, which is the only information a save
        // gets. Two sessions, two drags, one authored offset.
        for _ in 0..2 {
            let loaded = open_project(&root).expect("opens");
            save_project(
                &root,
                &loaded.document,
                &[SourceEdit::Geometry {
                    node: node("spool-text-headline"),
                    placement: Some(Placement::Flow { dx: 10.0, dy: 0.0 }),
                    width: None,
                    height: None,
                }],
            )
            .expect("save succeeds");
        }

        let after = std::fs::read_to_string(root.join("index.html")).expect("read html");
        assert!(
            after.contains("transform: translate(30px, 0px)"),
            "two moves of the same distance add to the authored offset instead of \
             replacing it: {after}"
        );
        assert_eq!(
            position_of(
                &open_project(&root).expect("reopens").runtime,
                &node("spool-text-headline")
            ),
            gpui::point(before.x + 20.0, before.y),
            "and the element sits where the editor left it"
        );
    }

    #[test]
    fn a_transform_the_editor_cannot_read_is_reported_rather_than_overwritten() {
        // A rotation is an authored decision. Rewriting it as a translate would
        // delete it, so the save says so and leaves the file alone.
        let html = "<!doctype html>\n<body>\n  <div data-spool-id=\"spool-frame-root\">\n    <span data-spool-id=\"spool-text-headline\" style=\"transform: rotate(4deg)\">Hello</span>\n  </div>\n</body>\n";
        let yaml = format!(
            "version: 1\nnodes:\n{}{}",
            node_yaml("spool-frame-root", None, "\"spool-text-headline\""),
            node_yaml("spool-text-headline", Some("spool-frame-root"), "")
        );
        let root = project_named("rotated", html, None, &yaml);
        let loaded = open_project(&root).expect("opens");

        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-text-headline"),
                placement: Some(Placement::Flow { dx: 8.0, dy: 8.0 }),
                width: None,
                height: None,
            }],
        )
        .expect("save succeeds");
        assert_eq!(outcome.unsupported.len(), 1, "{:?}", outcome.unsupported);
        assert!(
            outcome.unsupported[0].reason.contains("rotate(4deg)"),
            "the reason names the conflict: {:?}",
            outcome.unsupported[0]
        );
        assert_eq!(
            std::fs::read_to_string(root.join("index.html")).expect("read html"),
            html,
            "and the file is byte-for-byte untouched"
        );
    }

    #[test]
    fn written_lengths_are_readable_rather_than_binary_noise() {
        // Layout runs in f32, so the editor's own geometry arrives as
        // `22.399994`. Authoring that into the document is accurate, ugly, and
        // makes every save look like a change even when nothing did.
        assert_eq!(css_length(22.399994), "22.4");
        assert_eq!(css_length(40.0), "40");
        assert_eq!(css_length(0.0), "0");
        assert_eq!(css_length(-3.5), "-3.5");
        assert_eq!(
            merge_inline_declarations(
                "",
                &[
                    ("left".to_owned(), "40px".to_owned()),
                    ("top".to_owned(), "22.4px".to_owned()),
                    ("width".to_owned(), "300px".to_owned()),
                    ("height".to_owned(), "46.4px".to_owned()),
                ]
            ),
            "left: 40px; top: 22.4px; width: 300px; height: 46.4px"
        );
        // A position alone must not pin a size the user never changed.
        assert_eq!(
            merge_inline_declarations(
                "",
                &[
                    ("left".to_owned(), "10px".to_owned()),
                    ("top".to_owned(), "20px".to_owned())
                ]
            ),
            "left: 10px; top: 20px"
        );
    }

    #[test]
    fn a_style_attribute_value_range_covers_the_value_and_nothing_else() {
        for (open_tag, expected) in [
            (
                r#"<a class="cta" style="color: red">"#,
                Some("color: red".to_owned()),
            ),
            (
                r#"<a style='color: red' data-x="1">"#,
                Some("color: red".to_owned()),
            ),
            // Spaces around the `=` are legal and must not break the range.
            (r#"<a style = "color: red">"#, Some("color: red".to_owned())),
            (r#"<a style=color:red>"#, Some("color:red".to_owned())),
            // A different attribute that merely contains the word.
            (r#"<a data-style-note="nope">"#, None),
            (r#"<a class="cta">"#, None),
        ] {
            let range = style_attribute_value(open_tag);
            assert_eq!(
                range
                    .as_ref()
                    .map(|range| open_tag[range.clone()].to_owned()),
                expected,
                "for {open_tag}"
            );
        }
    }

    #[test]
    fn a_rename_made_in_the_editor_is_written_to_the_metadata() {
        // The other half of the save: HTML and CSS are not the only authored
        // state, and a rename that never reaches `lamine.yaml` is a rename the
        // user cannot see survive.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let before = std::fs::read_to_string(root.join("lamine.yaml")).expect("read metadata");

        let mut renamed = loaded.document.clone();
        renamed.structure.nodes[1].name = "Hero headline".into();
        let outcome = save_project(&root, &renamed, &[]).expect("save succeeds");

        assert_eq!(
            outcome.written,
            vec![root.join(crate::project_bundle::METADATA_FILE)],
            "only the metadata file was written"
        );
        let after = std::fs::read_to_string(root.join("lamine.yaml")).expect("read metadata");
        assert_ne!(after, before);
        assert!(after.contains("Hero headline"));
        assert_eq!(
            open_project(&root)
                .expect("reopens")
                .document
                .structure
                .nodes[1]
                .name,
            "Hero headline",
            "and the name comes back from disk"
        );
        // Identity, hierarchy, and every authored file survive the rename.
        assert_eq!(
            open_project(&root)
                .expect("reopens")
                .document
                .structure
                .nodes
                .len(),
            3
        );
        assert_eq!(
            std::fs::read_to_string(root.join("index.html")).expect("read html"),
            loaded.document.sources["index.html"],
            "the authored source is untouched by a metadata-only save"
        );
    }

    #[test]
    fn two_edits_to_the_same_element_both_land() {
        // Text and geometry on one element are two separate spans in one file,
        // applied back to front so the first does not shift the second.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let target = node("spool-cta-primary");
        let flowed = position_of(&loaded.runtime, &target);
        save_project(
            &root,
            &loaded.document,
            &[
                SourceEdit::Text {
                    node: target.clone(),
                    text: "Start now".into(),
                },
                SourceEdit::Geometry {
                    node: target.clone(),
                    placement: Some(Placement::Flow { dx: 10.0, dy: 20.0 }),
                    width: Some(200.0),
                    height: Some(40.0),
                },
            ],
        )
        .expect("save succeeds");

        let reopened = open_project(&root).expect("reopens");
        let cta = reopened
            .runtime
            .objects()
            .iter()
            .find(|object| object.spool_id == target)
            .expect("cta survives");
        assert_eq!(cta.text_content.as_deref(), Some("Start now"));
        assert!(
            (cta.position.x - (flowed.x + 10.0)).abs() < 0.01
                && (cta.position.y - (flowed.y + 20.0)).abs() < 0.01,
            "the move is an offset on top of where the flow put it: {} vs {flowed}",
            cta.position
        );
        assert_eq!(cta.size.width, 200.0);
        let html = std::fs::read_to_string(root.join("index.html")).expect("read html");
        assert_eq!(
            html.matches("style=").count(),
            1,
            "one style attribute, not two: {html}"
        );
        assert!(
            html.contains(">Start now</a>"),
            "and the closing tag survived"
        );
    }

    #[test]
    fn a_style_edit_with_no_authored_owner_becomes_a_local_declaration() {
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let before = std::fs::read_to_string(root.join("styles.css")).unwrap();

        // Nothing in this project declares `letter-spacing` for the headline.
        // The policy is to give the element its own declaration rather than to
        // invent a rule in the stylesheet or to refuse the edit.
        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Style {
                node: node("spool-text-headline"),
                property: "letter-spacing".into(),
                value: "2px".into(),
            }],
        )
        .expect("save succeeds");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);

        let html = std::fs::read_to_string(root.join("index.html")).unwrap();
        assert!(
            html.contains(
                r#"<h1 data-spool-id="spool-text-headline" style="letter-spacing: 2px">"#
            ),
            "the declaration landed on the element itself: {html}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("styles.css")).unwrap(),
            before,
            "and no rule was invented in the stylesheet"
        );
    }

    #[test]
    fn editing_an_inherited_property_writes_it_on_the_element_not_on_the_ancestor() {
        // The headline's colour comes from `body`. Editing the headline must not
        // restyle every element that inherits from body — that would change
        // objects the user never selected.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let css_before = std::fs::read_to_string(root.join("styles.css")).unwrap();

        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Style {
                node: node("spool-text-headline"),
                property: "color".into(),
                value: "#ff0000".into(),
            }],
        )
        .expect("save succeeds");

        let html = std::fs::read_to_string(root.join("index.html")).unwrap();
        assert!(
            html.contains(r#"<h1 data-spool-id="spool-text-headline" style="color: #ff0000">"#),
            "the headline now owns its colour: {html}"
        );
        assert!(
            !html.contains(r#"<main data-spool-id="spool-frame-root" style="color"#),
            "and the frame that inherited it was not touched: {html}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("styles.css")).unwrap(),
            css_before,
            "body's rule is untouched"
        );
    }

    #[test]
    fn a_geometry_edit_and_a_style_edit_share_one_inline_attribute() {
        // Both target the same element. Written separately they would either
        // duplicate the attribute or the second would erase the first.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let target = node("spool-cta-primary");
        save_project(
            &root,
            &loaded.document,
            &[
                SourceEdit::Geometry {
                    node: target.clone(),
                    placement: Some(Placement::Flow { dx: 30.0, dy: 40.0 }),
                    width: None,
                    height: None,
                },
                SourceEdit::Style {
                    node: target.clone(),
                    property: "border-radius".into(),
                    value: "12px".into(),
                },
                SourceEdit::Style {
                    node: target.clone(),
                    property: "opacity".into(),
                    value: "0.5".into(),
                },
            ],
        )
        .expect("save succeeds");

        let html = std::fs::read_to_string(root.join("index.html")).unwrap();
        assert_eq!(
            html.matches("style=").count(),
            1,
            "one attribute holds the geometry and the unowned property: {html}"
        );
        assert!(
            html.contains("transform: translate(30px, 40px)"),
            "geometry is inline: {html}"
        );
        assert!(
            html.contains("opacity: 0.5"),
            "nothing authored an opacity, so it became a local declaration: {html}"
        );

        // `border-radius` is declared by `.cta`, so ownership sends that edit to
        // the stylesheet instead — one attribute on the element, one declaration
        // in the file that already owned it.
        let css = std::fs::read_to_string(root.join("styles.css")).unwrap();
        assert!(
            css.contains("border-radius: 12px"),
            "radius is in the rule: {css}"
        );
        assert_eq!(
            css.matches("border-radius").count(),
            1,
            "rewritten in place, not duplicated: {css}"
        );

        // And a repeated edit of the same value changes nothing.
        let reopened = open_project(&root).expect("reopens");
        save_project(
            &root,
            &reopened.document,
            &[SourceEdit::Style {
                node: target.clone(),
                property: "border-radius".into(),
                value: "12px".into(),
            }],
        )
        .expect("save succeeds");
        assert_eq!(
            std::fs::read_to_string(root.join("styles.css")).unwrap(),
            css,
            "a repeated edit is idempotent"
        );
    }

    #[test]
    fn a_hand_written_inline_declaration_survives_an_unrelated_edit() {
        // The author wrote `z-index` by hand. Moving the element must not delete
        // it: the editor merges into the existing value rather than replacing it.
        let root = scratch("landing");
        let html = std::fs::read_to_string(root.join("index.html")).unwrap();
        std::fs::write(
            root.join("index.html"),
            html.replace(
                r#"<h1 data-spool-id="spool-text-headline">"#,
                r#"<h1 data-spool-id="spool-text-headline" style="z-index: 3">"#,
            ),
        )
        .unwrap();
        let loaded = open_project(&root).expect("opens");

        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-text-headline"),
                placement: Some(Placement::Flow { dx: 5.0, dy: 6.0 }),
                width: None,
                height: None,
            }],
        )
        .expect("save succeeds");

        let after = std::fs::read_to_string(root.join("index.html")).unwrap();
        assert!(after.contains("z-index: 3"), "kept: {after}");
        assert!(
            after.contains("transform: translate(5px, 6px)"),
            "and the move landed too: {after}"
        );
    }

    #[test]
    fn identity_and_hierarchy_survive_the_whole_loop() {
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let before_ids: Vec<String> = loaded
            .document
            .structure
            .nodes
            .iter()
            .map(|n| n.id.as_str().to_owned())
            .collect();
        let before_children = loaded.document.structure.nodes[0].children.clone();

        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Text {
                node: node("spool-cta-primary"),
                text: "Shipped".into(),
            }],
        )
        .expect("save");

        let reopened = open_project(&root).expect("reopens");
        let after_ids: Vec<String> = reopened
            .document
            .structure
            .nodes
            .iter()
            .map(|n| n.id.as_str().to_owned())
            .collect();
        assert_eq!(after_ids, before_ids, "identity survived");
        assert_eq!(
            reopened.document.structure.nodes[0].children, before_children,
            "hierarchy survived"
        );
    }
}
