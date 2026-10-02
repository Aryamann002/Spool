# Architecture Extraction: Rendering

## What is publicly known

| Product | Published information |
|---|---|
| **Figma** | **Scene graph data model**; C++/TypeScript client; WebGL renderer; **migrated to WebGPU (Sep 2025)**; a dedicated performance-testing framework |
| **tldraw** | **Eight named techniques**, documented in detail, with defaults and options |
| **Affinity** | **Nothing.** No engineering blog, no architecture article |
| **Canva** | **Nothing** for rendering; a third-party OpenAI case study mentions vision-based content parsing |

[ENGINEERING-DISCLOSED for Figma; DOCUMENTED for tldraw; documented gaps for Affinity and Canva]

### Figma's disclosed rendering facts

- "The renderer, scene graph data model, and other compute-intensive [systems are C++]. Figma's browser client
  code is primarily comprised of **C++ and TypeScript**." [THIRD-PARTY — conference talk, "A Tour of the C++
  Engine which Powers That Design MMO"]
- "All browsers provide a high-performance GPU compositor but the web doesn't have any way of hooking into the
  rendering algorithm and changing how [it works]" — hence a custom renderer. [ENGINEERING-DISCLOSED —
  "Building a professional design tool on the web", 2015]
- "We've updated our renderer to use **WebGPU**, unlocking new performance optimization opportunities."
  [ENGINEERING-DISCLOSED — "Figma Rendering: Powered by WebGPU", Sep 2025. The post body did not render in
  the retrieval used, so the specific techniques are **Unknown** from this source.]
- "Keeping Figma Fast" (Aug 2023) is about a **performance testing framework**, prompted by "a laptop
  crashed in an empty office". [ENGINEERING-DISCLOSED]

[INFERRED] Figma has a **document → scene graph → render tree → GPU** pipeline, with a C++/WASM core and a
TypeScript shell. The scene graph is a real architectural boundary.

### tldraw's eight techniques

[DOCUMENTED — `sdk-features/performance.mdx`]

| # | Technique | Detail |
|---|---|---|
| 1 | **Viewport culling** | Spatial index; off-screen shapes get `display:none`; "10,000 shapes might only render 50"; culled shapes remain selectable, hit-testable, and exportable; **selected and edited shapes are never culled** |
| 2 | **Reactive signals** | Fine-grained dependency tracking; changing one shape's colour does not re-render unrelated shapes |
| 3 | **Batched store updates** | Multi-shape create/update produces **one** notification; `editor.run()` batches across calls |
| 4 | **Debounced zoom** | `getEfficientZoomLevel()` returns a **stable** value during camera movement above `debouncedZoomThreshold` (500) shapes; updates to true zoom once the camera stops |
| 5 | **Geometry caching** | `editor.getShapeGeometry()`; the editor "handles caching, transforms, and bounds calculation" |
| 6 | **Level of detail** | See below |
| 7 | **Image resolution scaling** | `steppedScreenScale`, power-of-two, debounced |
| 8 | **Spatial/shape indexing** | Fractional index for z-order; computed indexes for hierarchy and bindings |

### LOD, concretely [DOCUMENTED]

- Sticky notes drop their box shadow for a plain bottom border.
- Dashed and dotted freehand strokes render as solid lines.
- Hatch pattern fill switches to a solid fallback colour.
- Text outlines turn off below `textShadowLod` (0.35), **always off on Safari**.
- All LOD transitions use `getEfficientZoomLevel()` so they are stable during camera motion.

### The identity-stability rule [DOCUMENTED — `sdk-features/culling.mdx`]

> "Both are reactive, and `getCulledShapes()` returns **the same `Set` instance while its contents are
> unchanged**, so it's cheap to read in `track` components or `useValue`."

[INFERRED] Referential stability is the mechanism that makes per-frame reading cheap. Without it, every read
would invalidate every consumer.

### Performance options [DOCUMENTED]

| Option | Default |
|---|---|
| `debouncedZoom` | `true` |
| `debouncedZoomThreshold` | `500` |
| `maxShapesPerPage` | `4000` |
| `textShadowLod` | `0.35` |
| `snapThreshold` | `8` (screen px) |

### Measurement [DOCUMENTED]

- `editor.performance` → `interaction-end` (fps, p95FrameTime, LoAF), `camera-end`, `shapes-created`,
  `shapes-updated`, `shapes-deleted`, `frame`.
- "with no overhead when no listeners are attached".
- `PerformanceApiAdapter` maps to native `performance.mark()`/`measure()`.
- Starting diagnostics: `getCurrentPageShapeIds().size`, `getCulledShapes().size`.

