# Phase 14 — conservative viewport-aware canvas culling

## Boundary

Phase 13 ended with the canvas constructing every document object's GPUI element tree on every render: 600,000 constructions per 10,000-object run, of which approximately 596,000 were offscreen while only 64–66 objects intersected the viewport. This phase gates construction of the base object element with one O(N) predicate. No spatial index, no dependency, no document-model, interaction, styling, selection or history change.

`Camera::affects_viewport(position, size)` expands the object's geometry box by `CULL_PADDING = 64.0` world units on every side and tests it against the viewport with `diagnostics::intersects` — the same inclusive screen-space AABB model the diagnostic visibility counters already used, so the constructed set is a superset of the diagnostic visibility set by construction (measured: 3,984 diagnostic intersections vs 4,969 constructions in a 10k pan run; 3,840 vs 4,320 in the other 10k workloads). Padding is world space, so it remains conservative across the 0.1–4.0 zoom range; 64 world units covers the 24-unit `LABEL_HEIGHT` label painted above frame and starter-artboard boxes, stroke widths, and roughly two to three lines of text overflow at zoom 1.

`should_construct(camera, object, editing_object)` exempts exactly one object: the actively edited text object, whose editing overlay, focus handle and caret must stay coherent regardless of camera position. Selection is deliberately not a parameter: selection outlines and resize handles are separate loops in `artboards` that run after the object loop, so culling a base element never removes interaction chrome, and an offscreen selected object correctly receives no base element.

The object loop is `document.objects().iter().filter(|object| should_construct(...))` with a `constructed` counter: no allocation, no cloned vector, no sorting — filtering preserves document order, so z-order is unchanged. `hit_test`, marquee `objects_in`, the document model, styles, selection and history semantics are untouched.

The predicate is conservative on uncertainty: any input the intersection model cannot evaluate (non-finite geometry, camera offset, zoom or viewport; non-positive zoom or viewport; negative size) returns `true`, so unevaluable state constructs instead of culls. This includes the pre-prepaint render where the viewport is still empty.

Counters: `objects_constructed` previously always equaled N; it now reports the actual construction count, and `objects_considered` / `objects_culled` are new. `geometry_intersecting` / `geometry_offscreen` still come from the unchanged diagnostic-only visibility scan, which runs outside the construction timer only when diagnostics are enabled; its timing is flat versus Phase 13 (10k: 0.46–0.50 ms/render in both phases), confirming it is measurement overhead, not production cost. The in-loop culling filter itself runs inside `canvas_elements`, so the After timings below already include the O(N) scan.

## Measurements

Production debug renderer, unchanged Phase 12/13 fixtures and frame-paced 60-update workloads. Two repetitions of all 12 cases completed (24 runs); every run recorded 60 updates and 60 canvas renders. Before values are from `PHASE13_RESULTS.json`, not a rebuilt old binary. Raw percentiles and all counters are preserved in `PHASE14_RESULTS.json`; raw logs are under `target/phase14`. One `text:10000` run 2 initially hit the case time bound and was rerun alone with `--run-start 2 --cases text:10000`; it completed normally.

### Element constructions per run (identical in run 1 and run 2)

| Objects | Workload | Considered | Constructed | Culled | Constructed share |
|---:|---|---:|---:|---:|---:|
|100|Pan|6,000|446|5,554|7.4%|
|1,000|Pan|60,000|4,969|55,031|8.3%|
|10,000|Pan|600,000|4,969|595,031|0.83%|
|100|Drag 10|6,000|540|5,460|9.0%|
|1,000|Drag 10|60,000|4,320|55,680|7.2%|
|10,000|Drag 10|600,000|4,320|595,680|0.72%|
|100|Selection|6,000|540|5,460|9.0%|
|1,000|Selection|60,000|4,320|55,680|7.2%|
|10,000|Selection|600,000|4,320|595,680|0.72%|
|100|Text insertion|6,000|540|5,460|9.0%|
|1,000|Text insertion|60,000|4,320|55,680|7.2%|
|10,000|Text insertion|600,000|4,320|595,680|0.72%|

At 10,000 objects the canvas now constructs 72–83 elements per render instead of 10,000 — a 99.17% (pan) / 99.28% (other workloads) reduction in constructions. The 1,000- and 10,000-object cases construct identical counts per run because the fixtures extend the world rather than the visible region: per-frame visible set size is a property of the viewport, not of N. This is the value proposition of culling, established without an index.

### Canvas construction timing (milliseconds, per-render averages, run 1 / run 2)

| Objects | Workload | Before (Phase 13) | After (Phase 14) | After/Before |
|---:|---|---:|---:|---:|
|100|Pan|0.101 / 0.101|0.020 / 0.020|0.20×|
|100|Drag 10|0.113 / 0.114|0.035 / 0.034|0.31×|
|100|Selection|0.109 / 0.110|0.033 / 0.033|0.30×|
|100|Text insertion|0.114 / 0.112|0.034 / 0.034|0.30×|
|1,000|Pan|0.965 / 0.965|0.176 / 0.177|0.18×|
|1,000|Drag 10|0.993 / 0.995|0.200 / 0.199|0.20×|
|1,000|Selection|0.984 / 0.981|0.178 / 0.176|0.18×|
|1,000|Text insertion|0.981 / 0.972|0.178 / 0.181|0.18×|
|10,000|Pan|9.692 / 9.669|0.946 / 1.000|0.10×|
|10,000|Drag 10|9.847 / 9.891|1.205 / 1.215|0.12×|
|10,000|Selection|9.711 / 9.683|1.024 / 0.979|0.10×|
|10,000|Text insertion|9.667 / 9.649|0.965 / 0.997|0.10×|

