//! The Layers panel: a tree view of the document's structure, not an object list.
//!
//! # What this surface is allowed to know
//!
//! Hierarchy is *structure*, which `lamine.yaml` already owns as
//! `parent`/`children` on each structural node (see
//! [`crate::source_document::StructuralNode`]). So the panel reads that
//! read-only and projects it into rows. It does not build a second geometry or
//! style store, and it never writes hierarchy back — a rename and a reparent
//! are semantic operations owned by the document, not by this view.
//!
//! # What this surface is not allowed to own
//!
//! Selection. `CanvasView::selection` is the one selection in the editor, and
//! every selection change here is expressed as `CanvasView::select_object`. A
//! range is not a second selection: it is the smallest set of additively
//! applied ids that leaves the canvas holding "what was selected, plus this
//! run". Nothing in this module stores a copy of what is selected.
//!
//! The two pieces of state the panel *does* keep are presentation, and both are
//! discarded on reload like every other runtime view state:
//!
//! - `collapsed`: which subtrees the user folded away.
//! - `cursor`: the row keyboard navigation is standing on, which doubles as the
//!   anchor a `⇧`-range measures from. It is a list affordance in the shape of a
//!   text caret — a position, not a set — and it is reconciled against the rows
//!   on every structural change, so it can never point at a row that is gone.
//!
//! # The interaction grammar
//!
//! Pointer and keyboard are one grammar, expressed by [`row_action`] and
//! [`navigation`] as pure functions so the rules can be tested without a
//! window. `docs/research/interaction/selection.md` records the corpus:
//! `Cmd/Ctrl` is additive selection, `⇧` is range selection, and every product
//! treats a second click on a row as "I want to edit this name" rather than as
//! selecting it twice.
//!
//! The arrow keys are deliberately *not* bound here. The canvas already owns
//! them for nudging and panning, and a panel that stole them would take that
//! away from every other surface. Sibling traversal, descent, ascent and
//! disclosure therefore take the `⌘`/`Ctrl` variants and `Tab`, which nothing
//! else claims.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Modifiers, Render, SharedString,
    StyleRefinement, WeakEntity, Window,
};

use crate::{
    canvas::{CanvasView, DesignObject, ObjectId, ObjectType},
    diagnostics,
    source_document::{LamineStructure, NodeId, StructuralNode},
    theme,
};

/// Row height, row gap, and the panel's bottom padding, held together so the
/// measured panel height and the rendered row cannot disagree.
const ROW_HEIGHT: f32 = 27.0;
const ROW_GAP: f32 = 4.0;
const PANEL_PADDING_BOTTOM: f32 = 14.0;

/// The panel's own left inset.
const ROW_PADDING_LEFT: f32 = 8.0;
const ROW_PADDING_RIGHT: f32 = 7.0;

/// One level of nesting, expressed as the width of a disclosure column.
///
/// Indentation is *made of* the disclosure gutter rather than added on top of
/// it: every level contributes exactly one column, and a leaf contributes an
/// empty one. That is the only arrangement in which a row and its parent put
/// their names in the same place, which is what keeps a tree readable once a
/// node gains children.
const INDENT_STEP: f32 = 13.0;

const PANEL_WIDTH: f32 = 203.0;

#[derive(Clone, Debug, PartialEq)]
struct LayerRow {
    id: ObjectId,
    name: SharedString,
    object_type: ObjectType,
    /// Nesting level; zero for a row with no drawn parent.
    depth: u16,
    /// The nearest ancestor the canvas can actually draw. See
    /// [`nearest_drawn_ancestor`].
    parent: Option<ObjectId>,
    /// Whether this row has foldable children in the tree, whether or not they
    /// are currently shown. A row with none renders no disclosure control, so a
    /// fold gesture can never appear to have done nothing.
    has_children: bool,
    /// Presentation-only state: whether this row is currently selected. It
    /// never touches the document, the structure revision, history or
    /// serialization; rows render their selected colors from it, so
    /// construction performs no membership scan.
    selected: bool,
    /// Presentation-only state: whether this row sits inside a selected
    /// ancestor.
    ///
    /// The canvas does not enforce tldraw's "no ancestor and descendant both
    /// selected" invariant, and Figma only warns about it, so a mixed selection
    /// is reachable from here. Rather than filter it out of a selection the
    /// canvas owns, a row that is *inside* the selection is drawn differently
    /// from a row that is *part* of it.
    inside_selection: bool,
    /// Precomputed element id, built once when the structure projection is
    /// (re)built. Row rebuilds clone it instead of formatting and allocating
    /// a fresh string on every construction.
    element_id: SharedString,
}

/// Which part of a row a pointer landed on.
///
/// The disclosure control is a child of the row, so without this the two would
/// have to be told apart by coordinates at the call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowHit {
    Disclosure,
    Body,
}

/// The modifiers a click arrived with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClickInput {
    clicks: usize,
    shift: bool,
    command: bool,
}

impl ClickInput {
    fn new(clicks: usize, modifiers: Modifiers) -> Self {
        Self {
            clicks,
            shift: modifiers.shift,
            // The shell treats Cmd and Ctrl interchangeably for its own
            // shortcuts; a panel that split them would disagree with the toolbar
            // on Windows and Linux for no reason.
            command: modifiers.platform || modifiers.control,
        }
    }
}

/// What a selection gesture means, independent of which surface performed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectMode {
    /// The only row in the selection afterwards.
    Replace,
    /// Add the row, or take it back out if it was already in.
    Additive,
    /// Add every row between the anchor and the target, in display order.
    Range,
}

/// What a click on a layer row does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowAction {
    Select(SelectMode),
    Rename,
    ToggleDisclosure,
}

/// The whole pointer grammar for a row, as one pure function.
///
/// A click replaces the selection, `Cmd/Ctrl`-click adds or removes one row,
/// and `⇧`-click selects the run between the anchor and the clicked row. A
/// second click opens a rename rather than selecting again.
fn row_action(hit: RowHit, input: ClickInput) -> RowAction {
    // Disclosure is not a selection: folding a row says nothing about which
    // object is the subject of the next operation, so it wins over modifiers.
    if hit == RowHit::Disclosure {
        return RowAction::ToggleDisclosure;
    }
    if input.clicks >= 2 {
        return RowAction::Rename;
    }
    let mode = if input.shift {
        SelectMode::Range
    } else if input.command {
        SelectMode::Additive
    } else {
        SelectMode::Replace
    };
    RowAction::Select(mode)
}

/// A navigation step, named for what it does rather than for which key reaches
/// it.
///
/// There is deliberately no "next row in display order" step: the only keys that
/// could carry it are the arrow keys, and the canvas already owns those for
/// nudging and panning. Ascend, descend and sibling traversal reach every row a
/// tree can be walked from without taking a key away from anywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Navigation {
    /// The next or previous row sharing the cursor's parent.
    Sibling(isize),
    /// The first row, or the last one.
    Extreme(bool),
    /// Unfold the cursor's children, or step into them if already unfolded.
    Reveal,
    /// Fold the cursor's children away, or step out to its parent.
    Hide,
    /// The cursor's parent.
    Parent,
    /// The cursor's first child. Reachable only after `Reveal` has declined to
    /// unfold anything, so descending never hides a subtree.
    Child,
    /// Open the rename the shell already owns for this row.
    Rename,
}

/// The keyboard grammar, as one pure function.
///
/// `command` is `Cmd` on macOS and `Ctrl` elsewhere. Only keys no other surface
/// claims appear here; see the module docs for why the arrow keys are absent.
fn navigation(key: &str, modifiers: Modifiers) -> Option<Navigation> {
    let command = modifiers.platform || modifiers.control;
    match key {
        "enter" | "f2" => Some(Navigation::Rename),
        "tab" => Some(Navigation::Sibling(if modifiers.shift { -1 } else { 1 })),
        // `⇧` is never in the match: it is a modifier on these keys, never a
        // way to reach them, so `⇧⌘↓` extends and `⌘↓` replaces.
        "up" if command => Some(Navigation::Parent),
        "down" if command => Some(Navigation::Reveal),
        "left" if command => Some(Navigation::Hide),
        "right" if command => Some(Navigation::Reveal),
        "home" => Some(Navigation::Extreme(false)),
        "end" => Some(Navigation::Extreme(true)),
        _ => None,
    }
}

/// Whether a navigation key extends the selection rather than replacing it.
///
/// Sibling traversal is excluded: `⇧Tab` is the previous sibling, not "select
/// everything up to the sibling", so it has no range reading to disambiguate.
/// Rename is excluded because it does not move a selection at all. `Reveal` and
/// `Hide` are included because they move whenever they are not folding, and
/// [`LayersView::navigate`] only consults this on the moving branch.
fn navigation_extends(step: Navigation, modifiers: Modifiers) -> bool {
    modifiers.shift
        && matches!(
            step,
            Navigation::Sibling(_)
                | Navigation::Extreme(_)
                | Navigation::Parent
                | Navigation::Reveal
                | Navigation::Hide
        )
}

/// A row whose own selection presentation moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationChange {
    pub id: ObjectId,
    pub selected: bool,
}

/// What one `synchronize` did.
#[derive(Default)]
struct SelectionRefresh {
    /// Rows whose own selected flag moved. Non-empty exactly when some row's
    /// `selected` flag moved.
    selected: Vec<PresentationChange>,
    /// Rows that entered or left a selected ancestor's subtree without their
    /// own membership changing. Counted rather than reported: the panel's rows
    /// already carry the flag, and the shell's notify decision is covered by
    /// `selected` (a row only becomes contained when an ancestor's own flag
    /// moves) and by `structure_changed` (a reparent changes containment with
    /// no membership change at all).
    contained: Vec<(ObjectId, bool)>,
}

/// The synchronization result the shell reacts to.
#[derive(Default)]
pub struct SyncOutcome {
    pub structure_changed: bool,
    pub presentation: Vec<PresentationChange>,
}

#[derive(Default)]
struct LayersProjection {
    revision: Option<u64>,
    rows: Vec<LayerRow>,
    selected: Vec<ObjectId>,
    /// Rows whose subtree is folded away. Presentation only: it never reaches
    /// the document, history or a save, and it is pruned against the rows on
    /// every structural change so a node cannot come back already folded.
    collapsed: BTreeSet<ObjectId>,
    /// The row keyboard navigation stands on, and the anchor a range measures
    /// from. Reconciled against `rows` on every structural change.
    cursor: Option<ObjectId>,
}

