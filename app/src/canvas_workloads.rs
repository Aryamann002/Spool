//! Debug-only frame-paced driver. Uses production mutation methods and renderer,
//! but intentionally bypasses OS input dispatch. Never active without both env vars.
use super::*;

const STEPS: u32 = 60;
const WARMUP: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Workload {
    Pan,
    Drag,
    Selection,
    Text,
    /// Exercises the document a real project was opened into, rather than a
    /// synthetic fixture. Requires `SPOOL_PROJECT` to have been set.
    Project,
}

fn config() -> Option<(Workload, usize)> {
    if !diagnostics::enabled() {
        return None;
    }
    let value = std::env::var("SPOOL_WORKLOAD").ok()?;
    let (kind, count) = value.split_once(':')?;
    let kind = match kind {
        "pan" => Workload::Pan,
        "drag" => Workload::Drag,
        "selection" => Workload::Selection,
        "text" => Workload::Text,
        "project" => Workload::Project,
        _ => return None,
    };
    let count: usize = count.parse().ok()?;
    // The project workload runs against whatever was opened, so the requested
    // count is only a positive sanity check, never a document size.
    if kind == Workload::Project {
        return (count > 0).then_some((kind, count));
    }
    [100, 1_000, 10_000]
        .contains(&count)
        .then_some((kind, count))
}

fn fixture(count: usize) -> Document {
    let objects = (0..count)
        .map(|index| DesignObject {
            id: ObjectId(index as u64 + 5),
            spool_id: node_id(format!("spool-workload-{index:016x}")),
            name: format!("Rectangle {}", index + 1),
            position: point((index % 100) as f32 * 120.0, (index / 100) as f32 * 120.0),
            size: size(80.0, 80.0),
            object_type: ObjectType::Rectangle,
            text_content: None,
            text_color: None,
            font_size: None,
            fill: default_style(ObjectType::Rectangle).fill,
            stroke: None,
        })
        .collect();
    Document {
        objects,
        next_id: count as u64 + 5,
        next_node_id: count as u64 + 1,
        next_names: [1, count as u64 + 1, 1, 1],
        layer_structure_revision: 0,
    }
}

impl CanvasView {
    /// World positions of the given runtime objects, for runtime probes.
    pub(super) fn positions_of(&self, ids: &[ObjectId]) -> Vec<(f32, f32)> {
        ids.iter()
            .map(|id| {
                let geometry = self.session.runtime.geometry(*id).expect("object exists");
                (geometry.position.x, geometry.position.y)
            })
            .collect()
    }
    pub(super) fn install_workload_fixture(&mut self) {
        if let Some((kind, count)) = config() {
            self.workload_running = true;
            // The project workload must keep the document that was opened from
            // disk; swapping in a synthetic fixture would defeat the point.
            if kind != Workload::Project {
                self.session.runtime = fixture(count);
            }
            if kind == Workload::Text {
                let object = &mut self.session.runtime.objects[0];
                object.object_type = ObjectType::Text;
                object.text_content = Some("Text".into());
            }
        }
    }

