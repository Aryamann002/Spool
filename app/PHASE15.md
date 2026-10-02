# Phase 15 — Retained Layers rows and selection presentation

## 1. Outcome, stated plainly

The goal was: a selection change should update the affected Layers rows instead of
reconstructing a 10,000-row tree. **That goal is not reachable with the GPUI
version this project pins**, and Phase 15 documents exactly why instead of
inventing a framework around it.

What Phase 15 delivers:

1. The exact GPUI retention model, read out of the vendored source (Section 2).
2. Selection state is stored **on the retained rows** as a presentation flag, so
   row construction performs no membership lookup and no allocation.
3. Selection changes are reduced to an **O(changed rows)** diff
   (`apply_selection`), counted separately from row construction
   (`row_presentation_updates` vs `layers_row_construction`).
4. Honest instrumentation that measures the new O(N) sweep as well
   (`layers_selection_sync`), because it is a cost this design introduces.
5. Structure invalidation is unchanged: create/duplicate/delete/undo/redo still
   invalidate the projection; selection never does (0 projection rebuilds in all
   24 benchmark runs and in every test).
6. Layers row construction at 10k objects: **26.0/24.8 ms → 22.7/22.6 ms**
   (−12.9% / −9.0%). The new shell-side sweep costs **1.56 ms** per selection
   change at 10k, so the *combined* selection-change construction path improves
   by **−7.0% / −2.7%**, not by an order of magnitude.
7. 117 tests pass (was 111), including 8 focused Layers tests and 2 new
   retained-row tests in the canvas module.

Phase 14's culling, hit testing, marquee, document model and history were not
touched.

## 2. What GPUI actually does (measured from the pinned source, not assumed)

`gpui = "0.2.2"` from `zed` rev `397cbc84de333eeb56849dc90782d365b37e60d7`,
vendored at `crates/gpui` in the cargo git checkout. Relevant facts:

| Question | Answer in this version | Source |
| --- | --- | --- |
| What does `Entity::cached(style)` cache? | The rendered **subtree of that one view entity**, laid out with a composer-supplied style. With a cached style, `render()` is *not* called during layout at all. | `crates/gpui/src/view.rs:281` (`ViewElement::cached` doc), `view.rs:423-445` (`request_layout_view`) |
| When is the cached subtree reused? | `prepaint_view` reuses the previously prepainted range only if `bounds`, `content_mask` and `text_style` match, **and** `!window.dirty_views.contains(&entity_id)`, **and** `!window.refreshing`. | `view.rs:466-530`, cache key at `view.rs:302-306`, reuse at `view.rs:486-498` |
| What invalidates it? | `Context::notify()` on the view entity (dirty set), a bounds/mask/style change, or caching disabled by the inspector picker. | `window.rs:170` (`invalidate_view`), `view.rs:431-433` |
| Does notifying a parent invalidate children? | No — dirtying propagates **upward**: `mark_view_dirty` inserts the view and every ancestor in the dispatch tree path, never descendants. | `window.rs:2148-2161` |
| Does a parent's re-render invalidate nested cached children? | **Yes.** While a cached view re-renders, it sets `window.refreshing = true` for the duration, and the reuse check requires `!window.refreshing`. Every nested cached view is therefore forced to re-render. | `view.rs:501-511` (set) vs `view.rs:489` (check); `window.rs:2272-2277` and `window.rs:3389` for the frame-level flag |
| Is there per-child memoization inside a view's `render()`? | No. `render()` is a plain function returning an element tree; it is rebuilt on every render of that view. Elements with stable ids reuse *layout/scene state*, not construction. | `view.rs:230-281`, `view.rs:505-509` |
| Can rows be made per-entity to get O(changed) construction? | Not effectively. Notifying a row entity dirties its **ancestors** (`mark_view_dirty`), which re-renders Layers, which sets `refreshing`, which forces every sibling row entity to re-render anyway. | `window.rs:2148-2161` + `view.rs:489` |

**Conclusion:** GPUI's retention granularity is the *entity*, and an invalidated
child always drags its ancestors (and therefore its siblings) through a
re-render. Rows in Spool are plain `div()` elements inside one `LayersView`
entity, so there is nothing below the entity boundary that can be retained.
This is not React: there is no virtual-DOM diff of children, no per-child
memoization, and no "notify only this child" path.

