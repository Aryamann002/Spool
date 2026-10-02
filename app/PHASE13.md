# Phase 13 — retained Layers and explicit invalidation

## Boundary

Layers previously cloned all `DesignObject`s and built every row inside each shell render. Its actual dependencies are document order, object ID, name, type/icon, and selection membership. Geometry, camera, styles and text content are not displayed.

`layers.rs` now owns a retained minimal projection (ID, `SharedString` name, type) and separate selected IDs. `Document::layer_structure_revision` changes only on successful insert/remove, covering creation, duplication, deletion and structural undo/redo. Shell checks the revision and selection; the document projection closure is evaluated only when the revision changes. Changed inputs notify the Layers entity. GPUI's existing `Entity::cached(StyleRefinement)` reuses its subtree otherwise. Plain entity embedding is not sufficient. Definite cached dimensions reproduce the original tree height, padding, gaps and sidebar width; existing flex shrink remains enabled.

Names are copied only into the structure projection, not complete objects/text/styles. Row callbacks hold a weak Canvas entity and express the same exclusive-selection intent. Inspector remains unchanged. No rename/reorder/group operations currently exist; future APIs affecting names/types/order must advance this revision. The revision is local invalidation metadata, not history state.

## Actual invalidation graph

Before: camera/geometry/transient text -> Canvas notify -> Shell render -> Layers projection + tree rebuild and Inspector rebuild.

After: camera/geometry/transient text -> Canvas notify -> Shell render -> unchanged Layers inputs -> cached Layers; Inspector still rebuilds.

Structure mutation -> document revision -> Canvas notify -> Shell synchronization -> Layers projection replacement + Layers notification -> row tree rebuild.

Selection -> Canvas notify -> Shell synchronization -> selected IDs update + Layers notification -> row tree rebuild, without structure projection replacement.

This is not shell isolation. Shell and Canvas still rendered 60 times in every paced workload. GPUI may invalidate cached views on refresh, changed bounds/text style, or notification; the cache is not a guarantee that unchanged data can never render.

## Measurements

Production debug renderer, unchanged Phase 12 fixtures and frame-paced 60-update workloads. Two repetitions of all 12 cases completed (24 runs). Ten settling callbacks and initial projection construction precede the reset. Counts below are per run, **excluding initial population**. Raw timer percentiles, notifications, Inspector work and all counters are preserved in `PHASE13_RESULTS.json`; raw logs are under `target/phase13`.

Before results are from `PHASE12_RESULTS.json`, not a newly rebuilt old binary. Before row counts follow the measured builds/copies and the original one-row-per-object loop. Phase 12 captured selection/text only at 1,000 objects: other sizes have no measured before comparator.

| Objects | Workload | Before projection/tree builds; rows | After projection builds | After tree builds; rows (run 1 / run 2) |
|---:|---|---|---:|---|
|100|Pan|60/60; 6,000|0|0; 0 / 0; 0|
|1,000|Pan|60/60; 60,000|0|0; 0 / 0; 0|
|10,000|Pan|60/60; 600,000|0|2; 20,000 / 0; 0|
|100|Drag 10|60/60; 6,000|0|0; 0 / 0; 0|
|1,000|Drag 10|60/60; 60,000|0|0; 0 / 0; 0|
|10,000|Drag 10|60/60; 600,000|0|0; 0 / 0; 0|
|100|Selection|not captured|0|59; 5,900 / 59; 5,900|
|1,000|Selection|60/60; 60,000|0|59; 59,000 / 59; 59,000|
|10,000|Selection|not captured|0|59; 590,000 / 59; 590,000|
|100|Text insertion|not captured|0|0; 0 / 0; 0|
|1,000|Text insertion|60/60; 60,000|0|0; 0 / 0; 0|
|10,000|Text insertion|not captured|0|0; 0 / 0; 0|

All selection runs recorded 60 selection synchronization updates but 59 Layers renders within the reporting interval. Do not equate update counts with rendered frames: notification scheduling/report boundaries differ. The final projection selection was updated. Selection still rebuilds **the complete row element tree**; this phase separates structure from presentation, not individual-row invalidation.

The two unexplained cache builds in the first 10k pan run had no projection or selection changes. Bounds/refresh/native scheduling may account for these; the precise trigger was not instrumented. Reported as observed, not removed as an outlier. An earlier implementation probe timed out at 8 seconds for 10k pan; the complete runner uses the original 120-second case bound and succeeded.

### Construction timing (milliseconds, per-render averages, run 1 / run 2)

| Objects | Pan canvas | Drag canvas | Selection canvas | Text canvas | Selection Layers |
|---:|---:|---:|---:|---:|---:|
|100|0.101 / 0.101|0.113 / 0.114|0.109 / 0.110|0.114 / 0.112|0.256 / 0.255|
|1,000|0.965 / 0.965|0.993 / 0.995|0.984 / 0.981|0.981 / 0.972|2.407 / 2.389|
|10,000|9.692 / 9.669|9.847 / 9.891|9.711 / 9.683|9.667 / 9.649|23.723 / 23.716|

Canvas timings have 60 samples/run; selection Layers timers have 59. No Layers timer exists for zero-work cases; absence is not a zero-duration sample. Two pan cache rebuilds averaged 23.375ms (too few samples for useful percentiles). At 10k, normal pan/drag previously paid approximately 23.6–23.9ms of Layers construction per render; that construction is now absent in 238 of the 240 measured pan/drag renders. These are construction timings, **not end-to-end frame latency**.

Canvas still constructed all N objects each render: 6,000/60,000/600,000 per run. At 10k, pan constructed 596,016 offscreen objects and drag constructed 596,160, out of 600,000. Approximately 64–66 objects intersected the viewport. No canvas optimization was implemented.

## Tests and validation

104 tests passed, zero failed, one existing timing harness ignored. Added revision/no-op tests, actual retained ID/name/order snapshots through create/delete/duplicate and undo/redo, selection/no-reprojection checks, camera/geometry/style/text non-invalidation checks, and cached dimension tests. Existing coverage remains intact.

Passed: `cargo fmt --check`, `cargo check --locked`, `cargo test --locked`, `cargo clippy --locked -- -D warnings`, `cargo build --locked`. The known `block v0.1.6` future-incompatibility warning remains. Runner initially failed with an introduced Python indentation error; corrected before the completed matrix. Runner now accepts all existing selection/text sizes, handles legitimately absent Layers timers, and writes to configurable `--output` (default `target/phase13`) without overwriting Phase 12 artifacts. Workload definitions were not changed.

## Runtime verification and next task

Launched normal application with both diagnostics environment variables removed. Inspected one screenshot: toolbar, Layers names/icons, Inspector, canvas and four starter artboards are present; no visible diagnostics. No reliable manual mouse/keyboard automation was available, so click/create/delete/duplicate/text gestures were not manually verified. Automated workloads exercise production rendering but do not substitute for manual input-dispatch validation.

**Next bounded optimization: viewport-aware canvas element construction.** Pan/drag now avoid Layers construction while canvas still pays approximately 9.7–9.9ms to build 10k objects, over 99% offscreen. Implement conservative culling with tests for text/strokes/selection and camera bounds; do not add an index yet. Full row reconstruction on selection remains an independently measured 23.7ms cost, and should not be described as solved.

Unknowns: actual layout/prepaint/paint/GPU cost, cached scene-reuse overhead, exact cause of the two pan cache rebuilds, manual interaction correctness, release-mode frame latency, and long-session history memory. Inspector is intentionally still broadly invalidated.
