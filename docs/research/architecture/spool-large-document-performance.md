# Spool — Large-Document Performance and Scalability

> **Research and a benchmark design, not an implementation.** It contains no Spool architecture decision.
> Evidence labels as in `spool-html-document-research.md`.

---

## 1. Executive summary

1. **The 10⁶–10⁷ target is 250–2,500× beyond the best-documented comparable editor.** tldraw ships
   `maxShapesPerPage: 4000` (`SOURCE-CODE FACT` via corpus `architecture/rendering.md`, from
   `sdk-features/performance.mdx`). Spool's stated target is 1,000,000–10,000,000. This is the central
   engineering fact of this document and every section below is subordinate to it.
2. **It is nonetheless demonstrably achievable, and natively.** Figma's renderer is C++ compiled to
   **native x64/arm64** as well as WASM, over an integrated **Dawn** stack shared by web and native
   (`ENGINEERING DISCLOSURE`, §16.2 of the research file). A native Rust renderer is architecturally the
   same kind of artefact.
3. **The scale answer is chunked persistent storage + streaming, not a faster parser.** Unreal's World
   Partition is the shipping precedent (`OFFICIAL DOCUMENTATION`, §3), and it solves *I/O*; everything else
   in the pipeline is a rendering problem.
4. **Cascading style is the one place where the document format changes the complexity class.** Blink
   conservatively over-approximates invalidation and documents upward-dependent selector changes as
   *immediate, possibly unnecessary* invalidations (`SOURCE-CODE FACT`). At 10⁶ nodes, a design where any
   class change can invalidate an unbounded subtree is not viable without an ownership convention.
5. **Startup is a storage problem, and the only durable fix is a derived cache.** The brief's stated pain —
   "open huge file → wait forever → editor becomes usable" — repeats on every open, so it must be solved by
   something that persists *between* opens. That is `Option E` in File 2.
6. **A benchmark must be designed before any of this is believed.** §11 specifies one, at 10 → 10⁷ objects,
   with 18 measured quantities and explicit budgets.

---

## 2. Scope and the target

**Target.** A Spool document with 10⁶ objects must open to an interactive, navigable, hit-testable state
quickly; 10⁷ must not crash and must degrade gracefully (see §9).

**What "object" means** (`INFERENCE` — this must be fixed before benchmarking or the numbers are
meaningless):

| Tier | Definition | Example |
|---|---|---|
| **T1 leaf** | A leaf visual object with geometry and style | rectangle, text run, image, SVG node |
| **T2 composite** | An object with children | frame, group, component instance, SVG `<g>` |
| **T3 text-derived** | A text object measured into multiple line boxes | one paragraph → N line boxes |
| **T4 render** | A GPU submission primitive after flattening | instance, quad, path batch |

`INFERENCE` — a "1M object" document means different things at different tiers, and the ratio matters:
a UI-heavy design is ~1:4 T1:T2, while a vector-heavy document is ~1:20. Benchmarks must report the
distribution, and the targets must be stated per tier. §11 does this.

**What is deliberately out of scope.** Render *quality* (LOD thresholds, colour management), export
formats (corpus `creative/export.md`), and Spool's current GPUI prototype rendering (corpus
`architecture/rendering.md` §"Spool prototype"). This document is about *scale*.

---

## 3. The shipping precedent: Unreal's World Partition

`OFFICIAL DOCUMENTATION` — Epic, *World Partition in Unreal Engine* (5.8 docs). Direct quotes:

> "Building large maps used to require developers to manually divide maps into sublevels, then use the Level
> streaming system to load and unload them as the player traversed the landscape. This method often created
> issues sharing files between multiple users, and **viewing the whole world in context became a difficult
> task.**"

> "The World Partition system works by storing your world in **a single persistent Level file** and
> **subdividing the space into streamable grid cells** using a configurable runtime grid. These cells are
> loaded and unloaded at runtime by the presence of **streaming sources**, such as the player."

> "Since Actors are saved to their own individual files using the **One File Per Actor** feature, you do not
> need to check out the Level file from source control to make changes to the Actors in the world. This frees
> up the Level file for others on your team."

Configurable knobs, quoted:

