#!/usr/bin/env bash
# Mutation check for the Agent 7 guarantees.
#
# Each mutation breaks one specific behaviour the milestone claims, runs the
# tests that are supposed to catch it, and requires the suite to FAIL. A
# mutation that survives means the corresponding test is not really testing the
# claim, which is worse than having no test at all.
set -uo pipefail

cd "$(dirname "$0")" || exit 1

# name | file | python replacement | test filter
# Every mutation is expected to be caught, so a surviving one is a failure.
# `set -e` is deliberately not used: a failing `cargo test` is the expected
# outcome here, not an error in the script.
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
    if cargo test --offline "$filter" >/tmp/spool-mutation.log 2>&1; then
      echo "SURVIVED  $name — the suite still passed with this broken"
      failures=$((failures + 1))
      result=1
    else
      echo "caught    $name"
      result=0
    fi
  else
    echo "NO-OP     $name — the mutation did not change the source"
    failures=$((failures + 1))
    result=1
  fi

  cp "$backup" "$file"
  rm -f "$backup"
  return $result
}

failures=0

# 1. The NodeId allocator no longer skips identities that are already live.
run_mutation "allocate_node_id stops checking live identities" \
  src/canvas.rs \
  'old = """            if !self
                .objects
                .iter()
                .any(|object| object.spool_id == candidate)
            {
                return candidate;
            }"""
new = """            return candidate;"""
assert old in source, "allocator pattern not found"
source = source.replace(old, new, 1)' \
  "a_new_identity_is_never_one_that_is_already_live"

# 2. Save ignores that an object's position changed.
run_mutation "save ignores a moved object" \
  src/canvas.rs \
  'old = """                let moved = before.position != geometry.position;"""
new = """                let moved = false;"""
assert old in source, "geometry diff pattern not found"
source = source.replace(old, new, 1)' \
  "a_moved_object_survives_undo_redo_and_then_the_save_loop"

# 3. Save stops reporting objects that have no authored element.
run_mutation "save silently drops session-created objects" \
  src/canvas.rs \
  'old = """                outcome
                    .unsupported
                    .push(crate::project_save::UnsupportedEdit {
                        node: object.spool_id.clone(),
                        kind: "object",
                        reason: "created in this session, so it has no authored element yet".into(),
                    });"""
new = """                continue;"""
assert old in source, "unsupported pattern not found"
source = source.replace(old, new, 1)' \
  "a_session_created_object_is_reported_rather_than_silently_dropped"

# 4. Text no longer carries the authored colour into the renderer.
run_mutation "authored text colour is dropped" \
  src/document_runtime_bridge.rs \
  'old = """                    text_color: node.text_color,"""
new = """                    text_color: None,"""
assert old in source, "text_color bridge pattern not found"
source = source.replace(old, new, 1)' \
  "a_loaded_project_renders_its_authored_text_style_and_geometry"

# 5. Authored text is not extracted at all.
run_mutation "authored text is not extracted" \
  src/document_runtime_bridge.rs \
  'old = """                    text_content: node.text.clone(),"""
new = """                    text_content: None,"""
assert old in source, "text_content bridge pattern not found"
source = source.replace(old, new, 1)' \
  "a_loaded_project_renders_its_authored_text_style_and_geometry"

# 6. Inheritance through an unmanaged wrapper is dropped.
run_mutation "wrapper text colour no longer inherits" \
  src/visual.rs \
  'old = """                .or_else(|| {
                    ancestry.get(&id).and_then(|tags| {"""
new = """                .or_else(|| {
                    None::<String>.map(|value: String| { ancestry.get(&id).and_then(|tags| {"""
assert old in source, "wrapper inheritance pattern not found"
source = source.replace(old, new, 1)
old2 = """                        })
                    })
                });"""
new2 = """                        })
                    }); value })
                });"""
assert old2 in source, "wrapper inheritance tail not found"
source = source.replace(old2, new2, 1)' \
  "a_wrapper_carries_its_text_colour_into_the_nodes_inside_it"

# 7. Style ownership is assumed instead of checked.
run_mutation "style ownership is not checked" \
  src/project_save.rs \
  'old = """    let html = document.sources.get(&node.source.file)?;"""
new = """    let _ = node;
    let html = document.sources.get(&node.source.file)?;"""
assert old in source, "ownership guard pattern not found"
source = source.replace(old, new, 1)
old2 = """    for name in sheets {
        let Some(contents) = document.sources.get(name) else {
            continue;
        };
        let sheet = Stylesheet::parse(contents);
        for spelling in ownership_spellings(property) {
            if let Some(range) = sheet.declaration_value_range(&tag, &classes, None, spelling) {
                return Some((name.clone(), range));
            }
        }
    }"""
new2 = """    let _ = (tag, classes);
    for name in sheets {
        let Some(contents) = document.sources.get(name) else {
            continue;
        };
        if contents.contains(&format!("{property}:")) {
            let at = contents.find(&format!("{property}:")).expect("present");
            let value_start = at + property.len() + 1;
            return Some((name.clone(), (value_start, contents[value_start..].find(|c: char| c == (chr(59))).map(|end| value_start + end).unwrap_or(contents.len()))));
        }
    }"""
assert old2 in source, "ownership loop pattern not found"
source = source.replace(old2, new2, 1)' \
  "a_style_edit_with_no_authored_owner_is_reported_not_invented"

# 8. An existing style attribute is duplicated instead of replaced.
run_mutation "existing style attribute is duplicated" \
  src/project_save.rs \
  'old = """                Some(range) => replacements.push((
                    absolute + range.start..absolute + range.end,
                    declarations.clone(),
                )),"""
new = """                Some(_range) => replacements.push((
                    absolute + open_end..absolute + open_end,
                    format!(" style=\\"{declarations}\\""),
                )),"""
assert old in source, "style attribute replace pattern not found"
source = source.replace(old, new, 1)' \
  "a_geometry_edit_survives_the_round_trip"

# 9. HTML edits are applied front to back, so later ranges land in the wrong place.
run_mutation "html edits are applied in the wrong order" \
  src/project_save.rs \
  'old = """    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));"""
new = """    replacements.sort_by_key(|(range, _)| range.start);"""
assert old in source, "html ordering pattern not found"
source = source.replace(old, new, 1)' \
  "two_edits_to_the_same_element_both_land"

# 10. An explicit width is read as a content box again, so geometry drifts on reopen.
run_mutation "explicit dimensions read as content boxes" \
  src/visual.rs \
  'old = """    let outer_width = match explicit_width {
        Some(width) if content_box => width + padding[1] + padding[3],
        Some(width) => width,
        None => available_width,
    };"""
new = """    let _ = content_box;
    let outer_width = match explicit_width {
        Some(width) => width + padding[1] + padding[3],
        None => available_width,
    };"""
assert old in source, "box-sizing width pattern not found"
source = source.replace(old, new, 1)' \
  "a_geometry_edit_survives_the_round_trip"

# 11. Attribute facts are read from the whole element, so a child styles its parent.
run_mutation "attributes are read from the whole element" \
  src/visual.rs \
  'old = """    let open_tag_end = fragment.find('"'"'>'"'"').unwrap_or(fragment.len());
    (tag, attribute_values(&fragment[..open_tag_end], "class"))"""
new = """    (tag, attribute_values(fragment, "class"))"""
assert old in source, "class scope pattern not found"
source = source.replace(old, new, 1)' \
  "a_parent_never_inherits_its_childs_class_or_text"

# 12. Metadata is never written, so a rename does not survive.
run_mutation "renames are not persisted" \
  src/project_save.rs \
  'old = """    if on_disk.document.structure != updated.structure {"""
new = """    if false {"""
assert old in source, "metadata write guard pattern not found"
source = source.replace(old, new, 1)' \
  "a_rename_made_in_the_editor_is_written_to_the_metadata"

if [ "$failures" -eq 0 ]; then
  echo
  echo "every mutation was caught"
else
  echo
  echo "$failures mutation(s) were not caught"
fi