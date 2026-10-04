#!/usr/bin/env python3
"""Mutation-test the selection and hierarchy parity tests.

Every mutation here breaks a rule the tests claim to enforce. A mutation that
does not change behaviour, does not compile, or matches no test is reported as
SURVIVED so it cannot be mistaken for a pass -- those three are the ways a
harness like this reports success while testing nothing.
"""

import re
import subprocess
import sys
from pathlib import Path

APP = Path(__file__).resolve().parent
CANVAS = APP / "src" / "canvas.rs"
HIERARCHY = APP / "src" / "hierarchy.rs"

# (label, file, original, replacement, test filter)
MUTATIONS = [
    (
        "click no longer climbs to the parent",
        HIERARCHY,
        "        if deep {\n            hit\n        } else {\n            self.topmost(hit)\n        }",
        "        let _ = deep;\n        hit",
        "a_click_resolves_through_the_hierarchy",
    ),
    (
        "cmd-click also climbs (deep select disabled)",
        HIERARCHY,
        "        if deep {\n            hit\n        } else {\n            self.topmost(hit)\n        }",
        "        let _ = deep;\n        self.topmost(hit)",
        "command_click_reaches_past_the_container",
    ),
    (
        "topmost stops at one level instead of the root",
        HIERARCHY,
        "        let mut current = id;\n        for _ in 0..=self.parent.len() {\n            let Some(parent) = self.parent.get(&current).copied() else {\n                return current;",
        "        let mut current = id;\n        for _ in 0..=0 {\n            let Some(parent) = self.parent.get(&current).copied() else {\n                return current;",
        "topmost_climbs_all_the_way_past_a_single_intermediate_level",
    ),
    (
        "nested ids are no longer filtered out",
        HIERARCHY,
        "                !ids.iter()\n                    .any(|other| *other != *id && self.is_within(*id, *other))",
        "                true",
        "a_selection_never_holds_an_object_and_its_ancestor",
    ),
    (
        "is_within becomes non-strict (a node is inside itself)",
        HIERARCHY,
        "        let mut current = id;\n        for _ in 0..=self.parent.len() {\n            let Some(parent) = self.parent.get(&current).copied() else {\n                return false;\n            };\n            if parent == ancestor {\n                return true;\n            }",
        "        let mut current = id;\n        for _ in 0..=self.parent.len() {\n            if current == ancestor {\n                return true;\n            }\n            let Some(parent) = self.parent.get(&current).copied() else {\n                return false;\n            };\n            if parent == ancestor {\n                return true;\n            }",
        "is_within_is_true_for_any_depth",
    ),
    (
        "canonical stops projecting into document order",
        HIERARCHY,
        "        self.order\n            .iter()\n            .copied()\n            .filter(|id| chosen.contains(id))\n            .collect()",
        "        ids.iter().copied().filter(|id| chosen.contains(id)).collect()",
        "selection_order_is_document_order_whatever_route",
    ),
    (
        "sibling traversal walks document order instead of siblings",
        HIERARCHY,
        "            .filter(|candidate| self.parent_of(*candidate) == parent)",
        "            .filter(|_| true)",
        "tab_walks_siblings_rather_than_document_order",
    ),
    (
        "sibling traversal stops wrapping",
        HIERARCHY,
        "        let next = if forward {\n            (index + 1) % len\n        } else {\n            (index + len - 1) % len\n        };\n        Some(run[next])",
        "        let _ = forward;\n        run.get(index + 1).copied()",
        "tab_cycles_through_the_roots_and_shift_tab",
    ),
    (
        "first_child returns the object itself instead of a child",
        HIERARCHY,
        "            .find(|candidate| self.parent_of(*candidate) == Some(id))",
        "            .find(|candidate| *candidate == id)",
        "descend_and_ascend_answer_where_the_next_level_is",
    ),
    (
        "select_roots keeps every node that has children (inverted filter)",
        CANVAS,
        "            .filter(|object| hierarchy.parent_of(object.id).is_none())",
        "            .filter(|object| hierarchy.parent_of(object.id).is_some())",
        "select_all_still_reaches_every_root",
    ),
    (
        "marquee stops normalizing its result",
        CANVAS,
        "            self.selection.replace_normalized(contained, &hierarchy);",
        "            self.selection.replace(contained);",
        "a_marquee_that_catches_a_frame_and_its_contents",
    ),
    (
        "toggle-off normalizes before removing (so it cannot remove)",
        CANVAS,
        "        if let Some(index) = self.selected.iter().position(|selected| *selected == id) {\n            self.selected.remove(index);\n            return;\n        }",
        "        self.selected = hierarchy.normalize(&self.selected);\n        if let Some(index) = self.selected.iter().position(|selected| *selected == id) {\n            self.selected.remove(index);\n            return;\n        }",
        "toggling_an_ancestor_out_of_a_nested_pair",
    ),
    (
        "ascend returns the current object instead of its parent",
        CANVAS,
        "            Traversal::Ascend => hierarchy.parent(current),",
        "            Traversal::Ascend => Some(current),",
        "traversal_descends_and_ascends_and_reports",
    ),
    (
        "descend returns the current object instead of its first child",
        CANVAS,
        "            Traversal::Descend => hierarchy.first_child(current),",
        "            Traversal::Descend => Some(current),",
        "traversal_descends_and_ascends_and_reports",
    ),
    (
        "an empty selection no longer lands on the first root",
        CANVAS,
        "            None => {\n                let Some(first) = hierarchy.order().first().copied() else {\n                    return false;\n                };",
        "            None => {\n                let Some(first) = None else {\n                    return false;\n                };",
        "traversal_descends_and_ascends_and_reports",
    ),
    (
        "the deep-select modifier is dropped entirely",
        CANVAS,
        "fn deep_select(modifiers: gpui::Modifiers) -> bool {\n    modifiers.platform || modifiers.control\n}",
        "fn deep_select(_modifiers: gpui::Modifiers) -> bool {\n    false\n}",
        "the_deep_select_modifier_is_named_separately",
    ),
    (
        "shift-tab stops resolving to the previous sibling",
        APP / "src" / "commands.rs",
        '            "tab" => Some(Traversal::Sibling(false)),',
        '            "tab" => None,',
        "the_traversal_keys_are_reachable",
    ),
    (
        "cmd-up stops resolving to ascend",
        APP / "src" / "commands.rs",
        '            "up" => Some(Command::Traverse(Traversal::Ascend)),',
        '            "up" => None,',
        "the_command_modifier_disarms_the_arrow_keys",
    ),
]