    pub(super) fn start_workload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workload_started {
            return;
        }
        self.workload_started = true;
        if let Some((kind, count)) = config() {
            cx.on_next_frame(window, move |this, window, cx| {
                this.workload_frame(kind, count, 0, window, cx)
            });
        }
    }

    fn workload_frame(
        &mut self,
        kind: Workload,
        count: usize,
        frame: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if frame == 0 {
            self.camera.offset = point(0.0, 0.0);
            self.camera.zoom = 1.0;
            if kind == Workload::Drag {
                let objects: Vec<_> = self
                    .session
                    .runtime
                    .objects
                    .iter()
                    .rev()
                    .take(10)
                    .map(|object| ObjectSnapshot {
                        id: object.id,
                        geometry: object.geometry(),
                    })
                    .collect();
                let selected_ids: Vec<_> = objects.iter().map(|object| object.id).collect();
                // True pre-gesture positions, captured before any pointer step,
                // so the runtime probe can tell "undo restored the start" from
                // "nothing ever moved".
                self.drag_start_positions = objects
                    .iter()
                    .map(|snapshot| (snapshot.geometry.position.x, snapshot.geometry.position.y))
                    .collect();
                self.selection.replace(selected_ids.clone());
                self.interaction = Interaction::PotentialMove(MoveGesture {
                    pointer_start_screen: point(0.0, 0.0),
                    pointer_start_world: point(0.0, 0.0),
                    click_selection: ClickSelection::SelectOnly(selected_ids[0]),
                    objects,
                    selected_ids,
                });
            }
            if kind == Workload::Text {
                self.begin_text_edit(ObjectId(5), window, cx);
            }
            if kind == Workload::Project {
                // Select the root the projection emitted and start a real drag
                // on it, so the gesture path runs against project state.
                let Some(root_id) = self.document_objects().first().map(|o| o.id) else {
                    eprintln!("spool_project_probe failed=no_runtime_objects");
                    self.workload_running = false;
                    return;
                };
                self.selection.replace(vec![root_id]);
                self.drag_start_positions = self.positions_of(&[root_id]).into_iter().collect();
                self.interaction = Interaction::PotentialMove(MoveGesture {
                    pointer_start_screen: point(0.0, 0.0),
                    pointer_start_world: point(0.0, 0.0),
                    objects: self
                        .document_objects()
                        .iter()
                        .map(|o| ObjectSnapshot {
                            id: o.id,
                            geometry: o.geometry(),
                        })
                        .collect(),
                    selected_ids: vec![root_id],
                    click_selection: ClickSelection::SelectOnly(root_id),
                });
            }
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        if frame < WARMUP {
            cx.on_next_frame(window, move |this, window, cx| {
                this.workload_frame(kind, count, frame + 1, window, cx)
            });
            return;
        }
        let step = frame - WARMUP;
        if step == 0 {
            diagnostics::reset();
            diagnostics::count(
                "initial_viewport_width_px",
                self.camera.viewport.width.round() as u64,
            );
            diagnostics::count(
                "initial_viewport_height_px",
                self.camera.viewport.height.round() as u64,
            );
        }
        if step == STEPS && kind == Workload::Project {
            // The whole product loop, against the project that was actually
            // opened from disk: move, undo, redo, save, reopen.
            self.project_loop_probe();
            self.workload_running = false;
            eprintln!("spool_workload_complete {kind:?}:{count}");
            return;
        }
        if step == STEPS {
            // The last update's frame has completed before this callback.
            diagnostics::report(&format!("{kind:?}:{count}"));
            if kind == Workload::Drag {
                let start = diagnostics::start();

                // Runtime evidence that the live commit path really goes
                // through EditSession -> SemanticHistory, and that one
                // committed drag is one undoable entry.
                let dragged: Vec<ObjectId> = self
                    .session
                    .runtime
                    .objects
                    .iter()
                    .rev()
                    .take(10)
                    .map(|object| object.id)
                    .collect();
                let resting = std::mem::take(&mut self.drag_start_positions);
                assert_eq!(
                    resting.len(),
                    dragged.len(),
                    "the probe must compare the same objects the gesture moved"
                );
                let depth_before = self.session.history.undo_len();

                self.finish_interaction(point(STEPS as f32 * 5.0, STEPS as f32 * 3.0));
                diagnostics::record("drag_finish_history", start);

                let depth_after = self.session.history.undo_len();
                let committed = self.positions_of(&dragged);

                let undid = self.undo_history();
                let after_undo = self.positions_of(&dragged);

                let redid = self.redo_history();
                let after_redo = self.positions_of(&dragged);

                let moved = committed != resting;
                eprintln!(
                    "spool_history_probe depth_before={depth_before} depth_after_commit={depth_after} \
exactly_one_entry={} objects_moved={} undo_ok={undid} undo_restored_start={} \
redo_ok={redid} redo_matches_commit={}",
                    depth_after - depth_before == 1,
                    moved,
                    after_undo == resting,
                    after_redo == committed,
                );
                diagnostics::report(&format!("{kind:?}:{count}:finished"));
            }
            // Runtime proof that pan, zoom and selection changes are runtime
            // state and never become history entries.
            let depth = self.session.history.undo_len();
            eprintln!(
                "spool_history_depth_probe kind={kind:?} depth={depth} runtime_state_excluded={}",
                depth == 0
            );
            self.workload_running = false;
            eprintln!("spool_workload_complete {kind:?}:{count}");
            return;
        }
        diagnostics::count("workload_updates", 1);
        let start = diagnostics::start();
        let pointer = point((step + 1) as f32 * 5.0, (step + 1) as f32 * 3.0);
        match kind {
            Workload::Pan => {
                self.camera.pan_from(
                    point(0.0, 0.0),
                    point(0.0, 0.0),
                    point(-pointer.x, -pointer.y),
                );
                diagnostics::count("canvas_notify", 1);
                cx.notify();
            }
            Workload::Drag | Workload::Project => {
                let before = std::mem::discriminant(&self.interaction);
                assert!(
                    self.update_interaction(pointer),
                    "synthetic drag interrupted at step {step}, state {before:?}, running {}",
                    self.workload_running
                );
                diagnostics::count("canvas_notify", 1);
                cx.notify();
            }
            Workload::Selection => {
                self.select_object(
                    ObjectId(if step.is_multiple_of(2) {
                        5
                    } else {
                        count as u64 + 4
                    }),
                    false,
                    cx,
                );
            }
            Workload::Text => {
                self.replace_editing_text(None, "x", cx);
            }
        }
        diagnostics::record("workload_update", start);
        cx.on_next_frame(window, move |this, window, cx| {
            this.workload_frame(kind, count, frame + 1, window, cx)
        });
    }

    /// running binary actually did.
    ///
    /// This is deliberately the production path — [`CanvasView::save_project`] and
    /// [`crate::project_open::open_project`] — so the probe proves the shipped
    /// lifecycle rather than a parallel imitation of it. Every value printed is read
    /// back out of the live canvas or off the filesystem after the write.
    fn project_loop_probe(&mut self) {
        let Some(root) = self.project_root.clone() else {
            eprintln!("spool_project_probe failed=no_project_open");
            return;
        };
        let dragged = self.selection().ids().to_vec();
        let resting = std::mem::take(&mut self.drag_start_positions);
        let depth_before = self.session.history.undo_len();

        self.finish_interaction(point(STEPS as f32 * 5.0, STEPS as f32 * 3.0));
        let depth_after = self.session.history.undo_len();
        let committed = self.positions_of(&dragged);

        let undid = self.undo_history();
        let after_undo = self.positions_of(&dragged);
        let redid = self.redo_history();
        let after_redo = self.positions_of(&dragged);

        let objects: Vec<(String, String, u64)> = self
            .document_objects()
            .iter()
            .map(|o| (o.spool_id.as_str().to_owned(), o.name.clone(), o.id.0))
            .collect();
        let appearance: Vec<(String, Option<String>, Option<u32>, u32)> = self
            .document_objects()
            .iter()
            .map(|o| {
                (
                    o.spool_id.as_str().to_owned(),
                    o.text_content.clone(),
                    o.fill.map(|fill| fill.color.to_rgb()),
                    o.font_size.unwrap_or(0.0) as u32,
                )
            })
            .collect();
        let persistent: Vec<String> = self
            .persistent_document()
            .structure
            .nodes
            .iter()
            .map(|n| n.id.as_str().to_owned())
            .collect();

        let save = match self.save_project() {
            Ok(outcome) => {
                let written: Vec<String> = outcome
                    .written
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                let unsupported: Vec<String> = outcome
                    .unsupported
                    .iter()
                    .map(|edit| format!("{}:{}", edit.node.as_str(), edit.kind))
                    .collect();
                eprintln!("spool_save_probe written={written:?} unsupported={unsupported:?}");
                true
            }
            Err(error) => {
                eprintln!("spool_save_probe failed={error}");
                false
            }
        };

        // Reopen from disk through the ordinary loader, not from memory.
        let reopened = match crate::project_open::open_project(&root) {
            Ok(loaded) => loaded,
            Err(error) => {
                eprintln!("spool_reopen_probe failed={error}");
                return;
            }
        };
        let reopened_geometry: Vec<(String, f32, f32, f32, f32)> = reopened
            .runtime
            .objects()
            .iter()
            .map(|o| {
                (
                    o.spool_id.as_str().to_owned(),
                    o.position.x,
                    o.position.y,
                    o.size.width,
                    o.size.height,
                )
            })
            .collect();
        let reopened_text: Vec<(String, Option<String>)> = reopened
            .runtime
            .objects()
            .iter()
            .map(|o| (o.spool_id.as_str().to_owned(), o.text_content.clone()))
            .collect();

        eprintln!(
        "spool_project_probe persistent_nodes={persistent:?} runtime_objects={objects:?}\n\
spool_project_appearance {appearance:?}\n\
spool_project_move_probe resting={resting:?} committed={committed:?} after_undo={after_undo:?} after_redo={after_redo:?} \
exactly_one_history_entry={} undo_restored_start={} redo_matches_commit={} undo_ok={undid} redo_ok={redid}",
        depth_after - depth_before == 1,
        after_undo == resting,
        after_redo == committed,
    );
        eprintln!(
            "spool_reopen_probe geometry={reopened_geometry:?} text={reopened_text:?} saved={save}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixtures_are_deterministic_and_keep_ids_out_of_starter_range() {
        for count in [100, 1_000, 10_000] {
            let a = fixture(count);
            let b = fixture(count);
            assert_eq!(a.objects(), b.objects());
            assert_eq!(a.objects().len(), count);
            assert_eq!(a.objects()[0].id, ObjectId(5));
            assert_eq!(a.objects()[count - 1].id, ObjectId(count as u64 + 4));
            assert_eq!(a.next_id, count as u64 + 5);
            assert!(a.objects().iter().any(|object| object.position.x > 1000.0));
        }
    }
}