Therefore this phase stops at the smallest useful optimization: retain the
structure projection, store presentation on the rows, and derive the
presentation diff in O(changed). Reconstructing a small row element to restyle it
is unavoidable; reconstructing *all* of them is unavoidable *per rebuild*, so the
only lever that scales is rebuilding fewer rows — which is the next phase's
recommendation (Section 12).

## 3. Architecture, before and after

Before (Phase 14):

```
AppShell::render
  └─ LayersView::synchronize(canvas)     // projection cached by revision
       └─ rebuilds projection on structure change only
  └─ cx.notify() on the Layers entity when structure OR selection changed
       └─ LayersView::render()           // constructs all N rows
            └─ per row: format!("layer-{id}")  +  membership lookup in the
                         selected set  (2 allocations + 1 hash per row)
```

After (Phase 15):

```
Document (structure revision)          selection (transient ids)
        │                                      │
        ▼                                      ▼
LayersProjection {                        apply_selection(rows, ids)
    revision: Option<u64>,                ├─ one linear sweep of retained rows
    rows: Vec<LayerRow {                  ├─ flips `selected` only where it differs
        id, name, object_type,            └─ returns the flipped rows  (O(changed))
        selected: bool,                    │
        element_id: SharedString,          ▼
    }>,                                  row_presentation_updates
    selected: Vec<ObjectId>,              │
}                                         ▼
        │                          cx.notify() on Layers (structure or
        ▼                          presentation changed)
project() on structure change only  ──► LayersView::render()  constructs N rows
                                       but reads only `row.selected` and
                                       `row.element_id` — no hashing, no format!
```

Presentation updates are reported by `SyncOutcome.presentation` and counted as
`row_presentation_updates`; `structure_changed` drives the rebuild decision.
`presentation_diff` remains as a test-only pure reference for the same diff.

## 4. Row identity

`LayerRow.id` is an `ObjectId`, never an array position. Row identity is
therefore stable across structural insertion/removal, and
`element_id` is precomputed once per projection (`layer-{id:?}`) so row
construction clones a `SharedString` instead of formatting one. Covered by
`row_identity_is_keyed_by_object_id_not_position` and
`retained_layer_rows_follow_create_delete_duplicate_and_history`.

## 5. Selection invalidation

`apply_selection(&mut rows, ids)`:

* builds a `HashSet` of the **new selection** (selections are small: 1–2 ids in
  every benchmark workload), not of all rows;
* sweeps the retained rows once, flipping `row.selected` only where membership
  changed, and returns those `(id, now_selected)` pairs;
* measured cost: **1,563 µs at 10k rows, 166 µs at 1k, 23 µs at 100** — about
  154 ns per row, i.e. linear in row count. It is reported honestly as
  `layers_selection_sync`, and it is the one O(N) cost this design adds.

When the selection is unchanged (`self.selected != ids` is false), the sweep is
skipped entirely — pan/drag/text workloads never pay it
(`layers_selection_sync` is absent in 18 of 24 runs).

Selection never touches `Document`, `layer_structure_revision`, geometry, style,
text or history. `layer_structure_revision_ignores_geometry_style_text_selection_and_camera`
and `selection_and_camera_changes_do_not_enter_history` assert this.

## 6. Structure invalidation

Unchanged from Phase 13/14: the projection is rebuilt only when
`canvas.layer_structure_revision()` differs from the revision the projection was
built at. Create, duplicate, delete, structural undo and redo all bump that
revision (through the existing document mutation paths), so the ordered id/name/
type row list is rebuilt and the cached style height follows
`tree_height(rows.len())`. Rows then reconcile by `ObjectId`. No arbitrary
reorder/grouping support was added, because those operations do not exist.

## 7. Instrumentation: what each counter actually measures