| Knob | Meaning | Spool analogue |
|---|---|---|
| `Cell Size` (e.g. `51200`) | Chunk granularity | Chunk size |
| `Loading Range` | How far ahead a streaming source loads | Viewport margin, in document units |
| `Priority` | Resolves when cells intersect multiple sources | Pinning (selected/hovered objects) |
| `Target State` (`Loaded` / `Activated`) | Two-stage residency | Index-loaded vs. render-ready |
| `Is Spatially Loaded` | Per-actor opt-out of streaming | Per-object "always resident" flag |
| `Enable Streaming` (editor) | Editor-only override | "Load everything" mode for export/print |
| `Data Layers` | Named visibility groups | Page/board sets |
| `HLOD` | Hierarchical Levels of Detail | Reduced representation at low zoom |

### 3.1 The four transferable ideas

`INFERENCE`.

1. **Two-stage residency is not optional.** `Loaded` vs. `Activated` is the difference between "the bytes
   are here" and "the GPU-ready representation is here". Spool needs at least: not-loaded → parsed →
   indexed → render-ready. §5.
2. **Streaming sources have priority and a shape.** A rectangular window with a margin is the shape; priority
   is what lets a selection or a hovered object stay resident across a pan.
3. **Editor override must exist.** You cannot use the streaming runtime to export the whole document, print
   it, or run a whole-document operation. `Enable Streaming` is Epic naming the same problem.
4. **One-file-per-actor is a source-control decision.** Epic's stated motivation is that a shared level
   file serialises multiple people. Spool's analogue is a shared page file. This is the strongest available
   evidence for the brief's multi-file direction.

### 3.2 What UE does *not* give

`INFERENCE`. UE streams *game actors*, which are small, mostly independent, and frequently never need to be
text-diffed. Spool's objects are **nested, style-inheriting, and text-authored**, so:

- chunk boundaries must not split a style-inheriting subtree (§4);
- a chunk is not self-sufficient (it references styles, tokens, assets, components) — so **chunk loading is
  really dependency-graph loading**;
- HLOD in UE is automatic (mesh simplification); in Spool, LOD is a *design* problem, because "what does
  this object look like at 10% zoom" has no canonical answer for a text box.

---

## 4. Chunking — the central structural decision

### 4.1 Chunk boundaries must respect three constraints

`INFERENCE`. A chunk boundary is legal only if all three hold:

| Constraint | Why | Violation symptom |
|---|---|---|
| **C1 — Style closure**: no declaration that affects a chunk's nodes lives outside it, except via an explicit inheritance edge | The cascade is a global read | Nodes render correctly only after unrelated files load |
| **C2 — Identity closure**: every referenced id resolves within the chunk or a declared dependency | Operations address by id | Dangling references; broken undo |
| **C3 — Tree closure**: a chunk boundary never falls inside a subtree that must move as a unit | Structural operations | "Move" splits across files |

### 4.2 The C1 problem, and why it forces a convention

`INFERENCE`. C1 is the hard one and it is a direct consequence of the cascade (research file §19). A
declaration in `styles/shared.css` can affect any node in any chunk. There are exactly three resolutions,
and this document does not choose between them:

| Resolution | Mechanism | Cost |
|---|---|---|
| **Scope** | `@scope` / container queries / naming discipline so styles are chunk-local | Requires author discipline or a linter; constrains what CSS Spool can use |
| **Compile** | Build a per-chunk effective-cascade at parse time and store it in the cache (Option E) | Cache must be invalidated correctly; the engine still has to exist |
| **Global** | Accept that a shared stylesheet is global, and pay the invalidation cost | The cascade is the dominant cost at 10⁶ |

`PROPOSED SPOOL DESIGN` — **chunk-local styles by default, with a small, explicit, declared global surface
(tokens).** This makes C1 hold structurally and confines the cascade to two scopes: chunk-local and
`:root`/token. The cost is that it constrains the document dialect, and that constraint is exactly the
"Spool layer" of File 2's Option C. **This is the strongest link between the format question and the scale
question, and it is easy to miss.**

### 4.3 Recommended chunk topology

`PROPOSED SPOOL DESIGN`.

