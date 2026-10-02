//! Source interpretation: HTML and CSS into runtime text, style, and geometry.
//!
//! # The chain
//!
//! ```text
//! authored HTML + CSS
//!     -> element facts      (tag, classes, text)      from the bound parse
//!     -> style resolution   (supported subset only)    see `style`
//!     -> block layout       (deterministic)            this module
//!     -> runtime text / style / geometry
//! ```
//!
//! This replaces the two placeholders that made a loaded project look fake: the
//! `PrototypeGeometry::placeholder(index)` column and the default canvas style.
//!
//! # Layout scope
//!
//! Block flow only. Children stack vertically inside their parent, margins and
//! padding are honoured, explicit `width`/`height` are honoured, and text
//! boxes are sized from their font size. There is no inline layout, no flexbox,
//! no floats, no absolute positioning, and no text wrapping. That is enough for
//! the current fixtures to look like their source, and it is deliberately a
//! separate stage so a better layout engine can replace it without touching
//! style resolution.
//!
//! Everything here is derived and disposable. Nothing writes back to source,
//! and `lamine.yaml` never learns any of these values.

use std::collections::BTreeMap;

use crate::canvas::{Color, Fill, ObjectStyle, Stroke};
use crate::source_binding::{Reconciliation, SourceIndex};
use crate::source_document::{NodeId, PersistentDocument};
use crate::style::{self, Stylesheet};

/// Width the layout uses for a top-level frame when CSS does not say.
pub const DEFAULT_CONTENT_WIDTH: f32 = 960.0;

/// What the renderer can draw for a node, derived from source.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeVisual {
    /// The authored text, trimmed. Empty when the element has no text.
    pub text: Option<String>,
    pub style: ObjectStyle,
    pub geometry: BoxGeometry,
    /// Text colour and size, kept beside the geometry because the canvas
    /// stores them on the object rather than in `ObjectStyle`.
    pub text_color: Option<Color>,
    pub font_size: f32,
    /// True when nothing authored a background or border for this node.
    ///
    /// Meaning the renderer still substitutes its own default paint. Reported so
    /// the gap between "styled by source" and "styled by the editor's defaults"
    /// is visible rather than assumed away.
    pub unstyled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Every node's visual model, keyed by persistent identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualModel {
    nodes: BTreeMap<NodeId, NodeVisual>,
}

impl VisualModel {
    pub fn get(&self, id: &NodeId) -> Option<&NodeVisual> {
        self.nodes.get(id)
    }

    /// Nodes that carried no authored style at all.
    ///
    /// Reported rather than hidden: these are the ones still relying on the
    /// canvas default, which is the visible edge of what CSS support covers.
    pub fn unstyled(&self) -> Vec<&NodeId> {
        self.nodes
            .iter()
            .filter(|(_, visual)| visual.unstyled)
            .map(|(id, _)| id)
            .collect()
    }
}

/// Build the visual model for a loaded project.
///
/// Reads the bound parse for element facts and every stylesheet the project
/// references, then lays the result out deterministically.
pub fn build(reconciliation: &Reconciliation, document: &PersistentDocument) -> VisualModel {
    let mut model = VisualModel::default();
    let stylesheets = collect_stylesheets(document);

    // Resolve each node's style first; layout needs the resolved box values.
    let mut resolved: BTreeMap<NodeId, Resolved> = BTreeMap::new();
    for node in &document.structure.nodes {
        let Some(index) = reconciliation.files.get(&node.source.file) else {
            continue;
        };
        let Some(binding) = index.find(&node.id) else {
            continue;
        };
        let facts = element_facts(index, binding.element_range.clone());
        let declarations = resolve_across(&stylesheets, &facts);
        resolved.insert(
            node.id.clone(),
            Resolved {
                facts,
                declarations,
            },
        );
    }
    // Text colour and size cascade down the tree. This runs before layout so
    // an inherited font size also sizes the node's text box.
    let ancestry: BTreeMap<NodeId, Vec<String>> = document
        .structure
        .nodes
        .iter()
        .filter_map(|node| {
            let index = reconciliation.files.get(&node.source.file)?;
            let binding = index.find(&node.id)?;
            Some((
                node.id.clone(),
                unmanaged_ancestors(index.source.as_str(), binding.element_range.start),
            ))
        })
        .collect();
    inherit_text_properties(&mut resolved, document, &stylesheets, &ancestry);

    // Lay out from the roots. Children are positioned by their parent's
    // content box, so every node is reached by walking down from a root.
    let mut geometry: BTreeMap<NodeId, BoxGeometry> = BTreeMap::new();
    let mut cursor = 0.0f32;
    for root in document
        .structure
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
    {
        layout_node(
            &resolved,
            &root.id,
            0.0,
            cursor,
            DEFAULT_CONTENT_WIDTH,
            &document.structure.nodes,
            &mut cursor,
            &mut geometry,
            // A root's containing block is the page.
            (0.0, 0.0),
        );
    }
    // A node not reachable from a root — malformed hierarchy, or a parent that
    // does not exist — still gets a box, so it is never silently missing from
    // the runtime. Recorded at a deterministic position.
    for (index, node) in document.structure.nodes.iter().enumerate() {
        geometry.entry(node.id.clone()).or_insert(BoxGeometry {
            x: 0.0,
            y: index as f32 * 48.0,
            width: DEFAULT_CONTENT_WIDTH,
            height: 48.0,
        });
    }

    for node in &document.structure.nodes {
        let Some(box_geometry) = geometry.get(&node.id).copied() else {
            continue;
        };
        model.nodes.insert(
            node.id.clone(),
            visual_of(&resolved, &node.id, box_geometry),
        );
    }
    model
}

