//! Reading the document's hierarchy without duplicating it.
//!
//! # Why this module exists
//!
//! The persistent document already owns hierarchy: `lamine.yaml` gives every
//! structural node a `parent` and a `children` run. The runtime document does
//! **not** — it is a flat, paint-ordered list of `DesignObject`s, because the
//! renderer draws absolute boxes and has no reason to know who is inside whom.
//!
//! That leaves every surface that needs to answer a structural question
//! ("is this child of that?", "what is the topmost thing under the pointer?")
//! reaching into `lamine.yaml` and walking it by hand. `canvas.rs` already had
//! one such walk for snapping, and `layers.rs` another for its rows. This
//! module is the single place that walk lives.
//!
//! # What it deliberately is not
//!
//! It is a **read model**, rebuilt from `(objects, structure)` on demand. It is
//! not cached on the document, not written to, and not a second hierarchy
//! store: nothing here survives a reload, and no operation mutates through it.
//! Hierarchy that disagreed with `lamine.yaml` would be a bug, so there is
//! exactly one answer and it is derived.
//!
//! # The one rule that shapes everything here
//!
//! A hit has to resolve to something a user can act on. `hit_test` returns the
//! deepest object containing a point, which is right for *rendering* (that is
//! what the user sees on top) and wrong for *selection* (a user clicking a
//! rectangle inside a card means the card). Figma resolves this with a nesting
//! depth it climbs; tldraw with a focused group. Spool has neither yet, so
//! [`Hierarchy::selection_target`] climbs to the topmost drawn ancestor, which
//! is Figma's depth-0 behaviour — the default a user meets first, and the one
//! that never surprises anyone by selecting an object they cannot see a boundary
//! around.
//!
//! `Cmd`/`Ctrl`-click is the documented escape hatch in all four products, and
//! [`Hierarchy::selection_target`] takes it as an explicit argument rather than
//! reading modifiers, so the grammar stays testable without a window.

use std::collections::HashMap;

use crate::{
    canvas::{DesignObject, ObjectId},
    source_document::{LamineStructure, NodeId},
};

/// A structural node's relationship to the objects the canvas can draw.
#[derive(Clone, Debug, Default)]
pub struct Hierarchy {
    /// The nearest ancestor the canvas can actually draw, per drawn object.
    ///
    /// Built from `lamine.yaml`'s own `parent` links but filtered through
    /// "does an object exist for this node", so an unsupported wrapper in the
    /// middle of a chain does not flatten the subtree below it. See
    /// [`nearest_drawn_ancestor`].
    parent: HashMap<ObjectId, ObjectId>,
    /// Drawn objects in document order.
    ///
    /// Selection is stored in this order rather than click order so that two
    /// users who selected the same set by different routes hold the same
    /// selection, and so a marquee and a click cannot disagree about what
    /// "the selection" is.
    order: Vec<ObjectId>,
}

impl Hierarchy {
    /// Project the authored structure over the objects the canvas can draw.
    ///
    /// `objects` is the runtime list, so anything the user drew this session
    /// that has no structural node yet is a root: nothing claims it.
    pub fn build(objects: &[DesignObject], structure: &LamineStructure) -> Self {
        let nodes: HashMap<&NodeId, &crate::source_document::StructuralNode> = structure
            .nodes
            .iter()
            .map(|node| (&node.id, node))
            .collect();
        let by_node: HashMap<&NodeId, ObjectId> = objects
            .iter()
            .map(|object| (&object.spool_id, object.id))
            .collect();
        let parent = objects
            .iter()
            .map(|object| {
                (
                    object.id,
                    nearest_drawn_ancestor(&object.spool_id, &nodes, &by_node),
                )
            })
            .filter_map(|(id, parent)| parent.map(|parent| (id, parent)))
            .collect();
        Self {
            parent,
            order: objects.iter().map(|object| object.id).collect(),
        }
    }

    /// Every drawn object, in document order.
    pub fn order(&self) -> &[ObjectId] {
        &self.order
    }