| Counter | Meaning |
| --- | --- |
| `layers_projection_rebuild` / `layers_projection` | Document walk + row projection rebuild (structure change only). |
| `layers_selection_sync` (new) | Wall time of `apply_selection`: the O(rows) sweep that flips presentation flags. |
| `layers_selection_update` | Number of selection-change events that reached the projection. |
| `row_presentation_updates` (new) | Number of rows whose presentation flag flipped — the O(changed) quantity. |
| `layers_build` | `LayersView::render()` invocations (full-tree constructions). |
| `layers_row_construction` | Rows constructed inside `render()` — this is O(N) *per rebuild*, because of Section 2. It is **not** equal to `row_presentation_updates` and is not presented as such. |
| `layers_tree_build` | Wall time of `LayersView::render()`. A construction timer only; it excludes layout, prepaint, paint and GPU. |
| `shell_render_build` | Wall time of `AppShell::render()`, which now contains the selection sweep. |
| `canvas_elements` / `canvas_render_build` / `visibility_scan` | Phase 14 canvas construction timers (unchanged). |
| `inspector_projection` | Inspector projection timer (unchanged, untouched this phase). |

A parent render count is **not** claimed to be a child rebuild count: the two are
reported separately, and the report states which one GPUI forces.

## 8. Benchmark matrix

Production renderer, same workload definitions as Phase 13/14: pan, drag,
selection, text × 100 / 1,000 / 10,000 objects × 2 runs, 60 paced updates each.
**24/24 runs completed with 60/60 updates and 60 canvas renders**; raw logs and
`summary.json` are in `app/target/phase15/`.

### 8.1 Selection workloads — Layers (the phase target)

| fixture | run | tree builds | rows built | tree build avg µs (P14 → P15) | total ms (P14 → P15) | presentation updates | sync avg µs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 100 | 1 | 59 → 59 | 5,900 → 5,900 | 278.7 → 257.6 (−7.6%) | 16.44 → 15.20 | 119 | 22.8 |
| 100 | 2 | 59 → 59 | 5,900 → 5,900 | 279.7 → 245.5 (−12.2%) | 16.50 → 14.48 | 119 | 24.4 |
| 1,000 | 1 | 59 → 59 | 59,000 → 59,000 | 2,480.5 → 2,287.1 (−7.8%) | 146.35 → 134.94 | 119 | 165.8 |
| 1,000 | 2 | 59 → 59 | 59,000 → 59,000 | 2,475.6 → 2,296.7 (−7.2%) | 146.06 → 135.50 | 119 | 166.6 |
| 10,000 | 1 | 59 → 59 | 590,000 → 590,000 | 26,044.1 → 22,693.0 (−12.9%) | 1,536.60 → 1,338.89 | 119 | 1,563.1 |
| 10,000 | 2 | 59 → 59 | 590,000 → 590,000 | 24,831.0 → 22,595.7 (−9.0%) | 1,465.03 → 1,333.15 | 119 | 1,565.0 |

Projection rebuilds: **0** in all six runs (and in all 24 runs overall).
Presentation updates: **119** for 60 selection changes at every size — i.e. the
two rows whose membership actually changed per change (one leaves, one enters),
which is the O(changed) behaviour the phase asked for.

### 8.2 Selection workloads — whole selection-change construction path

`shell_render_build` includes the new sweep, so the honest end-to-end
construction number is shell + Layers:

| fixture | run | Phase 14 (shell + tree) | Phase 15 (shell + tree) | delta |
| --- | --- | --- | --- | --- |
| 100 | 1 | 258.0 + 278.7 = 536.7 µs | 261.6 + 257.6 = 519.2 µs | −3.3% |
| 100 | 2 | 260.2 + 279.7 = 539.9 µs | 279.7 + 245.5 = 525.2 µs | −2.7% |
| 1,000 | 1 | 262.1 + 2,480.5 = 2,742.6 µs | 419.7 + 2,287.1 = 2,706.8 µs | −1.3% |
| 1,000 | 2 | 259.6 + 2,475.6 = 2,735.2 µs | 420.4 + 2,296.7 = 2,717.1 µs | −0.7% |
| 10,000 | 1 | 348.1 + 26,044.1 = 26,392 µs | 1,849.2 + 22,693.0 = 24,542 µs | −7.0% |
| 10,000 | 2 | 288.8 + 24,831.0 = 25,120 µs | 1,848.0 + 22,595.7 = 24,444 µs | −2.7% |

### 8.3 Pan / drag / text — Layers stays retained