struct Resolved {
    facts: ElementFacts,
    declarations: BTreeMap<String, String>,
}

struct ElementFacts {
    tag: String,
    classes: Vec<String>,
    text: String,
    /// Declarations authored on the element's own `style` attribute. These
    /// outrank the stylesheet, which is what an inline style means.
    inline: BTreeMap<String, String>,
}

/// Extract the tag, classes, and direct text of an element from the authored
/// bytes.
///
/// Uses the bound element range rather than a second parse, so the facts are
/// guaranteed to describe the same bytes the identity was resolved from.
fn element_facts(index: &SourceIndex, element_range: std::ops::Range<usize>) -> ElementFacts {
    let source = index.source.as_str();
    let fragment = source.get(element_range).unwrap_or("");
    let open_tag_end = fragment.find('>').unwrap_or(fragment.len());
    let open_tag = &fragment[..open_tag_end];
    let (tag, classes) = element_identity(fragment);
    let mut inline = BTreeMap::new();
    for value in attribute_values(open_tag, "style") {
        for part in value.split(';') {
            let Some((property, raw)) = part.split_once(':') else {
                continue;
            };
            let property = property.trim().to_ascii_lowercase();
            let raw = raw.trim();
            if !property.is_empty() && !raw.is_empty() {
                inline.insert(property, raw.to_owned());
            }
        }
    }
    // Text is the element's own content, minus its tags. Nested elements'
    // text is deliberately not flattened: a child that is itself a managed node
    // keeps its own text.
    let text = strip_tags(fragment);
    ElementFacts {
        tag,
        classes,
        text,
        inline,
    }
}

/// The authored tag and class list of one element fragment.
///
/// Shared deliberately: the save layer needs the same element identity this
/// module resolves style against, and two readers of an open tag would be two
/// chances to disagree about who owns a declaration.
///
/// Attributes are read from the element's own open tag only. Searching the
/// whole fragment would pick up a child's `class` and style the parent as if it
/// carried it.
pub fn element_identity(fragment: &str) -> (String, Vec<String>) {
    let mut tag = String::new();
    if let Some(open) = fragment.find('<') {
        let rest = &fragment[open + 1..];
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        tag = rest[..name_end].to_ascii_lowercase();
    }
    let open_tag_end = fragment.find('>').unwrap_or(fragment.len());
    (tag, attribute_values(&fragment[..open_tag_end], "class"))
}

fn attribute_values(fragment: &str, name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = fragment;
    while let Some(at) = rest.find(name) {
        rest = &rest[at + name.len()..];
        let Some(equals) = rest.find('=') else {
            continue;
        };
        let after = &rest[equals + 1..];
        let trimmed = after.trim_start();
        if trimmed.starts_with('"') || trimmed.starts_with('\'') {
            let quote = trimmed.as_bytes()[0] as char;
            let Some(end) = trimmed[1..].find(quote) else {
                break;
            };
            values.push(trimmed[1..1 + end].to_owned());
            rest = &trimmed[end + 2..];
        } else {
            let end = trimmed
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(trimmed.len());
            values.push(trimmed[..end].to_owned());
            rest = &trimmed[end..];
        }
    }
    values
}

