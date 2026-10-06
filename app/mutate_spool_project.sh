#!/usr/bin/env bash
# Mutation harness for the `.spool` project contract and the macOS lifecycle.
#
# A test that cannot fail proves nothing. Each mutation below breaks exactly one
# load-bearing rule in `spool_project.rs` or `lifecycle.rs`; the run passes only
# if a test fails. A mutation that leaves the suite green is an untested
# invariant, and is the finding, not the mutation.
#
# These two modules are the ones this milestone added, and they are also the two
# whose failures are quietest: a `.spool` boundary that stops checking the name
# still opens every valid project, and a lifecycle that stops installing the
# application menu still quits on ⌘Q for as long as a window exists. Both only
# misbehave in a case a person hits occasionally, which is exactly the kind of
# rule that goes untested unless something breaks it on purpose.
#
# Usage: ./mutate_spool_project.sh [--keep]
set -uo pipefail

cd "$(dirname "$0")" || exit 2

BOUNDARY="src/spool_project.rs"
LIFECYCLE="src/lifecycle.rs"
SHELL="src/shell.rs"

for file in "$BOUNDARY" "$LIFECYCLE"; do
  if ! grep -q "MUTATION HARNESS" "$file"; then
    echo "error: $file is missing the mutation marker; refusing to run" >&2
    exit 2
  fi
done

BACKUP_BOUNDARY="$(mktemp -t spool_project.rs.XXXXXX)"
BACKUP_LIFECYCLE="$(mktemp -t lifecycle.rs.XXXXXX)"
BACKUP_SHELL="$(mktemp -t shell.rs.XXXXXX)"
cp "$BOUNDARY" "$BACKUP_BOUNDARY"
cp "$LIFECYCLE" "$BACKUP_LIFECYCLE"
cp "$SHELL" "$BACKUP_SHELL"
restore() {
  cp "$BACKUP_BOUNDARY" "$BOUNDARY"
  cp "$BACKUP_LIFECYCLE" "$LIFECYCLE"
  cp "$BACKUP_SHELL" "$SHELL"
}
trap restore EXIT

passed=0
survived=0
runtime=0

# runtime <name> <old-snippet> <new-snippet>
#
# A mutant that a unit test *cannot* kill, and why that is acceptable here.
#
# `lifecycle::install` is the one function in these two modules that no unit test
# can reach: it takes a `&mut App`, and GPUI only hands out an `App` from a live
# platform, or from `TestAppContext`, which lives behind the `test-support`
# feature. That feature drags in `proptest`, `backtrace` and `http_client`
# test support, and this is a binary crate whose release dependency graph is a
# deliberate, reviewed thing — so it is not enabled for two assertions.
#
# The rules in that function body are therefore covered by *running the packaged
# application* instead: `app/scripts/package-macos.sh`, then
# launch -> Cmd+W -> Cmd+Q, which fails outright if `set_menus` is dropped, and
# launch -> Cmd+W, which fails if the window is not removed. That is a stronger
# check than a headless one, because it exercises AppKit rather than a stand-in.
# These mutants are reported separately so the gap stays visible instead of being
# quietly folded into "covered".
runtime() {
  local name="$1"
  VERDICT=runtime
  run "$LIFECYCLE" "lifecycle::" "$name" "$2" "$3"
  unset VERDICT
}

# run <file> <filter> <name> <old-snippet> <new-snippet>
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

  # A compile error or a run with no results means the mutation did not build,
  # which is not evidence about the tests.
  # Read through a here-string rather than a pipe. Under `set -o pipefail`, and
  # with `grep -q` exiting the instant it matches, a large enough `$out` leaves the
  # writer killed by SIGPIPE and the pipeline reports that instead of grep's
  # result — so a run where tests failed could be scored as a survivor. See
  # `mutate_interaction.sh` for the run where that actually happened.
  if ! grep -q "^test result:" <<<"$out"; then
    echo "ERROR $name (mutation did not compile)"
    grep -E "^error" <<<"$out" | head -3
    return
  fi

  if grep -q "FAILED" <<<"$out"; then
    local failed
    failed=$(grep -cE "^test .*FAILED" <<<"$out")
    printf 'KILL  %-56s (%s failing)\n' "$name" "$failed"
    passed=$((passed + 1))
  else
    case "${VERDICT:-survived}" in
      runtime)
        printf 'RTIME %-56s (needs a live App; see packaged-app check)\n' "$name"
        runtime=$((runtime + 1))
        ;;
      *)
        printf 'SURVIVED %-54s <-- untested invariant\n' "$name"
        survived=$((survived + 1))
        ;;
    esac
  fi
}

echo "== the .spool boundary =="