impl LayersProjection {
    fn synchronize(
        &mut self,
        revision: u64,
        objects: impl FnOnce() -> Vec<LayerRow>,
        selected: &[ObjectId],
    ) -> SyncOutcome {
        let structure_changed = self.revision != Some(revision);
        if structure_changed {
            let start = diagnostics::start();
            self.rows = objects();
            self.revision = Some(revision);
            diagnostics::count("layers_projection_rebuild", 1);
            diagnostics::record("layers_projection", start);
            self.reconcile_after_structure_change();
        }
        let selection_changed = self.selected != selected;
        // Containment is derived from membership *and* structure, so it has to
        // be recomputed whenever either moves. Both halves stay off the render
        // path: rows read their flags, they do not scan for them.
        if !selection_changed && !structure_changed {
            return SyncOutcome {
                structure_changed,
                presentation: Vec::new(),
            };
        }
        let start = diagnostics::start();
        let refresh = refresh_selection(&mut self.rows, selected);
        diagnostics::record("layers_selection_sync", start);
        if selection_changed {
            self.selected = selected.to_vec();
            // A row the user cannot see is a row they cannot check, so selecting
            // inside a folded subtree unfolds it. Folding by hand still sticks:
            // this only runs when membership actually moves.
            reveal(&self.rows, selected, &mut self.collapsed);
            diagnostics::count("layers_selection_update", 1);
        }
        diagnostics::count(
            "row_presentation_updates",
            (refresh.selected.len() + refresh.contained.len()) as u64,
        );
        diagnostics::count("row_containment_updates", refresh.contained.len() as u64);
        SyncOutcome {
            structure_changed,
            presentation: refresh.selected,
        }
    }

    /// Drop panel state that points at rows which are no longer there.
    fn reconcile_after_structure_change(&mut self) {
        let ids: HashSet<ObjectId> = self.rows.iter().map(|row| row.id).collect();
        if let Some(cursor) = self.cursor {
            if !ids.contains(&cursor) {
                self.cursor = None;
            }
        }
        let folded: BTreeSet<ObjectId> = self
            .rows
            .iter()
            .filter(|row| row.has_children)
            .map(|row| row.id)
            .collect();
        self.collapsed.retain(|id| folded.contains(id));
    }

    /// Where the cursor is standing, or where it should start.
    ///
    /// The cursor is reconciled against the rows when the structure changes, but
    /// it can still be pointing at a folded row, so a cursor with no visible row
    /// falls back to the selection — the row the user last acted on.
    fn cursor_position(&self, visible: &Visible<'_>) -> Option<usize> {
        self.cursor
            .and_then(|cursor| visible.position(cursor))
            .or_else(|| self.selected.iter().find_map(|id| visible.position(*id)))
    }

    /// The ids a range from the cursor to `target` would cover, in display
    /// order.
    ///
    /// `None` means "there is no range to speak of", which the caller treats as a
    /// plain selection: a range with nothing to measure from is not a range.
    fn range_from_cursor(&self, visible: &Visible<'_>, target: ObjectId) -> Option<Vec<ObjectId>> {
        let from = visible.position(self.cursor?)?;
        let to = visible.position(target)?;
        Some(visible.range(from, to))
    }

    /// The row a disclosure key would fold or unfold, rather than move.
    ///
    /// `Reveal` unfolds a folded row rather than descending into it, and `Hide`
    /// folds an unfolded row rather than jumping to the parent — the same
    /// two-key ladder every tree view uses, so one key never does two jobs.
    fn disclosure_step(&self, visible: &Visible<'_>, step: Navigation) -> Option<ObjectId> {
        let position = self.cursor_position(visible)?;
        let row = visible.row(position)?;
        let folded = self.collapsed.contains(&row.id);
        match step {
            Navigation::Reveal if row.has_children && folded => Some(row.id),
            Navigation::Hide if row.has_children && !folded => Some(row.id),
            _ => None,
        }
    }

    /// Where a navigation step lands, in display-order positions.
    ///
    /// A cursor of `-1` means "the panel has nothing selected yet", so a forward
    /// step lands on the first row and a backward step has nowhere to go.
    fn navigation_target(&self, visible: &Visible<'_>, step: Navigation) -> Option<usize> {
        if visible.len() == 0 {
            return None;
        }
        let cursor = self
            .cursor_position(visible)
            .map(|position| position as isize)
            .unwrap_or(-1);
        let on_cursor = (cursor >= 0).then_some(cursor as usize);
        match step {
            Navigation::Sibling(delta) => {
                // With no cursor there is no sibling run to walk, so a forward
                // step starts at the first row and a backward step has nowhere to
                // go.
                let run = on_cursor.map_or_else(
                    || (0..visible.len()).collect(),
                    |position| visible.siblings(position),
                );
                let moved = if delta < 0 {
                    run.iter()
                        .rev()
                        .copied()
                        .find(|position| (*position as isize) < cursor)
                } else {
                    run.iter()
                        .copied()
                        .find(|position| *position as isize > cursor)
                };
                moved.or_else(|| (cursor < 0 && delta > 0).then_some(0))
            }
            Navigation::Extreme(last) => Some(if last { visible.len() - 1 } else { 0 }),
            Navigation::Parent => on_cursor.and_then(|position| visible.parent_position(position)),
            // Only reachable after `Reveal` declined to unfold anything.
            Navigation::Child => on_cursor.and_then(|position| visible.first_child(position)),
            Navigation::Reveal | Navigation::Hide | Navigation::Rename => None,
        }
    }

    /// A read model over the rows in display order.
    ///
    /// The rows are already depth-first pre-order, so "the next row" is the
    /// next visible index, and "the siblings" are the contiguous run of visible
    /// rows that share the cursor's parent. Recomputed rather than cached
    /// because folding is a `BTreeSet` edit, and one source of truth for
    /// visibility is worth more than the allocation.
    fn visible(&self) -> Visible<'_> {
        Visible::new(&self.rows, &self.collapsed)
    }

    /// Fold or unfold one row's subtree.
    ///
    /// Folding a row that has no children is a no-op rather than a fold that the
    /// same gesture could never undo.
    fn toggle_disclosure(&mut self, id: ObjectId) -> bool {
        if self.collapsed.remove(&id) {
            diagnostics::count("layers_disclosure_toggle", 1);
            return true;
        }
        let foldable = self.rows.iter().any(|row| row.id == id && row.has_children);
        if foldable {
            self.collapsed.insert(id);
            diagnostics::count("layers_disclosure_toggle", 1);
            return true;
        }
        false
    }
}

/// Test-only pure references for `refresh_selection`, on a copy, so a diff can
/// be asserted without a projection.
#[cfg(test)]
fn presentation_diff(rows: &[LayerRow], next: &[ObjectId]) -> SelectionRefresh {
    let mut copy = rows.to_vec();
    refresh_selection(&mut copy, next)
}

/// The two selection flags every row ends up with, in row order.
#[cfg(test)]
fn flags_after(rows: &[LayerRow], next: &[ObjectId]) -> Vec<(ObjectId, bool, bool)> {
    let mut copy = rows.to_vec();
    refresh_selection(&mut copy, next);
    copy.iter()
        .map(|row| (row.id, row.selected, row.inside_selection))
        .collect()
}

/// Applies `next` selection membership to the retained rows and reports what
/// moved.
///
/// One linear pass plus one small hash set of the selection, not of the rows.
/// `inside_selection` is resolved from a stack of the current row's ancestors
/// rather than a parent search per row, because the rows are in pre-order, so
/// an ancestor is simply the row most recently seen at a shallower depth.
fn refresh_selection(rows: &mut [LayerRow], next: &[ObjectId]) -> SelectionRefresh {
    let after: HashSet<ObjectId> = next.iter().copied().collect();
    let mut refresh = SelectionRefresh::default();
    // `ancestors[i]` is whether the row `i` levels above the row being visited
    // is selected.
    let mut ancestors: Vec<bool> = Vec::new();
    for row in rows.iter_mut() {
        ancestors.truncate(row.depth as usize);
        let inside_selection = ancestors.iter().any(|selected| *selected);
        let selected = after.contains(&row.id);
        ancestors.push(selected);
        let selection_moved = row.selected != selected;
        if selection_moved {
            row.selected = selected;
            refresh.selected.push(PresentationChange {
                id: row.id,
                selected,
            });
        }
        if row.inside_selection != inside_selection {
            row.inside_selection = inside_selection;
            if !selection_moved {
                refresh.contained.push((row.id, inside_selection));
            }
        }
    }
    refresh
}

/// Unfolds every ancestor of every selected row.
fn reveal(rows: &[LayerRow], selected: &[ObjectId], collapsed: &mut BTreeSet<ObjectId>) {
    if collapsed.is_empty() {
        return;
    }
    let parents: BTreeMap<ObjectId, ObjectId> = rows
        .iter()
        .filter_map(|row| row.parent.map(|parent| (row.id, parent)))
        .collect();
    for id in selected {
        let mut ancestor = parents.get(id).copied();
        while let Some(current) = ancestor {
            collapsed.remove(&current);
            ancestor = parents.get(&current).copied();
        }
    }
}