/// Extract the element's own text, excluding any nested element's text.
///
/// A nested element that is itself a managed node keeps its own text, so
/// folding it into the parent would duplicate it in the runtime.
fn strip_tags(fragment: &str) -> String {
    let Some(open) = fragment.find('>') else {
        return String::new();
    };
    let inner = &fragment[open + 1..];
    // Trim the element's own closing tag.
    let inner = match inner.rfind("</") {
        Some(at) => &inner[..at],
        None => inner,
    };

    let mut out = String::new();
    let mut depth = 0usize;
    let bytes = inner.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            if depth == 0 {
                // Push one whole char, not one byte: multi-byte text must not
                // be split.
                let ch = inner[index..].chars().next().expect("char boundary");
                out.push(ch);
                index += ch.len_utf8();
                continue;
            }
            index += 1;
            continue;
        }
        // Consume the whole tag and classify it.
        let Some(close) = inner[index..].find('>') else {
            break;
        };
        let tag = &inner[index + 1..index + close];
        let self_closing = tag.trim_end().ends_with('/');
        if tag.starts_with('/') {
            depth = depth.saturating_sub(1);
        } else if !self_closing {
            depth += 1;
        }
        index += close + 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_stylesheets(document: &PersistentDocument) -> Vec<(String, Stylesheet)> {
    let mut sheets: Vec<(String, Stylesheet)> = document
        .sources
        .iter()
        .filter(|(name, _)| name.ends_with(".css"))
        .map(|(name, contents)| (name.clone(), Stylesheet::parse(contents)))
        .collect();
    // Deterministic order regardless of hash iteration.
    sheets.sort_by(|a, b| a.0.cmp(&b.0));
    sheets
}

/// Resolve declarations for an element across every stylesheet.
fn resolve_across(
    sheets: &[(String, Stylesheet)],
    facts: &ElementFacts,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (_, sheet) in sheets {
        for (property, resolved) in sheet.resolve(&facts.tag, &facts.classes, None) {
            out.insert(property, resolved.value);
        }
    }
    // An inline `style` attribute outranks the stylesheet.
    for (property, value) in &facts.inline {
        out.insert(property.clone(), value.clone());
    }
    out
}

