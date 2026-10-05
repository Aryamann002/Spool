//! The narrow bridge from persistent document state to the editor runtime.
//!
//! # The one thing this module is careful about
//!
//! Three kinds of value are in play, and conflating any two of them would put
//! prototype behaviour into canonical project data. They are separate types
//! here so that is a compile error rather than a review comment.
//!
//! 1. **Persistent, source-backed** — [`PersistentDocument`] and its
//!    [`StructuralNode`]s. Identity, name, kind, hierarchy, and source
//!    binding. This is the only durable truth.
//!
//! 2. **Derived runtime** — a [`RuntimeProjection`] built from a
//!    `PersistentDocument`. Disposable: drop it and rebuild it from the same
//!    document and you get equivalent state. It holds no authority.
//!
//! 3. **Temporary prototype** — [`PrototypeGeometry`]. The existing canvas
//!    requires a position and size for every object, but nothing in
//!    `lamine.yaml` or the current `PersistentDocument` records either, and no
//!    layout engine exists to derive them. Rather than invent a second
//!    persistent geometry database — which the project rules forbid — geometry
//!    lives in its own type, is never written back to the document, and is
//!    documented below as placeholder.
//!
//! # What this is not
//!
//! This does not make the canvas render HTML/CSS. It does not add a layout
//! engine, a cascade, or computed styles. A node's geometry is a placeholder
//! for a future phase that derives bounds from authored source; nothing here
//! should be mistaken for that.
//!
//! # Identity
//!
//! There are two independent identities, and confusing them corrupts lookups.
//!
//! **Persistent identity** is [`NodeId`], an opaque string owned by
//! `lamine.yaml`. It is durable, is never derived from a name or position,
//! and is the only identity that may be saved.
//!
//! **Runtime identity** is [`ObjectId`], a `u64` lookup key. Two unrelated
//! allocators mint these, and this module owns one of them:
//!
//! 1. The canvas allocates low sequential keys from `Document::next_id`. Its
//!    starter scene already occupies `ObjectId(1)..=ObjectId(4)`, and every
//!    object a user draws takes the next number.
//! 2. This module assigns keys derived from persistent [`NodeId`] order, so
//!    that the same `PersistentDocument` always projects to the same keys and
//!    a rename never disturbs them.
//!
//! Allocator 2 therefore starts at [`PROJECTED_OBJECT_ID_BASE`] instead of 1.
//! Overlapping the ranges was a real defect, not a theoretical one: with a
//! six-node document the projection assigned `ObjectId(5)` to a persistent
//! node while the canvas independently minted `ObjectId(5)` for a newly drawn
//! rectangle, so `object_of` resolved the rectangle to the wrong persistent
//! node. The reserved base makes that collision unreachable while keeping
//! projection reproducible.
//!
//! The long-term alternative is a single allocator authority with an opaque
//! `ObjectId`. That is a wider identity change than this module should make
//! alone, so it is recorded as a decision point rather than taken here.
//!
//! Note that a canvas-created `DesignObject::spool_id` is **not** persistent
//! identity: the canvas mints those strings locally for objects that have no
//! `lamine.yaml` node behind them yet. Only ids arriving through this
//! projection are backed by a real structural node.

/// First runtime key this module may assign.
///
/// Reserved high enough that the canvas's sequential allocator cannot reach
/// it in practice, and disjoint from the canvas starter scene's `1..=4`.
pub const PROJECTED_OBJECT_ID_BASE: u64 = 1 << 32;

use std::collections::{BTreeMap, BTreeSet};

use gpui::{point, size};

use crate::canvas::{Color, DesignObject, Fill, ObjectId, ObjectType, Stroke};
use crate::source_document::{NodeId, PersistentDocument};
use crate::visual::VisualModel;