/// The nearest ancestor of `node` that the canvas can actually draw.
///
/// A structural node of a kind with no canvas representation — a section, an
/// unsupported element — must not flatten its subtree: the child still nests
/// under the nearest ancestor the canvas *can* show, and becomes a root when
/// there is none. The walk is bounded by the node count, so a metadata cycle
/// terminates instead of hanging a panel.
fn nearest_drawn_ancestor(
    node: &NodeId,
    nodes: &HashMap<&NodeId, &StructuralNode>,
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

/// Projects the document into rows in depth-first pre-order.
///
/// Order is deterministic in the sense that matters for a panel: the same
/// document always produces the same rows in the same order, with no dependence
/// on hash iteration. Two authorities, in this order:
///
/// 1. The metadata `children` list orders siblings, because that list is the
///    authority for order — the same rule
///    `RuntimeProjection::in_hierarchy_order` follows.
/// 2. Anything the metadata never named — a locally minted object, or a child
///    whose parent link was never written — keeps document order, after the
///    named siblings.
///
/// Pre-order rather than the breadth-first order the runtime projection uses: a
/// tree panel has to put a child directly under its parent.
fn project(objects: &[DesignObject], structure: &LamineStructure) -> Vec<LayerRow> {
    if objects.is_empty() {
        return Vec::new();
    }
    let nodes: HashMap<&NodeId, &StructuralNode> = structure
        .nodes
        .iter()
        .map(|node| (&node.id, node))
        .collect();
    let by_node: HashMap<&NodeId, ObjectId> = objects
        .iter()
        .map(|object| (&object.spool_id, object.id))
        .collect();
    let by_id: HashMap<ObjectId, &DesignObject> =
        objects.iter().map(|object| (object.id, object)).collect();

    let parents: HashMap<ObjectId, Option<ObjectId>> = objects
        .iter()
        .map(|object| {
            (
                object.id,
                nearest_drawn_ancestor(&object.spool_id, &nodes, &by_node),
            )
        })
        .collect();

    let mut children: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
    let mut named: HashSet<ObjectId> = HashSet::with_capacity(objects.len());
    for node in &structure.nodes {
        let Some(parent) = by_node.get(&node.id) else {
            continue;
        };
        for child in &node.children {
            let Some(child) = by_node.get(child) else {
                // A child the canvas cannot draw is not a row; inventing one
                // would fabricate an object.
                continue;
            };
            // Only honour the named order where it agrees with the resolved
            // parent. A link that disagrees would file a row under a parent it
            // does not have, and a tree must not contradict itself.
            if parents.get(child).copied().flatten() != Some(*parent) {
                continue;
            }
            named.insert(*child);
            children.entry(*parent).or_default().push(*child);
        }
    }
    for object in objects {
        let Some(parent) = parents[&object.id] else {
            continue;
        };
        if named.contains(&object.id) {
            continue;
        }
        children.entry(parent).or_default().push(object.id);
    }

    let mut rows = Vec::with_capacity(objects.len());
    let mut emitted: HashSet<ObjectId> = HashSet::with_capacity(objects.len());
    // An explicit stack rather than recursion: depth is document data, and a
    // deep document must not be able to overflow the stack inside a render.
    let mut stack: Vec<(ObjectId, u16, Option<ObjectId>)> = objects
        .iter()
        .map(|object| object.id)
        .filter(|id| parents[id].is_none())
        .rev()
        .map(|id| (id, 0, None))
        .collect();
    while let Some((id, depth, parent)) = stack.pop() {
        // A metadata cycle loses the repeat, never the row.
        if !emitted.insert(id) {
            continue;
        }
        let has_children = children.get(&id).is_some_and(|kids| !kids.is_empty());
        rows.push(row_for(by_id[&id], depth, parent, has_children));
        if let Some(kids) = children.get(&id) {
            stack.extend(
                kids.iter()
                    .rev()
                    .filter(|kid| !emitted.contains(*kid))
                    .map(|kid| (*kid, depth + 1, Some(id))),
            );
        }
    }
    // Whatever the walk could not reach is still real. Emit it as a root rather
    // than dropping it, so a broken link loses hierarchy and not identity. It is
    // emitted with no parent and nothing foldable, because in a cycle there is
    // no display order to fold.
    for object in objects {
        if emitted.insert(object.id) {
            rows.push(row_for(object, 0, None, false));
        }
    }
    rows
}

fn row_for(
    object: &DesignObject,
    depth: u16,
    parent: Option<ObjectId>,
    has_children: bool,
) -> LayerRow {
    LayerRow {
        element_id: SharedString::from(format!("layer-{:?}", object.id)),
        id: object.id,
        name: object.name.clone().into(),
        object_type: object.object_type,
        depth,
        parent,
        has_children,
        selected: false,
        inside_selection: false,
    }
}

/// Which visible rows are shown, given which rows are folded away.
///
/// The rows are in pre-order, so a subtree is a contiguous run: once a row is
/// hidden, everything is hidden until a row at that row's depth or shallower
/// comes back.
fn visible_indices(rows: &[LayerRow], collapsed: &BTreeSet<ObjectId>) -> Vec<usize> {
    let mut visible = Vec::with_capacity(rows.len());
    let mut hidden_below: Option<u16> = None;
    for (index, row) in rows.iter().enumerate() {
        if let Some(depth) = hidden_below {
            if row.depth <= depth {
                hidden_below = None;
            }
        }
        if hidden_below.is_some() {
            continue;
        }
        visible.push(index);
        if collapsed.contains(&row.id) {
            hidden_below = Some(row.depth);
        }
    }
    visible
}

/// Display-order navigation over the layer tree.
struct Visible<'a> {
    rows: &'a [LayerRow],
    indices: Vec<usize>,
}

impl<'a> Visible<'a> {
    fn new(rows: &'a [LayerRow], collapsed: &BTreeSet<ObjectId>) -> Self {
        Self {
            rows,
            indices: visible_indices(rows, collapsed),
        }
    }

    fn len(&self) -> usize {
        self.indices.len()
    }

    fn row(&self, position: usize) -> Option<&'a LayerRow> {
        self.indices.get(position).map(|index| &self.rows[*index])
    }

    fn id(&self, position: usize) -> Option<ObjectId> {
        self.row(position).map(|row| row.id)
    }

    fn position(&self, id: ObjectId) -> Option<usize> {
        self.indices
            .iter()
            .position(|index| self.rows[*index].id == id)
    }

    /// The inclusive run of positions between two rows, in display order.
    fn range(&self, from: usize, to: usize) -> Vec<ObjectId> {
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        self.indices[low..=high]
            .iter()
            .map(|index| self.rows[*index].id)
            .collect()
    }

    /// The display-order positions of every row that shares `position`'s parent,
    /// nearest first.
    ///
    /// Not a contiguous range: in pre-order one sibling's whole subtree sits
    /// between it and the next sibling, so traversal has to step over the deeper
    /// rows rather than stop at them. Only a row at the cursor's own depth can end
    /// the run; a deeper one is somebody's descendant, not a third sibling.
    fn siblings(&self, position: usize) -> Vec<usize> {
        let Some(cursor) = self.row(position) else {
            return Vec::new();
        };
        let (depth, parent) = (cursor.depth, cursor.parent);
        let shares_parent = |other: &LayerRow| other.depth == depth && other.parent == parent;
        let mut run = vec![position];
        // Forward: step over the descendants of the siblings already passed
        // until a row at the cursor's own depth decides whether the run goes on.
        let mut next = position + 1;
        while let Some(row) = self.row(next) {
            if row.depth <= depth {
                if shares_parent(row) {
                    run.push(next);
                }
                break;
            }
            next += 1;
        }
        // The same walk in reverse. A deeper row here belongs to an earlier
        // sibling's subtree; only a shallower or a differently-parented row ends
        // the run.
        let mut previous = position;
        while previous > 0 {
            previous -= 1;
            let Some(row) = self.row(previous) else {
                break;
            };
            if row.depth <= depth {
                if !shares_parent(row) {
                    break;
                }
                run.insert(0, previous);
            }
        }
        run
    }

    /// The parent of the row at `position`, in display order.
    fn parent_position(&self, position: usize) -> Option<usize> {
        let parent = self.row(position)?.parent?;
        self.position(parent)
    }

    /// The first child of the row at `position`, in display order.
    ///
    /// Only the immediately following row can be the first child, and only if it
    /// names this row as its parent.
    fn first_child(&self, position: usize) -> Option<usize> {
        let parent = self.row(position)?.id;
        let child = self.row(position + 1)?;
        (child.parent == Some(parent)).then_some(position + 1)
    }
}

/// The intrinsic height of the panel for `rows` visible rows.
fn tree_height(rows: usize) -> f32 {
    rows as f32 * ROW_HEIGHT + rows.saturating_sub(1) as f32 * ROW_GAP + PANEL_PADDING_BOTTOM
}

fn glyph(object_type: ObjectType) -> &'static str {
    match object_type {
        ObjectType::Frame => "▱",
        ObjectType::Rectangle => "□",
        ObjectType::Ellipse => "○",
        ObjectType::Text => "T",
    }
}

/// The layers panel.
///
/// It holds a weak reference to the editor and reaches the one selection model
/// through it, exactly as the canvas reaches the document.
pub struct LayersView {
    canvas: WeakEntity<CanvasView>,
    projection: LayersProjection,
    /// A row the user asked to rename, waiting to be picked up by the shell.
    ///
    /// The rename itself is a text buffer, a caret and a history boundary, all
    /// of which the shell already owns for the Inspector. Rather than teach the
    /// panel a second editing model, the panel states the request and the shell
    /// opens its existing one — the same command, the same `SemanticOperation`,
    /// the same Escape behaviour, from either surface.
    rename_request: Option<ObjectId>,
    /// The row whose name is currently being edited, if any.
    ///
    /// Presentation only: the buffer and the commit live in the shell, so this
    /// is just the row that should show that it is open for renaming. It also
    /// tells the panel to keep its hands off the keyboard, because an open
    /// rename owns it.
    renaming: Option<ObjectId>,
    /// The row under the pointer. Transient, per frame, never in the document.
    hovered: Option<ObjectId>,
    /// Keyboard focus for the panel's tree.
    ///
    /// Taken when a row is clicked and released with Escape. The keys this
    /// panel handles are ones nothing else claims, so holding focus costs no
    /// other surface anything.
    focus_handle: Option<FocusHandle>,
}

impl LayersView {
    pub fn new(canvas: WeakEntity<CanvasView>) -> Self {
        Self {
            canvas,
            projection: LayersProjection::default(),
            rename_request: None,
            renaming: None,
            hovered: None,
            focus_handle: None,
        }
    }

    /// Show which row is being renamed, or clear it.
    pub fn set_renaming(&mut self, id: Option<ObjectId>) {
        self.renaming = id;
    }

    /// Take the pending rename request, if a row asked for one.
    pub fn take_rename_request(&mut self) -> Option<ObjectId> {
        self.rename_request.take()
    }

    fn request_rename(&mut self, id: ObjectId) {
        self.rename_request = Some(id);
    }

    /// Returns the synchronization outcome: `structure_changed` drives the
    /// shell's notify of this view; `presentation` carries the O(changed)
    /// selection diff that was counted as `row_presentation_updates`.
    pub fn synchronize(&mut self, canvas: &CanvasView) -> SyncOutcome {
        self.projection.synchronize(
            canvas.layer_structure_revision(),
            || {
                project(
                    canvas.document_objects(),
                    &canvas.persistent_document().structure,
                )
            },
            canvas.selection().ids(),
        )
    }