At 10,000 objects, canvas construction is 8.1×–9.9× faster (approximately 8.5–8.7 ms saved per render); at 1,000 objects, approximately 5×; at 100 objects, approximately 3.3–5× but only 0.07–0.08 ms absolute. These are construction timers, **not layout/prepaint/paint/GPU time**, which remains unmeasured. The remaining ~0.95–1.21 ms at 10k includes the O(N) filter itself plus construction of the visible set plus frame/content chrome loops.

Costs added or unchanged, reported as measured:

- `workload_update` rose by roughly 4–8 µs per update across all sizes (10k pan 8.4 → 12.7 µs, 10k text 11.6 → 17.9 µs), an increase that is approximately constant in N and therefore not the O(N) scan; drag at 10k (history/snapshot dominated, ~280–292 µs) was unchanged to slightly lower. These timers are small and noisy; the increase is real but an order of magnitude below the canvas savings.
- The diagnostic `visibility_scan` timer is unchanged from Phase 13 within noise (10k: 0.465–0.504 ms after vs 0.462–0.473 ms before) and does not run when diagnostics are disabled.
- Layers behavior is unchanged from Phase 13: zero Layers builds for pan/drag; selection still rebuilds the complete row tree (59 builds; 5,900 / 59,000 / 590,000 rows at 100 / 1,000 / 10,000). One anomaly, reported as observed: `text:10000` run 2 recorded 4 Layers builds averaging 25.70 ms (40,000 rows), consistent with text-tool transition frames; no projection or selection change explained them and they were not removed as outliers. Phase 13's two unexplained 10k pan cache builds did not recur (0 in both Phase 14 pan runs).

## Tests and validation

111 tests passed, zero failed, one existing timing harness ignored (Phase 13: 104). Seven new tests in `canvas.rs`: predicate coverage of inside/partial-edge/outside cases, negative world coordinates, large spanning objects, zoom and pan tracking, conservative construction when inputs cannot be evaluated, the superset property against the diagnostic model, and the text-edit exemption being the only exemption. Existing coverage, including hit testing, marquee and the `utf16_range_to_utf8` tests, remains intact.

Passed: `cargo fmt --check`, `cargo check --locked`, `cargo test --locked`, `cargo clippy --locked -- -D warnings`, `cargo build --locked`. The known `block v0.1.6` future-incompatibility warning remains.

## Runtime verification

Launched the normal application with both diagnostics environment variables removed. Screenshot budget of three inspections was used: (1) normal editor — toolbar, Layers, Inspector, canvas and four starter artboards present, no diagnostics visible; (2) after a middle-button drag pan — the canvas content moved exactly the commanded +300/+170 logical pixels, verified by pixel-diff bounding box with culling active; (3) Select-tool click on the Landing artboard — selection outline and resize handles, Layers row highlight and Inspector "Frame Landing" all correct with culling active (offscreen and partially visible objects alike).

Everything else was verified programmatically by pixel-diff against region captures (artifacts under `target/phase14`: `shot1/2/3.png`, `text_edit.png`, `zoom_pan_edit.png`, `verify14.py`, Swift CGEvent helpers): tool switching to Text, text object creation with active editing (76,820 pixels changed in the canvas probe region), selection click, marquee drag, and middle-button pan. Manual mouse input dispatch therefore works with culling enabled.

Honest limitation: the zoom popup at approximately absolute (1150, 187) opens, but clicking its menu rows never changed the zoom label and the popup could not be reopened afterwards, so "zoom to 200% applied interactively" was **not** verified, and the screenshot budget was already exhausted. Zoom correctness rests on unit tests (predicate verified at zoom 0.25/0.5/1/2/4, plus pan/zoom tracking tests) and on `camera_zoom_sum_milli` benchmark telemetry. Keyboard text entry was also not manually verified (no accessibility/keystroke automation available); the text workload exercises text replacement programmatically.

## Remaining bottleneck

At 10,000 objects the largest measured cost is now the Layers selection rebuild: 59 complete row-tree constructions averaging 24.8–26.0 ms each (590,000 rows/run), versus 1.0–1.2 ms of canvas construction. This is unchanged Phase 13 behavior and is not addressed here. The Inspector still rebuilds broadly on every shell render. Canvas construction retains ~0.95–1.21 ms at 10k (construction only; layout/prepaint/paint/GPU still unmeasured), and the O(N) predicate cost is absorbed within it — measurements do not currently justify spatial indexing, and the scan's growth with N should be re-measured at larger documents before considering one.

## Recommended next phase

**Row-level Layers invalidation on selection** (or, second, an explicit Inspector invalidation boundary): Phase 13 separated structure from presentation; the remaining 24.8–26.0 ms selection cost is full-row reconstruction driven by selected-ID changes, which a per-row selected-state update could retain. Keep viewport culling as-is; it has established the value of skipping offscreen construction without an index.
