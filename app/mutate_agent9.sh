#!/usr/bin/env bash
# Mutation check for the Agent 9 interaction guarantees.
#
# Same contract as `mutate_milestone7.sh`: each mutation breaks exactly one
# behaviour this milestone claims, the tests that are supposed to catch it must
# FAIL. A surviving mutation means the corresponding test is not really testing
# the claim, which is worse than having no test at all.
#
# The selection here is deliberately about *interaction semantics* — snapping,
# modifiers, hover, cancellation and the rename boundary — because those are the
# guarantees a user would notice breaking, and because each one has exactly one
# place in the code that could get it wrong.
set -uo pipefail

cd "$(dirname "$0")" || exit 1

failures=0

run_mutation() {
  local name="$1" file="$2" script="$3" filter="$4"
  local backup result
  backup="$(mktemp)"
  cp "$file" "$backup"

  python3 - "$file" <<PYEOF
import sys
path = sys.argv[1]
source = open(path).read()
$script
open(path, "w").write(source)
PYEOF

  if ! cmp -s "$backup" "$file"; then
    if cargo test --offline "$filter" >/tmp/spool-mutation9.log 2>&1; then
      echo "SURVIVED  $name — the suite still passed with this broken"
      failures=$((failures + 1))
      result=1
    else
      echo "caught    $name"
      result=0
    fi
  else
    echo "NO-OP     $name — the pattern did not match; the mutation never applied"
    failures=$((failures + 1))
    result=1
  fi

  cp "$backup" "$file"
  rm -f "$backup"
  return $result
}