```
.spool/
├── manifest.json          chunk table, dependency edges, index hashes, cascade order
├── pages/<page>.html      one chunk per page/board        ← load unit, edit unit, merge unit
├── components/<c>.html    one chunk per component
├── styles/tokens.css      global by design (the declared exception)
├── styles/<chunk>.css     co-located with its chunk
└── assets/                content-addressed, separately streamed
```

`INFERENCE` — three properties this buys: (a) a chunk's HTML and CSS are **co-located**, so C1 is local by
construction; (b) a page chunk is the **edit and merge unit**, so an agent's edit and a human's edit land in
different files; (c) **tokens are the only global**, so their invalidation blast radius is known and bounded
(they affect colour/spacing but not geometry or structure).

---

## 5. Persistent vs runtime vs GPU — the three representations

`INFERENCE` — this is the answer to the brief's Q8, stated as three tiers with explicit ownership.

| Tier | Contains | Lifetime | Rebuilt from | Never |
|---|---|---|---|---|
| **Persistent** | HTML/CSS/SVG text, manifest, asset blobs | forever | — | — |
| **Document runtime** | Node table, style table, resolved cascade for loaded chunks, layout boxes, geometry, spatial index, hierarchy, dependency graph | session | persistent, chunk by chunk | serialised to source |
| **GPU** | Instanced buffers, texture atlas, uniform blocks, instance ranges | frame-group | document runtime | treated as authoritative |

### 5.1 What must never be serialised

`PROPOSED SPOOL DESIGN`.

| Never persisted | Why |
|---|---|
| Computed/cascade result | Derived; would freeze the cascade into the source |
| Layout boxes | Derived; depends on font availability, which is machine state |
| Spatial index | Derived; rebuildable in O(n log n) |
| Text line boxes | Derived; depends on font metrics |
| Asset thumbnails, mips | Derived |
| Anything about selection, camera, tool, hover | **Constraint K1** — runtime state |

### 5.2 Determinism is the cache-safety precondition

`PROPOSED SPOOL DESIGN` — the cache is safe **iff** it is deterministic and content-addressed. Concretely:
same persistent bytes + same manifest + same asset set ⇒ byte-identical cache. That requires: no hash-map
iteration order in output, no timestamps, no absolute paths, no floating-point-format variation across
runs. Each of these is individually easy and collectively the entire risk. This is **gate G6** in File 2.

---

## 6. Startup — the brief's explicit product goal

The pain to eliminate: *open huge file → wait forever → editor becomes usable*.

### 6.1 The only structural fix that survives across opens

`INFERENCE`. Anything recomputed from scratch each open costs the same each open. The only way open-time
cost decreases over a user's session history is to **persist the derived work**. That is Option E, and it is
why §5.2's determinism requirement exists.

### 6.2 The startup sequence

```
t0  process start, manifest read (small: chunks, hashes, dependency edges)
t1  chunk table in memory; nothing else                       → app is interactive
t2  visible chunks: read → tree-sitter parse → style → layout  → first paint
t3  asset decode for visible chunks (thread pool)
t4  spatial index shard for visible chunks → hit-testing live
t5  cache validation for the rest; stream-parse in the background
t6  full document resident; all operations available
```

| Stage | Target budget | What it gates |
|---|---|---|
| t0 | < 20 ms | Window appears |
| t1 | < 50 ms | Interactive chrome, camera works |
| t2 | **< 500 ms** | First pixels |
| t4 | < 200 ms after t2 | Click, select, marquee |
| t6 | < 30 s (10⁶) / < 5 min (10⁷) | Whole-document ops |

`INFERENCE` — the 500 ms first-paint target is the brief's own framing ("open huge file → wait forever")
restated as a number. It is achievable at 10⁶ **only** if t2 touches a bounded number of chunks, which is
the direct argument for §4.

### 6.3 Cold vs warm

`INFERENCE`. These are different budgets and conflating them hides the real work:

- **Cold** (no cache): parse + cascade + layout + index from source. At 10⁶ this is the hard case. If the
  cache exists it is *not* cold.
