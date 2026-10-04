//! Source identity binding: relating `lamine.yaml` node IDs to authored HTML
//! elements without touching the author's bytes.
//!
//! This answers one question. Given authored HTML carrying `data-spool-id`
//! markers plus Spool metadata, can Spool say which element each NodeId refers
//! to, where that element lives in the file, and what surrounds it — without
//! parsing the document into a DOM and writing it back?
//!
//! It can, because this is built on a tree-sitter **concrete syntax tree**.
//! Every node has exact byte ranges and the tree reproduces the input exactly,
//! so unchanged regions are never re-serialized. `docs/research/architecture/
//! spool-html-document-research.md` (§8, §9) reaches the same conclusion and
//! rules out the alternatives on evidence: html5ever's `TreeSink` receives
//! only a line number, with no byte offsets, and lightningcss's `to_css()`
//! rewrites the whole stylesheet. Neither can splice.
//!
//! # What this is not
//!
//! This is **source identity/binding support**, not HTML editing support. The
//! only mutation is [`retarget_identity`], which changes a `data-spool-id`
//! value in place. There is no arbitrary HTML editing here.
//!
//! CSS is out of scope: a `.css` file is an opaque source file that may be
//! referenced, and this module never parses, resolves, or cascades it.

use std::collections::BTreeMap;
use std::path::Path;

use tree_sitter::{Node, Parser, Tree};

use crate::source_document::{LamineStructure, ModelError, NodeId};

/// The authored attribute that binds an HTML element to a Spool node.
pub const IDENTITY_ATTRIBUTE: &str = "data-spool-id";

/// Whether a source file holds markup this phase can bind identities in.
///
/// CSS is deliberately not one of these: this phase resolves element identity
/// only, and a stylesheet is an opaque referenced file.
pub fn is_markup(file: &str) -> bool {
    let extension = Path::new(file)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(extension.as_str(), "html" | "htm" | "xhtml" | "svg")
}

/// A byte range in one source file. Ranges are half-open: `start..end`.
pub type ByteRange = std::ops::Range<usize>;

/// One authored element that carries a `data-spool-id`, located by byte range.
///
/// This is a projection of the CST, not a DOM node: it holds ranges and the
/// element's tag name, and nothing that would require re-serializing source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementBinding {
    /// The authored `data-spool-id` value, already validated as a NodeId.
    pub id: NodeId,
    /// The authored tag name, e.g. `main`, `h1`, `a`.
    pub tag: String,
    /// Range of the whole element, including its tags.
    pub element_range: ByteRange,
    /// Range of the `data-spool-id` value token alone — inside the quotes for
    /// a quoted value, and the token itself when unquoted. This is the only
    /// range a write may touch.
    pub value_range: ByteRange,
    /// Whether the authored value was quoted. A rewrite preserves this.
    pub quoted: bool,
    /// Index of the nearest ancestor element that also carries an identity,
    /// when one exists. Structural parentage between *bound* elements.
    pub parent: Option<usize>,
    /// Indices of directly nested bound elements, in document order.
    pub children: Vec<usize>,
    /// The element's own text: the exact source range holding it, when it has
    /// one.
    ///
    /// `None` when the content is not a single run of text — an element with
    /// no children, with a nested element, or with markup between its words.
    /// Character references count as content, so `R&amp;D` is one run.
    /// That is not a failure: it is the honest answer to "which bytes does this
    /// node's text own?", and a writer that gets `None` reports the edit as
    /// unsupported instead of guessing at a range. Recorded from the same parse
    /// as `element_range`, so text ownership and element ownership cannot drift
    /// apart.
    pub text_range: Option<ByteRange>,
}

/// Every identity-bearing element in one file, in document order.
#[derive(Clone, Debug)]
pub struct SourceIndex {
    /// The exact bytes this index was built from. Kept so a caller can prove
    /// a round trip was byte-identical and so patches are checked against it.
    pub source: String,
    bindings: Vec<ElementBinding>,
    /// Every element in the file, identity-bearing or not, in document order.
    ///
    /// A census rather than a lookup: answering "which authored elements does
    /// this stylesheet rule govern?" needs the elements no node claims just as
    /// much as the ones it does, because a rule shared with an unmarked element
    /// is still shared. Recorded from the same parse as `bindings`, so a range
    /// here describes the same bytes a binding describes.
    elements: Vec<ByteRange>,
    /// Whether the parser recovered from a malformed region. Error recovery
    /// means ranges may not reflect a browser's tree, so it is surfaced.
    has_error: bool,
}

impl SourceIndex {
    /// Index the `data-spool-id` attributes in `source`.
    ///
    /// This never fails on malformed markup: tree-sitter recovers, and the
    /// index records that it did. Identity problems are reported later by
    /// [`reconcile`], which needs the metadata to compare against.
    pub fn parse(source: &str) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_html::LANGUAGE.into())
            .expect("tree-sitter-html is a statically linked language");
        let tree = parser.parse(source, None).expect("parsing in-memory text");

        let mut builder = IndexBuilder {
            source,
            bindings: Vec::new(),
            elements: Vec::new(),
            stack: Vec::new(),
            has_error: tree.root_node().has_error(),
        };
        builder.walk(tree.root_node());

        Self {
            source: source.to_owned(),
            bindings: builder.bindings,
            elements: builder.elements,
            has_error: builder.has_error,
        }
    }

    /// Parse and hold the CST. The tree is kept private: callers get byte
    /// ranges, which is what source-preserving edits need.
    fn parse_with_tree(source: &str) -> (Self, Tree) {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_html::LANGUAGE.into())
            .expect("tree-sitter-html is a statically linked language");
        let tree = parser.parse(source, None).expect("parsing in-memory text");

        let mut builder = IndexBuilder {
            source,
            bindings: Vec::new(),
            elements: Vec::new(),
            stack: Vec::new(),
            has_error: tree.root_node().has_error(),
        };
        builder.walk(tree.root_node());

        (
            Self {
                source: source.to_owned(),
                bindings: builder.bindings,
                elements: builder.elements,
                has_error: builder.has_error,
            },
            tree,
        )
    }

    pub fn bindings(&self) -> &[ElementBinding] {
        &self.bindings
    }

    /// The range of every authored element, in document order.
    ///
    /// Includes elements no `data-spool-id` claims. That is the point: a
    /// stylesheet rule that also governs an unmarked element is still a shared
    /// rule, and a writer that only asked about bound elements would not know.
    pub fn element_ranges(&self) -> &[ByteRange] {
        &self.elements
    }

    /// True when the parser had to recover from malformed markup.
    pub fn has_syntax_errors(&self) -> bool {
        self.has_error
    }

    /// Look up a binding by its authored id.
    pub fn find(&self, id: &NodeId) -> Option<&ElementBinding> {
        self.bindings.iter().find(|binding| &binding.id == id)
    }
}