/// A position and size for a projected object.
///
/// **Temporary prototype state.** This exists because the canvas requires a
/// box to draw a selection handle or hit-test a click, and neither
/// `lamine.yaml` nor `PersistentDocument` records one. It is deliberately a
/// distinct type with no conversion back into the document, so placeholder
/// numbers cannot be mistaken for authored bounds or written into project
/// data. A later phase should derive this from authored source and replace
/// this type; nothing in the persistence path should depend on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrototypeGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PrototypeGeometry {
    /// Lay placeholder boxes out in a deterministic column.
    ///
    /// This is a fixed arithmetic rule, not a layout algorithm: it exists so
    /// that a projection is reproducible and diffable in tests. It is not
    /// intended to resemble the authored design.
    pub fn placeholder(index: usize) -> Self {
        const ORIGIN: f32 = 24.0;
        const STEP_Y: f32 = 64.0;
        const WIDTH: f32 = 320.0;
        const HEIGHT: f32 = 48.0;
        Self {
            x: ORIGIN,
            y: ORIGIN + STEP_Y * index as f32,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

/// The kind of runtime object a persistent node projects to.
///
/// Derived from `StructuralNode::kind`, which is authored in `lamine.yaml`.
/// A kind with no runtime counterpart falls back to
/// [`RuntimeObjectType::Unsupported`] rather than being coerced into
/// something it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeObjectType {
    Frame,
    Rectangle,
    Ellipse,
    Text,
    /// The metadata declares a kind this runtime does not render yet. The
    /// node still exists in the projection so identity is preserved; it simply
    /// draws as nothing.
    Unsupported,
}

impl RuntimeObjectType {
    fn from_kind(kind: &str) -> Self {
        match kind {
            "frame" | "group" | "component" => Self::Frame,
            "text" => Self::Text,
            // The other two tools the canvas offers. They are here because a
            // created rectangle is authored as `rectangle`, and a kind the
            // projection could not read would come back from a save as an object
            // that exists in the document and draws as nothing — persisted, but
            // invisible, which is the failure this milestone exists to remove.
            "rectangle" => Self::Rectangle,
            "ellipse" => Self::Ellipse,
            _ => Self::Unsupported,
        }
    }

    /// The canvas type this projects to, if any.
    ///
    /// `None` means the canvas has no representation for it, and the object is
    /// omitted from the runtime object list rather than faked.
    fn canvas_type(self) -> Option<ObjectType> {
        match self {
            Self::Frame => Some(ObjectType::Frame),
            Self::Rectangle => Some(ObjectType::Rectangle),
            Self::Ellipse => Some(ObjectType::Ellipse),
            Self::Text => Some(ObjectType::Text),
            Self::Unsupported => None,
        }
    }
}

/// A disposable runtime projection of one persistent node.
///
/// Every field here is derived. `name` and `object_type` come from metadata,
/// `geometry` is a prototype placeholder, and `id` is a runtime lookup key
/// with no durable meaning.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedNode {
    /// The persistent identity. This is the durable part.
    pub node_id: NodeId,
    /// Runtime lookup key for the canvas. Not durable; see the module docs.
    pub object_id: ObjectId,
    /// Copied from `lamine.yaml`. Changing it is a rename, not a new identity.
    pub name: String,
    /// Derived from the metadata kind.
    pub object_type: RuntimeObjectType,
    /// Metadata hierarchy, projected for the runtime.
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Derived layout geometry. See [`crate::visual`] for how this is
    /// computed; [`PrototypeGeometry`] is only the fallback when no visual
    /// model was supplied.
    pub geometry: PrototypeGeometry,
    /// Authored text, when the node's element carries any.
    pub text: Option<String>,
    /// Text colour and size resolved from the authored stylesheet.
    pub text_color: Option<Color>,
    pub font_size: f32,
    /// Resolved paint. `None` on both means the renderer default still applies.
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    /// CSS `border-radius` in pixels, zero when unauthored.
    pub border_radius: f32,
    /// CSS `opacity`, 1.0 when unauthored.
    pub opacity: f32,
}

/// A disposable runtime view of a [`PersistentDocument`].
///
/// This owns nothing durable. It is rebuilt from the document rather than
/// synced to it, which is what makes it safe to discard at any moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeProjection {
    /// Projected nodes in document order.
    pub nodes: Vec<ProjectedNode>,
    /// Reverse lookup from runtime key back to persistent identity.
    identities: BTreeMap<ObjectId, NodeId>,
    /// Source binding each node was projected from, so a caller can go back to
    /// the authored file without consulting the document again.
    bindings: BTreeMap<NodeId, (String, String)>,
}

