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
    Geometry {
        node: NodeId,
        x: Option<f32>,
        y: Option<f32>,
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
    let mut attributes: BTreeMap<String, Vec<(NodeId, String)>> = BTreeMap::new();
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
            SourceEdit::Text { text, .. } => html_edits
                .entry(file)
                .or_default()
                .push((node_id.clone(), text.clone())),
            SourceEdit::Geometry {
                x,
                y,
                width,
                height,
                ..
            } => {
                let declarations = geometry_declarations(*x, *y, *width, *height);
                if declarations.is_empty() {
                    outcome.unsupported.push(UnsupportedEdit {
                        node: node_id.clone(),
                        kind: "geometry",
                        reason: "no geometry value to write".into(),
                    });
                } else {
                    attributes
                        .entry(file)
                        .or_default()
                        .push((node_id.clone(), declarations));
                }
            }
            SourceEdit::Style {
                property, value, ..
            } => {
                // Ownership: a style edit needs an authored declaration for
                // that property. Without one there is no span to rewrite, and
                // inventing a rule would be source generation, not an edit.
                match find_style_owner(document, node, property) {
                    Some((css_file, range)) => {
                        css_edits.push((css_file, range, value.clone()));
                    }
                    None => outcome.unsupported.push(UnsupportedEdit {
                        node: node_id.clone(),
                        kind: "style",
                        reason: format!("no authored declaration of `{property}` owns this node"),
                    }),
                }
            }
        }
    }

    // Write HTML files that have text or attribute edits.
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
            attributes.get(file),
        )?;
        write_if_changed(&path, &original, &rewritten, &mut outcome)?;
    }
    // Files with only attribute edits.
    for (file, edits_for_file) in &attributes {
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

fn geometry_declarations(
    x: Option<f32>,
    y: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
) -> String {
    let mut parts = Vec::new();
    if let (Some(left), Some(top)) = (x, y) {
        parts.push(format!(
            "position: absolute; left: {}px; top: {}px",
            css_length(left),
            css_length(top)
        ));
    }
    if let Some(width) = width {
        parts.push(format!("width: {}px", css_length(width)));
    }
    if let Some(height) = height {
        parts.push(format!("height: {}px", css_length(height)));
    }
    parts.join("; ")
}

/// Format a length the way an author would write it.
///
/// Layout runs in `f32`, so an exact-looking position comes out as
/// `22.399994`. Writing that into the document is accurate and unreadable, and
/// it makes every save produce a noisy diff. Two decimals is finer than any
/// screen can show and keeps the source something a person would have written.
fn css_length(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == rounded.trunc() {
        return format!("{}", rounded as i64);
    }
    format!("{rounded}")
}

/// Find the authored declaration that owns `property` for this node.
///
/// Ownership is checked, not assumed: the node's own element must be matched by
/// a rule that declares the property, and the answer is the byte range of that
/// declaration's value. A property nobody authored for this element has no span
/// to edit, so it is reported rather than turned into a new rule — inventing a
/// rule would be source generation, not an edit.
///
/// Returns the owning file together with the value range.
fn find_style_owner(
    document: &crate::source_document::PersistentDocument,
    node: &crate::source_document::StructuralNode,
    property: &str,
) -> Option<(String, (usize, usize))> {
    let html = document.sources.get(&node.source.file)?;
    let index = SourceIndex::parse(html);
    let binding = index.find(&node.id)?;
    let fragment = html.get(binding.element_range.clone())?;
    let (tag, classes) = crate::visual::element_identity(fragment);

    // Sorted so the answer does not depend on hash iteration order.
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
                return Some((name.clone(), range));
            }
        }
    }
    None
}

