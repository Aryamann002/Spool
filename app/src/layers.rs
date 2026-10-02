use std::collections::HashSet;

use gpui::{
    div, prelude::*, px, rgb, Context, Render, SharedString, StyleRefinement, WeakEntity, Window,
};

use crate::{
    canvas::{CanvasView, DesignObject, ObjectId, ObjectType},
    diagnostics, theme,
};

#[derive(Clone, Debug, PartialEq)]
struct LayerRow {
    id: ObjectId,
    name: SharedString,
    object_type: ObjectType,
    /// Presentation-only state: whether this row is currently selected. It
    /// never touches the document, the structure revision, history or
    /// serialization; rows render their selected colors from it, so
    /// construction performs no membership scan.
    selected: bool,
    /// Precomputed element id, built once when the structure projection is
    /// (re)built. Row rebuilds clone it instead of formatting and allocating
    /// a fresh string on every construction.
    element_id: SharedString,
}

/// A selection-presentation change for one row: `(row id, now selected)`.
type PresentationChange = (ObjectId, bool);

#[derive(Default)]
struct LayersProjection {
    revision: Option<u64>,
    rows: Vec<LayerRow>,
    selected: Vec<ObjectId>,
}

pub struct SyncOutcome {
    pub structure_changed: bool,
    pub presentation: Vec<PresentationChange>,
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
        }
        let selection_changed = self.selected != selected;
        let presentation = if selection_changed {
            // O(changed rows) detection: one linear scan of row ids, flipping
            // each row's presentation flag exactly where selection membership
            // changed, and reporting exactly those rows. Rows render their
            // colors from the per-row flag, so no membership lookup happens
            // during construction. GPUI applies these changes through a full
            // row-tree rebuild of the single cached Layers subtree (a parent
            // re-render invalidates all nested cached views — see the Phase 15
            // report), so the affected-row count is reported separately from
            // `layers_row_construction`.
            let changes = {
                let start = diagnostics::start();
                let changes = apply_selection(&mut self.rows, selected);
                diagnostics::record("layers_selection_sync", start);
                changes
            };
            self.selected = selected.to_vec();
            diagnostics::count("layers_selection_update", 1);
            diagnostics::count("row_presentation_updates", changes.len() as u64);
            changes
        } else {
            Vec::new()
        };
        SyncOutcome {
            structure_changed,
            presentation,
        }
    }
}

/// Test-only pure reference for `apply_selection`: the same O(changed rows)
/// diff, without mutating the rows. Production uses `apply_selection`.
#[cfg(test)]
fn presentation_diff(
    previous: &[ObjectId],
    next: &[ObjectId],
    rows: &[LayerRow],
) -> Vec<PresentationChange> {
    let before: HashSet<ObjectId> = previous.iter().copied().collect();
    let after: HashSet<ObjectId> = next.iter().copied().collect();
    rows.iter()
        .filter_map(
            |row| match (before.contains(&row.id), after.contains(&row.id)) {
                (true, false) => Some((row.id, false)),
                (false, true) => Some((row.id, true)),
                _ => None,
            },
        )
        .collect()
}

/// The mutating half of the presentation diff: applies `next` selection
/// membership to the retained rows and returns the rows whose flag flipped.
/// One linear scan plus two small hash sets (selections, not row totals).
fn apply_selection(rows: &mut [LayerRow], next: &[ObjectId]) -> Vec<PresentationChange> {
    let after: HashSet<ObjectId> = next.iter().copied().collect();
    let mut changes = Vec::new();
    for row in rows.iter_mut() {
        let now_selected = after.contains(&row.id);
        if row.selected != now_selected {
            changes.push((row.id, now_selected));
            row.selected = now_selected;
        }
    }
    changes
}

fn project(objects: &[DesignObject]) -> Vec<LayerRow> {
    objects
        .iter()
        .map(|object| LayerRow {
            element_id: SharedString::from(format!("layer-{:?}", object.id)),
            id: object.id,
            name: object.name.clone().into(),
            object_type: object.object_type,
            selected: false,
        })
        .collect()
}

fn tree_height(rows: usize) -> f32 {
    rows as f32 * 27.0 + rows.saturating_sub(1) as f32 * 4.0 + 14.0
}

pub struct LayersView {
    canvas: WeakEntity<CanvasView>,
    projection: LayersProjection,
}

impl LayersView {
    pub fn new(canvas: WeakEntity<CanvasView>) -> Self {
        Self {
            canvas,
            projection: LayersProjection::default(),
        }
    }