Layers was rebuilt **0 times** in all 18 pan/drag/text runs in Phase 15; Phase 14
rebuilt it 0 times in 17 of them, with one outlier (text:10000 run 2, 4 rebuilds /
40,000 rows). Canvas culling is
unchanged: `canvas_elements` at 10k is 927–943 µs (pan), 1,180–1,196 µs (drag),
957–962 µs (text) versus 946–1,000 / 1,205–1,215 / 965–997 µs in Phase 14. The
small differences in `shell_render_build` (e.g. 143–153 µs vs 171–173 µs on
pan:10000) are run-to-run variance: Layers never renders in those runs, so this
phase's code is not on that path.

No regression in the canvas after Phase 14: selection:10000 constructs 4,320 of
600,000 objects considered, culling 595,680.

## 9. Tests

117 passed, 0 failed, 1 ignored (the ignored one is the timing harness), up from
111. The 10 required scenarios map to:

| Required scenario | Test |
| --- | --- |
| Unchanged structure + selection change | `layers::tests::unchanged_selection_after_structure_change_is_silent`, `selection_diff_updates_only_changed_rows` (projection is not re-walked) |
| Old selection vs new selection | `selection_diff_updates_only_changed_rows` |
| One-row selection change | `selection_diff_updates_only_changed_rows` |
| Multi-selection change | `selection_diff_handles_single_multi_and_deselection` |
| Deselection | `selection_diff_handles_single_multi_and_deselection` |
| Structure change then selection | `structure_change_followed_by_selection_reports_the_new_row`, `rows_created_together_with_a_selection_are_reported_once` |
| Create/delete/duplicate preserve row identity | `canvas::tests::retained_layer_rows_follow_create_delete_duplicate_and_history`, `layers::tests::row_identity_is_keyed_by_object_id_not_position` |
| Structural undo/redo | `retained_layer_rows_follow_create_delete_duplicate_and_history`, `canvas::tests::layer_structure_revision_tracks_successful_mutations_and_history_order` |
| Selection does not change structure revision | `layer_structure_revision_ignores_geometry_style_text_selection_and_camera` |
| Selection does not create history entries | `selection_and_camera_changes_do_not_enter_history` |

Phase 13/14 assertions kept green: `projection_is_lazy_and_selection_is_separate`
(pan and selection must not project rows — enforced with `panic!` closures),
`retained_layer_rows_keep_names_and_skip_walks_for_selection_and_transient_changes`,
`cached_dimensions_preserve_intrinsic_tree_size_and_shrink`, and all seven
culling tests.

Validation (all clean):

```
cargo fmt --check
cargo check --locked
cargo test --locked      # 117 passed; 0 failed; 1 ignored
cargo clippy --locked -- -D warnings
cargo build --locked
```

The pre-existing `block v0.1.6` future-incompatibility note from a transitive
dependency is unchanged.

## 10. Runtime verification

Driver: `app/target/phase15/verify15.py`, artifacts in `app/target/phase15/`.
The app was launched **without** `SPOOL_DIAGNOSTICS` / `SPOOL_WORKLOAD`, window
bounds confirmed as `160 124 1480 992` (normal window), and interaction used
real OS-level synthetic events via `CGEventPost` (`clickmod.swift`, `ldrag.swift`)
— **not** the workload driver. Assertions are pixel-level: accent-wash band
detection in the Layers sidebar plus whole-region PNG diffs.

| # | Action | Assertion | Result |
| --- | --- | --- | --- |
| 1 | baseline | no Layers row has the selected wash | 0 bands, 0 accent pixels |
| 2 | click Landing on canvas | exactly one row highlighted **and** canvas pixels changed | 1 band (row 1), 18,655 accent px, canvas changed |
| 3 | Command-click Editor | selection replaced (Command is the zoom modifier, not additive) | 1 band (row 2) |
| 4 | click Landing, then Shift-click Editor | two rows highlighted, canvas changed | 2 bands (rows 1–2), 37,475 px |
| 5 | marquee drag over the canvas | all four rows highlighted | 4 bands, 74,786 px |
| 6 | click the first Layers row | one row highlighted and the canvas repaints | 1 band, canvas changed |

Nine of nine programmatic checks passed, twice, on the shipped binary. This also
confirms the identity mapping: selecting Editor highlights row 2, marquee
highlights rows 1–4 in document order, and a Layers row click narrows the
selection and updates the canvas.

