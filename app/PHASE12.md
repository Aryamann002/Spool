# Phase 12 — runtime render and invalidation evidence

## Scope and repository inspection

Instrumentation only: no culling, indexing, caching, new dependencies, document/history redesign, or visual changes. Initial tree already contained Phase 11 changes in `canvas.rs`, `PHASE11.md`, and `canvas_benchmarks.rs`, plus untracked `.DS_Store` and `app/digest.txt`; these were preserved. An unrelated `.freebuff/` appeared during the work and was left untouched.

Verified the production canvas still borrows its document/selection. `CanvasView::render` constructs the viewport and calls `artboards`; `artboards` builds every object's element tree plus overlays. GPUI later performs layout/prepaint/paint. Its custom canvas prepaint callback inserts the hitbox and updates camera viewport size, notifying if it changes. Its paint callback paints the grid and installs mouse move/up handlers. Pan updates camera then notifies; active move updates geometry via `update_interaction`, then notifies; mouse-up finishes the gesture and records one history command.

`AppShell::render` synchronously builds chrome, Layers, and the design inspector. Layers clones the document objects and creates a row per object. The inspector calls `selected_objects`, which performs linear ID lookups and clones results. There are no separate cached sidebar entities or explicit canvas subscriptions.

Checked the locked GPUI checkout at `397cbc8`: `Context::notify` calls `App::notify`; window entity tracking routes invalidation; `Window::mark_view_dirty` marks the notified view and its ancestor view path dirty. `Window::on_next_frame` schedules callbacks after a frame without itself dirtying the window. This explains the observed parent reconstruction without assuming React-style dependency behavior.

## Facilities and boundaries

- `src/diagnostics.rs`: debug opt-in through `SPOOL_DIAGNOSTICS`; thread-local counters, monotonic `Instant` timers, bounded 4,096-sample rings, nearest-rank percentiles, explicit reset/report. Static metric names; no per-object logging or per-event string formatting. Release counters/timers are no-ops and never read the environment.
- `src/canvas.rs`: canvas render/notification counts, viewport/zoom aggregates, viewport-resize notification counts, object-construction/geometry-visibility counts, construction and render-build timers.
- `src/shell.rs`: shell render, Layers/inspector build counts, projection-copy counts/timers, Layers tree construction and shell build timers.
- `src/canvas_workloads.rs`: debug-only deterministic fixture and frame-paced driver. Requires BOTH `SPOOL_DIAGNOSTICS` and a valid `SPOOL_WORKLOAD`. Normal documents/behavior remain unchanged. Live canvas pointer movement/down/up, zoom/pinch, and root editing/key policy are ignored only while a requested driver is active; do not interact with benchmark windows or resize them. Individual toolbar/sidebar listeners are not a fully sandboxed input system. Interrupted drags fail explicitly instead of producing apparently valid no-op samples.
- `measure_runtime.py`: standard-library-only launcher with per-case timeout, process cleanup, raw logs, report parsing, and persisted per-run summaries. Repeated invocations can select subsets and replace the same run/case result.
- `PHASE12_RESULTS.json`: captured 20 completed runs, including all counters and timing summaries. Raw logs/screenshots remain under ignored `target/`.

`canvas_elements` measures the existing `artboards` function: synchronous object and overlay element construction, including selected-object outline lookups. It does NOT measure GPUI layout, text shaping in prepaint, paint, GPU execution, or presentation. `canvas_render_build` includes setup and the diagnostic visibility scan. `shell_render_build` measures shell element construction, not the later child canvas render. `layers_tree_build` includes object projection and row construction, excluding surrounding sidebar chrome. Projection timers isolate copying/selected-object lookup from row/inspector element construction. These timers are nested; do not sum nested metrics.

Objects constructed means **document-object renderer invocations**, not every descendant GPUI element. Visibility is inclusive axis-aligned geometry-box intersection, excluding label/stroke/shadow overflow, not exact painted-pixel visibility. A separate O(N), allocation-free diagnostic visibility scan is outside `canvas_elements`. Its measured overhead is about 5–6 µs / 48–49 µs / 464–465 µs at 100 / 1,000 / 10,000 objects in the main runs; it is absent when disabled. First use reserves timer storage; reports allocate/sort only after the workload. Timings include small enabled counter/timer bookkeeping and are not a zero-overhead profiler.