/// Lay a node out in block flow, returning its box and advancing the cursor.
/// `parent_origin` is the border-box origin of the element this node sits in,
/// which is what an absolute `left`/`top` is measured from.
#[allow(clippy::too_many_arguments)]
fn layout_node(
    resolved: &BTreeMap<NodeId, Resolved>,
    id: &NodeId,
    x: f32,
    cursor: f32,
    available_width: f32,
    all_nodes: &[crate::source_document::StructuralNode],
    cursor_out: &mut f32,
    out: &mut BTreeMap<NodeId, BoxGeometry>,
    parent_origin: (f32, f32),
) -> Option<BoxGeometry> {
    let entry = resolved.get(id)?;
    let margin = entry
        .declarations
        .get("margin")
        .and_then(|value| style::parse_box(value))
        .unwrap_or([0.0; 4]);
    let padding = entry
        .declarations
        .get("padding")
        .and_then(|value| style::parse_box(value))
        .unwrap_or([0.0; 4]);

    let explicit_width = entry
        .declarations
        .get("width")
        .and_then(|value| style::parse_length(value));
    let explicit_height = entry
        .declarations
        .get("height")
        .and_then(|value| style::parse_length(value));

    // `position: absolute` takes the node out of flow and places it by its
    // authored `left`/`top`. This is the minimum needed for a moved object to
    // come back where the user left it.
    let absolute = entry.declarations.get("position").map(|value| value.trim()) == Some("absolute");
    let authored_left = entry
        .declarations
        .get("left")
        .and_then(|value| style::parse_length(value));
    let authored_top = entry
        .declarations
        .get("top")
        .and_then(|value| style::parse_length(value));

    // What an explicit width means. CSS defaults to a content box, but the
    // editor measures and writes the border box, so reading authored dimensions
    // as content boxes would grow every element by its padding each time a
    // project is saved and reopened. Border box is therefore the default here,
    // and `box-sizing: content-box` opts back into the CSS behaviour.
    let content_box = entry
        .declarations
        .get("box-sizing")
        .map(|value| value.trim())
        == Some("content-box");
    let outer_width = match explicit_width {
        Some(width) if content_box => width + padding[1] + padding[3],
        Some(width) => width,
        None => available_width,
    };

    // An absolute box is placed against its containing block, which is the
    // parent element. Resolving `left`/`top` from the parent's origin rather
    // than from the page is what makes a saved position mean the same thing in
    // the browser that it means here.
    let origin_x = if absolute {
        parent_origin.0 + authored_left.unwrap_or(x - parent_origin.0)
    } else {
        x
    };
    let top = if absolute {
        parent_origin.1 + authored_top.unwrap_or(cursor - parent_origin.1)
    } else {
        cursor + margin[0]
    };

    // Children flow inside this node's content box.
    let children: Vec<&crate::source_document::StructuralNode> = all_nodes
        .iter()
        .filter(|node| node.parent.as_ref() == Some(id))
        .collect();
    let content_left = x + padding[3];
    let content_width = (outer_width - padding[1] - padding[3]).max(0.0);

    let mut inner_cursor = top + padding[0];
    let mut content_height: f32 = 0.0;
    for child in &children {
        if let Some(child_box) = layout_node(
            resolved,
            &child.id,
            content_left,
            inner_cursor,
            content_width,
            all_nodes,
            &mut inner_cursor,
            out,
            // The parent's *placed* origin, which for an absolutely positioned
            // parent is not the `x` this node was laid out at.
            (origin_x, top),
        ) {
            content_height = content_height.max(
                (child_box.y + child_box.height + child_box_margin_bottom(resolved, &child.id))
                    - top,
            );
        }
    }

    let height = match explicit_height {
        Some(height) if content_box => height + padding[0] + padding[2],
        Some(height) => height,
        None => {
            let own_text_height = text_height(entry);
            (content_height + own_text_height + padding[0] + padding[2]).max(own_text_height)
        }
    };

    let geometry = BoxGeometry {
        x: origin_x,
        y: top,
        width: outer_width,
        height,
    };
    // An absolutely positioned node does not advance the flow cursor.
    *cursor_out = if absolute {
        *cursor_out
    } else {
        top + height + margin[2]
    };
    out.insert(id.clone(), geometry);
    Some(geometry)
}

/// Properties that inherit, and only those.
///
/// Provisional: CSS inherits a documented list of properties, and this
/// implements just the two that change how text looks. Box properties are
/// deliberately not inherited, which happens to be correct for every property
/// in [`style::SUPPORTED_PROPERTIES`] except `color` and `font-size`. The
/// nearest ancestor that declares the property wins, so a node's own
/// declaration always outranks an inherited one.
const INHERITED_PROPERTIES: &[&str] = &["color", "font-size"];

/// Tags of the elements that enclose `start` in `source` and have no node of
/// their own, outermost first.
///
/// A first-pass scan, not a parse: it tracks open and close tags with a stack and
/// only needs tag names, which is all inheritance needs. Void elements are
/// listed so `<img>` never appears to swallow its siblings. Used to carry a
/// wrapper's authored text colour — most often `body` — down to the nodes inside
/// it, which node-level ancestry alone would miss.
fn unmanaged_ancestors(source: &str, start: usize) -> Vec<String> {
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source",
        "track", "wbr",
    ];
    let mut open: Vec<String> = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < start && index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let Some(offset) = source[index..].find('>') else {
            break;
        };
        let tag = source[index + 1..index + offset].trim();
        index += offset + 1;
        if tag.starts_with('!') {
            // Comment, doctype, or CDATA: not an element boundary.
            continue;
        }
        if let Some(closing) = tag.strip_prefix('/') {
            if let Some(position) = open.iter().rposition(|open| open == closing) {
                open.truncate(position);
            }
            continue;
        }
        let name_end = tag
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        if name.is_empty() || tag.ends_with('/') || VOID.contains(&name.as_str()) {
            continue;
        }
        open.push(name);
    }
    open
}

