# Implementation roadmap

Work in order. Each milestone must meet its acceptance checks before the next subsystem begins.

## 1. Source-backed foundation (now)

Deliver stable opaque Spool node IDs; `lamine.yaml` load/save and validation; HTML source bindings; distinct persistent document and transient editor-state types; one semantic operation with undo/redo; a small fixture project. Integrate the model with the existing editor rather than creating a second app.

**Accept when:** round-trip preserves IDs, names, hierarchy, and bindings; duplicate/dangling metadata is rejected; operation/undo/redo restore exact values and preserve redo on no-op; external source bytes remain unchanged; focused model tests and the existing suite pass.

## 2. Source-preserving edits

Add incremental HTML/CSS parsing and patching for a bounded supported syntax, source provenance, external-edit reconciliation, and multi-file atomic writes.

**Accept when:** edits alter only intended source spans; parse/patch errors make no partial changes; unchanged regions remain byte-identical across fixtures; ambiguous cascade ownership is diagnosed.

## 3. Semantic editing and runtime projection

Move existing canvas operations onto the document operation boundary; derive runtime nodes, layout, and hit testing from source; preserve current tools and gestures.

**Accept when:** current editor workflows behave as before, all persistent edits are undoable through semantic operations, and rebuilding runtime from saved source produces equivalent editable nodes.

## 4. Renderer and scale, by evidence

Separate scene/paint data from GPUI where needed; add spatial indexing, culling, and incremental invalidation only against measured workloads.

**Accept when:** deterministic renderer/model tests pass and benchmarks demonstrate the targeted improvement without changing source semantics.

## 5. Advanced document concepts

Add frames/groups/masks/components, CSS ownership/cascade inspection, assets, and collaboration only when a concrete vertical slice requires them. Each concept must define source representation, metadata boundary, operation/history behavior, and migration path before implementation.

## Do not build prematurely

- A general browser-compatible CSS engine or complete cascade inspector.
- A proprietary duplicate of HTML/CSS visual properties in `lamine.yaml`.
- A browser DOM as the runtime scene graph.
- Binary persistence, cache infrastructure, million-node optimizations, spatial indexes, or a plugin/agent registry without a measured or user-facing need.
- Components, variants, constraints, collaboration, or a generalized transaction framework before a real operation requires them.
- A parallel demonstration app or wholesale rewrite of existing GPUI tools.

## Verification

Every milestone adds focused model/operation/source tests, then runs the app crate's existing test suite. Report any platform or dependency limitation with its exact failing command and output; do not label an unrun check as passing.