## Reproduction and workload characteristics

From `app`:

```sh
cargo build --locked
python3 measure_runtime.py --repeats 2
# Or bounded subsets:
python3 measure_runtime.py --repeats 1 --cases pan:100 pan:1000 pan:10000
python3 measure_runtime.py --repeats 1 --run-start 2 --cases drag:10000
```

Direct debug invocation: `SPOOL_DIAGNOSTICS=1 SPOOL_WORKLOAD=pan:1000 target/debug/Spool`. The driver reports completion and then leaves the window open; the Python launcher terminates it after reading the completion marker. Invalid workload values do not activate fixtures. Do not use the release executable for this debug-only driver.

Host: Apple M4 / arm64 macOS; rustc 1.96.0; Cargo debug/unoptimized profile. Application requests the normal 1480×960 window; actual viewport depends on native window sizing. Avoid input/window changes during measurements. Fixture: N rectangles, 100 columns, spacing 120 world units, 80×80 geometry, ordinary default fill, names and stable IDs starting at 5 (no bespoke starter frame IDs). IDs and next-name/ID counters are initialized consistently. N=100 is one long row; larger fixtures add rows. A large majority of objects are offscreen at zoom 1.

The driver settles for ten post-frame callbacks before reset, then issues 60 updates, one per completed frame. These are settling callbacks, not ten timed warmup drags. It calls the production mutation methods and renderer, but **bypasses OS event dispatch, hit testing, pointer capture, and input latency**. Pan uses pointer positions (-5i, -3i), from fixed zero offset/pointer, for i=1..60; drag selects the last ten objects and calls PotentialMove → Moving updates at (5i, 3i). These late IDs deliberately expose linear lookup cost and are not representative of every selection placement. Finish/history is timed separately after the last rendered update. Selection alternates first/last IDs. Text uses one text object at the first ID and inserts 60 `x` characters into the transient edit buffer through the existing editing method; it does not commit text during the measured interval.

## A. Render behavior

Main runs 1 and 2 produced identical counts in this table, **per run**:

| Objects | Workload | Updates | Canvas renders | Shell renders | Layers builds | Inspector builds | Object constructions |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | Pan | 60 | 60 | 60 | 60 | 60 | 6,000 |
| 1,000 | Pan | 60 | 60 | 60 | 60 | 60 | 60,000 |
| 10,000 | Pan | 60 | 60 | 60 | 60 | 60 | 600,000 |
| 100 | Drag 10 | 60 | 60 | 60 | 60 | 60 | 6,000 |
| 1,000 | Drag 10 | 60 | 60 | 60 | 60 | 60 | 60,000 |
| 10,000 | Drag 10 | 60 | 60 | 60 | 60 | 60 | 600,000 |
| 1,000 | Selection | 60 | 60 | 60 | 60 | 60 | 60,000 |
| 1,000 | Text insert | 60 | 60 | 60 | 60 | 60 | 60,000 |

Canvas notification counts were usually 60, but drag:1000 run 2 and text:1000 run 1 recorded 61, still with 60 renders. The earlier counters do not attribute that extra notification to a specific call site. It cannot be called an extra render. Later instrumentation separately tags viewport resize notifications.

## B. Construction timings

Microseconds. Each main run has 60 construction samples. Percentiles below are **run 1 only**, not invented pooled percentiles. The two means demonstrate repeatability; raw per-run p99 values are retained in JSON, but with 60 samples nearest-rank p99 is just the maximum and is not a robust tail estimate.