/// Fill in inherited text properties from the ancestor chain.
///
/// Without this, `body { color: var(--ink) }` never reaches a headline and every
/// text node falls back to the canvas default — which is exactly the "fake
/// visual bridge" this module exists to remove. Walking up rather than
/// iterating in document order keeps it correct whatever order the metadata
/// lists nodes in.
fn inherit_text_properties(
    resolved: &mut BTreeMap<NodeId, Resolved>,
    document: &PersistentDocument,
    sheets: &[(String, Stylesheet)],
    ancestry: &BTreeMap<NodeId, Vec<String>>,
) {
    let parents: BTreeMap<NodeId, Option<NodeId>> = document
        .structure
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.parent.clone()))
        .collect();
    let nodes: Vec<NodeId> = document
        .structure
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();

    for id in nodes {
        // Guard against a parent cycle, which metadata validation should already
        // have rejected, rather than looping forever on malformed input.
        let mut chain = Vec::new();
        let mut cursor = parents.get(&id).cloned().flatten();
        while let Some(current) = cursor {
            if chain.contains(&current) {
                break;
            }
            chain.push(current.clone());
            cursor = parents.get(&current).cloned().flatten();
        }
        // Collect first, then mutate: the ancestor lookup needs shared access to the
        // same map this node is about to be written into.
        let mut inherited: Vec<(&str, String)> = Vec::new();
        for property in INHERITED_PROPERTIES {
            if resolved
                .get(&id)
                .is_some_and(|entry| entry.declarations.contains_key(*property))
            {
                continue;
            }
            // Nearest managed ancestor first, then the nearest unmanaged
            // wrapper. `body { color: … }` lives on an element Spool has no node
            // for, so without the wrapper step that colour never reaches the
            // headline inside it.
            let value = chain
                .iter()
                .filter_map(|ancestor| resolved.get(ancestor))
                .find_map(|ancestor| ancestor.declarations.get(*property).cloned())
                .or_else(|| {
                    ancestry.get(&id).and_then(|tags| {
                        tags.iter().rev().find_map(|tag| {
                            sheets.iter().find_map(|(_, sheet)| {
                                sheet
                                    .resolve(tag, &[], None)
                                    .get(*property)
                                    .map(|resolved| resolved.value.clone())
                            })
                        })
                    })
                });
            if let Some(value) = value {
                inherited.push((property, value));
            }
        }
        if let Some(entry) = resolved.get_mut(&id) {
            for (property, value) in inherited {
                entry.declarations.insert(property.to_owned(), value);
            }
        }
    }
}

fn child_box_margin_bottom(resolved: &BTreeMap<NodeId, Resolved>, id: &NodeId) -> f32 {
    resolved
        .get(id)
        .and_then(|entry| entry.declarations.get("margin"))
        .and_then(|value| style::parse_box(value))
        .map(|margin| margin[2])
        .unwrap_or(0.0)
}

fn text_height(entry: &Resolved) -> f32 {
    if entry.facts.text.is_empty() {
        0.0
    } else {
        let font_size = entry
            .declarations
            .get("font-size")
            .and_then(|value| style::parse_length(value))
            .unwrap_or(16.0);
        font_size * 1.4
    }
}

fn visual_of(
    resolved: &BTreeMap<NodeId, Resolved>,
    id: &NodeId,
    geometry: BoxGeometry,
) -> NodeVisual {
    let entry = &resolved[id];
    let declarations = &entry.declarations;

    let background = declarations
        .get("background-color")
        .or_else(|| declarations.get("background"))
        .and_then(|value| style::parse_color(value));
    let border = declarations
        .get("border")
        .and_then(|value| border_color_and_width(value));

    let style_out = ObjectStyle {
        fill: background.map(|color| Fill { color }),
        stroke: border.map(|(color, width)| Stroke { color, width }),
    };

    let text_color = declarations
        .get("color")
        .and_then(|value| style::parse_color(value));
    let font_size = declarations
        .get("font-size")
        .and_then(|value| style::parse_length(value))
        .unwrap_or(16.0);

    NodeVisual {
        text: (!entry.facts.text.is_empty()).then(|| entry.facts.text.clone()),
        style: style_out,
        geometry,
        text_color,
        font_size,
        // No paint of its own means the renderer default fill and border still
        // apply. This is the visible edge of CSS coverage, so it is recorded
        // rather than hidden. Text colour is not part of it: an inherited or
        // authored colour is used as authored, and only falls back when nothing
        // authored one.
        unstyled: background.is_none() && border.is_none(),
    }
}