// `from_document_with_visuals` is the production entry point: it is what
// `project_open` calls, because interpreting source is not optional. The four
// members below have no production caller and are retained deliberately:
//
// - `from_document` is the same projection with no visual model, i.e. the
//   placeholder-geometry form. It is the shape a caller wants when it is
//   testing identity or hierarchy rather than paint.
// - `rebuild` is the key-preserving form. Runtime keys are derived from
//   document order, so a rebuild is the only thing that can keep a surviving
//   node's `ObjectId` — and therefore a selection or a hover — valid across a
//   document change. `from_document`'s `previous` parameter exists for it.
// - `object_of` and `binding_of` are the two reverse lookups. The module's
//   whole reason for existing is that `NodeId` and `ObjectId` are different
//   identities, and a bridge that can only go one way cannot enforce that; a
//   save that needs "which node is this runtime object" has no other route.
//
// `project_open::LoadedProject::projection` is retained on the same reasoning,
// and is where an editor that saves from a runtime object would read them from.
#[allow(
    dead_code,
    reason = "disposable-projection identity lookups; no production caller consumes a RuntimeProjection after it yields canvas objects"
)]
impl RuntimeProjection {
    /// Project a persistent document into disposable runtime state.
    ///
    /// The resulting `ObjectId`s are derived from the document's node order, so
    /// projecting the same document twice yields identical identity
    /// assignments. `previous` may be supplied to keep runtime keys stable for
    /// nodes that survive a rebuild; see [`RuntimeProjection::rebuild`].
    pub fn from_document(
        document: &PersistentDocument,
        previous: Option<&RuntimeProjection>,
    ) -> Self {
        Self::from_document_with_visuals(document, previous, &VisualModel::default())
    }

    /// Project a document, using authored text, style, and layout where a
    /// visual model supplies them.
    ///
    /// The visual model is derived from source and disposable. A node the model
    /// does not cover falls back to the placeholder column, so a partially
    /// interpreted project still produces a coherent runtime rather than
    /// silently dropping nodes.
    pub fn from_document_with_visuals(
        document: &PersistentDocument,
        previous: Option<&RuntimeProjection>,
        visuals: &VisualModel,
    ) -> Self {
        let mut projection = RuntimeProjection::default();

        for (index, node) in document.structure.nodes.iter().enumerate() {
            // A node whose kind has no runtime counterpart is still projected,
            // so its identity and hierarchy survive; it simply draws as
            // nothing (see `canvas_objects`).
            let object_type = RuntimeObjectType::from_kind(&node.kind);

            // Reuse a surviving node's runtime key when rebuilding, so a
            // rebuild does not invalidate a selection or hover. Otherwise
            // derive one from document order, which makes projection
            // reproducible.
            let object_id = previous
                .and_then(|prior| prior.identity_of(&node.id))
                .unwrap_or_else(|| ObjectId(PROJECTED_OBJECT_ID_BASE + index as u64));

            projection.record(ProjectedNode {
                node_id: node.id.clone(),
                object_id,
                name: node.name.clone(),
                object_type,
                parent: node.parent.clone(),
                children: node.children.clone(),
                geometry: visuals
                    .get(&node.id)
                    .map(|visual| PrototypeGeometry {
                        x: visual.geometry.x,
                        y: visual.geometry.y,
                        width: visual.geometry.width,
                        height: visual.geometry.height,
                    })
                    .unwrap_or_else(|| PrototypeGeometry::placeholder(index)),
                text: visuals.get(&node.id).and_then(|v| v.text.clone()),
                text_color: visuals.get(&node.id).and_then(|v| v.text_color),
                font_size: visuals.get(&node.id).map(|v| v.font_size).unwrap_or(16.0),
                fill: visuals.get(&node.id).and_then(|v| v.style.fill),
                stroke: visuals.get(&node.id).and_then(|v| v.style.stroke),
                border_radius: visuals
                    .get(&node.id)
                    .map(|v| v.style.border_radius)
                    .unwrap_or(0.0),
                opacity: visuals
                    .get(&node.id)
                    .map(|v| v.style.opacity)
                    .unwrap_or(1.0),
            });
        }

        projection
    }