    pub fn cached_style(&self) -> StyleRefinement {
        // Match the original intrinsic tree size, including sidebar border and
        // bottom padding, for the rows that are actually shown — folding a
        // subtree has to shrink the panel or the space it frees is just wasted.
        // Default flex-shrink (1) preserves constrained layouts.
        StyleRefinement::default()
            .w(px(PANEL_WIDTH))
            .h(px(tree_height(self.projection.visible().len())))
    }

    /// The one place selection leaves this panel.
    ///
    /// Every path goes through `CanvasView::select_object`, so the canvas stays
    /// the only selection model. A range is expressed as the smallest set of
    /// additively applied ids that leaves the canvas holding "what was selected,
    /// plus this run" — the run itself is never stored here as a selection.
    fn select(&mut self, mode: SelectMode, target: ObjectId, cx: &mut Context<Self>) {
        let range = (mode == SelectMode::Range)
            .then(|| {
                let visible = self.projection.visible();
                self.projection.range_from_cursor(&visible, target)
            })
            .flatten();
        // Any selection gesture moves the anchor, so a following `⇧`-range
        // measures from the row the user just acted on.
        self.projection.cursor = Some(target);
        let additive = mode == SelectMode::Additive;
        let _ = self.canvas.update(cx, |canvas, cx| match range {
            Some(range) => {
                for id in range {
                    if !canvas.selection().contains(id) {
                        canvas.select_object(id, true, cx);
                    }
                }
            }
            None => {
                canvas.select_object(target, additive, cx);
            }
        });
        cx.notify();
    }

    fn navigate(&mut self, step: Navigation, extend: bool, cx: &mut Context<Self>) {
        // Resolved against a snapshot of the visible order and reduced to owned
        // ids, because the projection cannot be borrowed while the landing is
        // being acted on.
        let landing: Option<(ObjectId, bool)> = {
            let visible = self.projection.visible();
            if let Some(id) = self.projection.disclosure_step(&visible, step) {
                Some((id, true))
            } else {
                // An already-unfolded row descends, and an already-folded row
                // ascends, so one key covers both halves of the ladder.
                let movement = match step {
                    Navigation::Reveal => Navigation::Child,
                    Navigation::Hide => Navigation::Parent,
                    other => other,
                };
                self.projection
                    .navigation_target(&visible, movement)
                    .and_then(|position| visible.id(position))
                    .map(|id| (id, false))
            }
        };
        let Some((id, fold)) = landing else {
            return;
        };
        if fold {
            self.projection.toggle_disclosure(id);
            cx.notify();
            return;
        }
        self.select(
            if extend {
                SelectMode::Range
            } else {
                SelectMode::Replace
            },
            id,
            cx,
        );
    }

    /// Apply one key. Returns whether the key belonged to the panel.
    fn keyboard(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // An open rename owns the keyboard: letters go into the name, and the
        // panel must not read them as navigation.
        if self.renaming.is_some() {
            return false;
        }
        if key == "escape" {
            // Escape is deliberately *not* consumed. It is already one ladder in
            // the shell, walked from the top, and giving the panel its own
            // Escape would make what it cancels depend on which listener saw
            // the key first. All the panel does is give the keyboard back, so
            // that a following arrow key nudges the canvas again.
            if self.focus_handle.is_some() {
                window.blur(cx);
                cx.notify();
            }
            return true;
        }
        let Some(step) = navigation(key, modifiers) else {
            return false;
        };
        if step == Navigation::Rename {
            let Some(cursor) = self.projection.cursor else {
                return true;
            };
            self.request_rename(cursor);
            cx.notify();
            cx.stop_propagation();
            return true;
        }
        cx.stop_propagation();
        let extend = navigation_extends(step, modifiers);
        self.navigate(step, extend, cx);
        true
    }
}

impl Render for LayersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        diagnostics::count("layers_build", 1);
        let start = diagnostics::start();
        // The focus handle is created here rather than in `new`, because only a
        // `Context` can mint one and `new` is called without one.
        let focus = match self.focus_handle.clone() {
            Some(handle) => handle,
            None => {
                let handle = cx.focus_handle();
                self.focus_handle = Some(handle.clone());
                handle
            }
        };
        let visible = self.projection.visible();
        let mut tree = div()
            .id("layers-tree")
            .flex()
            .flex_col()
            .size_full()
            .gap(px(ROW_GAP))
            .px(px(10.0))
            .pb(px(PANEL_PADDING_BOTTOM))
            .track_focus(&focus)
            // Capture phase, so the panel reads a key before the shell's
            // window-level handler does. It only runs while the panel holds
            // focus, because the dispatch path is built from the focused node.
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                this.keyboard(
                    event.keystroke.key.as_str(),
                    event.keystroke.modifiers,
                    window,
                    cx,
                );
            }));
        for index in &visible.indices {
            diagnostics::count("layers_row_construction", 1);
            let row = &self.projection.rows[*index];
            let id = row.id;
            let selected = row.selected;
            let inside_selection = row.inside_selection;
            let hovered = self.hovered == Some(id);
            let renaming = self.renaming == Some(id);
            let expanded = !self.projection.collapsed.contains(&id);
            let disclosure = row.has_children.then_some(if expanded { "▾" } else { "▸" });
            let icon = glyph(row.object_type);
            let row_focus = focus.clone();
            let disclosure_id = id;
            let mut element = div()
                .id(row.element_id.clone())
                .flex()
                .items_center()
                .h(px(ROW_HEIGHT))
                .pl(px(ROW_PADDING_LEFT))
                .pr(px(ROW_PADDING_RIGHT))
                .rounded_md()
                .bg(rgb(row_background(selected, hovered)))
                .border_l_2()
                .border_color(rgb(row_marker(selected, inside_selection, renaming)))
                .text_xs()
                .text_color(rgb(row_foreground(selected, hovered, renaming)))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    // Each row reports for itself, so a leave event from the row
                    // the pointer just left cannot clear the row it reached.
                    let next = (*hovered).then_some(id);
                    if this.hovered == next {
                        return;
                    }
                    if *hovered || this.hovered == Some(id) {
                        this.hovered = next;
                        cx.notify();
                    }
                }))
                .on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                        window.focus(&row_focus, cx);
                        let input = ClickInput::new(event.click_count(), event.modifiers());
                        match row_action(RowHit::Body, input) {
                            RowAction::Select(mode) => this.select(mode, id, cx),
                            RowAction::Rename => {
                                this.request_rename(id);
                                cx.notify();
                            }
                            // Unreachable: the body is not the disclosure control.
                            RowAction::ToggleDisclosure => {
                                this.projection.toggle_disclosure(id);
                                cx.notify();
                            }
                        }
                    }),
                )
                .child(
                    // One column per level, empty for a leaf, so a row and its
                    // parent put their name in the same place. Both arms are
                    // `Stateful<Div>` because the foldable arm carries a click
                    // handler and the spacer still needs the element id.
                    match disclosure {
                        Some(mark) => div()
                            .id(("layer-disclosure", id.0))
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .w(px(INDENT_STEP))
                            .h(px(ROW_HEIGHT))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                // Nested in the row, so folding must not also
                                // select: the row's own click would otherwise
                                // run as well.
                                cx.stop_propagation();
                                let input = ClickInput::new(event.click_count(), event.modifiers());
                                if row_action(RowHit::Disclosure, input)
                                    == RowAction::ToggleDisclosure
                                {
                                    this.projection.toggle_disclosure(disclosure_id);
                                    cx.notify();
                                }
                            }))
                            .child(mark),
                        None => div()
                            .id(("layer-disclosure", id.0))
                            .flex_shrink_0()
                            .w(px(INDENT_STEP))
                            .h(px(ROW_HEIGHT)),
                    },
                )
                .child(icon)
                .child(div().flex_1().min_w_0().truncate().child(row.name.clone()));
            if renaming {
                // The buffer and the caret live in the shell, so the row states
                // only that the name is open for editing.
                element = element.child("✎");
            }
            tree = tree.child(element);
        }
        diagnostics::record("layers_tree_build", start);
        tree
    }
}

/// Selected first, then hovered, then resting. A selected row must not lose its
/// background to a pointer passing over it.
fn row_background(selected: bool, hovered: bool) -> u32 {
    if selected {
        theme::ACCENT_WASH
    } else if hovered {
        theme::SURFACE_HOVER
    } else {
        theme::SURFACE
    }
}

/// The left edge is how a selected row is told apart at a glance, and the only
/// place a row that merely sits *inside* a selection can differ from one that is
/// *part* of it without inventing a second accent colour.
fn row_marker(selected: bool, inside_selection: bool, renaming: bool) -> u32 {
    if selected || renaming {
        theme::ACCENT
    } else if inside_selection {
        theme::ACCENT_WASH
    } else {
        theme::SURFACE
    }
}