    /// The nearest drawn ancestor of `id`, if it has one.
    pub fn parent_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.parent.get(&id).copied()
    }

    /// The outermost drawn ancestor of `id`, or `id` itself when it is a root.
    ///
    /// The walk is bounded by the node count so a metadata cycle terminates
    /// with an answer instead of hanging the editor.
    pub fn topmost(&self, id: ObjectId) -> ObjectId {
        let mut current = id;
        for _ in 0..=self.parent.len() {
            let Some(parent) = self.parent.get(&current).copied() else {
                return current;
            };
            current = parent;
        }
        current
    }

    /// Is `id` inside `ancestor`, at any depth?
    ///
    /// Strictly: a node is not inside itself. The walk steps to the parent
    /// *before* comparing, which is what makes it strict — an object is not its
    /// own container, and a caller filtering a selection has to be able to ask
    /// about every id against every other id without special-casing equality.
    pub fn is_within(&self, id: ObjectId, ancestor: ObjectId) -> bool {
        let mut current = id;
        for _ in 0..=self.parent.len() {
            let Some(parent) = self.parent.get(&current).copied() else {
                return false;
            };
            if parent == ancestor {
                return true;
            }
            current = parent;
        }
        false
    }

    /// What a click on `hit` should select.
    ///
    /// Figma's rule: a plain click selects the topmost ancestor of whatever was
    /// hit, so clicking a button inside a card selects the card and dragging
    /// moves the card. `deep` is the documented `Cmd`/`Ctrl` escape hatch that
    /// bypasses the climb and selects the object actually under the pointer.
    ///
    /// An id with no hierarchy is returned unchanged, so this is safe on the
    /// flat starter document and on freshly drawn objects.
    pub fn selection_target(&self, hit: ObjectId, deep: bool) -> ObjectId {
        if deep {
            hit
        } else {
            self.topmost(hit)
        }
    }

    /// Drop every id that is inside another id in the same set.
    ///
    /// tldraw's invariant, enforced here: "When the selection changes, the
    /// editor filters out any shape whose ancestor is also selected […] This
    /// prevents ambiguous situations where both a container and its contents
    /// are selected." Figma only warns about the combination, but a marquee
    /// reaching into a frame produces it *accidentally*, and every operation
    /// that reads the selection then has to guess which of the two was meant.
    ///
    /// Order is preserved, so the result stays in the order it arrived in.
    pub fn without_nested(&self, ids: &[ObjectId]) -> Vec<ObjectId> {
        ids.iter()
            .copied()
            .filter(|id| {
                !ids.iter()
                    .any(|other| *other != *id && self.is_within(*id, *other))
            })
            .collect()
    }

    /// `ids` deduplicated and in document order.
    ///
    /// The canonical form every selection ends up in. Two routes to the same
    /// set — a click-by-click build-up and a marquee, a panel click and a
    /// canvas click — produce byte-identical selections, which is what makes
    /// "the selection" something one model can answer.
    pub fn canonical(&self, ids: &[ObjectId]) -> Vec<ObjectId> {
        let chosen: std::collections::HashSet<ObjectId> = ids.iter().copied().collect();
        self.order
            .iter()
            .copied()
            .filter(|id| chosen.contains(id))
            .collect()
    }

    /// `ids` deduplicated, in document order, with nested ids dropped.
    ///
    /// The full invariant applied in one step, because every path that builds a
    /// selection from more than one id wants exactly this and getting it wrong
    /// in one place is how the invariant leaks.
    pub fn normalize(&self, ids: &[ObjectId]) -> Vec<ObjectId> {
        self.without_nested(&self.canonical(ids))
    }

    /// The objects that share `id`'s parent, in document order.
    ///
    /// `Tab`/`⇧Tab` traversal in Figma walks siblings, not "the next thing in
    /// document order", so a nested row does not become a sibling of its parent.
    /// A root's siblings are the roots.
    pub fn siblings(&self, id: ObjectId) -> Vec<ObjectId> {
        let parent = self.parent_of(id);
        self.order
            .iter()
            .copied()
            .filter(|candidate| self.parent_of(*candidate) == parent)
            .collect()
    }

    /// The next sibling in document order, wrapping at the end.
    ///
    /// Wrapping because Figma's `Tab` cycles: it is the only reliable way back
    /// to the first object once you are at the end of a long list.
    pub fn next_sibling(&self, id: ObjectId) -> Option<ObjectId> {
        self.step_sibling(id, true)
    }

    /// The previous sibling in document order, wrapping at the start.
    pub fn previous_sibling(&self, id: ObjectId) -> Option<ObjectId> {
        self.step_sibling(id, false)
    }

    fn step_sibling(&self, id: ObjectId, forward: bool) -> Option<ObjectId> {
        let run = self.siblings(id);
        let index = run.iter().position(|candidate| *candidate == id)?;
        let len = run.len();
        if len < 2 {
            return None;
        }
        let next = if forward {
            (index + 1) % len
        } else {
            (index + len - 1) % len
        };
        Some(run[next])
    }

    /// The first child of `id` in document order.
    ///
    /// `Enter` descends into a container; with no children there is nowhere to
    /// go, and answering `None` is what lets the caller leave the selection
    /// alone rather than collapsing it.
    pub fn first_child(&self, id: ObjectId) -> Option<ObjectId> {
        self.order
            .iter()
            .copied()
            .find(|candidate| self.parent_of(*candidate) == Some(id))
    }

    /// The parent of `id`, or `None` when it is already a root.
    ///
    /// `⇧Enter` ascends. A root has nowhere to ascend to, which is how the
    /// caller tells "escape upward" apart from "deselect".
    pub fn parent(&self, id: ObjectId) -> Option<ObjectId> {
        self.parent_of(id)
    }
}