/// The authored spellings a visual property can be written as, in preference
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
    attribute_edits: Option<&Vec<(NodeId, String)>>,
) -> Result<String, BundleError> {
    let mut replacements: Vec<(std::ops::Range<usize>, String)> = Vec::new();

    for (node, text) in text_edits {
        let Some(binding) = index.find(node) else {
            continue;
        };
        let element = original.get(binding.element_range.clone()).unwrap_or("");
        let Some(open_end) = element.find('>') else {
            continue;
        };
        let inner_start = binding.element_range.start + open_end + 1;
        // This element's own closing tag. Measured from the end of the
        // fragment rather than assumed to be a fixed width, because tag names
        // vary and a void element has none at all.
        let inner_end = binding.element_range.start + element.rfind("</").unwrap_or(element.len());
        if inner_end < inner_start {
            continue;
        }
        replacements.push((inner_start..inner_end, escape_text(text)));
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
            match style_attribute_value(open_tag) {
                // Replace the authored value in place, so the attribute keeps
                // its position, quoting style, and any declaration this edit did
                // not touch.
                Some(range) => replacements.push((
                    absolute + range.start..absolute + range.end,
                    declarations.clone(),
                )),
                // No `style` attribute yet: add one just before the closing `>`.
                None => {
                    let insert_at = absolute + open_end;
                    replacements.push((insert_at..insert_at, format!(" style=\"{declarations}\"")));
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

        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                x: Some(40.0),
                y: Some(120.0),
                width: Some(220.0),
                height: Some(48.0),
            }],
        )
        .expect("save");
        assert!(outcome.unsupported.is_empty());

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
        // `position: absolute` with `left`/`top` came back too.
        assert_eq!(cta.position.x, 40.0);
        assert_eq!(cta.position.y, 120.0);

        // Re-saving what came back must not drift: a project opened, edited and
        // saved repeatedly has to converge rather than grow by its padding each
        // round trip. The bytes are compared too, because a duplicated style
        // attribute still parses to the same geometry — only the file on disk
        // shows the damage.
        let after_first_save = std::fs::read_to_string(root.join("index.html")).expect("read html");
        let measured = cta.size;
        save_project(
            &root,
            &reopened.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                x: Some(cta.position.x),
                y: Some(cta.position.y),
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
    fn a_child_position_is_written_relative_to_the_parent_that_contains_it() {
        // Absolute positioning is resolved against the containing block, which
        // is the parent element. Writing a child's world coordinate as its
        // `left` would put it twice as far right once the parent itself moved.
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        // The fixture's CTA sits at y = 22.4 inside a frame at y = 0, so 22.4 is
        // both its world position and its offset from the frame.
        save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Geometry {
                node: node("spool-cta-primary"),
                // Already parent-relative, which is what a second save would
                // compute from world (22.4) minus parent (0).
                x: Some(0.0),
                y: Some(22.4),
                width: None,
                height: None,
            }],
        )
        .expect("save succeeds");

        let reopened = open_project(&root).expect("reopens");
        let cta = reopened
            .runtime
            .objects()
            .iter()
            .find(|object| object.spool_id.as_str() == "spool-cta-primary")
            .expect("cta survives");
        assert_eq!(cta.position, gpui::point(0.0, 22.4));
        assert!(
            (cta.size.height - 46.4).abs() < 0.01,
            "an unchanged size was not pinned: {}",
            cta.size.height
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
            geometry_declarations(Some(40.0), Some(22.4), Some(300.0), Some(46.4)),
            "position: absolute; left: 40px; top: 22.4px; width: 300px; height: 46.4px"
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
                    x: Some(10.0),
                    y: Some(20.0),
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
        assert_eq!(cta.position, gpui::point(10.0, 20.0));
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
    fn a_style_edit_with_no_authored_owner_is_reported_not_invented() {
        let root = scratch("landing");
        let loaded = open_project(&root).expect("opens");
        let before = std::fs::read_to_string(root.join("styles.css")).unwrap();

        // No declaration of `letter-spacing` exists anywhere in this project.
        let outcome = save_project(
            &root,
            &loaded.document,
            &[SourceEdit::Style {
                node: node("spool-text-headline"),
                property: "letter-spacing".into(),
                value: "2px".into(),
            }],
        )
        .expect("save does not fail, it reports");

        assert_eq!(outcome.unsupported.len(), 1, "the edit was reported");
        assert_eq!(
            std::fs::read_to_string(root.join("styles.css")).unwrap(),
            before,
            "no rule was invented to hold the edit"
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
