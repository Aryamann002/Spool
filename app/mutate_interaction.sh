#!/usr/bin/env bash
# Mutation harness for the click-versus-drag creation boundary.
#
# The bug this covers was silent twice over. A click with a shape tool did
# nothing at all — no object, no error — and a selection could not be deleted
# from the canvas at all, because the canvas never took keyboard focus. Neither
# looks like a crash; they look like a program quietly doing less than the user
# asked. A mutation harness is the only thing that makes that class of silence
# loud, so every rule below that turns a pointer into an object is mutated in
# turn and the run passes only if a test fails.
#
# `mutate_created_objects.sh` covers whether a created object reaches authored
# source. This harness covers the pointer rules in front of that: what counts as
# a click, what a click is worth, where it lands, what it leaves selected, and
# what it hands the tool back.
#
# Usage: ./mutate_interaction.sh
set -uo pipefail

cd "$(dirname "$0")" || exit 2

CANVAS="src/canvas.rs"
SHELL="src/shell.rs"
# Empty on purpose. The three behaviours this harness exists for — keyboard
# focus, the Text handover, and the toolbar highlight — can only be seen
# through a live window, so they live in `interaction_window_tests`. A filter
# would quietly exclude exactly the tests that make this harness meaningful.
FILTER=""

# No marker check here: unlike `operations.rs` and `project_save.rs`, the files
# this harness mutates predate the marker convention, and adding one to the
# canvas would be an unrelated edit. The guard exists to catch a file that was
# meant to carry it and has lost it, and neither of these ever did.
if grep -q "MUTATION HARNESS" "$CANVAS" || grep -q "MUTATION HARNESS" "$SHELL"; then
  echo "note: a marker appeared in a file this harness assumed had none;"
  echo "      re-check whether the guard below should now apply" >&2
fi

BACKUP_CANVAS="$(mktemp -t canvas.rs.XXXXXX)"
BACKUP_SHELL="$(mktemp -t shell.rs.XXXXXX)"
cp "$CANVAS" "$BACKUP_CANVAS"; cp "$SHELL" "$BACKUP_SHELL"
restore() { cp "$BACKUP_CANVAS" "$CANVAS"; cp "$BACKUP_SHELL" "$SHELL"; }
trap restore EXIT

passed=0
survived=0
equivalent=0

# equiv <file> <filter> <name> <old> <new>
#
# A mutant that is *expected* to leave the suite green, reported separately from
# a survivor. The invariant holds, but for a reason the reader has to be told.
equiv() {
  local file="$1" filter="$2" name="$3" old="$4" new="$5"
  VERDICT=equivalent
  run "$file" "$filter" "$name" "$old" "$new"
  unset VERDICT
}

run() {
  local file="$1" filter="$2" name="$3" old="$4" new="$5" mode="${6:-one}"
  restore
  if ! python3 -c "
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
mode = sys.argv[4]
source = open(path).read()
count = source.count(old)
if mode == 'all':
    # The toolbar read appears once per toolbar, and both must be honest; the
    # value is identical so there is no way to mutate only one meaningfully.
    if count < 1:
        print('ANCHOR-MISSING:' + str(count), file=sys.stderr)
        sys.exit(3)
else:
    if count != 1:
        print('ANCHOR-AMBIGUOUS:' + str(count), file=sys.stderr)
        sys.exit(3)
open(path, 'w').write(source.replace(old, new))
" "$file" "$old" "$new" "$mode"; then
    echo "SKIP  $name (anchor not unique or malformed)"
    return
  fi

  # Cargo decides whether to rebuild from file mtimes. A mutation written in the
  # same instant as the previous build can leave the mtime looking unchanged, and
  # the run then tests unmutated code and reports a false survivor. That is not a
  # hypothetical: two runs of this harness a minute apart disagreed about which
  # mutants survived. So: confirm the mutation is on disk, then let the clock move
  # past the last build before invoking cargo.
  if ! grep -qF "$new" "$file"; then
    echo "SKIP  $name (mutation is not on disk)"
    return
  fi
  sleep 1

  local out
  if [ -n "$filter" ]; then
    out=$(cargo test --offline "$filter" 2>&1)
  else
    out=$(cargo test --offline 2>&1)
  fi

  # No `test result:` line means the mutation did not build, which says nothing
  # about whether the tests are any good.
  #
  # These greps read `$out` through a here-string rather than a pipe, and that is
  # not a style preference. The harness runs under `set -o pipefail`, and
  # `grep -q` exits the moment it matches: on a large enough `$out` that leaves
  # the writer killed by SIGPIPE, pipefail reports that as the pipeline's status,
  # and a run in which twelve tests failed is scored as SURVIVED. It happened
  # here: the whole-suite runs are ~60KB, which is exactly where it starts. A
  # mutation harness that cannot tell a kill from a pass is worse than none.
  if ! grep -q "^test result:" <<<"$out"; then
    echo "ERROR $name (mutation did not compile)"
    printf '%s\n' "$out" | grep -E "^error" | head -2
    return
  fi

  if grep -q "FAILED" <<<"$out"; then
    local failed
    failed=$(grep -cE "^test .*FAILED" <<<"$out")
    printf 'KILL  %-56s (%s failing)\n' "$name" "$failed"
    passed=$((passed + 1))
  else
    if [ "${VERDICT:-survived}" = "equivalent" ]; then
      printf 'EQUIV  %-56s (cannot change behaviour)\n' "$name"
      equivalent=$((equivalent + 1))
    else
      printf 'SURVIVED %-54s <-- untested invariant\n' "$name"
      survived=$((survived + 1))
    fi
  fi
}