def run_tests(filter_text):
    return subprocess.run(
        ["cargo", "test", "--offline", filter_text],
        cwd=APP,
        capture_output=True,
        text=True,
    )


def tests_matched(output):
    """How many tests the filter actually ran, excluding the zero-match case."""
    match = re.search(r"(\d+) passed; (\d+) failed", output)
    return (int(match.group(1)), int(match.group(2))) if match else (0, 0)


def main():
    survivors = []
    for label, path, original, replacement, filter_text in MUTATIONS:
        source = path.read_text()
        if source.count(original) != 1:
            print(f"NO-OP   {label}: pattern matched {source.count(original)} times")
            survivors.append(label)
            continue

        path.write_text(source.replace(original, replacement, 1))
        try:
            result = run_tests(filter_text)
        finally:
            path.write_text(source)

        output = result.stdout + result.stderr
        if "error[" in output or "error: could not compile" in output:
            print(f"NO-BUILD {label}: the mutation does not compile")
            survivors.append(label)
            continue

        passed, failed = tests_matched(output)
        if passed + failed == 0:
            print(f"NO-TEST {label}: filter {filter_text!r} matched nothing")
            survivors.append(label)
            continue
        if failed > 0:
            print(f"caught  {label} ({failed} test(s) failed)")
        else:
            print(f"SURVIVED {label}")
            survivors.append(label)

    print()
    if survivors:
        print(f"{len(survivors)} of {len(MUTATIONS)} mutations survived:")
        for survivor in survivors:
            print(f"  - {survivor}")
        return 1
    print(f"all {len(MUTATIONS)} mutations caught")
    return 0


if __name__ == "__main__":
    sys.exit(main())