struct IndexBuilder<'a> {
    source: &'a str,
    bindings: Vec<ElementBinding>,
    /// Every element seen, whether or not it carries an identity.
    elements: Vec<ByteRange>,
    /// Indices of currently-open bound elements, outermost first.
    stack: Vec<usize>,
    has_error: bool,
}

impl<'a> IndexBuilder<'a> {
    fn walk(&mut self, node: Node) {
        if node.has_error() {
            self.has_error = true;
        }

        if node.kind() == "element" {
            self.visit_element(node);
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk(child);
        }
    }

    fn visit_element(&mut self, node: Node) {
        // Every element joins the census before anything else is decided, so
        // an unmarked element is as countable as a bound one.
        self.elements.push(node.byte_range());

        // An element may carry `data-spool-id` more than once. Both are
        // recorded here so `reconcile` can report the ambiguity instead of
        // silently binding to whichever came first.
        let identities = self.identity_attributes(node);
        let tag = first_tag_name(node)
            .map(|name| text(self.source, name).to_ascii_lowercase())
            .unwrap_or_default();

        if identities.is_empty() {
            // Not an identity element, but it may still contain some. Keep
            // walking with the current parent stack so nesting stays correct.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.walk(child);
            }
            return;
        }

        let parent = self.stack.last().copied();
        // Every identity on this element is recorded so reconcile can report a
        // duplicate as ambiguous, but only the first one establishes nesting:
        // a second attribute on the same element does not own children.
        let (first, rest) = identities.split_first().expect("identities was non-empty");
        let text_range = self.own_text_range(node);
        let opened = self.push_binding(
            first.clone(),
            &tag,
            node.byte_range(),
            parent,
            text_range.clone(),
        );
        for extra in rest {
            self.push_binding(
                extra.clone(),
                &tag,
                node.byte_range(),
                parent,
                text_range.clone(),
            );
        }
        if let Some(parent_index) = parent {
            self.bindings[parent_index].children.push(opened);
        }
        self.stack.push(opened);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk(child);
        }

        // Close this element, and any ancestors the parser left open.
        while let Some(top) = self.stack.last().copied() {
            self.stack.pop();
            if top == opened {
                break;
            }
        }
    }

    /// Record one bound element and return its index.
    fn push_binding(
        &mut self,
        identity: (NodeId, ByteRange, bool),
        tag: &str,
        element_range: ByteRange,
        parent: Option<usize>,
        text_range: Option<ByteRange>,
    ) -> usize {
        let index = self.bindings.len();
        self.bindings.push(ElementBinding {
            id: identity.0,
            tag: tag.to_owned(),
            element_range,
            value_range: identity.1,
            quoted: identity.2,
            parent,
            children: Vec::new(),
            text_range,
        });
        index
    }

    /// The source range this element's own text occupies.
    ///
    /// Text ownership is all-or-nothing on purpose. `<h1>Hello</h1>` owns
    /// `Hello`, and `<h1>Hello <em>world</em></h1>` owns nothing: rewriting one
    /// range there would either drop the `<em>` or write text across it. An
    /// element whose children are exactly content nodes owns the span from the
    /// first to the last of them, which covers the whitespace between them
    /// without ever crossing a tag.
    fn own_text_range(&self, element: Node) -> Option<ByteRange> {
        let mut content: Vec<Node> = Vec::new();
        let mut cursor = element.walk();
        for child in element.children(&mut cursor) {
            // The start tag and end tag are structural, not content.
            if matches!(child.kind(), "start_tag" | "end_tag") {
                continue;
            }
            if !is_content(child.kind()) {
                return None;
            }
            content.push(child);
        }
        let first = content.first()?;
        let last = content.last()?;
        let start = first.start_byte();
        let end = last.end_byte();
        (start < end).then_some(start..end)
    }

    /// Collect every `data-spool-id` attribute on this element, in order.
    ///
    /// Returns `(NodeId, value_range, quoted)`. A value that is absent, empty,
    /// or not a legal NodeId is skipped here and reported by `reconcile` via
    /// the element's recorded identity problems; this pass only records what
    /// can be bound.
    fn identity_attributes(&self, element: Node) -> Vec<(NodeId, ByteRange, bool)> {
        let mut found = Vec::new();
        // Attributes hang off `start_tag`, which is not among the element's
        // named children, so it is reached explicitly.
        let Some(start_tag) = element.child(0) else {
            return found;
        };
        let mut cursor = start_tag.walk();
        for child in start_tag.children(&mut cursor) {
            if child.kind() != "attribute" {
                continue;
            }
            let name = match child.named_child(0) {
                Some(name) if name.kind() == "attribute_name" => text(self.source, name),
                _ => continue,
            };
            // Attribute names are matched case-sensitively: HTML treats them
            // case-insensitively, but `lamine.yaml` selectors are exact, and
            // guessing at casing would make a binding unreproducible.
            if name != IDENTITY_ATTRIBUTE {
                continue;
            }
            if let Some((id, range, quoted)) = self.read_identity(child) {
                found.push((id, range, quoted));
            }
        }
        found
    }

    /// Read one `data-spool-id` attribute's value and its exact byte range.
    fn read_identity(&self, attribute: Node) -> Option<(NodeId, ByteRange, bool)> {
        let mut cursor = attribute.walk();
        let mut quoted = false;
        let mut value: Option<Node> = None;
        for child in attribute.children(&mut cursor) {
            match child.kind() {
                "quoted_attribute_value" => {
                    quoted = true;
                    value = child.named_child(0);
                }
                "attribute_value" => value = Some(child),
                _ => {}
            }
        }
        // No value at all (`data-spool-id`) or an empty one (`data-spool-id=""`).
        // Both are unbindable; reconcile reports them.
        let value = value?;
        let raw = text(self.source, value);
        let id = NodeId::new(raw).ok()?;
        Some((id, value.byte_range(), quoted))
    }
}

/// Whether a node kind is element *content* rather than markup.
///
/// tree-sitter-html calls a character reference `&amp;` its own `entity` node,
/// not a `text` node. It is content: `R&amp;D` is one run of text the author
/// wrote, and treating the entity as markup would make every element that uses
/// an ampersand, an em dash, or a non-breaking space permanently un-editable.
fn is_content(kind: &str) -> bool {
    matches!(kind, "text" | "entity")
}