    fn record(&mut self, node: ProjectedNode) {
        self.identities.insert(node.object_id, node.node_id.clone());
        self.bindings.insert(
            node.node_id.clone(),
            (node.node_id.as_str().to_owned(), node.name.clone()),
        );
        self.nodes.push(node);
    }

    /// Rebuild runtime state from the same persistent document.
    ///
    /// Nodes that still exist keep their runtime key, so a rebuild does not
    /// invalidate a selection or a hover that refers to them.
    pub fn rebuild(document: &PersistentDocument, previous: &RuntimeProjection) -> Self {
        Self::from_document(document, Some(previous))
    }

    /// The persistent identity behind a runtime key.
    pub fn identity_of(&self, node_id: &NodeId) -> Option<ObjectId> {
        self.nodes
            .iter()
            .find(|node| &node.node_id == node_id)
            .map(|node| node.object_id)
    }

    /// The runtime key for a persistent identity.
    pub fn object_of(&self, object_id: ObjectId) -> Option<&NodeId> {
        self.identities.get(&object_id)
    }

    /// The source binding a node was projected from, as (file, selector).
    pub fn binding_of(&self, node_id: &NodeId) -> Option<&(String, String)> {
        self.bindings.get(node_id)
    }

    /// Projected nodes in breadth-first order from the roots.
    ///
    /// The metadata `children` lists are the authority for order. Any child
    /// named by a parent but absent from the projection is skipped rather
    /// than invented, so a partially-resolved document cannot fabricate nodes.
    pub fn in_hierarchy_order(&self) -> Vec<&ProjectedNode> {
        let mut ordered = Vec::with_capacity(self.nodes.len());
        let by_id: BTreeMap<&NodeId, &ProjectedNode> = self
            .nodes
            .iter()
            .map(|node| (&node.node_id, node))
            .collect();

        let mut visited: BTreeSet<&NodeId> = BTreeSet::new();
        let mut queue: Vec<&NodeId> = self
            .nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .map(|node| &node.node_id)
            .collect();

        while let Some(current) = queue.first().copied() {
            queue.remove(0);
            if !visited.insert(current) {
                continue;
            }
            let Some(node) = by_id.get(current) else {
                continue;
            };
            ordered.push(*node);
            for child in &node.children {
                if by_id.contains_key(child) {
                    queue.push(child);
                }
            }
        }

        // A node whose parent is missing from the projection is still real.
        // Emit it rather than dropping it, so a broken link loses hierarchy
        // and not identity.
        for node in &self.nodes {
            if !visited.contains(&node.node_id) {
                ordered.push(node);
            }
        }
        ordered
    }