---

## The concept that must be extracted

### Renderer independence

**Evidence:** Figma's **scene graph data model** as a named concept between the document and the GPU
(ENGINEERING-DISCLOSED). tldraw's separation of `getGeometry()` (data) from `component()` (rendering)
(SOURCE-CODE).

[INFERRED] **Every mature editor has three layers:**

```
Document  →  Scene graph / paint tree  →  UI toolkit (or GPU)
```

The middle layer is what enables:
- **Export** (SVG/PDF) — the research brief asks directly whether "GPUI view hierarchy" can be coupled with
  "persistent design document hierarchy"; the answer in every mature product is **no**.
- **Headless rendering** — thumbnails, tests, previews.
- **AI verification** — `getSvgString` / `toImage` on demand.
- **Partial invalidation** — only the changed subtree's paint nodes change.
- **Caching** — a paint node can be memoised on its inputs.

[INFERRED] **None of these five are possible without the middle layer.**

### The document → scene graph question

> Research whether the document model is directly rendered or transformed into another representation.

| Product | Answer |
|---|---|
| Figma | **Transformed.** A named scene graph data model, in C++. |
| tldraw | **Transformed.** Per-shape `component()` produces view elements from shape records; geometry is cached separately. |
| Affinity | **Unknown** |
| Canva | **Unknown** |

[INFERRED] Two of two known answers are "transformed".

[INFERRED] The questions the brief raises — `Document → Scene Graph → Render Tree → GPU` vs.
`Document → Elements → Renderer` — are not actually alternatives. The first is a decomposition of the second.
The real question is **how many boundaries**, and the evidence suggests at least two: (a) document → paint
list, and (b) paint list → GPUI elements.

### Partial invalidation

[DOCUMENTED — tldraw] Reactive signals mean "changing a shape's props … only that shape's component
re-renders — not the entire canvas".

[INFERRED] In a non-reactive UI toolkit, the equivalent is a **dirty-set on the paint tree**: mark the
changed object's paint node and its ancestors as dirty; re-render only those.

### Culling and the cull boundary

[DOCUMENTED — tldraw]

> "Culled shapes stay in the DOM with `display: none`, so they cost nothing to render, and they stay in the
> store, so they **can still be selected, hit-tested, and exported**."

> "**Selected shapes and the shape being edited are never culled**, so users can always see what they're
> working with."

Opt-out via `ShapeUtil#canCull()`:
> "Reasons to disable culling include shapes with visual effects (shadows, glows) that extend beyond their
> bounds, shapes that measure their DOM to determine size, and shapes running animations that should continue
> off-screen."
> [DOCUMENTED]

[INFERRED] **Culling is a renderer decision, not a document one.** The document knows nothing about the
viewport. Spool's prototype currently does the culling test inline in `Render` against `CULL_PADDING`,
which is the right *place* but is not exposed as a reusable derived set.

### Zoom normalisation across rendering

[DOCUMENTED — tldraw] Debounced zoom. [INFERRED — others]

[INFERRED] There are **four** distinct zoom-derived quantities, and they should not share a single value:

| Quantity | Source of zoom |
|---|---|
| Coordinate transform | **True** zoom (exact) |
| LOD / effect simplification | **Debounced** zoom (stable) |
| Stroke widths, handle sizes | Debounced zoom |
| Text measurement | True zoom (must match layout) |

---

## Spool prototype: the current rendering

From `app/src/canvas.rs` and `app/PHASE14.md` / `PHASE15.md` [OBSERVED in source]

### What exists

1. **Viewport culling** (Phase 14). `CULL_PADDING = 64.0` world units, applied to every side of an object's
   geometry box; `should_construct(...)` decides whether to build the element tree.
   - The doc comment explains the padding rationale in detail: it must cover the 24-unit label, strokes, and
     text overflow, and it stays conservative across zoom because all overflow is specified in world units
     scaled by zoom.
2. **A `px!($value, $zoom)` macro** that scales world values to pixels.
3. **Zoom-adaptive background grid**: `while spacing * zoom < 24.0 { spacing *= 2 }` and
   `while spacing * zoom > 48.0 { spacing /= 2 }` — i.e. the grid spacing adapts to keep 24–48 screen px.
4. **`dot_size = (1.5 * zoom).clamp(1.0, 2.0)`** — screen-space clamping.
5. **A custom `Element` for text** (`CanvasTextPrepaint`, `CanvasTextInput`) with an `IntoElement`/
   `Element` implementation — a GPUI canvas-style text path, not a `div` with a text child.