fn text<'a>(source: &'a str, node: Node) -> &'a str {
    node.utf8_text(source.as_bytes())
        .expect("a parsed node range is valid UTF-8")
}

/// The element's tag name, read from its start tag.
///
/// `start_tag` is not among the element's named children, so it is reached
/// through `child(0)` rather than `children()`.
fn first_tag_name(element: Node) -> Option<Node> {
    let tag = element.child(0)?;
    // A void or self-closed element such as `<img … />` is a `self_closing_tag`
    // rather than a `start_tag`; both carry the name the same way.
    if !matches!(tag.kind(), "start_tag" | "self_closing_tag") {
        return None;
    }
    let mut cursor = tag.walk();
    let tag_name = tag
        .children(&mut cursor)
        .find(|node| node.kind() == "tag_name");
    tag_name
}

/// A binding problem that must be reported rather than resolved by guessing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingError {
    /// The same `data-spool-id` appears on more than one element or attribute.
    DuplicateIdentity {
        id: String,
        first: ByteRange,
        second: ByteRange,
    },
    /// An authored `data-spool-id` value is not a legal NodeId.
    MalformedIdentity { raw: String, range: ByteRange },
    /// An element carries `data-spool-id` with no usable value.
    EmptyIdentity { range: ByteRange },
    /// Metadata names a node that no authored element claims.
    MissingElement { id: NodeId, file: String },
    /// An authored element carries an identity no metadata node declares.
    UndeclaredElement { id: NodeId, file: String },
    /// The element a node binds to is not the one its metadata parent implies.
    ParentMismatch {
        id: NodeId,
        file: String,
        /// The parent metadata declares, or `None` if metadata says it is a root.
        expected: Option<NodeId>,
        /// The parent the authored nesting gives it, or `None` if it is one.
        actual: Option<NodeId>,
    },
    /// The metadata's source binding is not the `[data-spool-id="…"]` form
    /// this phase supports.
    UnsupportedSelector {
        id: NodeId,
        file: String,
        selector: String,
    },
    /// Metadata itself failed its own validation.
    Metadata(ModelError),
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateIdentity { id, first, second } => write!(
                f,
                "data-spool-id {id:?} appears twice ({first:?} and {second:?}); the binding would be ambiguous"
            ),
            Self::MalformedIdentity { raw, range } => write!(
                f,
                "data-spool-id value {raw:?} at {range:?} is not a legal Spool node ID"
            ),
            Self::EmptyIdentity { range } => write!(
                f,
                "data-spool-id at {range:?} has no value, so it cannot identify a node"
            ),
            Self::MissingElement { id, file } => write!(
                f,
                "node {} is declared in metadata but no element in {file} carries that identity",
                id.as_str()
            ),
            Self::UndeclaredElement { id, file } => write!(
                f,
                "element in {file} carries data-spool-id {} but no metadata node declares it",
                id.as_str()
            ),
            Self::ParentMismatch {
                id,
                file,
                expected,
                actual,
            } => write!(
                f,
                "node {} in {file} has parent {} in metadata but {actual:?} in the HTML",
                id.as_str(),
                describe_parent(expected)
            ),
            Self::UnsupportedSelector { id, file, selector } => write!(
                f,
                "node {} in {file} binds with selector {selector:?}; only [data-spool-id=\"…\"] is supported",
                id.as_str()
            ),
            Self::Metadata(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata(error) => Some(error),
            _ => None,
        }
    }
}

/// The resolved relationship between metadata and authored source.
#[derive(Clone, Debug)]
pub struct Reconciliation {
    /// One index per bound source file, keyed by the metadata's file name.
    pub files: BTreeMap<String, SourceIndex>,
    /// For each metadata node: where it was found, and how it nests.
    pub bound: BTreeMap<NodeId, BoundNode>,
}

/// A metadata node resolved against authored source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundNode {
    pub id: NodeId,
    pub file: String,
    /// Tag name of the element this node binds to.
    pub tag: String,
    /// Range of the bound element.
    pub element_range: ByteRange,
    /// Range of the `data-spool-id` value token.
    pub value_range: ByteRange,
    /// Resolved metadata parent, cross-checked against HTML nesting.
    pub parent: Option<NodeId>,
    /// Resolved children, cross-checked against HTML nesting.
    pub children: Vec<NodeId>,
}

impl Reconciliation {
    /// The bytes this reconciliation was built from, for round-trip proofs.
    pub fn source(&self, file: &str) -> Option<&str> {
        self.files.get(file).map(|index| index.source.as_str())
    }
}