echo "== what counts as a click =="

run "$CANVAS" "$FILTER" "a press that moved is a click too" \
  '} else if !gesture.moved {' \
  '} else if true {'

run "$CANVAS" "$FILTER" "sub-threshold movement is never recorded as movement" \
  'gesture.moved = gesture.moved || gesture.pointer_start_screen != screen;' \
  'gesture.moved = false;'

echo "== what a click is worth =="

run "$CANVAS" "$FILTER" "clicked frames are not 100x100" \
  'const DEFAULT_CREATION_SIZE: f32 = 100.0;' \
  'const DEFAULT_CREATION_SIZE: f32 = 120.0;'

run "$CANVAS" "$FILTER" "clicked text loses its existing box" \
  'const DEFAULT_TEXT_WIDTH: f32 = 180.0;' \
  'const DEFAULT_TEXT_WIDTH: f32 = 100.0;'

run "$CANVAS" "$FILTER" "text is clicked at the default square too" \
  'ObjectType::Text => size(DEFAULT_TEXT_WIDTH, DEFAULT_TEXT_HEIGHT),' \
  'ObjectType::Text => size(DEFAULT_CREATION_SIZE, DEFAULT_CREATION_SIZE),'

run "$CANVAS" "$FILTER" "a click creates nothing at all" \
  'default_creation_size(gesture.object_type),' \
  'size(0.0, 0.0),'

echo "== where a click lands =="

# `pointer_start_world` and `screen_to_world(press)` are the same number: both
# are the press point, converted once at pointer-down and once at pointer-up,
# and the camera does not move between them during an ordinary click. The
# anchor invariant itself is proved by the zoom test, which would catch an
# origin or viewport-centre anchor, so this rewrite cannot change behaviour.
equiv "$CANVAS" "$FILTER" "a click anchors somewhere other than the press" \
  '                } else if !gesture.moved {
                    // A click: pressed and released without moving. The object is
                    // placed at the same point a drag would start from, so the two
                    // share one anchor — its top-left, because
                    // `creation_geometry` puts a drag'"'"'s top-left at the press.
                    self.commit_creation(
                        gesture.object_type,
                        gesture.pointer_start_world,' \
  '                } else if !gesture.moved {
                    self.commit_creation(
                        gesture.object_type,
                        self.camera.screen_to_world(screen),'

echo "== what a click leaves behind =="

run "$CANVAS" "$FILTER" "a created object is not selected" \
  'self.selection
            .click(Some(object.id), false, &self.hierarchy());' \
  'let _ = object.id;'

run "$CANVAS" "$FILTER" "a created object joins the previous selection" \
  '.click(Some(object.id), false, &self.hierarchy());' \
  '.click(None, false, &self.hierarchy());'

echo "== the tool it hands back =="

run "$CANVAS" "$FILTER" "creation does not return to the Selection tool" \
  'fn finish_creation(&mut self) {
        self.abandon_interaction();
        self.marquee = None;
        self.clear_gesture_feedback();
        self.tool = Tool::Select;
    }' \
  'fn finish_creation(&mut self) {
        self.abandon_interaction();
        self.marquee = None;
        self.clear_gesture_feedback();
    }'


echo
echo
echo "== what a real window is for =="
# The rules below were, until recently, untestable: they are properties of a live
# GPUI `Window` rather than of any function, so a unit test holding the window that
# would do the work could not observe them. `interaction_window_tests` drives a
# real window with real events, so they are ordinary mutations now.

run "$CANVAS" "" "the canvas stops being a focus target" \
  'viewport = viewport.track_focus(focus_handle);' \
  'viewport = viewport;'

run "$CANVAS" "" "the Text handover commits the session it just opened" \
  '                                                this.tool = Tool::Select;' \
  '                                                this.set_tool(Tool::Select);'

run "$CANVAS" "" "a Text click never opens a session" \
  '                                    if this.tool == Tool::Text {' \
  '                                    if false {'

run "$SHELL" "" "the toolbar highlight ignores the tool in effect" \
  '        self.canvas.read(cx).tool() == tool' \
  '        matches!(tool, canvas::Tool::Select)'


echo
echo "== summary =="
printf 'killed:    %s\n' "$passed"
printf 'equivalent: %s (mutating these cannot change behaviour)\n' "$equivalent"
printf 'survived:  %s\n' "$survived"

restore
[ "$survived" -eq 0 ] || exit 1