# 1. The threshold is screen pixels, so the feel is zoom-independent. Without the
#    division a snap that is unreachable at 400% becomes unavoidable at 20%.
run_mutation "the snap threshold stops being measured in screen pixels" \
  src/snap.rs \
  'old = """    threshold_px / zoom
}"""
new = """    threshold_px
}"""
assert old in source, "world_threshold pattern not found"
source = source.replace(old, new, 1)' \
  "the_threshold_is_screen_pixels_so_the_feel_is_the_same_at_every_zoom"

# 2. A zero correction is not a snap. Without the guard, an object that already
#    sits in line reports a guide and an arrow-key nudge of an aligned object
#    becomes a no-op.
run_mutation "a zero-distance alignment counts as a snap" \
  src/snap.rs \
  'old = """            if distance == 0.0 || distance > threshold {"""
new = """            if distance > threshold {"""
assert old in source, "zero-distance guard not found"
source = source.replace(old, new, 1)' \
  "arriving_exactly_on_a_line_is_not_a_correction"

# 3. A pointer that has not moved proposes nothing. Without this an object a few
#    pixels off a neighbour jumps into alignment on mouse-down, and a click
#    creates history for a gesture that never happened.
run_mutation "a press with no movement still snaps the object" \
  src/snap.rs \
  'old = """    if delta == (0.0, 0.0) {"""
new = """    if false {"""
assert old in source, "zero-delta guard not found"
source = source.replace(old, new, 1)' \
  "a_pointer_that_has_not_moved_proposes_nothing_so_nothing_snaps"

# 4. Each axis compares against its own candidate lines. Sharing one list makes a
#    purely horizontal drag move the object vertically as well.
#
#    Re-derived when the axis loop moved into `candidate_lines(&rects, axis)`:
#    the mutation is now "both axes are handed the same list", which is the same
#    bug at the same seam.
run_mutation "the vertical axis compares against horizontal candidate lines" \
  src/snap.rs \
  'old = """    let mut guides = Vec::new();
    for axis in [Axis::Vertical, Axis::Horizontal] {
        if !lock.allows(axis) {
            continue;
        }
        let candidates = candidate_lines(&rects, axis);"""
new = """    let mut guides = Vec::new();
    let shared = candidate_lines(&rects, Axis::Vertical);
    for axis in [Axis::Vertical, Axis::Horizontal] {
        if !lock.allows(axis) {
            continue;
        }
        let candidates = &shared;"""
assert old in source, "per-axis candidate list not found"
source = source.replace(old, new, 1)' \
  "the_axes_are_considered_independently"

# 5. `⌘` suspends snapping for the whole gesture.
run_mutation "the command modifier no longer suspends snapping" \
  src/canvas.rs \
  'old = """    modifiers.platform || modifiers.control
}"""
new = """    false
}"""
assert old in source, "suspends_snap not found"
source = source.replace(old, new, 1)' \
  "only_the_command_modifier_suspends_snapping"

# Deliberately NOT mutated: whether the mouse-down handler reads the command key
# into `suspend_snap`. That line lives inside a GPUI window listener, so it is
# reachable neither from a unit test nor from the frame-paced probe, and a
# mutation for it would have nothing to run. The *rule* it consults is covered
# above and the gesture's *use* of the flag is covered by the ⌘-drag test; the
# one-line wiring between them is a known gap, listed in the milestone report.

# 6. Guides must not outlive the gesture that drew them.
run_mutation "snap guides survive the end of their gesture" \
  src/canvas.rs \
  'old = """    fn clear_gesture_feedback(&mut self) {
        self.snap_guides.clear();"""
new = """    fn clear_gesture_feedback(&mut self) {"""
assert old in source, "clear_gesture_feedback not found"
source = source.replace(old, new, 1)' \
  "snap_guides_disappear_when_the_gesture_ends"

# 7. `⇧` constrains a move to the dominant axis.
run_mutation "shift no longer constrains a move to one axis" \
  src/canvas.rs \
  'old = """        let lock = dragged_axis(raw, self.constrain_drag);"""
new = """        let lock = snap::AxisLock::Free;
        let _ = dragged_axis(raw, self.constrain_drag);"""
assert old in source, "constrain_drag branch not found"
source = source.replace(old, new, 1)' \
  "shift_constrains_a_move_to_the_axis_the_pointer_chose"

# 8. The constraint is sampled per movement, not latched at press time.
run_mutation "the shift constraint is latched when the drag starts" \
  src/canvas.rs \
  'old = """        self.constrain_drag = constrain;"""
new = """        if !self.constrain_drag {
            self.constrain_drag = constrain;
        }"""
assert old in source, "constrain sample not found"
source = source.replace(old, new, 1)' \
  "releasing_shift_mid_drag_lets_the_object_move_on_both_axes_again"

# 9. `⇧` preserves the aspect ratio on a corner resize.
run_mutation "shift no longer preserves the aspect ratio when resizing" \
  src/canvas.rs \
  'old = """    let delta = if proportional {"""
new = """    let delta = if false && proportional {"""
assert old in source, "proportional branch not found"
source = source.replace(old, new, 1)' \
  "shift_preserves_the_aspect_ratio_of_a_corner_resize"

# 10. A keyboard nudge is a direct set, never pulled back by a magnet.
run_mutation "the keyboard nudge snaps like a drag" \
  src/canvas.rs \
  'old = """        let recorded: Vec<GeometryChange> = changes"""
new = """        let (snapped, _) = self.snap_delta(
            snap::bounds_of(
                &changes
                    .iter()
                    .map(|(_, g)| snap::Rect::new(g.position.x, g.position.y, g.size.width, g.size.height))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            &changes.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            (dx, dy),
        );
        let recorded: Vec<GeometryChange> = changes"""
assert old in source, "nudge record point not found"
source = source.replace(old, new, 1)
source = source.replace("""                    position: point(before.position.x + dx, before.position.y + dy),""",
                        """                    position: point(before.position.x + snapped.0, before.position.y + snapped.1),""", 1)' \
  "a_nudge_is_never_pulled_back_by_an_alignment"

# 11. Hover tracks the pointer, and nothing is hoverable during a gesture.
run_mutation "hover stays on after the pointer leaves the object" \
  src/canvas.rs \
  'old = """        if hovered == self.hovered {
            return false;
        }"""
new = """        if hovered.is_none() || hovered == self.hovered {
            return false;
        }"""
assert old in source, "hover change detection not found"
source = source.replace(old, new, 1)' \
  "hovering_tracks_the_pointer_and_stops_when_a_gesture_starts"

# 12. A rename opened from Layers opens with the existing name selected, so the
#     first keystroke replaces rather than appends.
run_mutation "a rename appends to the old name instead of replacing it" \
  src/inspector.rs \
  'old = """                if self.select_all {
                    self.buffer.clear();
                    self.select_all = false;
                }
                self.buffer.push_str(other);
                RenameEffect::Continue"""
new = """                if false {
                    self.buffer.clear();
                    self.select_all = false;
                }
                self.buffer.push_str(other);
                RenameEffect::Continue"""
assert old in source, "select_all branch not found"
# `inspector.rs` has two edit sessions with the same select-all rule — one for a
# numeric field, one for a rename — and `replace(..., 1)` takes the *first*. The
# `RenameEffect` tail is what makes this pattern name the rename session rather
# than the field session; without it the mutation lands in the wrong one and
# SURVIVES, which is a broken mutation rather than a missing test.
assert source.count(old) == 1, f"pattern is ambiguous: {source.count(old)} matches"
source = source.replace(old, new, 1)' \
  "a_rename_opens_with_the_old_name_selected_so_the_first_key_replaces_it"

# 13. Escape abandons an open rename without touching the document.
run_mutation "escape commits an abandoned rename" \
  src/inspector.rs \
  'old = """            "escape" => {
                self.close();
                RenameEffect::Abandon
            }"""
new = """            "escape" => {
                let name = self.buffer.clone();
                self.close();
                RenameEffect::Commit(name)
            }"""
assert old in source, "escape arm not found"
source = source.replace(old, new, 1)' \
  "escape_abandons_a_rename_without_committing_anything"

# 14. Cancelling a gesture puts the geometry back and records nothing. The
#     shell's *ordering* of the Escape ladder needs a window to test, so this
#     mutation covers the rung it calls rather than the ladder around it.
#     Anchored on `abandon_interaction`, which is where the restore lives for
#     every caller that gives up an in-flight gesture.
run_mutation "a cancelled drag keeps the moved geometry" \
  src/canvas.rs \
  'old = """    fn abandon_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }
        self.interaction.restore(&mut self.session.runtime);"""
new = """    fn abandon_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }"""
assert old in source, "abandon_interaction not found"
source = source.replace(old, new, 1)' \
  "live_gesture_cancel_restores_geometry_and_records_nothing"

# 15. An Inspector section index must not be able to index past the array.
run_mutation "the typography section claims a slot that is already taken" \
  src/inspector.rs \
  'old = """    pub const TYPOGRAPHY: Self = Self(3);"""
new = """    pub const TYPOGRAPHY: Self = Self(2);"""
assert old in source, "typography slot not found"
source = source.replace(old, new, 1)' \
  "every_section_has_its_own_slot_in_range"

# 16. A `⇧`-click in Layers toggles the row instead of replacing the selection.
run_mutation "shift-clicking a layer row always replaces the selection" \
  src/layers.rs \
  'old = """    let mode = if input.shift {
        SelectMode::Range"""
new = """    let mode = if false && input.shift {
        SelectMode::Range"""
assert old in source, "row mode not found"
source = source.replace(old, new, 1)' \
  "a_click_replaces_command_click_is_additive_and_shift_click_is_a_range"

# 17. A double click on a layer row opens a rename rather than selecting.
run_mutation "double-clicking a layer row does not open a rename" \
  src/layers.rs \
  'old = """    if input.clicks >= 2 {"""
new = """    if false && input.clicks >= 2 {"""
assert old in source, "double click branch not found"
source = source.replace(old, new, 1)' \
  "a_second_click_renames_rather_than_selecting_again"

# 18. Snap targets exclude the moving object and its descendants.
#
#     Re-derived when the viewport bound was added in front of the descendant
#     filter: the mutation now drops only the descendant half, so it still
#     targets the same rule rather than the viewport one.
run_mutation "an object snaps to its own child" \
  src/canvas.rs \
  'old = """                self.camera.sees(*rect)
                    && !moving_nodes
                        .iter()
                        .any(|candidate| self.is_within(node, candidate))"""
new = """                let _ = node;
                self.camera.sees(*rect)"""
assert old in source, "snap target filter not found"
source = source.replace(old, new, 1)' \
  "an_object_does_not_snap_to_its_own_child"

# 19. Abandoning a gesture has to reconcile the selection as well as the
#     geometry. An `⌥`-drag created the copies and handed the selection to
#     them, so restoring the geometry and stopping there leaves the panel
#     holding ids the document no longer has.
run_mutation "a cancelled duplicate drag leaves a dead selection" \
  src/canvas.rs \
  'old = """        self.interaction = Interaction::None;
        self.retain_existing_selection();
        true
    }"""
new = """        self.interaction = Interaction::None;
        true
    }"""
assert old in source, "abandon_interaction selection reconcile not found"
source = source.replace(old, new, 1)' \
  "workflow_k_cancelling_a_duplicate_drag_leaves_no_dead_selection"

# 20. A history key arriving mid-drag must finish the gesture, not throw it
#     away. Discarding it makes one keystroke destroy the edit in flight with
#     no record *and* revert whatever came before it.
run_mutation "undo mid-drag drops the drag instead of finishing it" \
  src/canvas.rs \
  'old = """    fn undo_history(&mut self) -> bool {
        self.commit_text_edit();
        self.commit_in_flight_interaction();"""
new = """    fn undo_history(&mut self) -> bool {
        self.commit_text_edit();
        self.abandon_interaction();"""
assert old in source, "undo_history in-flight handling not found"
source = source.replace(old, new, 1)' \
  "workflow_l_undo_mid_drag_reverts_the_drag_rather_than_dropping_it"

# 21. The camera is frozen while a gesture owns the pointer. Without that, a
#     pinch landing mid-drag re-projects the pointer through a camera that
#     moved and the object jumps back toward where the drag began.
run_mutation "the camera moves under a gesture in flight" \
  src/canvas.rs \
  'old = """    fn pointer_gesture_active(&self) -> bool {
        self.pan.is_some()"""
new = """    fn pointer_gesture_active(&self) -> bool {
        false && self.pan.is_some()"""
assert old in source, "pointer_gesture_active not found"
source = source.replace(old, new, 1)' \
  "workflow_n_a_gesture_in_flight_owns_the_camera"

# 22. Save measures a change from what the source currently says, so the
#     baseline has to advance when the source is written. Pinned to the opening
#     state it re-applies every delta on top of the last one.
run_mutation "save keeps measuring from the project opening" \
  src/canvas.rs \
  'old = """        self.source_snapshot = self
            .opened_state()
            .into_iter()
            .filter(|(node, _)| !unwritten.contains(node))
            .collect();"""
new = """        let _ = unwritten;"""
assert old in source, "save snapshot refresh not found"
source = source.replace(old, new, 1)' \
  "workflow_p_undo_then_save_puts_the_authored_bytes_back"

if [ "$failures" -eq 0 ]; then
  echo
  echo "every mutation was caught"
else
  echo
  echo "$failures mutation(s) were not caught"
  exit 1
fi