/// Bind `lamine.yaml` metadata to the authored elements it names.
///
/// Returns every problem found, not just the first: a half-applied binding is
/// exactly the failure this phase exists to prevent, so the caller gets the
/// full picture and decides what to do. An empty result means every metadata
/// node resolved to exactly one authored element, and every authored identity
/// is declared.
pub fn reconcile(
    structure: &LamineStructure,
    sources: &BTreeMap<String, String>,
) -> Result<Reconciliation, Vec<BindingError>> {
    let mut errors = Vec::new();
    if let Err(error) = structure.validate() {
        errors.push(BindingError::Metadata(error));
        return Err(errors);
    }

    // Index every supplied HTML source. Metadata names the files it binds to,
    // but scanning all of them means a malformed identity in a file that
    // nothing references is still reported rather than silently ignored.
    let mut files: BTreeMap<String, SourceIndex> = BTreeMap::new();
    for (file, source) in sources {
        if is_markup(file) {
            files.insert(file.clone(), SourceIndex::parse(source));
        }
    }

    // Every authored identity must be unique within its file, and must be a
    // legal NodeId. Duplicates are ambiguous and are never auto-resolved.
    let mut owners: BTreeMap<(String, String), ByteRange> = BTreeMap::new();
    for (file, index) in &files {
        for binding in index.bindings() {
            let key = (file.clone(), binding.id.as_str().to_owned());
            match owners.get(&key) {
                Some(first) => errors.push(BindingError::DuplicateIdentity {
                    id: binding.id.as_str().to_owned(),
                    first: first.clone(),
                    second: binding.value_range.clone(),
                }),
                None => {
                    owners.insert(key, binding.value_range.clone());
                }
            }
        }
        report_malformed_identities(index, &mut errors);
    }

    // Bind each metadata node to exactly one element.
    let mut bound = BTreeMap::new();
    for node in &structure.nodes {
        // Only the `[data-spool-id="…"]` selector form is supported in this
        // phase; anything else is reported rather than approximated.
        let expected = format!("[{IDENTITY_ATTRIBUTE}=\"{}\"]", node.id.as_str());
        if node.source.selector != expected {
            errors.push(BindingError::UnsupportedSelector {
                id: node.id.clone(),
                file: node.source.file.clone(),
                selector: node.source.selector.clone(),
            });
            continue;
        }

        let Some(index) = files.get(&node.source.file) else {
            errors.push(BindingError::MissingElement {
                id: node.id.clone(),
                file: node.source.file.clone(),
            });
            continue;
        };
        let Some(binding) = index.find(&node.id) else {
            errors.push(BindingError::MissingElement {
                id: node.id.clone(),
                file: node.source.file.clone(),
            });
            continue;
        };

        // Cross-check metadata hierarchy against authored nesting. These are
        // reported as mismatches, not corrected: the source is canonical.
        let index_for_file = &files[&node.source.file];
        let actual_parent = binding
            .parent
            .map(|index| index_for_file.bindings()[index].id.clone());
        // The comparison is bidirectional. Checking only the case where
        // metadata declares a parent would miss the commoner disagreement:
        // metadata calls a node a root while the HTML nests it inside another.
        if node.parent != actual_parent {
            errors.push(BindingError::ParentMismatch {
                id: node.id.clone(),
                file: node.source.file.clone(),
                expected: node.parent.clone(),
                actual: actual_parent,
            });
        }
        let children: Vec<NodeId> = binding
            .children
            .iter()
            .map(|index| files[&node.source.file].bindings()[*index].id.clone())
            .collect();

        bound.insert(
            node.id.clone(),
            BoundNode {
                id: node.id.clone(),
                file: node.source.file.clone(),
                tag: binding.tag.clone(),
                element_range: binding.element_range.clone(),
                value_range: binding.value_range.clone(),
                parent: node.parent.clone(),
                children,
            },
        );
    }

    // Every authored identity should be declared by metadata. An undeclared
    // element is a real disagreement between the two, not noise.
    for (file, index) in &files {
        for binding in index.bindings() {
            if !bound.contains_key(&binding.id) {
                errors.push(BindingError::UndeclaredElement {
                    id: binding.id.clone(),
                    file: file.clone(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(Reconciliation { files, bound })
    } else {
        Err(errors)
    }
}

/// Render an optional parent for an error message.
fn describe_parent(parent: &Option<NodeId>) -> String {
    parent
        .as_ref()
        .map(|id| id.as_str().to_owned())
        .unwrap_or_else(|| "no parent".into())
}

/// Report identity values that could not be bound because they are malformed
/// or empty. These are found during indexing and reported here.
fn report_malformed_identities(index: &SourceIndex, errors: &mut Vec<BindingError>) {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_html::LANGUAGE.into())
        .expect("tree-sitter-html is a statically linked language");
    let Some(tree) = parser.parse(index.source.as_bytes(), None) else {
        return;
    };

    let mut problems = Vec::new();
    collect_identity_problems(tree.root_node(), index.source.as_bytes(), &mut problems);
    problems.sort_by_key(|(range, _)| range.start);
    problems.dedup_by_key(|(range, _)| range.clone());
    for (_, problem) in problems {
        errors.push(problem);
    }
}

fn collect_identity_problems(node: Node, source: &[u8], out: &mut Vec<(ByteRange, BindingError)>) {
    // `start_tag` is not among an element's named children, so attributes are
    // only reachable by visiting every child, not just the named ones.
    if node.kind() == "attribute" {
        let name = node
            .named_child(0)
            .filter(|name| name.kind() == "attribute_name")
            .and_then(|name| name.utf8_text(source).ok());
        if name == Some(IDENTITY_ATTRIBUTE) {
            let mut cursor = node.walk();
            let mut raw: Option<(String, ByteRange)> = None;
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "quoted_attribute_value" => {
                        if let Some(value) = child.named_child(0) {
                            raw = value
                                .utf8_text(source)
                                .ok()
                                .map(|text| (text.to_owned(), value.byte_range()));
                        }
                    }
                    "attribute_value" => {
                        raw = child
                            .utf8_text(source)
                            .ok()
                            .map(|text| (text.to_owned(), child.byte_range()));
                    }
                    _ => {}
                }
            }
            // Three distinct problems, none of which can be bound:
            //   - no value at all: `data-spool-id`
            //   - an empty value: `data-spool-id=""`
            //   - a value that is not a legal NodeId: `data-spool-id="a b"`
            let problem = match raw {
                Some((text, range)) if text.is_empty() => {
                    Some(BindingError::EmptyIdentity { range })
                }
                Some((text, range)) if NodeId::new(text.as_str()).is_err() => {
                    Some(BindingError::MalformedIdentity { raw: text, range })
                }
                Some(_) => None,
                // No value node at all: either the attribute was bare
                // (`data-spool-id`) or it was quoted but empty
                // (`data-spool-id=""`), which the grammar represents as a
                // `quoted_attribute_value` with no inner `attribute_value`.
                // Both mean the same thing to a binding.
                None => Some(BindingError::EmptyIdentity {
                    range: node.byte_range(),
                }),
            };
            if let Some(problem) = problem {
                out.push((node.byte_range(), problem));
            }
        }
    }

    // `children` (not `named_children`) so that anonymous structural nodes such
    // as `start_tag` are traversed and their attributes are reached.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identity_problems(child, source, out);
    }
}

/// The result of an identity edit, describing exactly what changed.
///
/// `edited_range` indexes [`contents`] — the file *after* the edit — while
/// `original_range` indexes the file it replaced. They differ by the length
/// delta, which is why both are named explicitly rather than sharing one
/// field that callers would have to guess the meaning of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityPatch {
    /// The full new file contents.
    pub contents: String,
    /// Range of the replaced value within the original file.
    pub original_range: ByteRange,
    /// Range of the new value within `contents`.
    pub edited_range: ByteRange,
    /// What was there before.
    pub before: String,
    /// What is there now.
    pub after: String,
}