Three screenshots were captured within budget — `shot1.png` (normal editor),
`shot2.png` (canvas selection with the matching Layers highlight), `shot3.png`
(marquee multi-selection) — and **all three were subsequently visually
reviewed** at 1:1 logical scale. What the images show, independently of the
pixel assertions above:

| Shot | Visually confirmed |
| --- | --- |
| 1 | Title bar, tool palette (Select active, 100% zoom, Share/Export), PAGES and LAYERS rail with `Landing / Editor / Features / Mobile`, four artboards on the canvas, and the Inspector reading "No selection — Select an object on the Layers." No LAYERS row is washed. |
| 2 | Exactly one LAYERS row — `Landing` — carries the accent wash, the Landing artboard has a blue outline with eight resize handles, and the Inspector switched to `Landing` with Position and size (X 0, W 430), Layout, Appearance, Effects, Typography and Export sections. |
| 3 | All four LAYERS rows carry the accent wash, four artboards are outlined, and the Inspector reads "4 objects selected — Style changes apply to selection." |

No layout shift, clipped row, missing glyph, stray debug overlay or wash
leaking onto an unselected row is visible in any shot, which is consistent with
appearance-relevant code (row height 27, gaps, padding, radii, typography, icon
glyphs, accent colors, hover and click behavior) being untouched this phase.
The visual pass is corroborating evidence for the programmatic checks, not a
substitute for them: it cannot show intermediate frames, hover states or
timing, and it is a single sample per interaction.

Also worth recording: this editor's additive modifier is **Shift**
(`canvas.rs:1815`); Command/Control is the scroll-wheel zoom modifier
(`canvas.rs:2181`). The phase brief asked for command multi-selection; the probe
shows command-click replaces the selection, which is pre-existing semantics and
was deliberately not changed.

## 11. Limitations

* The 10k selection rebuild still constructs 10,000 rows 59 times per run.
  Phase 15 reduced the *cost per row*, not the number of rows.
* The O(changed) presentation diff needs one linear sweep of the retained rows;
  that sweep is the new O(N) cost (1.56 ms at 10k) and is measured, not hidden.
* All numbers are **construction timers** from a debug build driving a real
  renderer. They are not end-to-end frame times; no FPS claim is made or
  implied. Paint and layout of 10k rows are outside every timer here (a sampled
  stack during a stalled run showed `TextLayout::paint` → `Scene::push_layer` →
  `BoundsTree::insert`, i.e. paint, not construction, is the next wall-clock
  cost in debug builds).
* Selection is a transient editor state; nothing in this phase is persisted or
  serialized.
* Layer reordering and grouping remain unimplemented, so row reconciliation only
  has to handle insertion/removal/duplicate today.

## 12. Remaining bottleneck and recommended next phase

**Bottleneck:** at 10k objects the dominant measured construction cost is still
the full row-tree rebuild — 22.6 ms per selection change for 10,000 rows,
≈226 ns per row — with paint and layout stacking on top of it. The Inspector's
broad invalidation remains a separate, untouched problem (`inspector_projection`
17–25 µs per shell render at 10k selection).

**Recommended Phase 16 — virtualize the Layers row list.** This is the same idea
that made Phase 14 work on the canvas: build only the rows inside the sidebar's
scroll viewport (≈30–60 rows) while keeping the intrinsic height from
`tree_height(rows.len())`, so scrolling, row identity, selection presentation and
the document all stay untouched. Expected effect: the per-selection-change Layers
construction drops from O(N) to O(visible) — roughly 22.6 ms → tens of µs at
10k — without any reactive framework. Phase 16 should also decide the small
open question this phase deliberately deferred: whether to replace the 1.56 ms
membership sweep with an `ObjectId → row` index or a 64-bit membership prefilter.
The Inspector invalidation boundary remains a candidate for Phase 17.

## 13. Files

Changed this phase: `app/src/layers.rs` (projection state, presentation flags,
`apply_selection`, sync outcome, diagnostics, tests), `app/src/shell.rs` (the
`AppShell::render` sync call), `app/src/canvas.rs` (tests only — production code
has no reference to `layers`), `app/measure_runtime.py` (console/JSON reporting
of the new counters), plus this report and `app/PHASE15_RESULTS.json`.
Benchmark and verification artifacts live under `app/target/` (gitignored).