| Objects | Workload | Mean run 1 | Mean run 2 | p50 run 1 | p95 run 1 | Total run 1, ms |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 100 | Pan | 98.408 | 98.844 | 97.250 | 102.750 | 5.904 |
| 1,000 | Pan | 966.957 | 967.259 | 964.542 | 985.083 | 58.017 |
| 10,000 | Pan | 9602.144 | 9601.046 | 9544.459 | 9839.334 | 576.129 |
| 100 | Drag 10 | 108.832 | 110.890 | 108.167 | 112.459 | 6.530 |
| 1,000 | Drag 10 | 998.709 | 999.248 | 993.834 | 1032.417 | 59.923 |
| 10,000 | Drag 10 | 9761.565 | 9764.869 | 9731.250 | 9950.542 | 585.694 |
| 1,000 | Selection | 997.717 | 989.774 | 993.916 | 1024.625 | 59.863 |
| 1,000 | Text insert | 1000.271 | 978.934 | 998.750 | 1031.041 | 60.016 |

Other mean costs, run 1 / run 2, µs:

| Objects | Workload | Shell build | Layers tree build | Update |
| ---: | --- | ---: | ---: | ---: |
| 100 | Pan | 369.345 / 371.277 | 242.663 / 242.242 | 4.704 / 5.309 |
| 1,000 | Pan | 2517.890 / 2511.201 | 2367.994 / 2362.546 | 7.799 / 7.513 |
| 10,000 | Pan | 23899.098 / 24114.441 | 23625.147 / 23833.790 | 10.349 / 10.596 |
| 100 | Drag 10 | 416.544 / 432.819 | 244.939 / 247.615 | 9.553 / 11.295 |
| 1,000 | Drag 10 | 2613.305 / 2614.329 | 2395.323 / 2389.457 | 37.911 / 38.501 |
| 10,000 | Drag 10 | 24410.081 / 24514.209 | 23833.279 / 23934.016 | 282.813 / 275.544 |
| 1,000 | Selection | 2648.767 / 2623.168 | 2376.276 / 2375.904 | 9.295 / 7.865 |
| 1,000 | Text insert | 2677.212 / 2601.171 | 2402.611 / 2355.408 | 14.310 / 10.484 |

Drag finish (final geometry application + command construction + history record), one sample per run: 100 objects 10.834 / 12.291 µs; 1,000 objects 59.084 / 63.125 µs; 10,000 objects 498.709 / 536.292 µs. Two samples do not support history percentiles. Individual lookup iterations were not instrumented: drag update contains ten linear `set_position` lookups and the first transition's selection update; the timer includes notify bookkeeping. Scaling of this late-ID workload is observed, but lookup-only time is not isolated.

## C. Invalidation evidence

- **Pan:** camera-only mutation → canvas notify → CanvasView and ancestor AppShell reconstruction → Layers and inspector rebuilt. Layers copies all N objects every time despite unchanged object names/order/selection.
- **Drag 10:** geometry mutation → the same graph. Canvas still builds every object; Layers is rebuilt although its displayed data is unchanged after initial selection. The inspector may need geometric updates; this is not an argument to freeze it.
- **Selection:** the same graph, including document-wide canvas and Layers reconstruction. Selection-derived chrome legitimately changes; these counts alone do not prove every associated rebuild is unnecessary.
- **Text:** transient buffer insert → the same graph and full canvas reconstruction, even though persistent document data remains unchanged until commit. Layers names/order/selection do not change during these inserts.

There is no evidence of multiple canvas renders per paced update in the completed cases. The evidence is **broad reconstruction per update**, not a demonstrated notification storm. Faster-than-frame OS event streams/coalescing remain unmeasured.

## D. Offscreen work

Geometry intersections summed across the 60 frames, main runs (counts match across runs 1 and 2):

| Objects | Workload | Intersecting constructions | Offscreen constructions | All constructions | Average intersecting/frame |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | Pan | 216 | 5,784 | 6,000 | 3.60 |
| 1,000 | Pan | 4,532 | 55,468 | 60,000 | 75.53 |
| 10,000 | Pan | 3,984 | 596,016 | 600,000 | 66.40 |
| 100 | Drag 10 | 480 | 5,520 | 6,000 | 8 |
| 1,000 | Drag 10 | 3,840 | 56,160 | 60,000 | 64 |
| 10,000 | Drag 10 | 3,840 | 596,160 | 600,000 | 64 |