- **Warm** (valid cache): read manifest, validate hashes, memory-map and read. The cache must be
  **hash-validated**, and validation must itself be incremental — checking 10⁶ hashes naively is itself a
  cost. `PROPOSED SPOOL DESIGN`: store a single aggregate root hash per chunk plus a per-chunk hash; validate
  the aggregate first, and only then verify individual chunks.
- **Partially warm** (some chunks invalidated): the common case after an agent edit. Must not re-derive the
  whole document. §7.

### 6.4 Memory mapping

`INFERENCE`. `mmap` is appropriate for the *persistent* text (read-mostly, no copy, OS page cache does the
work) and questionable for the *runtime* table (random access to fixed-size structs across a mapped file
risks page faults and unaligned reads; an arena is usually faster). Rule of thumb: mmap the text, `mmap`
the cache for validation-then-copy, arena the runtime.

### 6.5 What the browser does, and why it is not the model

`OFFICIAL DOCUMENTATION` — web.dev, *How large DOM sizes affect interactivity*:

> "CSS offers the `content-visibility` property, which is effectively a way to **lazily render off-screen DOM
> elements**."

`INFERENCE` — the web platform needed a dedicated CSS property to approximate what UE gets from World
Partition. That is strong evidence that Spool must build the streaming layer natively, and that
"Spool will just use the browser's lazy rendering" is not available as a shortcut.

---

## 7. External edits and incremental reparse at scale

### 7.1 The mechanism

`SOURCE-CODE FACT` — tree-sitter `api.h`: `ts_tree_edit` → `ts_parser_parse(old_tree)` →
`ts_tree_get_changed_ranges(old, new)`, whose doc comment states:

> "Characters outside these ranges have identical ancestor nodes in both trees."
> "the returned ranges may be slightly larger than the exact changed areas, but Tree-sitter attempts to make
> them as small as possible."

`INFERENCE`. This is the whole incremental-update mechanism, and its weakness is the word *attempts*: a
change near the root of a large file may return a wide range, which then forces wide re-style and re-layout.
§4.3's chunking bounds this, and gate G3 measures it.

### 7.2 The cost model

`INFERENCE`. An external edit's cost is:

```
cost ≈ parse(Δ) + cascade(changed selectors × matched nodes) + layout(dirty subtrees) + reindex(dirty bounds)
```

The cascade term is the only one that can be super-linear, because a selector match count is unbounded.
Blink's approach — statically compiled invalidation sets, four dirty bits per node, and a
`PendingInvalidationsMap` flushed lazily before style read (`SOURCE-CODE FACT`,
`third_party/blink/renderer/core/css/style-invalidation.md`) — is the reference. `INFERENCE` — Spool needs
the same three things (compiled invalidation metadata, per-node dirty bits, deferred pending set) or it
rebuilds too much.

### 7.3 Concurrency

`PROPOSED SPOOL DESIGN`. Parse on a thread pool; never parse the visible chunk on the UI thread (it would
break the t2 budget); coalesce watcher events per path within a debounce window; self-write suppression by
content hash, not by path. File 4 covers the conflict semantics.

---

## 8. Spatial indexing, hit testing, selection

### 8.1 Choice of index

`INFERENCE`, keyed to the workload:

| Workload | Best index | Why |
|---|---|---|
| Hit test (point) | R-tree, quadtree, or uniform grid | BVH is poor for point queries at 2D density spikes |
| Marquee (rect) | R-tree | The defining R-tree case |
| Overlap / occlusion | R-tree with a z-aware traversal | Needs painter's order |
| 10⁶ static-ish objects | **Uniform grid / chunk-local index**, no global structure | Cheapest; UE effectively does this via grid cells |
| Moving 10⁴ objects during a drag | R-tree with bulk reinsert, or a **deferred** index (query the moved set separately, merge at drop) | Avoids per-frame reinsertion |

`PROPOSED SPOOL DESIGN` — the corpus already requires a spatial index for five subsystems (`rendering.md`
Implication D: snapping, culling, hit-testing, marquee, focus). **One index, five consumers** is the same
argument the corpus makes for `ShapeUtil`. Chunk-local indexes merged on demand beat one global index at
10⁶, and they compose with §4's chunking rather than competing with it.