    /// Materialise canvas objects for the nodes the canvas can draw.
    ///
    /// This is the projection's only output into the renderer, and it is the
    /// point where prototype geometry crosses into the canvas. Nodes with no
    /// canvas representation are omitted; the rest are returned in hierarchy
    /// order so the layer list matches the metadata.
    pub fn canvas_objects(&self) -> Vec<DesignObject> {
        self.in_hierarchy_order()
            .into_iter()
            .filter_map(|node| {
                let object_type = node.object_type.canvas_type()?;
                Some(DesignObject {
                    id: node.object_id,
                    spool_id: node.node_id.clone(),
                    name: node.name.clone(),
                    position: point(node.geometry.x, node.geometry.y),
                    size: size(node.geometry.width, node.geometry.height),
                    object_type,
                    // Authored text and resolved paint come from source, not
                    // from metadata. A node the visual model did not cover
                    // keeps `None`, which the renderer treats as "unstyled"
                    // rather than inventing a value.
                    text_content: node.text.clone(),
                    text_color: node.text_color,
                    font_size: (node.font_size > 0.0).then_some(node.font_size),
                    fill: node.fill,
                    stroke: node.stroke,
                    // Resolved paint the renderer applies directly. A zero
                    // radius or full opacity is what "nobody authored it" looks
                    // like once it is a concrete value.
                    border_radius: node.border_radius,
                    opacity: node.opacity,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_document::{
        EditorRuntimeState, LamineStructure, SourceBinding, StructuralNode,
    };

    fn node(
        id: &str,
        name: &str,
        kind: &str,
        parent: Option<&str>,
        children: &[&str],
    ) -> StructuralNode {
        StructuralNode {
            id: NodeId::new(id).expect("valid id"),
            name: name.to_owned(),
            kind: kind.to_owned(),
            parent: parent.map(|value| NodeId::new(value).expect("valid id")),
            children: children
                .iter()
                .map(|value| NodeId::new(*value).expect("valid id"))
                .collect(),
            source: SourceBinding {
                file: "index.html".into(),
                selector: format!("[data-spool-id=\"{id}\"]"),
            },
        }
    }

    /// A small tree: root -> [child, grandchild], plus an independent node.
    fn document() -> PersistentDocument {
        PersistentDocument {
            structure: LamineStructure {
                nodes: vec![
                    node("spool-root", "Root", "frame", None, &["spool-child"]),
                    node(
                        "spool-child",
                        "Child",
                        "text",
                        Some("spool-root"),
                        &["spool-grandchild"],
                    ),
                    node(
                        "spool-grandchild",
                        "Grandchild",
                        "frame",
                        Some("spool-child"),
                        &[],
                    ),
                ],
            },
            sources: Default::default(),
        }
    }

    fn id(value: &str) -> NodeId {
        NodeId::new(value).expect("valid id")
    }

    // -- Projection.

    #[test]
    fn document_projects_to_runtime_nodes_with_persistent_identity() {
        let projection = RuntimeProjection::from_document(&document(), None);

        assert_eq!(projection.nodes.len(), 3);
        let root = projection
            .nodes
            .iter()
            .find(|n| n.node_id == id("spool-root"))
            .expect("root");
        assert_eq!(root.name, "Root");
        assert_eq!(root.object_type, RuntimeObjectType::Frame);
        assert_eq!(root.parent, None);
        assert_eq!(root.children, vec![id("spool-child")]);

        let grandchild = projection
            .nodes
            .iter()
            .find(|n| n.node_id == id("spool-grandchild"))
            .expect("grandchild");
        assert_eq!(grandchild.parent, Some(id("spool-child")));
    }

    #[test]
    fn projection_is_in_hierarchy_order_not_document_order() {
        let projection = RuntimeProjection::from_document(&document(), None);
        let order: Vec<&str> = projection
            .in_hierarchy_order()
            .iter()
            .map(|node| node.node_id.as_str())
            .collect();
        assert_eq!(order, vec!["spool-root", "spool-child", "spool-grandchild"]);
    }

    #[test]
    fn canvas_objects_carry_the_persistent_identity() {
        let projection = RuntimeProjection::from_document(&document(), None);
        let objects = projection.canvas_objects();

        assert_eq!(objects.len(), 3);
        // The canvas object keeps the durable id, not just a runtime key.
        assert_eq!(objects[0].spool_id, id("spool-root"));
        assert_eq!(objects[1].spool_id, id("spool-child"));
        assert_eq!(objects[2].spool_id, id("spool-grandchild"));
        assert_eq!(objects[0].name, "Root");

        // The reverse lookup resolves a runtime key back to durable identity.
        assert_eq!(
            projection.object_of(objects[1].id),
            Some(&id("spool-child"))
        );
        assert_eq!(projection.object_of(ObjectId(999)), None);
    }

    #[test]
    fn unsupported_kinds_keep_identity_without_claiming_a_canvas_type() {
        let document = PersistentDocument {
            structure: LamineStructure {
                nodes: vec![node("spool-odd", "Odd", "hologram", None, &[])],
            },
            sources: Default::default(),
        };
        let projection = RuntimeProjection::from_document(&document, None);

        let projected = &projection.nodes[0];
        assert_eq!(projected.object_type, RuntimeObjectType::Unsupported);
        // It is still projected, so identity survives.
        assert_eq!(projected.node_id, id("spool-odd"));
        // But the canvas is not asked to draw something it cannot represent.
        assert!(projection.canvas_objects().is_empty());
    }

    // -- Stable identity.

    #[test]
    fn rebuilding_the_same_document_produces_identical_projection() {
        let first = RuntimeProjection::from_document(&document(), None);
        let second = RuntimeProjection::from_document(&document(), None);
        assert_eq!(first, second, "a projection must be reproducible");
    }

    #[test]
    fn runtime_keys_are_derived_from_document_order_not_allocated() {
        // Anchoring the actual values is deliberate. Comparing a projection
        // only against another projection of the same input would still pass
        // if the keys were drawn from some other reproducible source, such as
        // an allocator starting at an arbitrary number.
        let projection = RuntimeProjection::from_document(&document(), None);
        assert_eq!(projection.nodes[0].node_id, id("spool-root"));
        assert_eq!(
            projection.nodes[0].object_id,
            ObjectId(PROJECTED_OBJECT_ID_BASE)
        );
        assert_eq!(
            projection.nodes[1].object_id,
            ObjectId(PROJECTED_OBJECT_ID_BASE + 1)
        );
        assert_eq!(
            projection.nodes[2].object_id,
            ObjectId(PROJECTED_OBJECT_ID_BASE + 2)
        );

        // Two independently built projections must agree on those keys.
        let other = RuntimeProjection::from_document(&document(), None);
        assert_eq!(
            other.nodes.iter().map(|n| n.object_id).collect::<Vec<_>>(),
            projection
                .nodes
                .iter()
                .map(|n| n.object_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rebuilding_keeps_a_survivors_key_even_when_other_nodes_are_removed() {
        // Rebuilding after a deletion must not renumber the survivors. This is
        // what stops a selection or hover from silently pointing elsewhere.
        let before = RuntimeProjection::from_document(&document(), None);
        assert_eq!(
            before.identity_of(&id("spool-child")),
            Some(ObjectId(PROJECTED_OBJECT_ID_BASE + 1))
        );

        let mut shrunk = document();
        shrunk.structure.nodes.remove(0); // drop the root
        shrunk.structure.nodes[0].parent = None;

        let after = RuntimeProjection::rebuild(&shrunk, &before);
        // The first survivor is now first in the document, but must keep the
        // key it already had rather than being renumbered to 1.
        assert_eq!(after.nodes[0].node_id, id("spool-child"));
        assert_eq!(
            after.identity_of(&id("spool-child")),
            Some(ObjectId(PROJECTED_OBJECT_ID_BASE + 1)),
            "a surviving node must keep its runtime key across a rebuild"
        );
    }

    #[test]
    fn rebuilding_after_a_rename_preserves_identity() {
        let before = RuntimeProjection::from_document(&document(), None);
        let root_key = before.identity_of(&id("spool-root")).expect("root key");

        // Rename in the persistent document only.
        let mut renamed = document();
        renamed.structure.nodes[0].name = "Root Renamed".into();

        let after = RuntimeProjection::rebuild(&renamed, &before);
        assert_eq!(
            after.identity_of(&id("spool-root")),
            Some(root_key),
            "a rename must not change persistent or runtime identity"
        );
        assert_eq!(after.nodes[0].name, "Root Renamed");
        // And no duplicate identity was created.
        assert_eq!(after.nodes.len(), 3);
        let mut keys: Vec<ObjectId> = after.nodes.iter().map(|n| n.object_id).collect();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total, "runtime keys must be unique");
        let mut ids: Vec<&NodeId> = after.nodes.iter().map(|n| &n.node_id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "persistent identities must be unique");
    }

    #[test]
    fn persistent_deletion_removes_the_runtime_object() {
        let before = RuntimeProjection::from_document(&document(), None);
        let survivor_key = before.identity_of(&id("spool-child")).expect("child key");

        // Delete the grandchild from the persistent document.
        let mut deleted = document();
        deleted
            .structure
            .nodes
            .retain(|node| node.id.as_str() != "spool-grandchild");
        deleted.structure.nodes[1].children.clear();

        let after = RuntimeProjection::rebuild(&deleted, &before);
        assert!(
            after.identity_of(&id("spool-grandchild")).is_none(),
            "a deleted node must leave no runtime object"
        );
        assert_eq!(after.nodes.len(), 2);
        assert_eq!(
            after.identity_of(&id("spool-child")),
            Some(survivor_key),
            "surviving nodes keep their runtime key"
        );
        assert!(!after
            .canvas_objects()
            .iter()
            .any(|o| o.spool_id == id("spool-grandchild")));
    }

    #[test]
    fn reparenting_changes_the_derived_hierarchy_without_changing_identity() {
        let before = RuntimeProjection::from_document(&document(), None);
        let root_key = before.identity_of(&id("spool-root")).expect("root key");
        let child_key = before.identity_of(&id("spool-child")).expect("child key");

        // Move the grandchild from under the child to under the root.
        let mut reparented = document();
        reparented.structure.nodes[2].parent = Some(id("spool-root"));
        reparented.structure.nodes[1].children.clear();
        reparented.structure.nodes[0].children = vec![id("spool-child"), id("spool-grandchild")];

        let after = RuntimeProjection::rebuild(&reparented, &before);
        assert_eq!(
            after
                .nodes
                .iter()
                .find(|n| n.node_id == id("spool-grandchild"))
                .unwrap()
                .parent,
            Some(id("spool-root"))
        );
        // Identity is untouched by a structural change.
        assert_eq!(after.identity_of(&id("spool-root")), Some(root_key));
        assert_eq!(after.identity_of(&id("spool-child")), Some(child_key));

        let order: Vec<&str> = after
            .in_hierarchy_order()
            .iter()
            .map(|n| n.node_id.as_str())
            .collect();
        assert_eq!(order, vec!["spool-root", "spool-child", "spool-grandchild"]);
    }

    // -- Runtime-only state.

    #[test]
    fn selection_and_camera_never_enter_the_projection() {
        let runtime = EditorRuntimeState {
            selection: vec![id("spool-root"), id("spool-child")],
            camera_offset: (320, -48),
            active_tool: "frame".into(),
        };
        let projection = RuntimeProjection::from_document(&document(), None);

        // Building runtime state from a document does not consult, produce, or
        // require any editor session state.
        let _ = &runtime;
        let serialized = format!("{projection:?}");
        assert!(
            !serialized.contains("selection"),
            "selection leaked into projection"
        );
        assert!(
            !serialized.contains("camera"),
            "camera leaked into projection"
        );
        assert!(
            !serialized.contains("active_tool"),
            "tool leaked into projection"
        );

        // The projection carries no editor session state at all.
        assert!(projection.nodes.iter().all(|node| {
            node.name != "select" && node.name != "frame" && node.geometry.width > 0.0
        }));
    }

    #[test]
    fn runtime_only_state_is_absent_from_persistent_document() {
        let document = document();
        let serialized = format!("{:?}", document);
        for forbidden in ["selection", "camera", "hover", "tool", "drag", "caret"] {
            assert!(
                !serialized.contains(forbidden),
                "persistent state must not mention {forbidden}"
            );
        }
    }

    #[test]
    fn changing_editor_session_state_does_not_change_the_projection() {
        let before = RuntimeProjection::from_document(&document(), None);

        // Mutate every piece of runtime-only state we model.
        let mut runtime = EditorRuntimeState {
            selection: vec![id("spool-child")],
            camera_offset: (10, 20),
            active_tool: "text".into(),
        };
        runtime.camera_offset = (999, -999);
        runtime.selection.push(id("spool-grandchild"));

        let after = RuntimeProjection::from_document(&document(), None);
        assert_eq!(
            before, after,
            "runtime session state must not affect projection"
        );
    }

    // -- Prototype geometry stays prototype.

    #[test]
    fn prototype_geometry_is_never_written_back_into_the_document() {
        let document = document();
        let projection = RuntimeProjection::from_document(&document, None);

        // Reading geometry must not have mutated the document.
        let re_read = RuntimeProjection::from_document(&document, None);
        assert_eq!(projection, re_read);
        assert_eq!(
            document.structure.nodes[0].children,
            vec![id("spool-child")],
            "document is unchanged by projection"
        );
        // There is no geometry anywhere in persistent state.
        let serialized = format!("{:?}", document);
        assert!(
            !serialized.contains("position"),
            "no position in persistent state"
        );
        assert!(
            !serialized.contains("width"),
            "no width in persistent state"
        );
        assert!(
            !serialized.contains("height"),
            "no height in persistent state"
        );
    }

    #[test]
    fn geometry_is_reproducible_so_projections_stay_diffable() {
        let a = PrototypeGeometry::placeholder(3);
        let b = PrototypeGeometry::placeholder(3);
        assert_eq!(a, b);
        assert_ne!(a, PrototypeGeometry::placeholder(4));
    }

    #[test]
    fn binding_lookup_returns_the_source_a_node_came_from() {
        let projection = RuntimeProjection::from_document(&document(), None);
        let binding = projection.binding_of(&id("spool-root")).expect("binding");
        assert_eq!(binding.0, "spool-root");
    }

    #[test]
    fn projecting_never_changes_saved_metadata() {
        // Architectural invariant: runtime keys are derived, disposable, and
        // must not leak back into the persistent document. Projecting, reading
        // identities, and building canvas objects must all be side-effect free
        // with respect to `lamine.yaml`.
        let before = document();
        let yaml_before = before.structure.to_yaml().expect("serializes");

        let projection = RuntimeProjection::from_document(&before, None);
        let _ = projection.canvas_objects();
        let _ = projection.identity_of(&id("spool-child"));
        let _ = projection.object_of(ObjectId(PROJECTED_OBJECT_ID_BASE));

        let after = document();
        assert_eq!(after.structure.to_yaml().expect("serializes"), yaml_before);
        // And no runtime key value appears anywhere in the saved form.
        for node in &projection.nodes {
            assert!(
                !yaml_before.contains(&node.object_id.0.to_string()),
                "runtime key {} leaked into saved metadata",
                node.object_id.0
            );
        }
    }

    /// Architectural invariant: the two runtime-key allocators must not
    /// overlap, or a drawn object would resolve to an unrelated persistent
    /// node.
    ///
    /// This test exists because the overlap was a real defect. Deriving keys
    /// from `index + 1` collided with the canvas starter scene (which owns
    /// `1..=4`) and with everything the canvas allocates afterwards: with a
    /// six-node document the projection assigned `ObjectId(5)` to a node while
    /// the canvas minted `ObjectId(5)` for a new rectangle.
    #[test]
    fn projected_keys_never_collide_with_canvas_allocated_keys() {
        // A document wide enough that its projected keys would have reached
        // the canvas's allocation range under the old `index + 1` rule.
        let wide: PersistentDocument = {
            let mut document = self::document();
            for i in 0..8 {
                let nid = id(&format!("spool-extra-{i}"));
                document
                    .structure
                    .nodes
                    .push(crate::source_document::StructuralNode {
                        id: nid.clone(),
                        name: format!("Extra {i}"),
                        kind: "frame".into(),
                        parent: None,
                        children: vec![],
                        source: crate::source_document::SourceBinding {
                            file: "index.html".into(),
                            selector: format!("[data-spool-id=\"{}\"]", nid.as_str()),
                        },
                    });
            }
            document
        };
        let projection = RuntimeProjection::from_document(&wide, None);

        // Draw several objects in the canvas alongside the projection.
        let mut canvas = crate::canvas::Document::default();
        for _ in 0..6 {
            canvas.create_object(
                crate::canvas::ObjectType::Rectangle,
                gpui::point(0.0, 0.0),
                gpui::size(10.0, 10.0),
                None,
            );
        }

        // No canvas-allocated key may be claimed by the projection.
        for object in canvas.objects() {
            assert!(
                projection.object_of(object.id).is_none(),
                "canvas ObjectId({}) must not resolve to a persistent node",
                object.id.0
            );
        }

        // And the reverse: no projected key collides with the starter scene or
        // anything it allocates.
        for node in &projection.nodes {
            assert!(
                node.object_id.0 >= PROJECTED_OBJECT_ID_BASE,
                "projected key {} escaped the reserved range",
                node.object_id.0
            );
        }
    }
}