fn row_foreground(selected: bool, hovered: bool, renaming: bool) -> u32 {
    if selected || renaming {
        theme::ACCENT
    } else if hovered {
        theme::TEXT
    } else {
        theme::TEXT_SECONDARY
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    /// Entity-free mirror of `LayersView` synchronization: the same projection
    /// and the same presentation diff, with no GPUI context required.
    ///
    /// This is the *canvas* tests' view of the panel — it exists so a document
    /// mutation can be asserted to change (or not change) the layer rows. It
    /// deliberately stops at row synchronization: navigation and disclosure are
    /// the panel's own grammar, and their types are private to this module, so
    /// those are exercised by this module's tests rather than exposed through a
    /// `pub(super)` seam.
    #[derive(Default)]
    pub struct RetainedLayers {
        projection: LayersProjection,
        document_walks: usize,
        presentation: Vec<(ObjectId, bool)>,
    }

    impl RetainedLayers {
        pub fn synchronize(&mut self, canvas: &CanvasView) -> Vec<(ObjectId, SharedString)> {
            let outcome = self.projection.synchronize(
                canvas.layer_structure_revision(),
                || {
                    self.document_walks += 1;
                    project(
                        canvas.document_objects(),
                        &canvas.persistent_document().structure,
                    )
                },
                canvas.selection().ids(),
            );
            self.presentation = outcome
                .presentation
                .into_iter()
                .map(|change| (change.id, change.selected))
                .collect();
            self.projection
                .rows
                .iter()
                .map(|row| (row.id, row.name.clone()))
                .collect()
        }

        /// How many times the panel walked the document. A panning or selecting
        /// change must not show up here.
        pub fn document_walks(&self) -> usize {
            self.document_walks
        }

        /// The selection the panel last saw. It is reported, never owned: the
        /// canvas remains the only selection model.
        pub fn selected(&self) -> &[ObjectId] {
            &self.projection.selected
        }

        /// Rows whose own selected flag moved by the last synchronize, in row
        /// order.
        pub fn presentation_updates(&self) -> &[(ObjectId, bool)] {
            &self.presentation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_document::SourceBinding;
    use gpui::{point, size};

    /// The panel without a GPUI context: a projection plus the display-order
    /// reads the interaction grammar is written against.
    ///
    /// Deliberately local to these tests. `test_support::RetainedLayers` is the
    /// canvas tests' mirror of the panel and stops at row synchronization; the
    /// navigation types are private to this module, so routing them through a
    /// `pub(super)` seam would only leak them.
    #[derive(Default)]
    struct Panel(LayersProjection);

    impl Panel {
        fn load(&mut self, objects: &[DesignObject], structure: &LamineStructure) {
            self.0.synchronize(0, || project(objects, structure), &[]);
        }

        fn load_with_selection(
            &mut self,
            objects: &[DesignObject],
            structure: &LamineStructure,
            selected: &[ObjectId],
        ) {
            self.0
                .synchronize(0, || project(objects, structure), selected);
        }

        fn rebuild(
            &mut self,
            revision: u64,
            objects: &[DesignObject],
            structure: &LamineStructure,
        ) {
            self.0
                .synchronize(revision, || project(objects, structure), &[]);
        }

        fn set_cursor(&mut self, id: ObjectId) {
            self.0.cursor = Some(id);
        }

        fn cursor(&self) -> Option<ObjectId> {
            self.0.cursor
        }

        fn toggle_disclosure(&mut self, id: ObjectId) -> bool {
            self.0.toggle_disclosure(id)
        }

        fn collapsed(&self) -> &BTreeSet<ObjectId> {
            &self.0.collapsed
        }

        fn visible(&self) -> Vec<(ObjectId, u16)> {
            let visible = self.0.visible();
            (0..visible.len())
                .filter_map(|position| visible.row(position).map(|row| (row.id, row.depth)))
                .collect()
        }

        fn panel_height(&self) -> f32 {
            tree_height(self.0.visible().len())
        }

        fn row_at(&self, position: usize) -> Option<ObjectId> {
            self.0.visible().id(position)
        }

        fn range_from_cursor(&self, target: ObjectId) -> Option<Vec<ObjectId>> {
            self.0.range_from_cursor(&self.0.visible(), target)
        }

        fn navigation_target(&self, step: Navigation) -> Option<usize> {
            self.0.navigation_target(&self.0.visible(), step)
        }

        fn disclosure_step(&self, step: Navigation) -> Option<ObjectId> {
            self.0.disclosure_step(&self.0.visible(), step)
        }
    }

    fn modifiers(shift: bool, command: bool) -> Modifiers {
        Modifiers {
            shift,
            platform: command,
            ..Modifiers::default()
        }
    }

    fn click(clicks: usize, shift: bool, command: bool) -> ClickInput {
        ClickInput {
            clicks,
            shift,
            command,
        }
    }

    fn object(id: ObjectId, node: &str, object_type: ObjectType) -> DesignObject {
        DesignObject {
            id,
            spool_id: NodeId::new(node).expect("valid id"),
            name: node.to_owned(),
            position: point(0.0, 0.0),
            size: size(10.0, 10.0),
            object_type,
            text_content: None,
            text_color: None,
            font_size: None,
            fill: None,
            stroke: None,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }

    fn node(id: &str, parent: Option<&str>, children: &[&str]) -> StructuralNode {
        StructuralNode {
            id: NodeId::new(id).expect("valid id"),
            name: id.to_owned(),
            kind: "frame".to_owned(),
            parent: parent.map(|parent| NodeId::new(parent).expect("valid id")),
            children: children
                .iter()
                .map(|child| NodeId::new(*child).expect("valid id"))
                .collect(),
            source: SourceBinding {
                file: "index.html".to_owned(),
                selector: format!("#{id}"),
            },
        }
    }

    /// A three-root tree with two levels of nesting.
    ///
    /// ```text
    /// landing
    /// ├── hero
    /// │   ├── title
    /// │   └── cta
    /// └── nav
    ///     └── logo
    /// about
    /// ```
    fn tree() -> (Vec<DesignObject>, LamineStructure) {
        let objects = vec![
            object(ObjectId(1), "landing", ObjectType::Frame),
            object(ObjectId(2), "hero", ObjectType::Frame),
            object(ObjectId(3), "title", ObjectType::Text),
            object(ObjectId(4), "cta", ObjectType::Rectangle),
            object(ObjectId(5), "nav", ObjectType::Frame),
            object(ObjectId(6), "logo", ObjectType::Text),
            object(ObjectId(7), "about", ObjectType::Frame),
        ];
        let structure = LamineStructure {
            nodes: vec![
                node("landing", None, &["hero", "nav"]),
                node("hero", Some("landing"), &["title", "cta"]),
                node("title", Some("hero"), &[]),
                node("cta", Some("hero"), &[]),
                node("nav", Some("landing"), &["logo"]),
                node("logo", Some("nav"), &[]),
                node("about", None, &[]),
            ],
        };
        (objects, structure)
    }

    /// The projected outline as `(name, depth)`, which is what the panel draws.
    fn outline(objects: &[DesignObject], structure: &LamineStructure) -> Vec<(String, u16)> {
        project(objects, structure)
            .into_iter()
            .map(|row| (row.name.to_string(), row.depth))
            .collect()
    }

    /// The ids of a projection, in display order, with no metadata at all — so
    /// every object is a root.
    fn names_from_objects(objects: &[DesignObject]) -> Vec<ObjectId> {
        project(objects, &LamineStructure::default())
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    fn names(rows: &[(ObjectId, u16)]) -> Vec<ObjectId> {
        rows.iter().map(|(id, _)| *id).collect()
    }

    // ---------------------------------------------------------------- grammar

    #[test]
    fn a_click_replaces_command_click_is_additive_and_shift_click_is_a_range() {
        assert_eq!(
            row_action(RowHit::Body, click(1, false, false)),
            RowAction::Select(SelectMode::Replace)
        );
        assert_eq!(
            row_action(RowHit::Body, click(1, false, true)),
            RowAction::Select(SelectMode::Additive)
        );
        assert_eq!(
            row_action(RowHit::Body, click(1, true, false)),
            RowAction::Select(SelectMode::Range)
        );
    }

    #[test]
    fn shift_outranks_command_because_a_range_is_a_kind_of_addition() {
        // Both modifiers present must mean one thing, not two. A range already
        // adds to the selection, so it is the more specific reading.
        assert_eq!(
            row_action(RowHit::Body, click(1, true, true)),
            RowAction::Select(SelectMode::Range)
        );
    }

    #[test]
    fn a_second_click_renames_rather_than_selecting_again() {
        for input in [
            click(2, false, false),
            click(2, false, true),
            click(2, true, false),
            click(3, true, true),
        ] {
            assert_eq!(row_action(RowHit::Body, input), RowAction::Rename);
        }
    }

    #[test]
    fn the_disclosure_control_never_selects_and_never_renames() {
        // Folding says nothing about which object is the subject of the next
        // operation, so no modifier and no click count can turn it into one.
        for input in [
            click(1, false, false),
            click(1, true, false),
            click(1, false, true),
            click(2, false, false),
            click(3, true, true),
        ] {
            assert_eq!(
                row_action(RowHit::Disclosure, input),
                RowAction::ToggleDisclosure
            );
        }
    }

    // ------------------------------------------------------------- hierarchy

    #[test]
    fn a_child_appears_directly_under_its_parent_at_the_next_depth() {
        let (objects, structure) = tree();
        assert_eq!(
            outline(&objects, &structure),
            vec![
                ("landing".into(), 0),
                ("hero".into(), 1),
                ("title".into(), 2),
                ("cta".into(), 2),
                ("nav".into(), 1),
                ("logo".into(), 2),
                ("about".into(), 0),
            ]
        );
    }

    #[test]
    fn hierarchy_is_projected_even_when_the_document_order_is_not_a_tree_order() {
        // The runtime hands over objects in breadth-first document order, which
        // is not a display order: a child can come before its parent. A tree
        // panel still has to nest rather than echo the order it was given.
        let (mut objects, structure) = tree();
        objects.sort_by_key(|object| match object.id {
            ObjectId(3) => 0,
            ObjectId(1) => 1,
            ObjectId(4) => 2,
            ObjectId(2) => 3,
            ObjectId(6) => 4,
            ObjectId(5) => 5,
            _ => 6,
        });
        assert_eq!(
            names_from_objects(&objects),
            vec![
                ObjectId(3),
                ObjectId(1),
                ObjectId(4),
                ObjectId(2),
                ObjectId(6),
                ObjectId(5),
                ObjectId(7)
            ],
            "the document order really is not a pre-order"
        );
        assert_eq!(
            outline(&objects, &structure),
            vec![
                ("landing".into(), 0),
                ("hero".into(), 1),
                ("title".into(), 2),
                ("cta".into(), 2),
                ("nav".into(), 1),
                ("logo".into(), 2),
                ("about".into(), 0),
            ]
        );
    }

    #[test]
    fn the_metadata_children_list_is_the_authority_for_sibling_order() {
        let objects = vec![
            object(ObjectId(1), "root", ObjectType::Frame),
            object(ObjectId(2), "second", ObjectType::Text),
            object(ObjectId(3), "first", ObjectType::Text),
        ];
        // `first` comes later in the document but earlier in the children list.
        let structure = LamineStructure {
            nodes: vec![
                node("root", None, &["first", "second"]),
                node("first", Some("root"), &[]),
                node("second", Some("root"), &[]),
            ],
        };
        assert_eq!(
            outline(&objects, &structure),
            vec![
                ("root".into(), 0),
                ("first".into(), 1),
                ("second".into(), 1),
            ]
        );
    }

    #[test]
    fn a_child_the_metadata_never_named_keeps_document_order_after_the_named_ones() {
        // `adopted` and `late` name `root` as their parent but `root`'s children
        // list never names them, which is what a half-written parent link looks
        // like. They must still land under `root`, in document order, after the
        // children the list does name.
        let objects = vec![
            object(ObjectId(1), "root", ObjectType::Frame),
            object(ObjectId(2), "named", ObjectType::Text),
            object(ObjectId(3), "adopted", ObjectType::Text),
            object(ObjectId(4), "late", ObjectType::Text),
        ];
        let structure = LamineStructure {
            nodes: vec![
                node("root", None, &["named"]),
                node("named", Some("root"), &[]),
                node("adopted", Some("root"), &[]),
                node("late", Some("root"), &[]),
            ],
        };
        assert_eq!(
            outline(&objects, &structure),
            vec![
                ("root".into(), 0),
                ("named".into(), 1),
                ("adopted".into(), 1),
                ("late".into(), 1),
            ]
        );
    }

    #[test]
    fn an_object_with_no_metadata_node_at_all_is_a_root() {
        // A canvas object minted in this session has a well-formed id but no
        // structural node behind it yet. It has no parent in the document, so it
        // is a root — never silently adopted by whatever happens to be nearby.
        let (mut objects, structure) = tree();
        objects.push(object(
            ObjectId(8),
            "spool-node-0001",
            ObjectType::Rectangle,
        ));
        assert_eq!(
            outline(&objects, &structure).last(),
            Some(&("spool-node-0001".to_owned(), 0u16))
        );
    }

    #[test]
    fn a_child_of_a_container_the_cannot_draw_still_nests_under_the_nearest_drawn_one() {
        // A section or an unsupported kind has no canvas object, so it has no
        // row. Its children must not be promoted to roots: the hierarchy the
        // user authored is still there, minus the row that cannot be drawn.
        let objects = vec![
            object(ObjectId(1), "page", ObjectType::Frame),
            object(ObjectId(2), "card", ObjectType::Frame),
        ];
        let structure = LamineStructure {
            nodes: vec![
                node("page", None, &["section"]),
                node("section", Some("page"), &["card"]),
                node("card", Some("section"), &[]),
            ],
        };
        assert_eq!(
            outline(&objects, &structure),
            vec![("page".into(), 0), ("card".into(), 1)]
        );
    }

    #[test]
    fn a_child_whose_whole_ancestry_is_undrawable_becomes_a_root() {
        let objects = vec![object(ObjectId(1), "card", ObjectType::Frame)];
        let structure = LamineStructure {
            nodes: vec![
                node("section", None, &["group"]),
                node("group", Some("section"), &["card"]),
                node("card", Some("group"), &[]),
            ],
        };
        assert_eq!(outline(&objects, &structure), vec![("card".into(), 0)]);
    }

    #[test]
    fn a_parent_link_that_disagrees_with_the_children_list_cannot_contradict_the_tree() {
        // `root` names `child` as its own child, while `child` names `other` as
        // its parent. Filing `child` under `root` anyway would put it at a depth
        // its own parent link contradicts, so the named order is dropped and
        // the resolved parent wins.
        let objects = vec![
            object(ObjectId(1), "root", ObjectType::Frame),
            object(ObjectId(2), "other", ObjectType::Frame),
            object(ObjectId(3), "child", ObjectType::Text),
        ];
        let structure = LamineStructure {
            nodes: vec![
                node("root", None, &["child"]),
                node("other", None, &["child"]),
                node("child", Some("other"), &[]),
            ],
        };
        assert_eq!(
            outline(&objects, &structure),
            vec![("root".into(), 0), ("other".into(), 0), ("child".into(), 1),]
        );
    }

    #[test]
    fn a_metadata_cycle_terminates_and_loses_no_row() {
        let objects = vec![
            object(ObjectId(1), "a", ObjectType::Frame),
            object(ObjectId(2), "b", ObjectType::Frame),
        ];
        let structure = LamineStructure {
            nodes: vec![node("a", Some("b"), &["b"]), node("b", Some("a"), &["a"])],
        };
        let rows = project(&objects, &structure);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![ObjectId(1), ObjectId(2)],
            "both rows survive; the cycle only loses the repeat"
        );
    }

    #[test]
    fn a_deep_chain_projects_without_recursing() {
        // The walk is iterative precisely because depth is document data: a
        // chain deep enough to overflow a recursive walk must still render.
        let depth = 5_000u64;
        let mut objects = Vec::with_capacity(depth as usize);
        let mut nodes = Vec::with_capacity(depth as usize);
        for index in 0..depth {
            objects.push(object(
                ObjectId(index + 1),
                &format!("n{index}"),
                ObjectType::Frame,
            ));
            let parent = (index > 0).then(|| format!("n{}", index - 1));
            nodes.push(node(&format!("n{index}"), parent.as_deref(), &[]));
        }
        let rows = project(&objects, &LamineStructure { nodes });
        assert_eq!(rows.len(), depth as usize);
        assert_eq!(rows.last().map(|row| row.depth), Some((depth - 1) as u16));
    }

    #[test]
    fn projection_is_deterministic_and_independent_of_declaration_order() {
        let (objects, structure) = tree();
        let expected = outline(&objects, &structure);
        for _ in 0..8 {
            assert_eq!(outline(&objects, &structure), expected);
        }
        // The metadata may declare its nodes in any order — the `children` lists
        // are what order the panel — so re-declaring must not reorder a row.
        let mut reversed = structure.nodes.clone();
        reversed.reverse();
        assert_eq!(
            outline(&objects, &LamineStructure { nodes: reversed }),
            expected
        );
    }

    #[test]
    fn row_identity_is_keyed_by_object_id_not_position() {
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        let landing = rows
            .iter()
            .find(|row| row.id == ObjectId(1))
            .expect("landing row");
        assert!(landing.has_children);
        assert_eq!(landing.parent, None);
        let reversed = objects.iter().rev().cloned().collect::<Vec<_>>();
        // The element id is a function of the object, so a rebuild in a
        // different order still resolves to the same GPUI element.
        assert_eq!(
            project(&reversed, &structure)
                .iter()
                .find(|row| row.id == ObjectId(1))
                .expect("landing row")
                .element_id,
            landing.element_id
        );
    }

    // ------------------------------------------------------------ disclosure

    #[test]
    fn folding_a_row_hides_its_whole_subtree_and_nothing_else() {
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        let shown = |collapsed: &BTreeSet<ObjectId>| {
            let visible = visible_indices(&rows, collapsed);
            rows.iter()
                .enumerate()
                .filter(|(index, _)| visible.contains(index))
                .map(|(_, row)| row.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            shown(&BTreeSet::new()),
            vec![
                ObjectId(1),
                ObjectId(2),
                ObjectId(3),
                ObjectId(4),
                ObjectId(5),
                ObjectId(6),
                ObjectId(7)
            ]
        );
        assert_eq!(
            shown(&BTreeSet::from([ObjectId(1)])),
            vec![ObjectId(1), ObjectId(7)],
            "folding `landing` takes hero, nav and everything under them"
        );
    }

    #[test]
    fn nested_folding_composes() {
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        let shown = |collapsed: BTreeSet<ObjectId>| {
            let visible = visible_indices(&rows, &collapsed);
            rows.iter()
                .enumerate()
                .filter(|(index, _)| visible.contains(index))
                .map(|(_, row)| row.id)
                .collect::<Vec<_>>()
        };
        // Folding `hero` keeps `hero` itself and takes its two children with it,
        // leaving its sibling `nav` and its child `logo` alone: folding a subtree
        // is scoped to that subtree.
        assert_eq!(
            shown(BTreeSet::from([ObjectId(2)])),
            vec![
                ObjectId(1),
                ObjectId(2),
                ObjectId(5),
                ObjectId(6),
                ObjectId(7)
            ]
        );
        // Folding `landing` as well takes `nav` too, because `nav` is inside it
        // whether or not it was itself folded.
        assert_eq!(
            shown(BTreeSet::from([ObjectId(1), ObjectId(2)])),
            vec![ObjectId(1), ObjectId(7)]
        );
        // Unfolding the child again does not unfold the parent.
        assert_eq!(
            shown(BTreeSet::from([ObjectId(1)])),
            vec![ObjectId(1), ObjectId(7)]
        );
    }

    #[test]
    fn folding_a_subtree_shrinks_the_panel() {
        let mut projection = Panel::default();
        let (objects, structure) = tree();
        projection.load(&objects, &structure);
        assert_eq!(projection.panel_height(), tree_height(7));
        projection.toggle_disclosure(ObjectId(1));
        assert_eq!(projection.panel_height(), tree_height(2));
        projection.toggle_disclosure(ObjectId(1));
        assert_eq!(projection.panel_height(), tree_height(7));
    }

    #[test]
    fn folding_a_leaf_is_a_no_op_rather_than_a_fold_nothing_can_unfold() {
        let (objects, structure) = tree();
        assert!(!project(&objects, &structure)
            .iter()
            .any(|row| row.id == ObjectId(3) && row.has_children));
        let mut projection = LayersProjection {
            rows: project(&objects, &structure),
            ..LayersProjection::default()
        };
        assert!(
            !projection.toggle_disclosure(ObjectId(3)),
            "a leaf has no subtree to fold"
        );
        assert!(projection.collapsed.is_empty());
        assert!(
            !projection.toggle_disclosure(ObjectId(99)),
            "an absent row is a no-op"
        );
        assert!(projection.toggle_disclosure(ObjectId(2)));
        assert_eq!(
            projection.collapsed.iter().copied().collect::<Vec<_>>(),
            vec![ObjectId(2)]
        );
        assert!(projection.toggle_disclosure(ObjectId(2)));
        assert!(projection.collapsed.is_empty());
    }

    #[test]
    fn selecting_inside_a_folded_subtree_unfolds_it() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.toggle_disclosure(ObjectId(1));
        assert_eq!(names(&projection.visible()), vec![ObjectId(1), ObjectId(7)]);

        // `logo` is two levels down; selecting it has to reveal the path to it.
        projection.load_with_selection(&objects, &structure, &[ObjectId(6)]);
        assert!(
            projection.collapsed().is_empty(),
            "the whole ancestor path is unfolded, not just the parent"
        );
        assert_eq!(
            names(&projection.visible()),
            vec![
                ObjectId(1),
                ObjectId(2),
                ObjectId(3),
                ObjectId(4),
                ObjectId(5),
                ObjectId(6),
                ObjectId(7)
            ]
        );
    }

    #[test]
    fn folding_by_hand_survives_a_rebuild_but_does_not_outlive_its_row() {
        let (mut objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.toggle_disclosure(ObjectId(2));
        assert!(projection.collapsed().contains(&ObjectId(2)));

        // A structural change that does not touch the fold keeps it: the user
        // folded this on purpose.
        objects.push(object(ObjectId(8), "footer", ObjectType::Frame));
        let mut nodes = structure.nodes.clone();
        nodes.push(node("footer", None, &[]));
        projection.rebuild(1, &objects, &LamineStructure { nodes });
        assert!(projection.collapsed().contains(&ObjectId(2)));

        // Removing the row removes the fold, so re-adding the node does not
        // produce a subtree the user cannot see and did not ask to hide.
        objects.retain(|object| object.id != ObjectId(2));
        let mut nodes = structure.nodes.clone();
        nodes.retain(|node| node.id.as_str() != "hero");
        for node in &mut nodes {
            node.children.retain(|child| child.as_str() != "hero");
        }
        projection.rebuild(2, &objects, &LamineStructure { nodes });
        assert!(!projection.collapsed().contains(&ObjectId(2)));

        objects.push(object(ObjectId(2), "hero", ObjectType::Frame));
        let mut nodes = structure.nodes.clone();
        nodes.push(node("hero", Some("landing"), &[]));
        projection.rebuild(3, &objects, &LamineStructure { nodes });
        assert!(
            projection.collapsed().is_empty(),
            "`hero` came back as a leaf, so there is no subtree left to hide"
        );
    }

    #[test]
    fn a_cursor_on_a_deleted_row_is_dropped_rather_than_left_dangling() {
        let (mut objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(3));
        assert_eq!(projection.cursor(), Some(ObjectId(3)));
        objects.retain(|object| object.id != ObjectId(3));
        let mut nodes = structure.nodes.clone();
        nodes.retain(|node| node.id.as_str() != "title");
        for node in &mut nodes {
            node.children.retain(|child| child.as_str() != "title");
        }
        projection.rebuild(1, &objects, &LamineStructure { nodes });
        assert_eq!(projection.cursor(), None);
    }

    // ------------------------------------------------------ selection grammar

    #[test]
    fn the_selection_diff_updates_only_the_rows_that_moved() {
        let (objects, structure) = tree();
        let mut projection = LayersProjection::default();
        projection.synchronize(
            0,
            || project(&objects, &structure),
            &[ObjectId(1), ObjectId(2)],
        );
        // Old selection {landing, hero}, new {hero, title}: `hero` is a member in
        // both, so only `landing` loses its flag and only `title` gains one.
        let outcome =
            projection.synchronize(0, || panic!("no rebuild"), &[ObjectId(2), ObjectId(3)]);
        assert_eq!(
            outcome
                .presentation
                .iter()
                .map(|change| (change.id, change.selected))
                .collect::<Vec<_>>(),
            vec![(ObjectId(1), false), (ObjectId(3), true)]
        );
    }

    #[test]
    fn a_row_inside_a_selected_ancestor_is_reported_separately_from_a_selected_row() {
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        // Selecting the page selects one row. Its descendants are not members,
        // so their own flag does not move — but every one of them is inside the
        // selection, which is a different thing to say and has to be said.
        let refresh = presentation_diff(&rows, &[ObjectId(1)]);
        assert_eq!(
            refresh.selected,
            vec![PresentationChange {
                id: ObjectId(1),
                selected: true
            }]
        );
        assert_eq!(
            refresh.contained,
            vec![
                (ObjectId(2), true),
                (ObjectId(3), true),
                (ObjectId(4), true),
                (ObjectId(5), true),
                (ObjectId(6), true)
            ]
        );
    }

    #[test]
    fn nesting_is_reported_for_every_descendant_not_just_the_child() {
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        assert_eq!(
            flags_after(&rows, &[ObjectId(1)]),
            vec![
                (ObjectId(1), true, false),
                (ObjectId(2), false, true),
                (ObjectId(3), false, true),
                (ObjectId(4), false, true),
                (ObjectId(5), false, true),
                (ObjectId(6), false, true),
                (ObjectId(7), false, false),
            ]
        );
    }

    #[test]
    fn a_parent_and_a_child_can_both_be_selected_and_are_told_apart() {
        // The canvas does not enforce tldraw's no-ancestor-and-descendant
        // invariant and Figma only warns about it, so the panel must not filter
        // a selection it does not own: it distinguishes "part of" from "inside".
        let (objects, structure) = tree();
        let rows = project(&objects, &structure);
        let refresh = presentation_diff(&rows, &[ObjectId(1), ObjectId(3)]);
        assert_eq!(
            refresh.selected,
            vec![
                PresentationChange {
                    id: ObjectId(1),
                    selected: true
                },
                PresentationChange {
                    id: ObjectId(3),
                    selected: true
                },
            ]
        );
        assert_eq!(
            refresh.contained,
            vec![
                (ObjectId(2), true),
                (ObjectId(4), true),
                (ObjectId(5), true),
                (ObjectId(6), true),
            ],
            "a selected child makes its parent's other children inside the selection too"
        );
        assert_eq!(
            flags_after(&rows, &[ObjectId(1), ObjectId(3)]),
            vec![
                (ObjectId(1), true, false),
                (ObjectId(2), false, true),
                (ObjectId(3), true, true),
                (ObjectId(4), false, true),
                (ObjectId(5), false, true),
                (ObjectId(6), false, true),
                (ObjectId(7), false, false),
            ],
            "`title` is both part of the selection and inside it"
        );
    }

    #[test]
    fn a_containment_change_always_comes_with_a_selected_change_to_notify_on() {
        // The shell notifies on `structure_changed || !presentation.is_empty()`,
        // so this invariant is what keeps a nested selection from going stale.
        let (objects, structure) = tree();
        let previous = [ObjectId(3)];
        let next = [ObjectId(3), ObjectId(1)];
        let mut projection = LayersProjection::default();
        projection.synchronize(0, || project(&objects, &structure), &previous);
        let outcome = projection.synchronize(0, || panic!("no rebuild"), &next);
        assert!(
            !outcome.presentation.is_empty(),
            "adding an ancestor moves at least the ancestor's own flag"
        );
        let outcome = projection.synchronize(0, || panic!("no rebuild"), &[ObjectId(1)]);
        assert!(!outcome.presentation.is_empty());
    }

    #[test]
    fn a_reparent_changes_containment_without_changing_membership() {
        // This is the case `structure_changed` exists for: the selection is
        // identical across the reparent, so no row's membership moved, but which
        // rows sit *inside* the selection did. The rebuild re-establishes the
        // membership it lost, and `structure_changed` is what guarantees the
        // shell notifies for it.
        let objects = vec![
            object(ObjectId(1), "a", ObjectType::Frame),
            object(ObjectId(2), "b", ObjectType::Frame),
            object(ObjectId(3), "child", ObjectType::Text),
        ];
        let before = LamineStructure {
            nodes: vec![
                node("a", None, &[]),
                node("b", None, &["child"]),
                node("child", Some("b"), &[]),
            ],
        };
        let after = LamineStructure {
            nodes: vec![
                node("a", None, &["child"]),
                node("b", None, &[]),
                node("child", Some("a"), &[]),
            ],
        };
        let mut projection = LayersProjection::default();
        projection.synchronize(0, || project(&objects, &before), &[ObjectId(1)]);
        let outcome = projection.synchronize(1, || project(&objects, &after), &[ObjectId(1)]);
        assert!(outcome.structure_changed);
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|row| (row.id, row.selected, row.inside_selection))
                .collect::<Vec<_>>(),
            vec![
                (ObjectId(1), true, false),
                (ObjectId(3), false, true),
                (ObjectId(2), false, false),
            ],
            "`child` now sits under `a`, so it is inside the selection"
        );
    }

    #[test]
    fn selection_and_structure_are_still_projected_lazily() {
        let (objects, structure) = tree();
        let mut projection = LayersProjection::default();
        assert!(
            projection
                .synchronize(0, || project(&objects, &structure), &[])
                .structure_changed
        );
        assert!(
            !projection
                .synchronize(0, || panic!("unchanged structure must not walk"), &[])
                .structure_changed
        );
        assert!(
            !projection
                .synchronize(
                    0,
                    || panic!("selection must not project rows"),
                    &[ObjectId(1)]
                )
                .structure_changed
        );
        assert_eq!(projection.selected, vec![ObjectId(1)]);
        assert!(
            !projection
                .synchronize(0, || panic!("a pan must not project rows"), &[ObjectId(1)])
                .structure_changed
        );
    }

    // ----------------------------------------------------------- range select

    #[test]
    fn a_range_covers_every_row_between_the_anchor_and_the_target() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(2));
        assert_eq!(
            projection.range_from_cursor(ObjectId(5)),
            Some(vec![ObjectId(2), ObjectId(3), ObjectId(4), ObjectId(5)])
        );
    }

    #[test]
    fn a_range_backwards_is_the_same_run_in_display_order() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(7));
        assert_eq!(
            projection.range_from_cursor(ObjectId(3)),
            Some(vec![
                ObjectId(3),
                ObjectId(4),
                ObjectId(5),
                ObjectId(6),
                ObjectId(7)
            ])
        );
    }

    #[test]
    fn a_range_is_measured_in_visible_order_so_folded_rows_are_not_reachable() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        // `landing` folds away hero and its subtree, so a range from `landing`
        // to `about` cannot select rows the user cannot see.
        projection.toggle_disclosure(ObjectId(1));
        projection.set_cursor(ObjectId(1));
        assert_eq!(
            projection.range_from_cursor(ObjectId(7)),
            Some(vec![ObjectId(1), ObjectId(7)])
        );
    }

    #[test]
    fn a_range_with_nothing_to_measure_from_is_not_a_range() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        assert_eq!(
            projection.range_from_cursor(ObjectId(3)),
            None,
            "no anchor yet: `⇧` on its own means the row the user pressed it on"
        );
        projection.set_cursor(ObjectId(3));
        projection.toggle_disclosure(ObjectId(2));
        assert_eq!(
            projection.range_from_cursor(ObjectId(4)),
            None,
            "a folded anchor is not a range endpoint"
        );
        assert_eq!(
            projection.range_from_cursor(ObjectId(99)),
            None,
            "an absent row is not a range endpoint"
        );
    }

    // ---------------------------------------------------- keyboard navigation

    #[test]
    fn the_plain_arrow_keys_stay_with_the_canvas() {
        // `AppShell::canvas_arrow_key` already owns nudging and panning. A panel
        // that consumed them would take that away from every other surface.
        for key in ["up", "down", "left", "right"] {
            assert_eq!(
                navigation(key, modifiers(false, false)),
                None,
                "{key} must not belong to the layers panel"
            );
            assert_eq!(
                navigation(key, modifiers(true, false)),
                None,
                "shift-{key} is the canvas's ten-times nudge"
            );
        }
    }

    #[test]
    fn the_keyboard_grammar_is_the_corpus_bindings_on_keys_nothing_else_claims() {
        assert_eq!(
            navigation("tab", modifiers(false, false)),
            Some(Navigation::Sibling(1))
        );
        assert_eq!(
            navigation("tab", modifiers(true, false)),
            Some(Navigation::Sibling(-1))
        );
        assert_eq!(
            navigation("up", modifiers(false, true)),
            Some(Navigation::Parent)
        );
        assert_eq!(
            navigation("down", modifiers(false, true)),
            Some(Navigation::Reveal)
        );
        assert_eq!(
            navigation("left", modifiers(false, true)),
            Some(Navigation::Hide)
        );
        assert_eq!(
            navigation("right", modifiers(false, true)),
            Some(Navigation::Reveal)
        );
        assert_eq!(
            navigation("home", modifiers(false, false)),
            Some(Navigation::Extreme(false))
        );
        assert_eq!(
            navigation("end", modifiers(false, false)),
            Some(Navigation::Extreme(true))
        );
        for key in ["enter", "f2"] {
            assert_eq!(
                navigation(key, modifiers(false, false)),
                Some(Navigation::Rename)
            );
        }
        assert_eq!(navigation("delete", modifiers(false, false)), None);
        assert_eq!(navigation("v", modifiers(false, false)), None);
    }

    #[test]
    fn only_the_ways_of_moving_a_selection_extend_it() {
        let extend = navigation_extends;
        assert!(extend(Navigation::Sibling(1), modifiers(true, true)));
        assert!(extend(Navigation::Parent, modifiers(true, true)));
        assert!(extend(Navigation::Reveal, modifiers(true, true)));
        assert!(extend(Navigation::Hide, modifiers(true, true)));
        assert!(extend(Navigation::Extreme(true), modifiers(true, false)));
        assert!(
            !extend(Navigation::Rename, modifiers(true, false)),
            "opening a rename does not move a selection"
        );
        assert!(!extend(Navigation::Parent, modifiers(false, true)));
    }

    #[test]
    fn the_ends_of_the_panel_are_reachable_from_anywhere() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        assert_eq!(projection.row_at(0), Some(ObjectId(1)));
        assert_eq!(projection.row_at(6), Some(ObjectId(7)));
        // With no selection the extremes still resolve, because they do not need
        // a cursor to know where the panel starts and ends.
        assert_eq!(
            projection.navigation_target(Navigation::Extreme(false)),
            Some(0)
        );
        assert_eq!(
            projection.navigation_target(Navigation::Extreme(true)),
            Some(6)
        );
        // And with the cursor deep in the tree.
        projection.set_cursor(ObjectId(3));
        assert_eq!(
            projection.navigation_target(Navigation::Extreme(false)),
            Some(0)
        );
        assert_eq!(
            projection.navigation_target(Navigation::Extreme(true)),
            Some(6)
        );
    }

    #[test]
    fn the_ends_of_the_panel_move_when_a_subtree_is_folded_away() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        // `logo` is the last row, and it goes away with `nav`.
        assert_eq!(projection.row_at(5), Some(ObjectId(6)));
        projection.toggle_disclosure(ObjectId(5));
        assert_eq!(projection.row_at(5), Some(ObjectId(7)));
        assert_eq!(
            projection.navigation_target(Navigation::Extreme(true)),
            Some(5)
        );
    }

    #[test]
    fn sibling_traversal_skips_descendants() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        // `hero`'s next sibling is `nav`. `title` and `cta` come first in
        // display order and belong to `hero` itself, so traversal has to step
        // over them rather than stop at them.
        projection.set_cursor(ObjectId(2));
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(1)),
            Some(4)
        );
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(-1)),
            None,
            "`landing` is `hero`'s parent, not its previous sibling"
        );

        // From `nav` the run runs the other way: `title` and `cta` are `hero`'s
        // descendants and must not read as `nav`'s previous sibling either.
        projection.set_cursor(ObjectId(5));
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(1)),
            None,
            "`logo` is `nav`'s child, and `about` is `landing`'s sibling, so \
             `nav` has no next sibling"
        );
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(-1)),
            Some(1)
        );

        // Roots are siblings of each other, with the whole of `landing`'s subtree
        // in between.
        projection.set_cursor(ObjectId(1));
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(1)),
            Some(6)
        );

        // With no cursor at all, a forward step starts at the top and a backward
        // step has nowhere to go.
        let mut no_cursor = Panel::default();
        no_cursor.load(&objects, &structure);
        assert_eq!(no_cursor.navigation_target(Navigation::Sibling(1)), Some(0));
        assert_eq!(no_cursor.navigation_target(Navigation::Sibling(-1)), None);
    }

    #[test]
    fn descent_and_ascent_follow_the_tree_not_the_row_order() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(1));
        assert_eq!(projection.navigation_target(Navigation::Child), Some(1));
        projection.set_cursor(ObjectId(4));
        assert_eq!(
            projection.navigation_target(Navigation::Parent),
            Some(1),
            "a leaf ascends to its parent"
        );
        projection.set_cursor(ObjectId(1));
        assert_eq!(projection.navigation_target(Navigation::Parent), None);
    }

    #[test]
    fn reveal_folds_nothing_when_unfolded_and_descends_instead() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(1));
        assert_eq!(
            projection.disclosure_step(Navigation::Reveal),
            None,
            "`landing` is already unfolded, so revealing it descends instead"
        );
        projection.toggle_disclosure(ObjectId(1));
        assert_eq!(
            projection.disclosure_step(Navigation::Reveal),
            Some(ObjectId(1)),
            "a folded row unfolds rather than descending into nothing"
        );
    }

    #[test]
    fn hide_folds_an_unfolded_row_and_ascends_from_a_leaf() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load(&objects, &structure);
        projection.set_cursor(ObjectId(1));
        assert_eq!(
            projection.disclosure_step(Navigation::Hide),
            Some(ObjectId(1))
        );
        projection.set_cursor(ObjectId(3));
        assert_eq!(
            projection.disclosure_step(Navigation::Hide),
            None,
            "a leaf has nothing to fold, so hiding it ascends"
        );
        assert_eq!(projection.navigation_target(Navigation::Parent), Some(1));
    }

    #[test]
    fn the_cursor_falls_back_to_the_selection_when_its_own_row_is_folded_away() {
        let (objects, structure) = tree();
        let mut projection = Panel::default();
        projection.load_with_selection(&objects, &structure, &[ObjectId(6)]);
        // `logo` is selected but folded away with `nav`, so it has no visible
        // row: the cursor falls back to nothing rather than to an invisible one,
        // and a forward step starts at the top.
        projection.toggle_disclosure(ObjectId(5));
        projection.set_cursor(ObjectId(6));
        assert_eq!(
            names(&projection.visible()),
            vec![
                ObjectId(1),
                ObjectId(2),
                ObjectId(3),
                ObjectId(4),
                ObjectId(5),
                ObjectId(7)
            ]
        );
        assert_eq!(
            projection.navigation_target(Navigation::Sibling(1)),
            Some(0)
        );

        // With the cursor's own row visible again, navigation resumes from it:
        // `logo` is the sixth row, so `about` is its next sibling's sibling and
        // a forward sibling step from `nav` has nowhere to go.
        projection.toggle_disclosure(ObjectId(5));
        projection.set_cursor(ObjectId(5));
        assert_eq!(projection.navigation_target(Navigation::Sibling(1)), None);
        assert_eq!(
            projection.navigation_target(Navigation::Parent),
            Some(0),
            "ascending out of `nav` lands on `landing`, whose own sibling \
             `about` is a separate `Tab` away"
        );
    }

    #[test]
    fn an_empty_panel_has_nowhere_to_navigate_to() {
        let mut projection = Panel::default();
        projection.load(&[], &LamineStructure::default());
        assert!(projection.visible().is_empty());
        assert_eq!(projection.panel_height(), tree_height(0));
        for step in [
            Navigation::Sibling(1),
            Navigation::Sibling(-1),
            Navigation::Extreme(true),
            Navigation::Parent,
            Navigation::Child,
        ] {
            assert_eq!(projection.navigation_target(step), None, "{step:?}");
        }
        assert_eq!(projection.disclosure_step(Navigation::Reveal), None);
    }

    // ---------------------------------------------------------------- layout

    #[test]
    fn the_panel_is_as_tall_as_the_rows_it_shows() {
        assert_eq!(tree_height(0), 14.0);
        assert_eq!(tree_height(1), 41.0);
        assert_eq!(tree_height(4), 134.0);
        assert_eq!(tree_height(1000), 31010.0);
        let style = StyleRefinement::default()
            .w(px(PANEL_WIDTH))
            .h(px(tree_height(4)));
        assert_eq!(style.size.width, Some(px(PANEL_WIDTH).into()));
        assert_eq!(style.size.height, Some(px(134.0).into()));
        assert_eq!(style.flex_shrink, None);
    }

    #[test]
    fn every_object_type_has_a_glyph_so_no_row_is_unlabelled() {
        for object_type in [
            ObjectType::Frame,
            ObjectType::Rectangle,
            ObjectType::Ellipse,
            ObjectType::Text,
        ] {
            assert!(!glyph(object_type).is_empty(), "{object_type:?}");
        }
        // Distinct glyphs: a tree whose rows are distinguishable by shape alone
        // survives being read without their names.
        let glyphs = [
            glyph(ObjectType::Frame),
            glyph(ObjectType::Rectangle),
            glyph(ObjectType::Ellipse),
            glyph(ObjectType::Text),
        ];
        let unique: HashSet<&str> = glyphs.iter().copied().collect();
        assert_eq!(unique.len(), glyphs.len());
    }
}