### 8.2 The drag problem specifically

`INFERENCE`. The highest-frequency spatial operation in a design editor is *drag*: every pointer move changes
10²–10⁵ bounds. Three viable strategies:

| Strategy | Cost | Risk |
|---|---|---|
| Reinsert into R-tree per frame | O(k log n) per frame, k = moved | Fine at 10², degrades at 10⁴ |
| **Deferred**: exclude moved from the index during the gesture, test them linearly, reinsert on drop | O(k) per frame | Marqee/drag interplay must special-case the moved set |
| Grid with per-cell bucket swap | O(k) | High churn in dense regions |

`PROPOSED SPOOL DESIGN` — **deferred exclusion**. It is the same idea as tldraw's rule that "culled shapes
remain selectable and exportable" and "selected and edited shapes are never culled" (corpus
`rendering.md`): keep the *active* set out of the index and handle it explicitly. This is a design pattern
with prior art in the corpus, not a novel bet.

### 8.3 Selection at scale

`INFERENCE`. Selection is `K1` runtime state, so it does not belong in the cache or the document. But a
selection of 10⁵ objects must be rendered (outline), transformed (move), and inspected. All three need a
**selection-local acceleration structure** built once at selection time. `PROPOSED SPOOL DESIGN`: building it
is an operation, so it participates in history like any other; discarding it at deselect is not a document
change.

---

## 9. Rendering at scale

### 9.1 The reference implementation is Figma's native build

