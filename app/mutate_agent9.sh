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
run_mutation "the vertical axis compares against horizontal candidate lines" \
  src/snap.rs \
  'old = """        [moved.top(), moved.center_y(), moved.bottom()],
        horizontal_lines.into_iter(),"""
new = """        [moved.top(), moved.center_y(), moved.bottom()],
        vertical_lines.into_iter(),"""
assert old in source, "horizontal line list not found"
source = source.replace(old, new, 1)' \
  "a_drag_near_a_neighbour_snap_into_line_and_say_so"

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
  'old = """        let raw = if self.constrain_drag {"""
new = """        let raw = if false && self.constrain_drag {"""
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
  src/shell.rs \
  'old = """                if self.select_all {
                    self.buffer.clear();
                    self.select_all = false;
                }
                self.buffer.push_str(other);"""
new = """                if false {
                    self.buffer.clear();
                    self.select_all = false;
                }
                self.buffer.push_str(other);"""
assert old in source, "select_all branch not found"
source = source.replace(old, new, 1)' \
  "a_rename_opens_with_the_old_name_selected_so_the_first_key_replaces_it"

# 13. Escape abandons an open rename without touching the document.
run_mutation "escape commits an abandoned rename" \
  src/shell.rs \
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
run_mutation "a cancelled drag keeps the moved geometry" \
  src/canvas.rs \
  'old = """    fn cancel_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }
        self.interaction.restore(&mut self.session.runtime);"""
new = """    fn cancel_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }"""
assert old in source, "cancel_interaction not found"
source = source.replace(old, new, 1)' \
  "live_gesture_cancel_restores_geometry_and_records_nothing"

# 15. An Inspector section index must not be able to index past the array.
run_mutation "the typography section claims a slot that is already taken" \
  src/shell.rs \
  'old = """const SECTION_TYPOGRAPHY: usize = 3;"""
new = """const SECTION_TYPOGRAPHY: usize = 2;"""
assert old in source, "typography slot not found"
source = source.replace(old, new, 1)' \
  "every_inspector_section_has_its_own_slot_in_range"

# 16. A `⇧`-click in Layers toggles the row instead of replacing the selection.
run_mutation "shift-clicking a layer row always replaces the selection" \
  src/layers.rs \
  'old = """        RowAction::Select { additive: shift }"""
new = """        RowAction::Select { additive: false }"""
assert old in source, "row additive not found"
source = source.replace(old, new, 1)' \
  "a_row_click_replaces_the_selection_shifted_click_toggles_and_a_double_click_renames"

# 17. A double click on a layer row opens a rename rather than selecting.
run_mutation "double-clicking a layer row does not open a rename" \
  src/layers.rs \
  'old = """    if click_count >= 2 {"""
new = """    if false && click_count >= 2 {"""
assert old in source, "double click branch not found"
source = source.replace(old, new, 1)' \
  "a_row_click_replaces_the_selection_shifted_click_toggles_and_a_double_click_renames"

# 18. Snap targets exclude the moving object and its descendants.
run_mutation "an object snaps to its own child" \
  src/canvas.rs \
  'old = """                !moving_nodes
                    .iter()
                    .any(|candidate| self.is_within(node, candidate))"""
new = """                true"""
assert old in source, "snap target filter not found"
source = source.replace(old, new, 1)' \
  "an_object_does_not_snap_to_its_own_child"

if [ "$failures" -eq 0 ]; then
  echo
  echo "every mutation was caught"
else
  echo
  echo "$failures mutation(s) were not caught"
  exit 1
fi