/// The single mutation primitive in this phase: change one element's
/// `data-spool-id` value, leaving every other byte of the file untouched.
///
/// The replacement is written into the exact byte range the CST reported for
/// the value token, so surrounding whitespace, attribute order, quote style,
/// comments, and unrelated elements are preserved verbatim. No reparse and no
/// reserialization happens; this is a splice.
///
/// A value that differs only in characters already legal in a NodeId keeps the
/// authored quoting. The new value is otherwise written as authored, and the
/// caller is responsible for having validated `new_id`.
pub fn retarget_identity(
    source: &str,
    binding: &ElementBinding,
    new_id: &NodeId,
) -> Result<IdentityPatch, BindingError> {
    // Re-read the recorded range from the live source so a stale binding is an
    // error rather than a silent corruption of unrelated text.
    let (index, _tree) = SourceIndex::parse_with_tree(source);
    let current = index
        .find(&binding.id)
        .ok_or_else(|| BindingError::MissingElement {
            id: binding.id.clone(),
            file: String::new(),
        })?;
    if current.value_range != binding.value_range {
        // The file changed under us. Refuse rather than splice the wrong span.
        return Err(BindingError::MalformedIdentity {
            raw: binding.id.as_str().to_owned(),
            range: binding.value_range.clone(),
        });
    }

    let original_range = current.value_range.clone();
    let before = source[original_range.clone()].to_owned();
    let after = new_id.as_str().to_owned();

    // Splice, do not re-serialize: copy the bytes before the value, write the
    // new one, then copy the bytes after it. Nothing else is reconstructed,
    // so whitespace, attribute order, quote style, comments and unrelated
    // elements survive verbatim.
    let mut contents = String::with_capacity(source.len() + after.len());
    contents.push_str(&source[..original_range.start]);
    contents.push_str(&after);
    contents.push_str(&source[original_range.end..]);

    Ok(IdentityPatch {
        edited_range: original_range.start..original_range.start + after.len(),
        contents,
        original_range,
        before,
        after,
    })
}

#[test]
fn a_bound_element_owns_exactly_the_text_the_author_wrote() {
    // Text ownership is what makes a text edit source-preserving. The range
    // must be the authored run itself: writing anywhere else would edit the
    // markup around it.
    let html = "<!doctype html>\n<body>\n  <h1 data-spool-id=\"h\">Design in source</h1>\n  <a data-spool-id=\"a\" class=\"cta\">Start</a>\n  <div data-spool-id=\"d\"><span>nested</span></div>\n  <p data-spool-id=\"e\"></p>\n</body>\n";
    let index = SourceIndex::parse(html);
    let id = |value: &str| NodeId::new(value).expect("valid");

    let heading = index.find(&id("h")).expect("h1 binds");
    let range = heading
        .text_range
        .clone()
        .expect("the heading owns its text");
    assert_eq!(&html[range], "Design in source");

    let link = index.find(&id("a")).expect("a binds");
    assert_eq!(
        &html[link.text_range.clone().expect("link owns its text")],
        "Start"
    );

    // Mixed content owns nothing, and says so.
    assert_eq!(
        index.find(&id("d")).expect("div binds").text_range,
        None,
        "an element wrapping a child must not claim a text range that spans it"
    );
    // An empty element has nothing to own.
    assert_eq!(index.find(&id("e")).expect("p binds").text_range, None);
}