/// The nearest ancestor of `node` that the canvas can actually draw.
///
/// A structural node of a kind with no canvas representation — a section, an
/// unsupported element — must not flatten its subtree: the child still nests
/// under the nearest ancestor the canvas *can* show, and becomes a root when
/// there is none. The walk is bounded by the node count, so a metadata cycle
/// terminates instead of hanging an editor.
fn nearest_drawn_ancestor(
    node: &NodeId,
    nodes: &HashMap<&NodeId, &crate::source_document::StructuralNode>,
    objects: &HashMap<&NodeId, ObjectId>,
) -> Option<ObjectId> {
    let mut cursor = nodes.get(node).and_then(|found| found.parent.as_ref());
    for _ in 0..=nodes.len() {
        let current = cursor?;
        if let Some(object) = objects.get(current) {
            return Some(*object);
        }
        cursor = nodes.get(current).and_then(|found| found.parent.as_ref());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_document::{SourceBinding, StructuralNode};

    fn node(id: &str, parent: Option<&str>) -> StructuralNode {
        StructuralNode {
            id: NodeId::new(id).unwrap(),
            name: id.to_owned(),
            kind: "frame".to_owned(),
            parent: parent.map(|parent| NodeId::new(parent).unwrap()),
            children: Vec::new(),
            source: SourceBinding {
                file: "index.html".to_owned(),
                selector: id.to_owned(),
            },
        }
    }

    fn object(id: ObjectId, spool: &str) -> DesignObject {
        DesignObject {
            id,
            spool_id: NodeId::new(spool).unwrap(),
            name: format!("object-{id:?}"),
            position: gpui::point(0.0, 0.0),
            size: gpui::size(10.0, 10.0),
            object_type: crate::canvas::ObjectType::Frame,
            text_content: None,
            text_color: None,
            font_size: None,
            fill: None,
            stroke: None,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }

    /// card > (title, body) inside a frame, plus an unrelated root.
    fn nested() -> (Hierarchy, [ObjectId; 5]) {
        let structure = LamineStructure {
            nodes: vec![
                node("frame", None),
                node("card", Some("frame")),
                node("title", Some("card")),
                node("body", Some("card")),
                node("loose", None),
            ],
        };
        let objects = vec![
            object(ObjectId(1), "frame"),
            object(ObjectId(2), "card"),
            object(ObjectId(3), "title"),
            object(ObjectId(4), "body"),
            object(ObjectId(5), "loose"),
        ];
        (
            Hierarchy::build(&objects, &structure),
            [
                ObjectId(1),
                ObjectId(2),
                ObjectId(3),
                ObjectId(4),
                ObjectId(5),
            ],
        )
    }

    #[test]
    fn a_click_selects_the_topmost_ancestor_not_the_child_under_the_pointer() {
        let (h, ids) = nested();
        let [frame, _card, title, _body, _loose] = ids;
        assert_eq!(h.selection_target(title, false), frame);
        assert_eq!(h.selection_target(frame, false), frame);
    }

    #[test]
    fn command_click_selects_the_object_actually_under_the_pointer() {
        let (h, ids) = nested();
        let [_frame, _card, title, _body, _loose] = ids;
        assert_eq!(h.selection_target(title, true), title);
    }

    #[test]
    fn topmost_climbs_all_the_way_past_a_single_intermediate_level() {
        // A two-deep document cannot tell a walk that climbs to the root from one
        // that climbs exactly once: both answer the parent. Only a third level
        // separates them.
        let structure = LamineStructure {
            nodes: vec![
                node("a", None),
                node("b", Some("a")),
                node("c", Some("b")),
                node("d", Some("c")),
            ],
        };
        let objects = vec![
            object(ObjectId(1), "a"),
            object(ObjectId(2), "b"),
            object(ObjectId(3), "c"),
            object(ObjectId(4), "d"),
        ];
        let h = Hierarchy::build(&objects, &structure);
        assert_eq!(h.topmost(ObjectId(4)), ObjectId(1), "three levels up");
        assert_eq!(h.topmost(ObjectId(3)), ObjectId(1), "two levels up");
        assert_eq!(h.topmost(ObjectId(2)), ObjectId(1), "one level up");
        assert_eq!(h.topmost(ObjectId(1)), ObjectId(1), "already there");
        // And the same answer through the click path a user actually takes.
        assert_eq!(h.selection_target(ObjectId(4), false), ObjectId(1));
    }

    #[test]
    fn a_root_has_no_parent_and_topmost_is_itself() {
        let (h, ids) = nested();
        let [_frame, _card, _title, _body, loose] = ids;
        assert_eq!(h.parent(loose), None);
        assert_eq!(h.topmost(loose), loose);
    }

    #[test]
    fn is_within_is_true_for_any_depth_and_false_for_a_node_itself() {
        let (h, ids) = nested();
        let [frame, card, title, _body, loose] = ids;
        assert!(h.is_within(title, frame), "two levels down");
        assert!(h.is_within(card, frame), "one level down");
        assert!(!h.is_within(frame, title), "not upwards");
        assert!(!h.is_within(title, title), "not itself");
        assert!(!h.is_within(title, loose), "not a sibling branch");
    }

    #[test]
    fn nested_ids_are_dropped_from_a_mixed_set() {
        let (h, ids) = nested();
        let [frame, card, title, body, _loose] = ids;
        // The descendant wins over its ancestor when the ancestor is not there,
        // and loses when it is: the frame says "the whole card".
        assert_eq!(h.without_nested(&[frame, title]), vec![frame]);
        assert_eq!(h.without_nested(&[card, title]), vec![card]);
        // Neither one present means the descendant stands for itself.
        assert_eq!(h.without_nested(&[title]), vec![title]);
        // Siblings are not nested and both stay.
        assert_eq!(h.without_nested(&[title, body]), vec![title, body]);
    }

    #[test]
    fn normalization_canonicalizes_order_duplicates_and_nesting_together() {
        let (h, ids) = nested();
        let [frame, card, title, body, loose] = ids;
        // Click order, a duplicate, and a nested pair, all at once.
        let messy = vec![body, title, body, loose, card];
        assert_eq!(h.normalize(&messy), vec![card, loose]);
        // The same set reached in document order is the same selection.
        assert_eq!(h.normalize(&[frame, loose]), vec![frame, loose]);
        assert_eq!(h.normalize(&[loose, frame]), vec![frame, loose]);
    }

    #[test]
    fn normalization_keeps_a_descendant_when_no_ancestor_is_selected() {
        let (h, ids) = nested();
        let [_frame, card, title, body, loose] = ids;
        assert_eq!(h.normalize(&[title, loose]), vec![title, loose]);
        let _ = (card, body);
    }

    #[test]
    fn sibling_traversal_skips_descendants_and_wraps() {
        let (h, ids) = nested();
        let [frame, _card, title, body, loose] = ids;
        // frame's children are just `card`; card's children are title, body.
        assert_eq!(h.siblings(title), vec![title, body]);
        assert_eq!(h.next_sibling(title), Some(body));
        assert_eq!(h.next_sibling(body), Some(title), "wraps past the end");
        assert_eq!(h.previous_sibling(body), Some(title));
        assert_eq!(h.next_sibling(frame), Some(loose), "roots are siblings");
        assert_eq!(h.previous_sibling(frame), Some(loose), "and they wrap");
    }

    #[test]
    fn a_lone_sibling_has_nowhere_to_step_to() {
        let (h, ids) = nested();
        let [_frame, card, _title, _body, _loose] = ids;
        assert_eq!(h.next_sibling(card), None);
    }

    #[test]
    fn descend_and_ascend_answer_where_the_next_level_is() {
        let (h, ids) = nested();
        let [frame, card, title, _body, _loose] = ids;
        assert_eq!(h.first_child(frame), Some(card));
        assert_eq!(h.first_child(card), Some(title));
        assert_eq!(h.first_child(title), None, "a leaf has nothing inside");
        assert_eq!(h.parent(title), Some(card));
        assert_eq!(h.parent(frame), None);
    }

    #[test]
    fn an_undrawn_wrapper_does_not_flatten_the_subtree_below_it() {
        // `ghost` has no drawn object, so `card` must still nest under `frame`.
        let structure = LamineStructure {
            nodes: vec![
                node("frame", None),
                node("ghost", Some("frame")),
                node("card", Some("ghost")),
            ],
        };
        let objects = vec![object(ObjectId(1), "frame"), object(ObjectId(2), "card")];
        let h = Hierarchy::build(&objects, &structure);
        assert_eq!(h.parent_of(ObjectId(2)), Some(ObjectId(1)));
        assert_eq!(h.topmost(ObjectId(2)), ObjectId(1));
    }

    #[test]
    fn a_metadata_cycle_terminates_instead_of_hanging() {
        let structure = LamineStructure {
            nodes: vec![node("a", Some("b")), node("b", Some("a"))],
        };
        let objects = vec![object(ObjectId(1), "a"), object(ObjectId(2), "b")];
        let h = Hierarchy::build(&objects, &structure);
        // Whatever it answers, it must answer.
        let _ = h.topmost(ObjectId(1));
        let _ = h.is_within(ObjectId(1), ObjectId(2));
    }

    #[test]
    fn a_flat_document_has_no_hierarchy_to_collapse() {
        let h = Hierarchy::build(&[], &LamineStructure::default());
        assert_eq!(h.order(), &[] as &[ObjectId]);
        assert_eq!(h.normalize(&[]), Vec::new());
    }

    #[test]
    fn an_object_with_no_structural_node_is_a_root() {
        // Freshly drawn objects carry a minted NodeId that no node claims.
        let structure = LamineStructure {
            nodes: vec![node("frame", None)],
        };
        let objects = vec![
            object(ObjectId(1), "frame"),
            object(ObjectId(2), "spool-node-minted"),
        ];
        let h = Hierarchy::build(&objects, &structure);
        assert_eq!(h.parent_of(ObjectId(2)), None);
        assert_eq!(h.topmost(ObjectId(2)), ObjectId(2));
        assert_eq!(h.siblings(ObjectId(2)), vec![ObjectId(1), ObjectId(2)]);
    }
}