/// Parse a `border` shorthand into its colour and width.
fn border_color_and_width(value: &str) -> Option<(Color, f32)> {
    let mut width = 1.0f32;
    let mut color = None;
    for part in value.split_whitespace() {
        if let Some(parsed) = style::parse_length(part) {
            width = parsed;
        } else if let Some(parsed) = style::parse_color(part) {
            color = Some(parsed);
        }
    }
    color.map(|color| (color, width))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixture(fixture: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(fixture)
    }

    fn node(id: &str) -> NodeId {
        NodeId::new(id).expect("valid id")
    }

    /// Write a throwaway project so a case the fixtures deliberately do not
    /// cover still runs against the real loader.
    fn scratch(name: &str, yaml: &str, html: &str, css: Option<&str>) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "spool-visual-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create project");
        std::fs::write(root.join("lamine.yaml"), yaml).expect("write metadata");
        std::fs::write(root.join("index.html"), html).expect("write html");
        if let Some(css) = css {
            std::fs::write(root.join("styles.css"), css).expect("write css");
        }
        root
    }

    fn node_yaml(id: &str, name: &str, kind: &str, parent: Option<&str>, children: &str) -> String {
        format!(
            "  - id: \"{id}\"\n    name: \"{name}\"\n    kind: \"{kind}\"\n    parent: {parent}\n    file: \"index.html\"\n    selector: \"[data-spool-id=\\\"{id}\\\"]\"\n    children: [{children}]\n",
            parent = parent
                .map(|parent| format!("\"{parent}\""))
                .unwrap_or_else(|| "null".to_owned()),
        )
    }

    #[test]
    fn the_landing_fixture_produces_authored_text_paint_and_geometry() {
        let visuals = crate::project_open::open_project(fixture("landing"))
            .expect("landing opens")
            .visuals;

        let frame = visuals.get(&node("spool-frame-root")).expect("frame");
        assert_eq!(
            frame.geometry,
            BoxGeometry {
                x: 0.0,
                y: 0.0,
                width: DEFAULT_CONTENT_WIDTH,
                // The frame holds the headline and the CTA and nothing else.
                height: 22.4 + 46.4,
            }
        );
        assert!(
            frame.unstyled,
            "no background or border is authored for the frame"
        );

        let headline = visuals.get(&node("spool-text-headline")).expect("headline");
        assert_eq!(
            headline.text.as_deref(),
            Some("Design in source, structure in Spool")
        );
        assert_eq!(
            headline.text_color,
            Some(Color::from_rgb(0x16161d)),
            "the ink custom property on :root, inherited from body"
        );
        assert!(headline.unstyled, "it has a colour but no paint of its own");

        let cta = visuals.get(&node("spool-cta-primary")).expect("cta");
        assert_eq!(cta.text.as_deref(), Some("Start designing"));
        assert_eq!(
            cta.style.fill,
            Some(Fill {
                color: Color::from_rgb(0x3b5bfd)
            }),
            "the accent custom property resolves through var()"
        );
        assert_eq!(cta.text_color, Some(Color::from_rgb(0xffffff)));
        assert!(!cta.unstyled, "the CTA is fully styled");
        assert_eq!(
            cta.geometry,
            BoxGeometry {
                x: 0.0,
                // Below the headline: block flow, in document order.
                y: 22.4,
                width: DEFAULT_CONTENT_WIDTH,
                // Text line plus 12px of vertical padding, top and bottom.
                height: 22.4 + 24.0,
            }
        );
    }

    #[test]
    fn a_parent_never_inherits_its_childs_class_or_text() {
        // The regression this exists for: reading attributes from the whole
        // element fragment rather than its own open tag made `main` match
        // `.cta`, so the frame came out with the button's blue background and
        // its label.
        let visuals = crate::project_open::open_project(fixture("landing"))
            .expect("landing opens")
            .visuals;
        let frame = visuals.get(&node("spool-frame-root")).expect("frame");
        assert_eq!(frame.style.fill, None, "the frame has no authored fill");
        assert_eq!(frame.text, None, "and no text of its own");
    }

    #[test]
    fn an_inline_style_outranks_the_stylesheet() {
        let yaml = format!(
            "version: 1\nnodes:\n{}{}",
            node_yaml("spool-card", "Card", "frame", None, ""),
            node_yaml("spool-label", "Label", "text", Some("spool-card"), ""),
        );
        let html = "<!doctype html>\n<body>\n  <div data-spool-id=\"spool-card\" style=\"background-color: #ff0000; padding: 4px\">\n    <span data-spool-id=\"spool-label\" style=\"color: #00ff00\">Hi</span>\n  </div>\n</body>\n";
        let root = scratch(
            "inline",
            &yaml,
            html,
            Some(".card, div { background-color: #0000ff; }\n"),
        );
        let visuals = crate::project_open::open_project(&root)
            .expect("opens")
            .visuals;

        assert_eq!(
            visuals.get(&node("spool-card")).expect("card").style.fill,
            Some(Fill {
                color: Color::from_rgb(0xff0000)
            }),
            "the inline declaration wins over the stylesheet"
        );
        assert_eq!(
            visuals.get(&node("spool-label")).expect("label").text_color,
            Some(Color::from_rgb(0x00ff00))
        );
        assert_eq!(
            visuals
                .get(&node("spool-label"))
                .expect("label")
                .text
                .as_deref(),
            Some("Hi")
        );
    }

    #[test]
    fn a_moved_element_comes_back_where_the_editor_left_it() {
        // What a save writes is an absolutely positioned inline style; this is
        // the read side of that same pair.
        let yaml = format!(
            "version: 1\nnodes:\n{}",
            node_yaml("spool-box", "Box", "frame", None, "")
        );
        let html = "<!doctype html>\n<body>\n  <div data-spool-id=\"spool-box\" style=\"position: absolute; left: 40px; top: 120px; width: 220px; height: 48px\">Box</div>\n</body>\n";
        let root = scratch("absolute", &yaml, html, None);
        let visuals = crate::project_open::open_project(&root)
            .expect("opens")
            .visuals;
        let box_ = visuals.get(&node("spool-box")).expect("box");
        assert_eq!(
            box_.geometry,
            BoxGeometry {
                x: 40.0,
                y: 120.0,
                width: 220.0,
                height: 48.0
            },
            "an authored position is honoured exactly"
        );
    }

    #[test]
    fn layout_is_deterministic_for_the_same_project() {
        let first = crate::project_open::open_project(fixture("landing"))
            .expect("opens")
            .visuals;
        let second = crate::project_open::open_project(fixture("landing"))
            .expect("opens")
            .visuals;
        assert_eq!(
            first, second,
            "the same source must always produce the same model"
        );
        assert_eq!(first.unstyled().len(), 2, "the frame and the headline");
    }

    #[test]
    fn unmanaged_ancestors_finds_the_wrapper_a_node_sits_in() {
        let source = "<!doctype html>\n<html>\n  <body class=\"theme\">\n    <main>\n      <img src=\"a.png\" />\n      <h1>Hi</h1>\n    </main>\n  </body>\n</html>\n";
        let start = source.find("<h1>").expect("h1 present");
        assert_eq!(
            unmanaged_ancestors(source, start),
            vec!["html".to_owned(), "body".to_owned(), "main".to_owned()],
            "the still-open wrappers, outermost first"
        );

        // A void element must not appear to contain what follows it.
        let img = source.find("<img").unwrap();
        let after_img = source[img..].find('>').unwrap() + img + 1;
        assert_eq!(
            unmanaged_ancestors(source, after_img),
            vec!["html".to_owned(), "body".to_owned(), "main".to_owned()],
            "an `<img>` opens nothing, so its siblings share its ancestors"
        );

        // A closed element is no longer an ancestor.
        let after = source.find("</main>").unwrap() + "</main>".len();
        assert_eq!(
            unmanaged_ancestors(source, after),
            vec!["html".to_owned(), "body".to_owned()]
        );
    }

    #[test]
    fn a_wrapper_carries_its_text_colour_into_the_nodes_inside_it() {
        // `body { color: … }` is authored on an element Spool has no node for.
        // Without the wrapper step the headline would fall back to the canvas
        // default and the opened project would not look like its source.
        let visuals = crate::project_open::open_project(fixture("landing"))
            .expect("landing opens")
            .visuals;
        assert_eq!(
            visuals
                .get(&node("spool-text-headline"))
                .expect("headline")
                .text_color,
            Some(Color::from_rgb(0x16161d)),
            "body's authored ink reaches the headline inside it"
        );
        assert_eq!(
            visuals
                .get(&node("spool-cta-primary"))
                .expect("cta")
                .text_color,
            Some(Color::from_rgb(0xffffff)),
            "and a node's own declaration still outranks the inherited one"
        );
        let unstyled: Vec<&str> = visuals.unstyled().iter().map(|id| id.as_str()).collect();
        assert_eq!(
            unstyled,
            vec!["spool-frame-root", "spool-text-headline"],
            "only the CTA authors any paint, so it is the only fully styled node"
        );
    }
}