run "$BOUNDARY" "spool_project::" "any directory is accepted as a project" \
  'if Path::new(&name).extension().and_then(|ext| ext.to_str()) != Some(PROJECT_EXTENSION) {
            return Err(ProjectBoundaryError::NotAProjectName { path: root, name });
        }' \
  'let _ = name;'

run "$BOUNDARY" "spool_project::" "a missing manifest is accepted" \
  'ProjectBundle::locate_metadata(&root).map_err(|error| match error {
            BundleError::MissingMetadata { .. } => {
                ProjectBoundaryError::MissingManifest { path: root.clone() }
            }
            other => unreachable!("locate_metadata only reports MissingMetadata: {other}"),
        })?;' \
  'let _ = &root;'

run "$BOUNDARY" "spool_project::" "a file is accepted as a project directory" \
  'if !root.is_dir() {
            return Err(ProjectBoundaryError::NotADirectory { path: root });
        }' \
  'let _ = &root;'

echo "== creating a project =="

run "$BOUNDARY" "spool_project::" "a new project's name need not end in .spool" \
  'if Path::new(&name).extension().and_then(|ext| ext.to_str()) != Some(PROJECT_EXTENSION) {
        return Err(ProjectCreateError::NotAProjectName { path: root, name });
    }' \
  'let _ = &name;'

run "$BOUNDARY" "spool_project::" "creating overwrites whatever was there" \
  'if !empty {
            return Err(ProjectCreateError::AlreadyExists { path: root });
        }' \
  'let _ = empty;'

run "$BOUNDARY" "spool_project::" "a new project directory is never created" \
  'std::fs::create_dir_all(&root).map_err(|source| ProjectCreateError::Io {
        path: root.clone(),
        source,
    })?;' \
  ''

run "$BOUNDARY" "spool_project::" "the new object gets no identity attribute" \
  'selector: format!("[data-spool-id=\"{NEW_ROOT_ID}\"]"),' \
  'selector: String::new(),'

run "$BOUNDARY" "spool_project::" "the new object is an undrawable kind" \
  'kind: "frame".to_owned(),' \
  'kind: "shape".to_owned(),'

run "$BOUNDARY" "spool_project::" "the manifest is not written at all" \
  'ProjectBundle::from_document(&root, document)
        .and_then(|bundle| bundle.save())
        .map_err(ProjectCreateError::Bundle)?;' \
  'let _ = (ProjectBundle::from_document(&root, document), ProjectCreateError::NoName);'

echo "== the command line =="

run "$BOUNDARY" "spool_project::" "the first of two project paths is chosen" \
  'Some(_) => {
            let count = 2 + named.count();
            Err(ProjectBoundaryError::TooManyProjects { count })
        }' \
  'Some(_) => Ok(first.map(PathBuf::from)),'

run "$SHELL" "shell::tests" "the development override wins over the command line" \
  'requested
        .map(ProjectRequest::Project)
        .or_else(|| development_override.map(ProjectRequest::DevelopmentOverride))' \
  'development_override
        .map(ProjectRequest::DevelopmentOverride)
        .or_else(|| requested.map(ProjectRequest::Project))'

echo "== lifecycle =="

run "$LIFECYCLE" "lifecycle::" "cmd-q is bound to the wrong key" \
  'KeyBinding::new("cmd-q", Quit, None),' \
  'KeyBinding::new("ctrl-q", Quit, None),'

run "$LIFECYCLE" "lifecycle::" "cmd-w is bound to the wrong key" \
  'KeyBinding::new("cmd-w", CloseWindow, None),' \
  'KeyBinding::new("ctrl-w", CloseWindow, None),'

run "$LIFECYCLE" "lifecycle::" "the Quit menu item is replaced by a separator" \
  'vec![Menu::new("Spool").items([MenuItem::action("Quit Spool", Quit)])]' \
  'vec![Menu::new("Spool").items([MenuItem::separator()])]'

run "$LIFECYCLE" "lifecycle::" "the menu's Quit item dispatches another action" \
  'MenuItem::action("Quit Spool", Quit)' \
  'MenuItem::action("Quit Spool", CloseWindow)'

runtime "no application menu is installed" \
  'cx.set_menus(menus());' \
  ''

runtime "closing a window no longer removes it" \
  'let _ = window.update(cx, |_, window, _| window.remove_window());' \
  'let _ = window;'

runtime "the window removal is not deferred" \
  'cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        });' \
  'let _ = window.update(cx, |_, window, _| window.remove_window());'

echo
echo "== summary =="
printf 'killed:    %s\n' "$passed"
printf 'rtime:     %s (covered by the packaged-app run, not by a unit test)\n' "$runtime"
printf 'survived:  %s\n' "$survived"

restore
[ "$survived" -eq 0 ] || exit 1