#[test]
fn text_ownership_survives_attributes_and_whitespace_inside_the_element() {
    // The span runs from the first to the last text node, so the
    // whitespace between them is included and the surrounding markup is
    // never inside it.
    let html = "<!doctype html>\n<body>\n  <a data-spool-id=\"a\"\n     class=\"cta\"\n     href=\"#x\">Start designing</a>\n</body>\n";
    let index = SourceIndex::parse(html);
    let binding = index
        .find(&NodeId::new("a").expect("valid"))
        .expect("a binds");
    let range = binding.text_range.clone().expect("owns its text");
    assert_eq!(&html[range.clone()], "Start designing");
    assert!(
        range.start > binding.element_range.start && range.end < binding.element_range.end,
        "the text range sits strictly inside the element"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_document::{SourceBinding, StructuralNode};

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/awkward")
            .join(name)
    }

    fn awkward_source() -> String {
        std::fs::read_to_string(fixture_path("loose.html")).expect("awkward fixture")
    }

    fn awkward_metadata() -> LamineStructure {
        LamineStructure::from_yaml(
            &std::fs::read_to_string(fixture_path("lamine.yaml")).expect("awkward metadata"),
        )
        .expect("awkward metadata parses")
    }

    fn awkward_sources() -> BTreeMap<String, String> {
        BTreeMap::from([("loose.html".to_owned(), awkward_source())])
    }

    fn id(value: &str) -> NodeId {
        NodeId::new(value).expect("legal node id")
    }

    // -- Invariant 1: parse -> bind -> emit without edits is byte-identical.

    #[test]
    fn index_reproduces_the_authored_bytes_exactly() {
        let source = awkward_source();
        let index = SourceIndex::parse(&source);

        // The index holds the exact input, not a re-serialization of it.
        assert_eq!(index.source, source);
        assert_eq!(index.source.as_bytes(), source.as_bytes());

        // Every binding's element range must slice back to real authored text
        // beginning with '<', which proves ranges point at elements.
        for binding in index.bindings() {
            let slice = &source[binding.element_range.clone()];
            assert!(
                slice.starts_with('<'),
                "range should start an element: {slice:?}"
            );
            assert!(!index.has_syntax_errors(), "fixture should parse cleanly");
        }
    }

    #[test]
    fn reconcile_binds_every_node_and_reports_nothing() {
        let reconciliation =
            reconcile(&awkward_metadata(), &awkward_sources()).expect("fixture reconciles");

        // Six metadata nodes, six authored identities.
        assert_eq!(reconciliation.bound.len(), 6);
        assert_eq!(
            reconciliation.source("loose.html").expect("source held"),
            awkward_source()
        );

        let root = &reconciliation.bound[&id("spool-frame-root")];
        assert_eq!(root.tag, "main");
        assert_eq!(root.children.len(), 4);
        assert_eq!(root.parent, None);

        // Nesting is read from the authored HTML, not assumed.
        let body = &reconciliation.bound[&id("spool-text-body")];
        assert_eq!(body.tag, "p");
        assert_eq!(body.children, vec![id("spool-text-emphasis")]);
        let emphasis = &reconciliation.bound[&id("spool-text-emphasis")];
        assert_eq!(emphasis.tag, "span");
        assert_eq!(emphasis.parent, Some(id("spool-text-body")));

        assert_eq!(reconciliation.bound[&id("spool-image-mark")].tag, "img");
        assert_eq!(reconciliation.bound[&id("spool-cta-primary")].tag, "a");
    }

    #[test]
    fn identities_inside_comments_and_text_are_not_elements() {
        let index = SourceIndex::parse(&awkward_source());
        let ids: Vec<&str> = index
            .bindings()
            .iter()
            .map(|binding| binding.id.as_str())
            .collect();

        // The comment markers and the plain-text mention must not appear.
        assert!(!ids.contains(&"spool-not-real"), "comment markup was bound");
        assert!(
            !ids.contains(&"spool-inside-comment"),
            "comment text was bound"
        );
        assert_eq!(ids.len(), 6);
        // The headline's title attribute contains the id as text; that is an
        // attribute value, not an identity attribute, so it is not double-bound.
        assert_eq!(
            ids.iter()
                .filter(|value| **value == "spool-text-headline")
                .count(),
            1
        );

        // The census covers every authored element in the same file, which for
        // this fixture is exactly the six bound ones: it is a hand-written
        // fragment with no wrapper markup.
        assert_eq!(index.element_ranges().len(), ids.len());
    }

    // -- Invariant 2: an identity edit changes only the intended byte range.

    #[test]
    fn retargeting_changes_only_the_intended_byte_range() {
        let source = awkward_source();
        let index = SourceIndex::parse(&source);
        let binding = index
            .find(&id("spool-text-emphasis"))
            .expect("emphasis bound")
            .clone();

        let before = source.clone();
        let patch = retarget_identity(&source, &binding, &id("spool-text-body-emphasis"))
            .expect("retarget succeeds");

        // The recorded range is exactly the value token, inside the quotes.
        assert_eq!(&before[patch.original_range.clone()], "spool-text-emphasis");
        assert_eq!(patch.before, "spool-text-emphasis");
        assert_eq!(patch.after, "spool-text-body-emphasis");

        // The edited range is the new value's span in the new file.
        assert_eq!(
            &patch.contents[patch.edited_range.clone()],
            "spool-text-body-emphasis"
        );
        // It starts where the original value started.
        assert_eq!(patch.edited_range.start, patch.original_range.start);

        // The byte-preservation guarantee: everything before the edit is
        // untouched, and everything after it is untouched too.
        assert_eq!(
            &patch.contents[..patch.edited_range.start],
            &before[..patch.original_range.start],
            "bytes before the edit must be identical"
        );
        assert_eq!(
            &patch.contents[patch.edited_range.end..],
            &before[patch.original_range.end..],
            "bytes after the edit must be identical"
        );
        assert_eq!(
            patch.contents.len(),
            before.len() + patch.after.len() - patch.before.len(),
            "the file grew by exactly the difference between the two values"
        );
    }

    #[test]
    fn retargeting_preserves_surrounding_authored_style() {
        // Uses awkward HTML on purpose: single quotes, spaces around '=',
        // extra whitespace, attribute reordering, and comments.
        let source =
            "<p   class='body body'    data-spool-id = 'spool-x'  >\n  <!-- keep me -->\n</p>";
        let index = SourceIndex::parse(source);
        let binding = index.find(&id("spool-x")).expect("bound").clone();

        assert!(binding.quoted, "single quotes are still a quoted value");
        let patch = retarget_identity(source, &binding, &id("spool-y")).expect("retarget");
        assert_eq!(
            patch.contents,
            "<p   class='body body'    data-spool-id = 'spool-y'  >\n  <!-- keep me -->\n</p>"
        );
    }

    #[test]
    fn retargeting_quoted_and_unquoted_values_keeps_authored_quoting() {
        for (source, expected) in [
            (
                r#"<div data-spool-id="spool-a"></div>"#,
                r#"<div data-spool-id="spool-b"></div>"#,
            ),
            (
                r#"<div data-spool-id='spool-a'></div>"#,
                r#"<div data-spool-id='spool-b'></div>"#,
            ),
            (
                r#"<div data-spool-id=spool-a></div>"#,
                r#"<div data-spool-id=spool-b></div>"#,
            ),
        ] {
            let index = SourceIndex::parse(source);
            let binding = index.find(&id("spool-a")).expect("bound");
            let patch = retarget_identity(source, binding, &id("spool-b")).expect("retarget");
            assert_eq!(patch.contents, expected);
        }
    }

    #[test]
    fn retargeting_refuses_a_stale_binding() {
        let source = r#"<div data-spool-id="spool-a"></div>"#;
        let index = SourceIndex::parse(source);
        let mut binding = index.find(&id("spool-a")).expect("bound").clone();

        // Simulate the file moving underneath a held binding.
        binding.value_range.start += 1;
        assert!(retarget_identity(source, &binding, &id("spool-b")).is_err());

        // An identity that no longer exists in the source is also refused.
        assert!(retarget_identity(
            source,
            &index.find(&id("spool-a")).unwrap().clone(),
            &id("spool-c")
        )
        .is_ok());
        assert!(retarget_identity(
            "<p></p>",
            &index.find(&id("spool-a")).unwrap().clone(),
            &id("spool-b")
        )
        .is_err());
    }

    #[test]
    fn edited_source_rebinds_and_stays_byte_stable() {
        let source = awkward_source();
        let metadata = awkward_metadata();
        let index = SourceIndex::parse(&source);

        // Rename one node's identity in the HTML, then move the metadata
        // selector with it, exactly as an identity edit would.
        let binding = index.find(&id("spool-image-mark")).expect("bound").clone();
        let patch =
            retarget_identity(&source, &binding, &id("spool-image-logo")).expect("retarget");

        // Renaming a node's identity in source must be mirrored in metadata:
        // its own id and selector, plus any parent's reference to it.
        let mut moved = metadata.clone();
        for node in &mut moved.nodes {
            if node.id.as_str() == "spool-image-mark" {
                node.id = id("spool-image-logo");
                node.source.selector = r#"[data-spool-id="spool-image-logo"]"#.to_owned();
            }
            for child in &mut node.children {
                if child.as_str() == "spool-image-mark" {
                    *child = id("spool-image-logo");
                }
            }
        }

        let reconciled = reconcile(
            &moved,
            &BTreeMap::from([("loose.html".to_owned(), patch.contents.clone())]),
        )
        .expect("edited project still reconciles");
        assert_eq!(reconciled.bound[&id("spool-image-logo")].tag, "img");

        // Re-binding the edited bytes reproduces them exactly.
        let reindexed = SourceIndex::parse(&patch.contents);
        assert_eq!(reindexed.source, patch.contents);
    }

    // -- Invariants 3 and 4: mismatches are explicit.

    #[test]
    fn duplicate_identity_is_rejected_as_ambiguous() {
        let source = r#"<div data-spool-id="spool-a"></div><p data-spool-id="spool-a"></p>"#;
        let index = SourceIndex::parse(source);
        assert_eq!(index.bindings().len(), 2, "both identities are recorded");

        let metadata = LamineStructure {
            nodes: vec![structural("spool-a", None, Vec::new())],
        };
        let errors = reconcile(
            &metadata,
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("duplicate identity must fail");

        assert!(
            errors
                .iter()
                .any(|error| matches!(error, BindingError::DuplicateIdentity { id, .. } if id == "spool-a")),
            "expected DuplicateIdentity, got {errors:?}"
        );
    }

    #[test]
    fn duplicate_attribute_on_one_element_is_reported() {
        let source = r#"<div data-spool-id="spool-a" data-spool-id="spool-a"></div>"#;
        let metadata = LamineStructure {
            nodes: vec![structural("spool-a", None, Vec::new())],
        };
        let errors = reconcile(
            &metadata,
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("duplicate attribute must fail");
        assert!(errors
            .iter()
            .any(|error| matches!(error, BindingError::DuplicateIdentity { .. })));
    }

    #[test]
    fn malformed_and_empty_identities_are_reported() {
        // Each case must be reported, and reported as exactly one problem.
        for (source, matches_expected) in [
            // A value that is not a legal NodeId.
            (r#"<div data-spool-id="has space"></div>"#, 1),
            // An empty quoted value and a bare attribute are both empty.
            (r#"<div data-spool-id=""></div>"#, 1),
            (r#"<div data-spool-id></div>"#, 1),
        ] {
            let metadata = LamineStructure { nodes: vec![] };
            let errors = reconcile(
                &metadata,
                &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
            )
            .expect_err("unbindable identity must be reported");
            let reported = errors
                .iter()
                .filter(|error| {
                    matches!(
                        error,
                        BindingError::MalformedIdentity { .. } | BindingError::EmptyIdentity { .. }
                    )
                })
                .count();
            assert_eq!(reported, matches_expected, "for {source}: {errors:?}");
        }
    }

    #[test]
    fn metadata_without_an_element_is_reported() {
        let source = r#"<div data-spool-id="spool-present"></div>"#;
        let metadata = LamineStructure {
            nodes: vec![
                structural("spool-present", None, Vec::new()),
                structural("spool-absent", None, Vec::new()),
            ],
        };
        let errors = reconcile(
            &metadata,
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("unbound metadata must fail");
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, BindingError::MissingElement { id, .. } if id.as_str() == "spool-absent")),
            "expected MissingElement, got {errors:?}"
        );
    }

    #[test]
    fn html_identity_without_metadata_is_reported() {
        let source =
            r#"<div data-spool-id="spool-known"></div><div data-spool-id="spool-stray"></div>"#;
        let metadata = LamineStructure {
            nodes: vec![structural("spool-known", None, Vec::new())],
        };
        let errors = reconcile(
            &metadata,
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("undeclared identity must fail");
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, BindingError::UndeclaredElement { id, .. } if id.as_str() == "spool-stray")),
            "expected UndeclaredElement, got {errors:?}"
        );
    }

    #[test]
    fn parent_disagreement_between_metadata_and_html_is_reported() {
        let source = r#"<main data-spool-id="spool-parent"><span data-spool-id="spool-child"></span></main>"#;
        // Metadata claims the child hangs off nothing, which the HTML refutes.
        // Metadata claims the child is a root, while the HTML nests it inside
        // the parent. The disagreement is the thing under test.
        let metadata = LamineStructure {
            nodes: vec![
                structural("spool-parent", None, vec!["spool-child"]),
                structural("spool-child", None, Vec::new()),
            ],
        };
        let errors = reconcile(
            &metadata,
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("parent mismatch must fail");
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, BindingError::ParentMismatch { id, actual, .. }
                if id.as_str() == "spool-child" && actual.as_ref().map(|value| value.as_str()) == Some("spool-parent"))),
            "expected ParentMismatch, got {errors:?}"
        );
    }

    #[test]
    fn unsupported_selector_form_is_reported_not_approximated() {
        let source = r#"<div data-spool-id="spool-a"></div>"#;
        let mut node = structural("spool-a", None, Vec::new());
        node.source.selector = "#css-id-selector".to_owned();
        let errors = reconcile(
            &LamineStructure { nodes: vec![node] },
            &BTreeMap::from([("f.html".to_owned(), source.to_owned())]),
        )
        .expect_err("non-identity selector must be reported");
        assert!(errors
            .iter()
            .any(|error| matches!(error, BindingError::UnsupportedSelector { .. })));
    }

    #[test]
    fn missing_source_file_prevents_binding_rather_than_guessing() {
        let metadata = LamineStructure {
            nodes: vec![structural("spool-a", None, Vec::new())],
        };
        let errors = reconcile(&metadata, &BTreeMap::new()).expect_err("no source must fail");
        assert!(errors
            .iter()
            .any(|error| matches!(error, BindingError::MissingElement { .. })));
    }

    #[test]
    fn invalid_metadata_is_reported_before_any_binding() {
        let mut first = structural("spool-a", None, Vec::new());
        first.name = "Same".to_owned();
        let mut second = structural("spool-b", None, Vec::new());
        second.name = "Same".to_owned();
        let errors = reconcile(
            &LamineStructure {
                nodes: vec![first, second],
            },
            &BTreeMap::from([(
                "f.html".to_owned(),
                r#"<div data-spool-id="spool-a"></div>"#.to_owned(),
            )]),
        )
        .expect_err("duplicate names must fail");
        assert!(matches!(
            errors[0],
            BindingError::Metadata(ModelError::DuplicateName(_))
        ));
    }

    // -- Structural relationships.

    #[test]
    fn deep_nesting_reports_parents_and_children_in_document_order() {
        let source = "<div data-spool-id=\"spool-a\"><div data-spool-id=\"spool-b\"><div data-spool-id=\"spool-c\"></div></div><div data-spool-id=\"spool-d\"></div></div>";
        let index = SourceIndex::parse(source);

        let a = index.find(&id("spool-a")).expect("a");
        let b = index.find(&id("spool-b")).expect("b");
        let c = index.find(&id("spool-c")).expect("c");
        let d = index.find(&id("spool-d")).expect("d");

        assert_eq!(a.parent, None);
        assert_eq!(b.parent, Some(0));
        assert_eq!(c.parent, Some(1));
        assert_eq!(d.parent, Some(0));
        assert_eq!(a.children, vec![1, 3], "children are in document order");
        assert_eq!(c.children, Vec::<usize>::new());

        // Ranges nest correctly and are ordered by position.
        assert!(a.element_range.start < b.element_range.start);
        assert!(b.element_range.start < c.element_range.start);
        assert!(c.element_range.start < d.element_range.start);
    }

    #[test]
    fn unrelated_elements_between_targets_do_not_disturb_binding() {
        let source = concat!(
            "<section><p>no identity here</p>",
            "<div data-spool-id=\"spool-a\"></div>",
            "<span>text that says data-spool-id=\"spool-a\" in the middle</span>",
            "<div data-spool-id=\"spool-b\"></div></section>"
        );
        let index = SourceIndex::parse(source);
        assert_eq!(
            index.bindings().len(),
            2,
            "text and unmarked elements are ignored"
        );
        assert!(index.find(&id("spool-a")).is_some());
        assert!(index.find(&id("spool-b")).is_some());
    }

    #[test]
    fn structural_syntax_errors_are_flagged_rather_than_silently_accepted() {
        // tree-sitter-html flags genuine syntax errors such as a stray closer.
        let flagged = SourceIndex::parse("<div data-spool-id=\"spool-a\"></span>");
        assert!(
            flagged.has_syntax_errors(),
            "a syntax error must be visible, not hidden"
        );

        // Known limitation of the tree-sitter-html grammar: it is a pragmatic
        // subset of the HTML5 algorithm, so some recoveries — notably an
        // unclosed inline element such as `<div><span></div>` — do not set the
        // error flag. Ranges stay usable because they are byte offsets into
        // the original text, but `has_syntax_errors` is not a complete
        // well-formedness check. A future phase may want an explicit
        // well-formedness pass; this phase does not pretend to have one.
        let recovered = SourceIndex::parse("<div data-spool-id=\"spool-a\"><span></div>");
        assert_eq!(
            recovered.bindings().len(),
            1,
            "the bound identity still resolves"
        );
        assert_eq!(
            &recovered.source[recovered.bindings()[0].value_range.clone()],
            "spool-a",
            "ranges remain exact even when the grammar recovered"
        );
    }

    // -- The element census a style-ownership check reads.

    #[test]
    fn the_element_census_counts_elements_no_identity_claims() {
        // A shared stylesheet rule is shared with elements Spool has no node
        // for, so the census has to see them. It reads the same parse as the
        // bindings: each range slices back to real authored markup.
        let source = concat!(
            "<!doctype html>\n",
            "<html lang=\"en\">\n",
            "  <head><title>Landing</title></head>\n",
            "  <body class=\"page\">\n",
            "    <!-- <div data-spool-id=\"spool-ghost\"></div> -->\n",
            "    <main data-spool-id=\"spool-frame-root\">\n",
            "      <h1 data-spool-id=\"spool-text-headline\">Hi</h1>\n",
            "      <a class=\"cta\" href=\"#start\">Start</a>\n",
            "    </main>\n",
            "  </body>\n",
            "</html>\n"
        );
        let index = SourceIndex::parse(source);
        let census: Vec<&str> = index
            .element_ranges()
            .iter()
            .map(|range| &source[range.clone()])
            .collect();

        // html, head, title, body, main, h1, a — and not the commented-out div,
        // and not a bare text run.
        assert_eq!(census.len(), 7, "{census:?}");
        assert!(census.contains(&"<title>Landing</title>"));
        assert!(
            census.contains(&r##"<a class="cta" href="#start">Start</a>"##),
            "an element no identity claims is still an element: {census:?}"
        );
        // Checked against each element's own start tag: the comment is inside
        // `<body>`, so `body`'s slice legitimately contains the ghost's text.
        fn open_tag(slice: &str) -> &str {
            &slice[..slice.find('>').unwrap_or(slice.len())]
        }
        assert!(
            !census
                .iter()
                .any(|slice| open_tag(slice).contains("spool-ghost")),
            "markup inside a comment is not an element: {census:?}"
        );
        assert_eq!(
            census.iter().filter(|slice| slice.starts_with('<')).count(),
            census.len(),
            "every entry begins a tag"
        );
        // Ranges nest the way the document does, which is what makes a census
        // usable without re-parsing: an ancestor's range contains its
        // descendants', and the whole list is in document order.
        let main = index
            .element_ranges()
            .iter()
            .position(|range| source[range.start..range.end].starts_with("<main"))
            .expect("main is an element");
        let body = index
            .element_ranges()
            .iter()
            .position(|range| source[range.start..range.end].starts_with("<body"))
            .expect("body is an element");
        let range_at = |at: usize| index.element_ranges()[at].clone();
        assert!(
            main < range_at(body).start && body < main,
            "census is in document order"
        );
        assert!(
            range_at(body).start < range_at(main).start && range_at(main).end <= range_at(body).end,
            "main sits inside body"
        );
        for at in [main + 1, main + 2] {
            assert!(
                range_at(main).start < range_at(at).start && range_at(at).end <= range_at(main).end,
                "the headline and the link sit inside main"
            );
        }
    }

    #[test]
    fn the_census_and_the_bindings_describe_the_same_bytes() {
        // The census is collected during the same walk that produces the
        // bindings, so every bound element's range appears in it verbatim — and
        // it holds elements no identity claims, which is the whole reason it
        // exists.
        let source = concat!(
            "<body>\n",
            "  <div data-spool-id=\"spool-a\">\n",
            "    <p data-spool-id=\"spool-b\">B</p>\n",
            "    <span>no identity here</span>\n",
            "  </div>\n",
            "</body>\n"
        );
        let index = SourceIndex::parse(source);
        for binding in index.bindings() {
            assert!(
                index.element_ranges().contains(&binding.element_range),
                "{} should appear in the census",
                binding.id.as_str()
            );
        }
        // body, div, p, span: two bound elements and two that are not.
        assert_eq!(index.element_ranges().len(), 4);
        assert_eq!(index.bindings().len(), 2);
        assert!(source[index.element_ranges()[3].clone()].starts_with("<span>"));
    }

    /// Build a metadata node carrying the identity selector this phase supports.
    fn structural(id_value: &str, parent: Option<&str>, children: Vec<&str>) -> StructuralNode {
        let node_id = id(id_value);
        StructuralNode {
            id: node_id.clone(),
            name: format!("Node {id_value}"),
            kind: "frame".into(),
            parent: parent.map(id),
            children: children.into_iter().map(id).collect(),
            source: SourceBinding {
                file: "f.html".into(),
                selector: format!(r#"[data-spool-id="{id_value}"]"#),
            },
        }
    }
}
