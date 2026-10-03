#!/usr/bin/env python3
"""Mutation check for the visual/layout model.

Same contract as `mutate_agent9.sh`: each mutation breaks exactly one behaviour
the box model claims, and the tests that are supposed to catch it must FAIL. A
surviving mutation means the corresponding test is not really testing the claim,
which is worse than having no test at all.

The selection is about the rules that are easy to state and easy to get subtly
wrong: where a box starts, what the flow does with margins, and which half of a
cascade decides the winner.

Mutations are applied to a clean worktree of HEAD at `../.verify`, never to the
main tree, so a mutation cannot collide with anyone else working in the checkout
and every other crate is in its committed state. The harness refuses to run if
that worktree has not been created:

    git worktree add .verify HEAD
    echo '.verify/' >> .git/info/exclude
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# This file lives in `app/`, so the repository root is two levels up. Derived
# rather than hard-coded so the harness runs wherever the checkout lives.
APP = Path(__file__).resolve().parent
ROOT = APP.parent
SRC = APP / "src"
VERIFY_ROOT = ROOT / ".verify" / "app"
TARGET_DIR = APP / "target"

# (name, source file, pattern, replacement, test filter)
MUTATIONS: list[tuple[str, str, str, str, str]] = [
    (
        "padding counted twice, as the old height formula did",
        "visual.rs",
        "content_top + content_extent + metrics.padding[2] + metrics.border,",
        "content_top\n                + content_extent\n                + metrics.padding[0]\n                + metrics.padding[2]\n                + metrics.border,",
        "visual::tests::padding_is_counted_once_not_twice",
    ),
    (
        "left/right margins ignored, as the old layout did",
        "visual.rs",
        "x: flow.x + metrics.margin[3],",
        "x: flow.x,",
        "visual::tests::horizontal_margins_move_a_child_and_shrink_it",
    ),
    (
        "a child's margins do not shrink the width it fills",
        "visual.rs",
        "let offered = (available_width - metrics.margin[1] - metrics.margin[3]).max(0.0);",
        "let offered = available_width;",
        "visual::tests::horizontal_margins_move_a_child_and_shrink_it",
    ),
    (
        "sibling margins added instead of collapsed",
        "visual.rs",
        "Some(bottom) => bottom + margin_bottom.max(child_margin_top),",
        "Some(bottom) => bottom + margin_bottom + child_margin_top,",
        "visual::tests::adjoining_sibling_margins_collapse_to_the_larger",
    ),
    (
        "a parent's height grows to contain its last child's bottom margin",
        "visual.rs",
        "flow_bottom: origin_y + height,",
        "flow_bottom: origin_y + height + metrics.margin[2],",
        "visual::tests::a_last_childs_bottom_margin_collapses_out_of_its_parent",
    ),
    (
        "a per-side longhand is ignored",
        "visual.rs",
        'if let Some(value) = entry.length(&format!("{property}-{edge}")) {',
        'if let Some(value) = None::<f32> {',
        "visual::tests::a_last_childs_bottom_margin_collapses_out_of_its_parent",
    ),
    (
        "the containing block is the border box, as the old layout used",
        "visual.rs",
        "let containing_box = metrics.containing_block((0.0, 0.0), width, height);",
        "let containing_box = ContainingBlock { x: 0.0, y: 0.0, width, height };",
        "visual::tests::an_absolute_child_is_placed_against_the_padding_edge",
    ),
    (
        "right/bottom resolve from the near edge",
        "visual.rs",
        "(None, Some(right)) => containing_block.x + containing_block.width - right - width,",
        "(None, Some(right)) => containing_block.x + right,",
        "visual::tests::right_and_bottom_place_a_box_against_the_far_edges",
    ),
    (
        "min/max never clamp",
        "visual.rs",
        "    value\n        .max(min.unwrap_or(0.0))\n        .min(max.unwrap_or(f32::INFINITY))",
        "    let _ = (min, max);\n    value",
        "visual::tests::min_and_max_width_and_height_clamp_the_used_box",
    ),
    (
        "box-sizing: content-box ignored",
        "visual.rs",
        'if entry.is("box-sizing", "content-box") {\n                authored + metrics.horizontal_inset()\n            } else {\n                authored\n            },',
        "authored,",
        "visual::tests::box_sizing_selects_which_frame_an_authored_width_is_measured_in",
    ),
    (
        "display: none ignored, as the old layout did",
        "visual.rs",
        'if entry.is("display", "none") {\n        mark_subtree_not_rendered(id, all_nodes, not_rendered);\n        return None;\n    }',
        "",
        "visual::tests::display_none_takes_a_node_and_its_subtree_out_of_the_model",
    ),
    (
        "a transform is dropped on an absolutely positioned box",
        "visual.rs",
        "let offset = translate_of(entry);",
        'let offset = if entry.is("position", "absolute") {\n            (0.0, 0.0)\n        } else {\n            translate_of(entry)\n        };',
        "visual::tests::an_absolute_box_still_honours_its_own_transform",
    ),
    (
        "a transform does not carry the subtree",
        "visual.rs",
        "    for (_, geometry) in subtree.iter_mut() {\n        geometry.x += shift.0;\n        geometry.y += shift.1;\n    }",
        "    for (_, geometry) in subtree.iter_mut() {\n        geometry.x += origin_x;\n        geometry.y += origin_y;\n    }",
        "visual::tests::a_transform_carries_the_whole_subtree_but_not_the_flow",
    ),
    (
        "the id never reaches the cascade, as the old caller hardcoded it",
        "visual.rs",
        "sheet.resolve(&facts.tag, &facts.classes, facts.id.as_deref())",
        "sheet.resolve(&facts.tag, &facts.classes, None)",
        "visual::tests::an_id_selector_reaches_the_element_that_carries_the_id",
    ),
    (
        "a wrapper contributes nothing",
        "visual.rs",
        "sheets.iter().find_map(|(_, sheet)| {",
        "sheets.iter().take(0).find_map(|(_, sheet)| {",
        "visual::tests::a_wrapper_is_styled_by_its_classes_and_id_too",
    ),
    (
        "attribute lookup matches a substring again",
        "visual.rs",
        "let starts_an_attribute = at == 0\n            || rest[..at]\n                .chars()\n                .next_back()\n                .is_some_and(|before| before.is_whitespace() || before == '/');",
        "let starts_an_attribute = true;",
        "visual::tests::a_data_attribute_is_not_read_as_a_style_hook",
    ),
    (
        "a void element swallows the rest of the text",
        "visual.rs",
        "} else if self_closing || VOID_ELEMENTS.contains(&name.as_str()) {",
        "} else if self_closing {",
        "visual::tests::a_void_element_does_not_swallow_the_text_after_it",
    ),
    (
        "stylesheets cascade in filename order, as the old collector did",
        "visual.rs",
        "    order\n        .into_iter()\n        .map(|name| {",
        "    order.sort();\n    order\n        .into_iter()\n        .map(|name| {",
        "visual::tests::later_stylesheets_win_across_files_the_way_the_document_links_them",
    ),
    (
        "an href is not resolved against the page that links it",
        "visual.rs",
        "                segments.pop()?;",
        "                segments.last();",
        "visual::tests::a_href_resolves_against_the_page_that_links_it",
    ),
    (
        "the cascade prefers a longhand over a later shorthand",
        "style.rs",
        'fn property_group(property: &str) -> String {\n    match property {\n        "background" | "background-color" => "background-color".to_owned(),\n        other => other.to_owned(),\n    }\n}',
        "fn property_group(property: &str) -> String {\n    property.to_owned()\n}",
        "style::tests::the_background_shorthand_and_its_longhand_are_one_property",
    ),
    (
        "the winning span is reported for a fixed spelling, not the winner",
        "style.rs",
        "self.cascaded(tag, classes, id)\n            .get(&property_group(property))\n            .copied()",
        'if property == "background" {\n            return None;\n        }\n        self.cascaded(tag, classes, id)\n            .get(&property_group(property))\n            .copied()',
        "style::tests::the_winning_spans_belong_to_the_declaration_that_actually_won",
    ),
    (
        "specificity is not consulted: author order alone decides",
        "style.rs",
        "let ids = u32::from(self.id.is_some()) * 10_000;",
        "let ids = 0;",
        "style::tests::an_id_rule_outranks_a_class_rule_whatever_the_order",
    ),
    (
        "a compound selector refuses a tag after a class, as the old parser did",
        "style.rs",
        "            if is_class {\n                selector.classes.push(name);",
        "            if is_class {\n                if selector.tag.is_some() {\n                    return None;\n                }\n                selector.classes.push(name);",
        "style::tests::a_compound_selector_matches_an_element_that_carries_every_part",
    ),
    (
        "a multi-class selector needs only one of its classes",
        "style.rs",
        "            .all(|expected| classes.iter().any(|class| class == expected))",
        "            .any(|expected| classes.iter().any(|class| class == expected))",
        "style::tests::a_multi_class_selector_needs_all_of_its_classes",
    ),
    (
        "a colourless border is dropped instead of taking up space",
        "style.rs",
        "    (components > 0).then_some(BorderStyle { width, color })",
        "    color\n        .is_some()\n        .then_some(BorderStyle { width, color })",
        "style::tests::a_border_shorthand_reports_geometry_and_paint_separately",
    ),
    (
        "`border: none` is a parse failure rather than an explicit zero",
        "style.rs",
        "return (components == 0).then_some(BorderStyle {\n                width: 0.0,\n                color: None,\n            });",
        "return None;",
        "style::tests::a_border_shorthand_reports_geometry_and_paint_separately",
    ),
]


def main() -> int:
    if not (VERIFY_ROOT / "Cargo.toml").is_file():
        print(
            f"no verification worktree at {VERIFY_ROOT}. Create one first:\n"
            "    git worktree add .verify HEAD\n"
            "    echo '.verify/' >> .git/info/exclude",
            file=sys.stderr,
        )
        return 1

    for name in ("visual.rs", "style.rs"):
        shutil.copy(SRC / name, VERIFY_ROOT / "src" / name)

    failures = 0
    for name, filename, pattern, replacement, test_filter in MUTATIONS:
        target = VERIFY_ROOT / "src" / filename
        backup = Path(tempfile.mkdtemp()) / filename
        shutil.copy(target, backup)

        source = target.read_text()
        mutated = source.replace(pattern, replacement, 1)
        target.write_text(mutated)

        if mutated == source:
            print(f"NO-OP     {name} - the pattern did not match; the mutation never applied")
            failures += 1
        else:
            completed = subprocess.run(
                ["cargo", "test", "--offline", test_filter],
                cwd=VERIFY_ROOT,
                env={**os.environ, "CARGO_TARGET_DIR": str(TARGET_DIR)},
                capture_output=True,
            )
            output = completed.stdout.decode() + completed.stderr.decode()
            ran = [line for line in output.splitlines() if line.startswith("test ")]
            if "error[" in output or "error: could not compile" in output:
                print(f"NO-BUILD  {name} - the mutation does not compile, so it proved nothing")
                failures += 1
            elif not ran:
                print(f"NO-TEST   {name} - the filter matched nothing, so it proved nothing")
                failures += 1
            elif completed.returncode == 0:
                print(f"SURVIVED  {name} - the suite still passed with this broken")
                failures += 1
            else:
                print(f"caught    {name}")

        shutil.copy(backup, target)
        shutil.rmtree(backup.parent, ignore_errors=True)

    for name in ("visual.rs", "style.rs"):
        shutil.copy(SRC / name, VERIFY_ROOT / "src" / name)

    print()
    if failures == 0:
        print("all mutations caught")
    else:
        print(f"{failures} mutation(s) survived or did not apply")
    return failures


if __name__ == "__main__":
    sys.exit(main())