`ENGINEERING DISCLOSURE` — Figma, *"Figma Rendering: Powered by WebGPU"* (Sep 2025), second-hand via
techfeed.io (the official page's body did not render in retrieval):

| Technique | Mechanism | Spool analogue |
|---|---|---|
| **Native build** | C++ → x64/arm64 native, *and* C++ → WASM via Emscripten | Rust native; same source shape |
| **Shared foundation** | Integrated **Dawn** (Chromium's WebGPU) for both web and native | A GPU abstraction with one backend |
| **Explicit-argument draws** | `context->draw(vertexBuffer, framebuffer, {texture}, material, …)` replacing global-state binding | No global render state; every resource passed explicitly |
| **Batched uniform upload** | `encodeDraw(...)` × N, then one `submit()` | Per-frame uniform arena |
| **GPU fallback** | Dynamic WebGPU → WebGL on device loss (Windows driver faults) | Required; see §9.4 |
| **Compute shaders** | Disclosed roadmap for blur | Blur/backdrop-filter at scale |
| **RenderBundles** | Disclosed roadmap to cut CPU overhead | Pre-recorded draw sequences per material |

### 9.2 What the renderer must do at 10⁶

`INFERENCE` — the pipeline, with the technique that bounds each stage:

```
visible set (spatial query)          → O(log n + k)
LOD selection                        → O(k)
flatten to render primitives         → O(k), cached per object, invalidated on change
GPU buffer update (delta, not full)  → O(changed)
draw submission (instanced)          → O(primitives), batched per material
```

`INFERENCE` — the two operations that must **never** be O(document):

1. **Buffer upload must be delta-based.** Re-uploading a 10⁶-instance buffer per frame is 10⁶ × stride
   bytes; even at 32 bytes that is 32 MB/frame. Figma's `encodeDraw`/`submit` batching and WebGPU's
   RenderBundles are both answers to this.
2. **LOD must be zoom-stable.** The corpus already records tldraw's `getEfficientZoomLevel()`, which returns
   a stable value during camera motion, precisely so LOD does not thrash. Any LOD keyed directly to raw
   zoom flickers on every wheel tick. `PROPOSED SPOOL DESIGN`: adopt the stability rule; it is documented,
   battle-tested, and free.

### 9.3 Dirty regions

`INFERENCE` — with a scene graph (corpus `rendering.md` Implication A) dirty regions are a natural
by-product: a changed node invalidates its own paint subtree. The complication specific to Spool is
**invalidation from outside the subtree** — a token change or a shared-class edit repaints everything using
it. That is the cascade cost again (§4.2, §7.2), arriving at the renderer. `PROPOSED SPOOL DESIGN`: the
dependency graph used for style invalidation should be *the same graph* the renderer consults, so one change
produces one invalidation set that both subsystems consume. One mechanism, two consumers — the corpus's
recurring strongest pattern.

### 9.4 Device loss

`INFERENCE` — Figma shipped dynamic WebGPU→WebGL fallback after Windows device loss was observed in the
field (`ENGINEERING DISCLOSURE`, §9.1 table). At Spool's performance tier this is not hypothetical. GPUI's
surface behaviour on device loss is not documented anywhere in the corpus. `PROPOSED SPOOL DESIGN`: treat
"GPU device lost" as a first-class recoverable state with a defined degraded mode, and measure it in the
benchmark (§11.6).

### 9.5 Memory

`INFERENCE` — the budget that actually matters, per object, at 10⁶:

| Structure | Bytes/object (estimate) | 10⁶ total |
|---|---|---|
| Persistent text (incl. ~30 B `data-spool-id`) | 100–400 | 100–400 MB |
| Runtime node record (id, bounds, z, style handle) | 64–128 | 64–128 MB |
| Resolved style (shared/interned) | ~0 amortised | small |
| Geometry cache | 32–256 | 32–256 MB |
| Spatial index | 32–64 | 32–64 MB |
| GPU instance buffer | 32–64 | 32–64 MB |
| Text line boxes (if any object has many) | unbounded | the largest wildcard |

`INFERENCE` — the last row is the wildcard: **text-heavy documents can dominate everything else combined**.
Benchmark plans that use rectangles only will produce entirely misleading numbers. §11 requires a text-bearing
fixture profile.

---

## 10. Streaming, caching, chunking — consolidated

| Technique | Required at 10⁶ | Evidence |
|---|---|---|
| Chunked persistent storage | **Yes** | UE World Partition; research §10 |
| Two-stage residency (parsed / render-ready) | **Yes** | UE `Loaded` / `Activated` |
| Streaming with priority + viewport margin | **Yes** | UE streaming sources |
| Persistent content-addressed cache | **Yes** | §6.1; Option E |
| Lazy + background parse | **Yes** | §6.2 |
| Editor "load everything" mode | **Yes** | UE `Enable Streaming` |
| LOD with zoom stability | **Yes** | tldraw `getEfficientZoomLevel()` |
| Per-chunk dependency graph | **Yes** | lightningcss `analyze_dependencies` |
| Budgets (nodes/bytes/time per project) | **Yes** | research §23.3 (usvg has none) |

---

## 11. Benchmark plan

**Design only.** Nothing is implemented here. The purpose is that when the numbers eventually exist, they
are comparable and they test the claims this document makes.

### 11.1 Fixture profiles

Four profiles, because rectangles-only is misleading (§9.5):

| Profile | Composition | Why |
|---|---|---|
| **P1 flat** | 10⁶ leaf rectangles, no children, one flat list, one class | Isolates index/render throughput |
| **P2 nested** | 10⁶ nodes as 10⁴ frames × 100 children | Hierarchy, transforms, tree operations |
| **P3 styled** | 10⁵ nodes, 10⁴ rules, heavy selector use, custom properties | **The cascade profile** — the one that finds the real ceiling |
| **P4 text/vector** | 10⁵ objects with text runs and SVG subtrees | Tests the wildcard row in §9.5 |
| **P5 imported** | A real imported website | End-to-end realism |

### 11.2 Sizes

`10 · 10² · 10³ · 10⁴ · 10⁵ · 10⁶ · 10⁷` objects. **10⁷ is measured for memory and load, and for render only
in the visible-region case** — a 10⁷-instance full-document render is not a target.

### 11.3 Measured quantities and budgets

| # | Quantity | Measure | Budget (10⁵) | Budget (10⁶) | Notes |
|---|---|---|---|---|---|
| 1 | Cold startup → interactive | t0→t1 | < 100 ms | < 200 ms | Manifest only |
| 2 | Cold startup → first paint | t0→t2 | < 300 ms | **< 500 ms** | The brief's headline |
| 3 | Warm startup → first paint | t0→t2 + cache | < 100 ms | < 200 ms | Cache valid |
| 4 | Full load | t0→t6 | < 3 s | < 30 s | All chunks resident |
| 5 | Parse throughput | MB/s, nodes/s | ≥ 100 MB/s | ≥ 100 MB/s | tree-sitter |
| 6 | Incremental reparse | ms for a 1-node change in a 10⁶-node file | < 5 ms | **< 20 ms** | Gate G3 |
| 7 | Changed-range width | bytes returned for a 1-node change | < 4 KB | **< 64 KB** | Gate G3 |
| 8 | Cascade recompute | ms after a class change | < 10 ms | **< 100 ms** | The cascade ceiling |
| 9 | Memory (resident) | MB, per profile | < 2 GB | **< 24 GB** | §9.5 table |
| 10 | Pan / zoom | frame time, p95 | < 16 ms | **< 16 ms** | Visible-region only |
| 11 | Hit test | µs, p95 | < 20 µs | < 50 µs | |
| 12 | Marquee select | ms for 10⁵ hits | < 50 ms | < 200 ms | |
| 13 | Move (drag) | frame time while dragging 10⁴ objects | < 16 ms | < 16 ms | Deferred-exclusion strategy |
| 14 | Resize | frame time | < 16 ms | < 16 ms | |
| 15 | Text editing | caret latency per keystroke | < 16 ms | < 16 ms | P4 only |
| 16 | Style edit → paint | ms end to end | < 100 ms | < 300 ms | The visual-editing loop |
| 17 | Save | ms + bytes written | < 500 ms, minimal diff | < 5 s, minimal diff | Assert byte-identical outside splices |
| 18 | External file update | ms to visible pixels | < 100 ms | < 500 ms | Watcher → paint |
| 19 | Undo / redo | ms | < 16 ms | < 16 ms | |
| 20 | Export (whole doc) | s | < 10 s | < 5 min | Requires load-everything mode |
| 21 | Agent context extraction | ms for a subtree | < 50 ms | < 200 ms | File 4 |

### 11.4 Method requirements

1. **Report the object-tier distribution** (§2) with every number. A "10⁶" number without composition is
   not a result.
2. **Report p50/p95/p99, never mean.** Frame time means are the classic way to hide a stutter.
3. **Report peak RSS**, not steady-state.
4. **Warm up**, and discard the first run; first-run numbers are JIT/cache-fill artefacts.
5. **Assert minimality**, not just speed, for 17: save must be byte-identical outside the spliced ranges.
   A save that rewrites a file fails the benchmark regardless of how fast it was.
6. **Run each configuration at least 5 times** and report variance. A 10⁷ measurement is expensive.
7. **Machine profile recorded**: CPU, RAM, GPU, OS, filesystem (SSD vs. network), because 4 and 9 are
   storage-bound.

### 11.5 The three benchmarks that would falsify the plan

`INFERENCE`. Stated so they can be run early and cheaply:

| Falsifier | What it proves | Budget |
|---|---|---|
| **B-cascade**: P3 at 10⁵ with 10⁴ rules; change one class | Whether the cascade is viable at all | < 100 ms recompute |
| **B-reparse**: 10⁶-node file; change one attribute | Whether incremental reparse holds at scale | < 20 ms, < 64 KB changed |
| **B-cache**: 10⁶ nodes; cold vs warm startup | Whether the cache earns its complexity | warm < 40% of cold |

### 11.6 A benchmark the corpus would not have thought to add

`PROPOSED SPOOL DESIGN` — **device-loss injection**: force a GPU device loss mid-gesture and measure
recovery. Figma shipped fallback because this happened in production (§9.1). Nobody benchmarks it. Spool
should, because it is the failure mode most likely to reach a user as a crash.

---

## 12. Anti-patterns to avoid

`INFERENCE`, drawn from the evidence rather than invented.

| Anti-pattern | Why it is wrong | Evidence |
|---|---|---|
| "Use the browser DOM and `content-visibility`" | Approximates UE with a CSS property; still requires a browser engine in a native app | web.dev; §6.5 |
| "Parse faster" | Parse is not the bottleneck; cascade, layout and draw submission are | §7.2, §9.2 |
| "One global R-tree for 10⁶ objects" | Constant factors and reinsertion cost; chunk-local + merge wins | §8.1 |
| "Re-upload the instance buffer every frame" | 32 MB/frame at 10⁶ | §9.2 |
| "LOD keyed to raw zoom" | Flickers on every wheel tick | tldraw's zoom-stability rule |
| "Cache without a determinism proof" | A non-deterministic cache is a correctness hazard | §5.2; gate G6 |
| "Benchmark rectangles only" | Text-heavy documents dominate memory; the result is meaningless | §9.5 |
| "Treat 10⁷ like 10⁶" | 10⁷ is a load-and-stream target, not a full-render target | §11.2 |
| "Make the doc one file for simplicity" | Concedes merge and agent-context immediately | research §10, §12.2 |

---

## 13. Open questions

1. **What is the real cascade ceiling on hardware Spool targets?** No published measurement of a
   design-editor-scale CSS engine exists that this research found.
2. **How much of the 10⁶ budget should go to text?** The wildcard row in §9.5 may dominate, and it depends
   entirely on font-shaping cost, which is unmeasured here.
3. **Does GPUI's retained-dispatch model survive 10⁶ entities?** Corpus gap **G8** remains open and this
   research did not close it. It is a framework-level constraint that could invalidate the whole renderer
   plan.
4. **What does chunk size trade off?** Smaller chunks ⇒ better merge and context, worse per-chunk overhead and
   more dependency edges. Gate G3 constrains it but does not answer it.
5. **Is `mmap` actually faster than read+parse for the persistent text?** Plausible, unmeasured.
6. **What is the device-loss behaviour of GPUI's GPU backend?** Unknown; §11.6 exists because of it.
7. **Does the cache survive an OS/browser font change?** Fonts are machine state that affects layout, so the
   cache key must include font identity — or layout caches are silently wrong across machines. `INFERENCE`:
   this is easy to get wrong and is worth a specific test.

---

## 14. Evidence gaps

| Gap | Why it matters | Method |
|---|---|---|
| **Cascade ceiling (G2/B-cascade)** | Largest scale risk; gates the whole architecture | Synthetic 10⁴-rule stylesheet over 10⁵ nodes |
| **CST scale (G3/B-reparse)** | Sets chunk size, which sets everything | tree-sitter at 1–100 MB |
| **Text cost** | May dominate memory | P4 profile with real fonts |
| **Cache determinism (G6)** | Cache correctness precondition | 100 runs, varying hash seeds, byte-compare |
| **GPUI at 10⁶ entities** | Corpus G8 | Local experiment on the vendored source |
| **Font-identity cache keying** | Silent layout corruption across machines | Cross-machine cache reuse test |
| **Real GPU numbers** | Every budget in §11.3 is a hypothesis | Implement the harness before committing to the format |

---

## 15. References

Evidence is cited in `spool-html-document-research.md` §28 and not repeated. This file adds:

1. Epic Games, *World Partition in Unreal Engine* — https://dev.epicgames.com/documentation/en-us/unreal-engine/world-partition-in-unreal-engine — primary source for §3, §4, §10.
2. web.dev, *How large DOM sizes affect interactivity, and what you can do about it* — https://web.dev/articles/dom-size-and-interactivity — primary source for §6.5, §12.
3. Chromium Blink, *CSS Style Invalidation in Blink* — https://chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/core/css/style-invalidation.md — primary source for §7.2.
4. tree-sitter `lib/include/tree_sitter/api.h` — primary source for §7.1.
5. Figma, *Figma Rendering: Powered by WebGPU* (Sep 2025), via https://techfeed.io/entries/68d31867d0e17253abfbe736 — second-hand; primary page body did not render.
6. `docs/research/architecture/rendering.md` (existing Spool corpus, read-only) — tldraw `maxShapesPerPage`, `getEfficientZoomLevel()`, culling, LOD, spatial-index requirement, GPUI retention gap (G8).
7. `docs/research/architecture/spool-html-document-research.md` — §5, §7, §10, §12, §19, §22.
8. `docs/research/architecture/spool-html-document-architecture.md` — Option E (§5.2, gate G6).