    /// Returns the synchronization outcome: `structure_changed` drives the
    /// shell's notify of this view; `presentation` carries the O(changed)
    /// selection diff that was counted as `row_presentation_updates`.
    pub fn synchronize(&mut self, canvas: &CanvasView) -> SyncOutcome {
        self.projection.synchronize(
            canvas.layer_structure_revision(),
            || project(canvas.document_objects()),
            canvas.selection().ids(),
        )
    }

    pub fn cached_style(&self) -> StyleRefinement {
        // Match the original intrinsic tree size, including sidebar border and
        // bottom padding. Default flex-shrink (1) preserves constrained layouts.
        StyleRefinement::default()
            .w(px(203.0))
            .h(px(tree_height(self.projection.rows.len())))
    }
}

impl Render for LayersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        diagnostics::count("layers_build", 1);
        let start = diagnostics::start();
        let mut tree = div()
            .flex()
            .flex_col()
            .size_full()
            .gap_1()
            .px(px(10.0))
            .pb(px(14.0));
        for row in &self.projection.rows {
            diagnostics::count("layers_row_construction", 1);
            let id = row.id;
            let selected = row.selected;
            let icon = match row.object_type {
                ObjectType::Frame => "▱",
                ObjectType::Rectangle => "□",
                ObjectType::Ellipse => "○",
                ObjectType::Text => "T",
            };
            tree = tree.child(
                div()
                    .id(row.element_id.clone())
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(27.0))
                    .pl(px(8.0))
                    .pr(px(7.0))
                    .rounded_md()
                    .bg(rgb(if selected {
                        theme::ACCENT_WASH
                    } else {
                        theme::SURFACE
                    }))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .text_xs()
                    .text_color(rgb(if selected {
                        theme::ACCENT
                    } else {
                        theme::TEXT_SECONDARY
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let _ = this.canvas.update(cx, |canvas, cx| {
                            canvas.select_object(id, false, cx);
                        });
                    }))
                    .child(icon)
                    .child(row.name.clone()),
            );
        }
        diagnostics::record("layers_tree_build", start);
        tree
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    /// Entity-free mirror of `LayersView` synchronization: same projection,
    /// same presentation diff, no GPUI context required.
    #[derive(Default)]
    pub struct RetainedLayers {
        projection: LayersProjection,
        document_walks: usize,
        presentation: Vec<PresentationChange>,
    }

    impl RetainedLayers {
        pub fn synchronize(&mut self, canvas: &CanvasView) -> Vec<(ObjectId, SharedString)> {
            let outcome = self.projection.synchronize(
                canvas.layer_structure_revision(),
                || {
                    self.document_walks += 1;
                    project(canvas.document_objects())
                },
                canvas.selection().ids(),
            );
            self.presentation = outcome.presentation;
            self.projection
                .rows
                .iter()
                .map(|row| (row.id, row.name.clone()))
                .collect()
        }

        pub fn document_walks(&self) -> usize {
            self.document_walks
        }

        pub fn selected(&self) -> &[ObjectId] {
            &self.projection.selected
        }

        /// Presentation changes detected by the most recent `synchronize`.
        pub fn presentation_updates(&self) -> &[PresentationChange] {
            &self.presentation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starter_rows() -> Vec<LayerRow> {
        [
            ObjectId::LANDING,
            ObjectId::EDITOR,
            ObjectId::FEATURES,
            ObjectId::MOBILE,
        ]
        .into_iter()
        .map(row_for)
        .collect()
    }

    fn row_for(id: ObjectId) -> LayerRow {
        LayerRow {
            element_id: SharedString::from(format!("layer-{id:?}")),
            id,
            name: SharedString::from(format!("row-{id:?}")),
            object_type: ObjectType::Frame,
            selected: false,
        }
    }

    fn extra_row(id: ObjectId) -> LayerRow {
        LayerRow {
            element_id: SharedString::from(format!("layer-{id:?}")),
            id,
            name: SharedString::from(format!("row-{id:?}")),
            object_type: ObjectType::Text,
            selected: false,
        }
    }

    #[test]
    fn projection_is_lazy_and_selection_is_separate() {
        let rows = starter_rows();
        let mut projection = LayersProjection::default();
        let outcome = projection.synchronize(0, || rows.clone(), &[]);
        assert!(outcome.structure_changed);
        assert!(outcome.presentation.is_empty());
        assert_eq!(projection.rows.len(), 4);
        assert_eq!(
            projection.rows[0].name,
            SharedString::from(format!("row-{:?}", ObjectId::LANDING))
        );
        assert_eq!(projection.rows[0].object_type, ObjectType::Frame);
        let unchanged = projection.rows.clone();
        let outcome = projection.synchronize(
            0,
            || panic!("unchanged structure must not walk document"),
            &[],
        );
        assert!(!outcome.structure_changed);
        assert!(outcome.presentation.is_empty());
        assert_eq!(projection.rows, unchanged);
        let chosen = [ObjectId::EDITOR, ObjectId::LANDING];
        let outcome =
            projection.synchronize(0, || panic!("selection must not project rows"), &chosen);
        assert!(!outcome.structure_changed);
        assert_eq!(projection.selected, chosen);
        assert!(
            !projection
                .synchronize(0, || panic!("pan must not project rows"), &chosen)
                .structure_changed
        );
        let outcome = projection.synchronize(1, || Vec::new(), &[]);
        assert!(outcome.structure_changed);
        assert!(outcome.presentation.is_empty());
        assert!(projection.rows.is_empty());
        assert!(projection.selected.is_empty());
        assert!(projection.selected.is_empty());
    }

    #[test]
    fn selection_diff_updates_only_changed_rows() {
        let rows = starter_rows();
        // Old selection {A, B}, new selection {B, C}: B does not change.
        let previous = [ObjectId::LANDING, ObjectId::EDITOR];
        let next = [ObjectId::EDITOR, ObjectId::FEATURES];
        assert_eq!(
            presentation_diff(&previous, &next, &rows),
            vec![(ObjectId::LANDING, false), (ObjectId::FEATURES, true)]
        );
    }

    #[test]
    fn selection_diff_handles_single_multi_and_deselection() {
        let rows = starter_rows();
        // One-row selection.
        assert_eq!(
            presentation_diff(&[], &[ObjectId::EDITOR], &rows),
            vec![(ObjectId::EDITOR, true)]
        );
        // One-row deselection.
        assert_eq!(
            presentation_diff(&[ObjectId::EDITOR], &[], &rows),
            vec![(ObjectId::EDITOR, false)]
        );
        // Multi-selection growth, in row order.
        assert_eq!(
            presentation_diff(&[], &[ObjectId::MOBILE, ObjectId::LANDING], &rows),
            vec![(ObjectId::LANDING, true), (ObjectId::MOBILE, true)]
        );
        // Partial deselection.
        assert_eq!(
            presentation_diff(
                &[ObjectId::LANDING, ObjectId::MOBILE],
                &[ObjectId::MOBILE],
                &rows
            ),
            vec![(ObjectId::LANDING, false)]
        );
        // Identical selections produce no work.
        assert!(presentation_diff(&[ObjectId::LANDING], &[ObjectId::LANDING], &rows).is_empty());
        // Unknown ids never appear because they have no row.
        assert!(presentation_diff(&[ObjectId(99)], &[], &rows).is_empty());
    }

    #[test]
    fn unchanged_selection_after_structure_change_is_silent() {
        let rows = starter_rows();
        let mut projection = LayersProjection::default();
        assert!(
            projection
                .synchronize(0, || rows.clone(), &[])
                .structure_changed
        );
        assert_eq!(projection.revision, Some(0));

        // Structure change: new row appended, no selection change.
        let extra = ObjectId(101);
        let mut grown = rows.clone();
        grown.push(extra_row(extra));
        let outcome = projection.synchronize(1, || grown.clone(), &[]);
        assert!(outcome.structure_changed);
        assert!(outcome.presentation.is_empty());
        assert_eq!(projection.revision, Some(1));
        // The retained rows are identical apart from the presentation flags
        // (structure replacement preserves identity, names, types and ids).
        let ids_after_structure: Vec<ObjectId> = projection.rows.iter().map(|r| r.id).collect();
        let names_after_structure: Vec<SharedString> =
            projection.rows.iter().map(|r| r.name.clone()).collect();

        // Selection alone: no structure rebuild, same revision; only the
        // presentation flag of the selected row moves.
        let revision = projection.revision;
        let outcome = projection.synchronize(
            1,
            || panic!("selection must not rebuild structure"),
            &[extra],
        );
        assert!(!outcome.structure_changed);
        // Selection never changes the structure revision.
        assert_eq!(projection.revision, revision);
        let ids_after_selection: Vec<ObjectId> = projection.rows.iter().map(|r| r.id).collect();
        assert_eq!(ids_after_selection, ids_after_structure);
        for (row, name) in projection.rows.iter().zip(&names_after_structure) {
            assert_eq!(&row.name, name);
        }
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|r| r.selected)
                .collect::<Vec<_>>(),
            vec![false, false, false, false, true]
        );
        assert_eq!(outcome.presentation, vec![(extra, true)]);
        // Membership view stays in sync for rendering.
        assert!(projection.selected.contains(&extra));
    }

    #[test]
    fn structure_change_followed_by_selection_reports_the_new_row() {
        let rows = starter_rows();
        let mut projection = LayersProjection::default();
        assert!(
            projection
                .synchronize(0, || rows.clone(), &[])
                .structure_changed
        );
        let extra = ObjectId(101);
        let mut grown = rows.clone();
        grown.push(extra_row(extra));
        let outcome = projection.synchronize(1, || grown.clone(), &[]);
        assert!(outcome.structure_changed);
        assert!(outcome.presentation.is_empty());

        // Next sync: selection only, row already retained by ObjectId.
        let outcome = projection.synchronize(
            1,
            || panic!("selection must not rebuild structure"),
            &[extra],
        );
        assert!(!outcome.structure_changed);
        assert_eq!(outcome.presentation, vec![(extra, true)]);
        assert_eq!(projection.rows.last().map(|row| row.id), Some(extra));
    }

    #[test]
    fn rows_created_together_with_a_selection_are_reported_once() {
        let rows = starter_rows();
        let mut projection = LayersProjection::default();
        assert!(
            projection
                .synchronize(0, || rows.clone(), &[])
                .structure_changed
        );
        let extra = ObjectId(101);
        let mut grown = rows.clone();
        grown.push(extra_row(extra));
        // Structure and selection change in the same synchronization.
        let outcome = projection.synchronize(1, || grown, &[extra]);
        assert!(outcome.structure_changed);
        assert_eq!(outcome.presentation, vec![(extra, true)]);
        assert!(projection.selected.contains(&extra));
    }

    #[test]
    fn row_identity_is_keyed_by_object_id_not_position() {
        let rows = starter_rows();
        let mut projection = LayersProjection::default();
        assert!(
            projection
                .synchronize(0, || rows.clone(), &[])
                .structure_changed
        );
        let starter_ids: Vec<ObjectId> = projection.rows.iter().map(|row| row.id).collect();

        // Append: existing rows keep their identity and element ids.
        let extra = ObjectId(101);
        let mut grown = rows.clone();
        grown.push(extra_row(extra));
        assert!(
            projection
                .synchronize(1, || grown.clone(), &[])
                .structure_changed
        );
        let ids_now: Vec<ObjectId> = projection.rows.iter().map(|row| row.id).collect();
        assert_eq!(&ids_now[..starter_ids.len()], &starter_ids[..]);
        assert_eq!(ids_now.last(), Some(&extra));

        // Remove the first row: every other row keeps its id, name and
        // precomputed element id; identity never derives from array position.
        let filtered: Vec<LayerRow> = projection
            .rows
            .iter()
            .filter(|row| row.id != ObjectId::LANDING)
            .cloned()
            .collect();
        assert!(
            projection
                .synchronize(2, || filtered, &[])
                .structure_changed
        );
        let landing = rows[0].clone();
        assert!(!projection
            .rows
            .iter()
            .any(|row| row.id == ObjectId::LANDING));
        for (before, after) in rows.iter().skip(1).zip(projection.rows.iter()) {
            assert_eq!(before.id, after.id);
            assert_eq!(before.element_id, after.element_id);
            assert_eq!(before.name, after.name);
        }
        // Restore (undo): the same ObjectId maps back to the same element id.
        assert!(
            projection
                .synchronize(3, || grown.clone(), &[])
                .structure_changed
        );
        assert_eq!(projection.rows[0].element_id, landing.element_id);
        assert_eq!(projection.rows[0].id, landing.id);
    }

    #[test]
    fn cached_dimensions_preserve_intrinsic_tree_size_and_shrink() {
        assert_eq!(tree_height(0), 14.0);
        assert_eq!(tree_height(1), 41.0);
        assert_eq!(tree_height(4), 134.0);
        assert_eq!(tree_height(1000), 31010.0);
        let style = StyleRefinement::default()
            .w(px(203.0))
            .h(px(tree_height(4)));
        assert_eq!(style.size.width, Some(px(203.0).into()));
        assert_eq!(style.size.height, Some(px(134.0).into()));
        assert_eq!(style.flex_shrink, None);
    }
}
