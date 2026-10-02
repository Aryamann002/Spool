# Phase 11 — editor architecture and performance

## Scope and working tree

Initial working tree: only an untracked root `.DS_Store`; left untouched. No dependencies, visuals, interaction policy, ordering, IDs, or history semantics changed.

## Architecture discovered

- `main.rs`: GPUI application/window bootstrap and text action bindings.
- `canvas.rs`: persistent `Document` with an ordered `Vec<DesignObject>`, monotonic IDs/names, geometry/style/text; explicit `DocumentCommand` variants and `History`; transient selection/tool/gesture/text buffers; camera; GPUI pointer/input adapters; element construction and bespoke starter-artboard content. These responsibilities have logical boundaries but share one module and `CanvasView` owns them together.
- `shell.rs`: chrome, Layers, inspector, shortcuts, popovers, and application composition. Calls CanvasView intent methods rather than directly applying history commands. Also participates in editing lifecycle (commit on outside clicks, Escape policy) and mirrors active tool state.
- `theme.rs`: presentation constants.

Typical move: GPUI mouse-down → viewport-local screen point → camera world conversion → resize-handle test or reverse-order document hit test → PotentialMove snapshots of selected geometry → screen-space threshold → Moving mutates geometry from original snapshots → mouse-up records one geometry command → notify → element construction. Escape restores snapshots without a command. Creation/deletion/duplication/styles use explicit commands; text edits buffer transient content and commit one session command. Camera and selection do not enter history. Undo/redo replay affected object changes and reconcile selection.

Existing 89 tests cover camera, creation, selection, transforms, cancellation, IDs/order, commands/history, style, text Unicode/IME conversions, text selection, and shell shortcuts/inspector values without launching GPUI. Rendering, focus/event propagation, and shell refresh behavior are not covered end-to-end.

## Confirmed costs and risks

Confirmed code operations (not necessarily frame-time bottlenecks):

- Canvas previously cloned the entire document and selection every render. Names and text were copied even on camera-only changes. Removed in this phase.
- Canvas builds every object's element tree, including offscreen objects; clipping is not construction culling. Starter frames have large bespoke trees selected by fixed IDs.
- Layers copies full objects and inspector clones selected objects. Object lookup is linear; K selected-object lookups can cost O(KN).
- Move/resize and history replay use linear setters; `set_geometry` searches twice. Hit tests and marquee scan the document. Selection membership is linear.
- History stores affected-object changes, not whole-document snapshots per pointer event. History is unbounded; insert/delete store object payloads and text stores before/after strings.
- Text shaping occurs in GPUI prepaint and pointer caret queries; grid paints viewport-dependent dots.

Unmeasured risks: element construction/layout/paint may dominate at large N; shell invalidation may amplify updates or leave derived chrome stale; pointer events may outpace useful frames. These need tracing/runtime evidence before fixes. Document geometry currently uses GPUI value types, so it is not yet framework-independent or serialization-ready. Pages are shell presentation state, not persistent document pages; hierarchy is not implemented.

## Selective tldraw reference study

Individual source files consulted on mutable `main`, not a full checkout:

- https://github.com/tldraw/tldraw/blob/main/packages/editor/src/lib/editor/Editor.ts — active tool identity derived from state; event queues/batched changes; rendering order and viewport culling separate from persistent records, with selected/editing exemptions.
- https://github.com/tldraw/tldraw/blob/main/packages/editor/src/lib/editor/tools/StateNode.ts — explicit enter/exit/event routing prevents delivery into a child replaced during a transition.
- https://github.com/tldraw/tldraw/blob/main/packages/editor/src/lib/editor/managers/HistoryManager/HistoryManager.ts — record diffs and transaction/stop boundaries; pending-diff accumulation avoids repeated whole-diff work.

Translation: keep Rust enum interactions and affected-object commands; no React abstractions, generic reactive store, or state-machine framework needed here. Camera/rendering should read document data rather than duplicate it. Keep history boundaries explicit and later derive shell tool identity from editor authority.

## Baseline measurements

Reproduce from `app`:

```sh
cargo test --locked core_workload_baseline -- --ignored --nocapture --test-threads=1
```

macOS host, Cargo debug/unoptimized test profile; 1,000 iterations and 10 warmups per operation. Synthetic ordered grid of text objects, each with a name and 108-byte text payload. Fixture construction is outside timing. No new dependencies. Units: microseconds/operation.