6. **`diagnostics::intersects`** — an AABB helper used by culling.
7. **The generated scene is drawn by `impl Render for CanvasView`**, which builds a GPUI element tree
   directly from `Document` data.

### PHASE15's documented finding about GPUI retention [ENGINEERING-DISCLOSED from the vendored source]

| Question | Answer in GPUI 0.2.2 (zed rev `397cbc84…`) |
|---|---|
| What does `Entity::cached(style)` cache? | The rendered subtree of that view entity; with a cached style, `render()` is not called during layout |
| When is the subtree reused? | Only if `bounds`, `content_mask`, `text_style` match **and** `!window.dirty_views.contains(&entity_id)` **and** `!window.refreshing` |
| What invalidates it? | `Context::notify()` on the view entity, a bounds/mask/style change, or the inspector picker |
| Does notifying a parent invalidate children? | **No** — dirtying propagates **upward**: `mark_view_dirty` inserts the view and every ancestor, never descendants |
| Does a parent's re-render invalidate nested cached children? | **Yes** — while a cached view re-renders it sets `window.refreshing = true`, and the reuse check requires `!window.refreshing`. Every nested cached view is forced to re-render |
| Is there per-child memoization inside a view's `render()`? | **No.** `render()` is a plain function returning an element tree, rebuilt on every render of that view. Elements with stable ids reuse layout/scene state, not construction |
| Can rows be made per-entity to get O(changed) construction? | **Not effectively.** Notifying a row entity dirties its ancestors, which re-renders the parent, which sets `refreshing`, which forces every sibling row to re-render |

> "**Conclusion:** GPUI's retention granularity is the *entity*, and an invalidated child always drags its
> ancestors (and therefore its siblings) through a re-render. Rows in Spool are plain `div()` elements inside
> one `LayersView` entity, so there is nothing below the entity boundary that can be retained. This is not
> React: there is no virtual-DOM diff of children, no per-child memoization, and no 'notify only this child'
> path."

Measured results from PHASE15 at 10,000 objects:
- Layers row construction: 26.0/24.8 ms → 22.7/22.6 ms (−12.9% / −9.0%)
- The new shell-side selection sweep: **1.56 ms** per selection change
- Combined selection-change construction path: **−7.0% / −2.7%**, "not by an order of magnitude"
- Projection rebuilds in selection-only changes: **0**

### What the current rendering lacks

| Missing | Consequence |
|---|---|
| **A paint/scene representation** | No headless rendering; export would duplicate layout/text/assets |
| **Debounced zoom** | LOD decisions would flicker during camera motion |
| **A spatial index** | `hit_test` and `objects_in` are O(n) linear scans |
| **Geometry caching** | `WorldRect::contains_object` is recomputed per object per query |
| **LOD rules** | No simplification at low zoom |
| **Partial invalidation** | Any `cx.notify()` re-renders the whole canvas |
| **Image resolution LOD** | No images yet |
| **A culled-shape set exposed as derived state** | Culling is inline in `Render` |

[INFERRED] **The `px!($zoom)` macro and the adaptive-grid loop already demonstrate the right instinct**
(zoom-normalised screen-space constants). PHASE14's `CULL_PADDING` doc comment shows careful reasoning about
what must be conservative across zoom.

[INFERRED] **The finding that matters architecturally is not the numbers; it is that GPUI's retention
model is entity-level with upward dirty propagation.** That means:
- Fine-grained per-object retention is **not achievable** in this GPUI version.
- A `document → paint tree` boundary would still help (it enables caching and export) but would **not**
  enable tldraw-style per-shape re-render isolation, because the bottleneck is GPUI's entity model, not the
  element tree.

[INFERRED] Therefore: **a paint tree is valuable for export, headless rendering, caching, and AI verification,
but is not by itself a rendering-performance solution in GPUI.** Performance would need either (a) fewer
`notify()` calls, (b) coarser entities (one entity per viewport-tile, say), or (c) a different UI toolkit
model. All three are large decisions.

---

## Candidate architectural implications

### Implication A — a paint/scene representation is required, for export and AI more than for speed

**Evidence:** Figma's scene graph (ENGINEERING-DISCLOSED); tldraw's `getGeometry()` / `component()` split
and its export + AI-verification use of `getSvgString`/`toImage` (DOCUMENTED).

**Why it matters:** Export, thumbnails, headless rendering, and agent verification all require rendering
without a UI toolkit. The brief explicitly warns against coupling `GPUI view hierarchy` to the document.

