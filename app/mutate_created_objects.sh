#!/usr/bin/env bash
# Mutation harness for the created-object persistence boundary.
#
# Created objects used to reach the canvas and not the document, so every save
# reported them as unsupported and a reopen lost them. Nothing failed loudly; the
# object simply was not there any more. That is the class of bug this harness
# exists to make impossible to reintroduce silently: each mutation below breaks
# exactly one rule that turns a creation into authored source, and the run passes
# only if a test fails.
#
# `app/mutate_ops.sh` covers `operations.rs`. `app/mutate_spool_project.sh` covers
# the `.spool` boundary and the macOS lifecycle. `mutate_milestone7.sh` has stale
# anchors against the current `project_save.rs` and is left alone; that debt is
# tracked separately rather than folded in here.
#
# Usage: ./mutate_created_objects.sh
set -uo pipefail

cd "$(dirname "$0")" || exit 2

SAVE="src/project_save.rs"
OPS="src/operations.rs"
CANVAS="src/canvas.rs"
FILTER="canvas::tests"

for file in "$SAVE" "$OPS"; do
  if ! grep -q "MUTATION HARNESS" "$file"; then
    echo "error: $file is missing the mutation marker; refusing to run" >&2
    exit 2
  fi
done

BACKUP_SAVE="$(mktemp -t project_save.rs.XXXXXX)"
BACKUP_OPS="$(mktemp -t operations.rs.XXXXXX)"
BACKUP_CANVAS="$(mktemp -t canvas.rs.XXXXXX)"
cp "$SAVE" "$BACKUP_SAVE"; cp "$OPS" "$BACKUP_OPS"; cp "$CANVAS" "$BACKUP_CANVAS"
restore() { cp "$BACKUP_SAVE" "$SAVE"; cp "$BACKUP_OPS" "$OPS"; cp "$BACKUP_CANVAS" "$CANVAS"; }
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
  local file="$1" filter="$2" name="$3" old="$4" new="$5"
  restore
  if ! python3 -c "
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
source = open(path).read()
if source.count(old) != 1:
    print('ANCHOR-AMBIGUOUS:' + str(source.count(old)), file=sys.stderr)
    sys.exit(3)
open(path, 'w').write(source.replace(old, new, 1))
" "$file" "$old" "$new"; then
    echo "SKIP  $name (anchor not unique or malformed)"
    return
  fi

  local out
  out=$(cargo test --offline "$filter" 2>&1)

  # No `test result:` line means the mutation did not build, which says nothing
  # about whether the tests are any good.
  # Read through a here-string rather than a pipe. Under `set -o pipefail`, and
  # with `grep -q` exiting the instant it matches, a large enough `$out` leaves the
  # writer killed by SIGPIPE and the pipeline reports that instead of grep's
  # result — so a run where tests failed could be scored as a survivor. See
  # `mutate_interaction.sh` for the run where that actually happened.
  if ! grep -q "^test result:" <<<"$out"; then
    echo "ERROR $name (mutation did not compile)"
    grep -E "^error" <<<"$out" | head -2
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

echo "== the structural half: a node joins and leaves the document =="

run "$OPS" "operations::" "an inserted node is not added to the structure" \
  'Self::Insert { .. } => insert_node(document, node, node_index, child_index),' \
  'Self::Insert { .. } => remove_node(document, &node.id),'

run "$OPS" "operations::" "insert and remove are not inverses" \
  'Self::Remove { .. } => Self::Insert {' \
  'Self::Remove { .. } => Self::Remove {'

run "$OPS" "operations::" "a node does not join its parent's children" \
  'if !found.children.contains(&node.id) {
                found.children.insert(at, node.id.clone());
            }' \
  'let _ = (at, node);'

run "$OPS" "operations::" "a removed node stays in its parent's children" \
  'node.children.retain(|child| child != id);' \
  ''

run "$OPS" "operations::" "insertion ignores the recorded position" \
  'let index = node_index.min(document.structure.nodes.len());' \
  'let index = document.structure.nodes.len();'

echo "== the authored half: the element reaches the source =="

run "$SAVE" "$FILTER" "the created element carries no identity" \
  'markup.push_str(&format!(" style=\"{style}\""));' \
  ''

run "$SAVE" "$FILTER" "the identity attribute is not authored" \
  '    let mut markup = format!(
        "<{tag} data-spool-id=\"{id}\"",
        tag = element.tag,
        id = node.as_str()
    );' \
  '    let mut markup = format!("<{tag}", tag = element.tag);'

run "$SAVE" "$FILTER" "the element's declarations are dropped" \
  'if !style.is_empty() {' \
  'if false {'

run "$SAVE" "$FILTER" "the element's text is not authored" \
  '    if let Some(text) = &element.text {
        markup.push_str(&escape_text(text));
    }' \
  ''

run "$SAVE" "$FILTER" "creation lands after the parent's close tag" \
  'let at = parent.content_end;' \
  'let at = parent.element_range.end;'

run "$SAVE" "project_save::tests" "the new element ignores its siblings' indentation" \
  '            .find(|range| {
                range.start > parent.element_range.start && range.end <= parent.element_range.end
            })' \
  '            .find(|_range| false)'

run "$SAVE" "$FILTER" "the parent's close tag loses its own line" \
  'format!("{}\n{}", insertion.markup, tail),' \
  'insertion.markup.clone(),'

run "$SAVE" "$FILTER" "two children of one parent collapse into one" \
  'let insertion = group.entry(parent).or_insert_with(|| PlannedInsertion {' \
  'let insertion = group.entry(node_id.clone()).or_insert_with(|| PlannedInsertion {'

run "$SAVE" "$FILTER" "a removal takes the element with it" \
  'replacements.push((start..removal.range.end, String::new()));' \
  ''

run "$SAVE" "$FILTER" "a file with only a creation is not written at all" \
  'if !markup_files.contains(&file) {
            markup_files.push(file);
        }' \
  ''

echo "== the editor half: creation reaches both =="

run "$CANVAS" "$FILTER" "creation only touches the runtime document" \
  '        operations.push(SemanticOperation::Runtime(DocumentCommand::insert(
            placements,
        )));' \
  '        operations.truncate(0);'

# Expected-equivalent, and the reason matters. `next_node_id_after` is the second
# of two guards: `Document::allocate_node_id` already skips any identity a *runtime
# object* holds, and every node in a reopened project is a runtime object. So
# resetting this counter alone cannot collide for any node the canvas draws. It
# exists for the nodes the canvas does not draw — an `unrendered` node is in the
# document and absent from the runtime — and no test creates one, because the
# editor cannot. Kept as a test rather than prose: if a future change makes
# `allocate_node_id` stop checking the runtime, this mutant starts failing and
# says so.
equiv "$CANVAS" "$FILTER" "the identity allocator ignores the loaded project" \
  '.map_or(1, |highest| highest + 1)' \
  '.map_or(1, |_highest| 1)'

run "$CANVAS" "$FILTER" "creation ignores the selected container" \
  '.find(|node| node.parent.is_none())' \
  '.find(|node| node.id.as_str().len() > 400)'

run "$CANVAS" "$FILTER" "a created element is authored without its position" \
  '("position".to_owned(), "absolute".to_owned()),' \
  ''

echo
echo "== summary =="
printf 'killed:    %s\n' "$passed"
printf 'equivalent: %s (mutating these cannot change behaviour)\n' "$equivalent"
printf 'survived:  %s\n' "$survived"

restore
[ "$survived" -eq 0 ] || exit 1