| Objects | Old snapshot clone | Slice borrow | Hit-test miss | Marquee query | Move 10 last objects | Build/record/drop 10-object history |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 8.278 | 0.008 | 1.103 | 0.925 | 2.927 | 3.620 |
| 1,000 | 66.557 | 0.006 | 8.612 | 5.681 | 24.403 | 24.999 |
| 10,000 | 671.875 | 0.006 | 85.704 | 45.224 | 246.516 | 245.409 |

Post-change rerun retained the old clone operation as a reference: 6.619 / 65.450 / 670.142 µs; borrowing 0.006 µs at each size. Other rerun values: hit miss 0.891 / 8.510 / 84.825; marquee 0.714 / 5.658 / 45.042; move 2.474 / 23.992 / 242.133; history 3.231 / 24.904 / 244.730. Ordinary run-to-run variation is not a claimed improvement. Borrow timings are near the measurement floor, not meaningful speedup ratios.

The production clone is removed, but these numbers are **not canvas frame timings**. Movement excludes event dispatch/notify/render; history measures geometry lookup, command construction, recording, and disposal, not undo replay; marquee covers a fixed 400×400 area and full-containment queries. No allocation counter or release-profile measurements taken. Synthetic text payloads influence clone cost.

Not measured: visible canvas frame/layout/paint, Layers/inspector refresh, GPU work, text shaping, full pointer-to-frame latency. Those require a running GPUI window, representative documents, separate element/prepaint/paint spans and macOS Instruments (Time Profiler/Allocations), plus scripted or manual interactions. Next instrumentation should count CanvasView/AppShell renders and measure build/prepaint time during fixed pan/drag sequences at these object counts; compare repeated runs and frame percentiles rather than extrapolating from this debug harness.

## One improvement delivered

`CanvasView::render` now lends `&Document` and `&Selection` to `artboards`. Element construction consumes derived owned element data synchronously; no document borrow escapes into deferred callbacks. GPUI callbacks continue using Entity handles. The compiler enforces read-only access across this boundary. Existing active text-edit snapshots, marquee snapshots, and element-required strings are intentionally unchanged.

This removes unconditional O(N + text/name bytes) copying per canvas render without caches, invalidation rules, spatial indexing, a new editor copy, or changes to interaction/history. It also establishes the renderer's read-only interface. No file split was justified for this narrowly scoped change.

Files: `src/canvas.rs` (borrowed boundary and regression test), `src/canvas_benchmarks.rs` (ignored reproducible timing harness), this report. New test builds borrowed object/outline/handle elements at min/default/max zoom for supported object types, including Unicode text, and verifies document data is unchanged. This tests element construction, not GPUI layout or paint.

## Validation and runtime

Before render changes: 89 tests passed, plus the explicit timing harness. After changes:

- `cargo fmt --check`: passed.
- `cargo check --locked`: passed.
- `cargo test --locked`: 90 passed, 0 failed, 1 ignored timing harness.
- `cargo clippy --locked -- -D warnings`: passed.
- `cargo build --locked`: passed.
- Explicit timing harness rerun: passed.

Known `block v0.1.6` future-incompatibility warning remains.

Runtime: `open -n target/debug/Spool` opened Terminal rather than displaying the application in the first screenshot. Direct executable launch through a bounded subprocess succeeded, remained running until deliberately terminated, and the second screenshot showed toolbar, Layers, inspector, grid, and all four starter artboards. Two screenshots inspected, stored only under ignored `target/`. No pointer/keyboard automation or manual interaction verification performed. This confirms initial visual rendering, not dragging, undo/redo, focus handling, or runtime frame-rate gains.

## Target boundaries and next task

Gradually establish persistent document data/operations; transient editor authority for selection/tools/interactions; enum-driven deterministic tool transitions; camera and read-only render calculations; history operating on explicit changes; shell expressing intents and reading lightweight projections. Do not extract all of them at once.

Next bounded task: instrument canvas element construction and shell/canvas render counts for fixed pan and multi-object drag workloads at 100/1,000/10,000 objects. Confirm invalidation correctness and identify whether offscreen construction or ID lookup dominates before choosing culling or indexing. Remaining priority costs are uncullled element trees, sidebar/inspector deep copies, and O(KN) geometry/selection lookups.