**Approaches:**
- **A. No paint tree.** Write a separate SVG/PDF exporter that re-implements layout and text. Duplicates
  everything; guaranteed to diverge.
- **B. A paint tree** (`PaintList` of resolved, laid-out primitives with world coordinates), built from the
  document and consumed by both a GPUI renderer and an exporter.
- **C. B + caching** keyed on object revisions.

**Tradeoffs:** B is a real investment but it is the enabling piece for three other capabilities.

**Decision: TBD — requires architecture review.** [INFERRED] This is arguably the highest-leverage
architectural question in the research, because export and AI both depend on it.

---

### Implication B — culling should be a reusable derived set, not inline in `Render`

**Evidence:** tldraw DOCUMENTED (`getNotVisibleShapes()` / `getCulledShapes()` as reactive, identity-stable
sets, with selected/edited excluded and a `canCull()` opt-out).

**Why it matters:** The same set is needed by hit testing, marquee, snapping (candidates are filtered to the
viewport), export (to decide nothing), and AI context.

**Approaches:**
- **A. Keep culling inline.** Works; not reusable.
- **B. A derived culled-set**, invalidated by camera and document changes.
- **C. B + a per-object `can_cull` override** for shapes with effects beyond their bounds.

**Decision: TBD — requires architecture review.** Low effort; directly extends Phase 14.

---

### Implication C — debounced zoom

**Evidence:** tldraw DOCUMENTED with the threshold and the rationale.

**Why it matters:** Without it, every LOD rule flickers during a pan/zoom. Spool has no LOD rules yet, but it
has zoom-dependent chrome (grid spacing, dot size) which could flicker.

**Decision: TBD.** Low effort when LOD exists.

---

### Implication D — a spatial index is required by five subsystems

**Evidence:** tldraw DOCUMENTED (culling + bindings index + shape query indexes).

**Spool relevance:** `hit_test` is `objects.iter().rev().find(...)`; `objects_in` is a full filter; both are
O(n). At 10,000 objects (the Phase 14/15 benchmark size) that is measurable.

**Consumers:** culling, hit testing, marquee, snapping candidates, AI context summaries.

**Decision: TBD — requires architecture review.**

---

### Implication E — GPUI's entity model bounds per-object render isolation

**Evidence:** PHASE15's measurement-based conclusion from the vendored GPUI source.

[INFERRED] **This is a finding about the framework, not about Spool's architecture.** It means:
- A paint tree will not by itself make rendering fast in GPUI.
- Faster rendering needs fewer invalidations or coarser entities.
- The right response is **not** to optimise now (the brief says "do not optimize prematurely") but to
  **record the constraint** so it informs the eventual UI-technology decision.

**Decision: record; revisit after the paint tree exists.** [PROPOSED]

## Open questions

1. Is there a paint/scene representation between the document and GPUI?
2. Is rendering headless-capable?
3. Is culling a reusable derived set?
4. Is there a spatial index, and what owns its invalidation?
5. Is there a debounced zoom?
6. What is the LOD policy?
7. How are images resolved at the right scale?
8. Given GPUI's entity-level retention, is a paint tree an export feature rather than a performance feature?

## Sources

- tldraw: `sdk-features/performance.mdx` ⭐⭐⭐, `sdk-features/culling.mdx` ⭐⭐, `sdk-features/shape-indexing.mdx`,
  `sdk-features/store.mdx` (computed caches, query indexes), `sdk-features/shapes.mdx`,
  `sdk-features/geometry.mdx`, `sdk-features/ticks.mdx`, `docs/ai.mdx` (export for verification)
- Figma: "Figma Rendering: Powered by WebGPU" (Sep 2025) — https://www.figma.com/blog/figma-rendering-powered-by-webgpu/;
  "Keeping Figma Fast" (Aug 2023) — https://www.figma.com/blog/keeping-figma-fast/;
  "Building a professional design tool on the web" (Dec 2015);
  "A Tour of the C++ Engine which Powers That Design MMO" (conference talk) — THIRD-PARTY;
  Cornell CS 5152 lab (`.fig` as a scene-graph format) — THIRD-PARTY
- Spool prototype: `app/src/canvas.rs` (`CULL_PADDING`, `should_construct`, `px!` macro, grid drawing,
  `CanvasTextPrepaint`, `render_object`, `diagnostics::intersects`), `app/src/diagnostics.rs`,
  `app/PHASE14.md`, `app/PHASE15.md` (GPUI retention analysis, measurements)