Selection/text: 3,840 intersecting and 56,160 offscreen constructions at N=1,000 per main run. Every object is visited and constructed; overlays/descendant elements are not included in these object counts. Culling is plainly a candidate, but the timer does not separately assign cost to visible versus offscreen trees, so no exact culling savings are claimed.

Additional pan probes (runs 3 and 4) are retained, not hidden. They again produced 60 renders/builds and N×60 constructions. Run 3 offscreen totals were 55,684 / 595,663 at N=1,000 / 10,000. The initial viewport was 918×874; per-frame viewport/zoom telemetry was added afterward. Run 4 measured mean viewport 918×1011, zoom 1, no in-interval viewport-resize notifications, and 55,468 / 595,468 offscreen constructions. The first 16 runs did not record viewport dimensions. The exact cause of run 3's visibility variation was not separately captured; native input/viewport conditions cannot be assumed identical across all launches. Visibility totals must be interpreted against actual viewport/camera telemetry, not as viewport-independent fixture constants. Construction cost conclusions remain consistent: run 4 means were 0.956 / 9.475 ms for canvas and 2.331 / 23.318 ms for Layers.

## E. One next optimization

**Separate the Layers view's invalidation boundary from camera/transient canvas state.**

The strongest measured cost is rebuilding all Layers rows: ~23.6–23.9 ms per render at 10,000 objects in the main runs, compared with ~9.6–9.8 ms canvas element construction and ~0.28 ms drag update. Camera-only updates and transient text insertion have no effect on Layer row data. A bounded next phase should make Layers a separately retained view driven by names/order/types/selection changes, while keeping inspector and zoom readouts correct. GPUI can dirty ancestors, so merely moving a function into a new file is not sufficient; validate an actual retained view/notification boundary against these counters. No such optimization is implemented here.

This selects the largest measured avoidable construction region rather than claiming Layers dominates total frame time. Layout/paint could change the end-to-end priority.

## F. Unknowns and limitations

- GPUI layout/prepaint/paint/GPU/presentation times and frame percentiles; use macOS Instruments/GPUI profiling with representative documents.
- OS input dispatch/capture, event bursts, coalescing, pointer-to-present latency, and hardware text/IME interaction.
- Release-build costs; all runtime measurements here are debug.
- Allocation counts and exact visible/offscreen per-object timing.
- Detailed lookup-only and text shaping time; mixed complex artboards/text may differ greatly from rectangles.
- The origin of occasional additional notifications in early runs and run 3 camera/visibility variation. Final diagnostics provide viewport/zoom aggregates and resize notification counts for subsequent investigations.

## Verification and failed attempts

All required commands passed on the completed implementation: `cargo fmt --check`, `cargo check --locked`, `cargo test --locked` (98 passed, one ignored Phase 11 timing harness), `cargo clippy --locked -- -D warnings`, `cargo build --locked`. New tests cover counters/reset, percentile helper, bounded sample retention/totals, viewport geometry intersections/invalid values, and deterministic fixtures. Existing read-only rendering and interaction/history tests remain. Python report-parser smoke test passed. The Phase 11 ignored harness was also explicitly rerun. Known `block v0.1.6` future-incompatibility warning remains.

Initial validation failed Clippy's field-reassignment, manual-is-multiple-of, and const-thread-local lints; fixed and rerun successfully. The initial all-in-one matrix exceeded its 240-second terminal timeout. An interrupted drag yielded only eight renders and was discarded, not included as a valid drag baseline. Follow-up attempts explicitly rejected interrupted drags. A too-strict runner check also rejected 61 notifications/60 updates; removed because notifications can legitimately coalesce and are evidence, not a fixed invariant. Successfully completed the matrix in bounded subsets, two main repetitions plus four additional pan probes. These failures are tooling/measurement limitations, not hidden successes; the exact external cause of the interrupted gesture was not proven.

Launched the normal application with both diagnostics environment variables removed. It remained alive until deliberately terminated; one screenshot inspected showed toolbar, Layers, Inspector, grid, and all four starter artboards intact. No manual mouse/keyboard interaction verification was performed. Programmatic mutation workloads rendered in the actual GPUI application, but are not equivalent to manual drag or pan verification. Diagnostics add no visible UI.
