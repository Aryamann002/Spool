# Spool — HTML/CSS as Canonical Document Representation: Evidence Research

> **This is research, not a design.** It contains no Spool architecture decision. Where the evidence points,
> the point is recorded as an implication and the decision is deferred.
>
> It is also written to be able to conclude the hypothesis is **bad**. It does.

---

## Evidence labels used in this document

| Label | Meaning |
|---|---|
| `SOURCE-CODE FACT` | Read directly from a source file at a pinned revision; file and line given |
| `OFFICIAL DOCUMENTATION` | Specification or official project documentation |
| `ENGINEERING DISCLOSURE` | A vendor's own account of its internal implementation |
| `ACADEMIC RESEARCH` | Peer-reviewed or preprint literature |
| `OBSERVED BEHAVIOR` | Measured or directly observed behaviour |
| `INFERENCE` | My reading of the above. Not stated by any source |
| `PROPOSED SPOOL DESIGN` | A shape I am proposing for discussion. Not evidence |

**Headline claim, stated up front.** HTML/CSS is **viable as a persistent *text* representation** and
**viable as an *authoring interchange* format**, on strong primary-source evidence. It is **not established**
as viable as Spool's *runtime* representation, and the specific mechanism a visual editor depends on —
determining what caused a pixel and editing exactly that — has **no off-the-shelf Rust solution** and is
the highest-risk part of the hypothesis. The dominant architectural risk is **not** the format. It is that
**cascading, non-local styling is semantically incompatible with deterministic direct manipulation**, and
the cost of escaping that is a proprietary layer larger than a "minimal Spool metadata" layer usually
assumes. §7 and §12 are the load-bearing sections.

---

## 1. Executive summary

Ten claims, each traceable to a numbered section.

1. **HTML's hierarchy, naming, and text are genuinely free.** DOM nesting, `id`, `class`, `alt`, `aria-*`,
   custom `data-*` attributes, and inline SVG are all shipped, specified, tooled, and already understood by
   both humans and models. §4. `SOURCE-CODE FACT` + `OFFICIAL DOCUMENTATION`.
2. **CSS expresses roughly 80% of a modern design-editor's visual vocabulary natively** — gradients, filters,
   blend modes, masks, clip paths, transforms, container queries, custom properties. But the four things a
   design editor treats as first-class — **constraints, vector paths, blend-group compositing, and
   prototyping** — are not CSS concepts. §5, §6, §7. `OFFICIAL DOCUMENTATION`.
3. **Source-preserving HTML/CSS editing in Rust is solved, and not by the parsers most people would reach
   for.** tree-sitter's `ts_tree_edit` + `ts_parser_parse(old_tree)` + `ts_tree_get_changed_ranges` give
   exact incremental reparse and precise change ranges. html5ever **cannot** be used for this at all: its
   `TreeSink` receives a **line number and nothing else**. §9. `SOURCE-CODE FACT`.
4. **"What caused this pixel?" requires a full CSS engine, and Chrome needed 13 result channels to answer
   it.** `CSS.getMatchedStylesForNode` returns inline styles, attribute styles, matched rules, pseudo
   elements, an **inherited ancestor chain**, keyframes, position-try rules, `@property` registrations, and
   more. `CSSComputedStyleProperty` — the actual computed value — carries **only `name` and `value`, with no
   source location at all**. §12. `SOURCE-CODE FACT`.
5. **Cascade blast radius is unbounded, and the browser's own solution is conservative
   over-approximation.** Blink compiles CSS into invalidation sets, keeps four dirty bits per node, and
   explicitly documents that it "err[s] on the side of correctness" and that upward-dependent selector
   changes become *immediate, possibly unnecessary* invalidations. §12. `SOURCE-CODE FACT`.
6. **HTML's text representation wins on Git mergeability for a specific, nameable reason.** JSON has no
   trailing delimiter, so appending a key *necessarily rewrites the previous last line*; two concurrent
   appends conflict every time. CSS's declaration-per-line layout does not have this failure mode.
   §12, §22. `OBSERVED BEHAVIOR`.
7. **A million-object target rules out using a browser DOM as the runtime, decisively.** The most
   architecturally sophisticated editor in the existing Spool corpus ships `maxShapesPerPage: 4000`.
   Lighthouse treats >1,400 DOM nodes as "excessive". No browser ships a design editor at 10⁷ objects.
   §15, §16. `OBSERVED BEHAVIOR` + `OFFICIAL DOCUMENTATION`.
8. **But 10⁶–10⁷ object documents are achievable natively, and there is direct evidence.** Figma's renderer
   is C++ compiled both to WASM (browser) and **native x64/arm64**, over an integrated **Dawn** (WebGPU)
   stack shared between web and native; Unreal's World Partition streams a single persistent level
   subdivided into grid cells with **one file per actor**, so actors never contend for the level file in
   source control. §16. `ENGINEERING DISCLOSURE` + `OFFICIAL DOCUMENTATION`.
9. **Website import is structurally lossy in a way that is not currently documented anywhere.** Browser
   DOMs are produced by *script*, so `getOuterHTML` returns a DOM whose `style` attributes are a
   serialisation of a computed result, not authored source. Import is a **normalisation** problem, not a
   parsing problem. §14. `INFERENCE` over `OFFICIAL DOCUMENTATION`.
10. **Security is tractable in Rust but has one sharp default.** usvg is `#![forbid(unsafe_code)]`, drops
    script/animation/events by construction, and detects recursive elements — but its **default image
    resolver allows local file access**. Opening an untrusted SVG with library defaults reads local files.
    §24. `SOURCE-CODE FACT`.

**Verdict shape (INFERENCE).** The evidence supports a *specific* architecture and contradicts a different
one. It supports: **HTML/CSS as the persistent source of truth, stored as source-preserving CSTs, with a
Rust-native runtime that never uses a browser DOM, and a bounded, explicitly-scoped Spool layer for identity
and for style ownership.** It contradicts: **"HTML/CSS is canonical, therefore the runtime can be CSS's
semantics."** The runtime cannot be CSS's semantics, because CSS's semantics are a cascade and a design
editor's semantics are direct assignment. Someone must own that translation, and the honest finding is that
this *is* the proprietary layer — it is just smaller than a document schema.

---

## 2. Research scope

**In scope.** Whether HTML + CSS (+ SVG) can serve as the persistent, structured, editable document
representation of a general-purpose visual design editor; the mechanism for source-preserving bidirectional
editing; the cascade/provenance problem; scalability to 10⁶–10⁷ objects; startup; external-file editing;
agent editing; identity; security; and the format comparison the brief requires.

**Out of scope, deliberately.** Figma/Affinity/Canva internal architecture beyond what is already in
`docs/research/`; interaction profiles; typography detail; export formats (already covered in
`docs/research/creative/export.md`); rendering internals of Spool's existing prototype beyond what bears on
the runtime-vs-source question. No visual/browser-browsing was performed; the image budget was not spent.

**Corpus overlap check.** The existing Spool corpus contains **no** HTML/CSS document-model research. A
case-insensitive count across all 52 files finds HTML mentioned 4× maximum in any single file, and every
occurrence is in a *developer-handoff export* context (`creative/export.md`, `ai/generation.md`,
`products/figma.md`). `architecture/document-model.md` derives its 26 concepts purely from the four
products, none of which uses HTML as a document. This is a genuine gap, not a restatement. Corpus files
consulted and **not modified**: `README.md`, `architecture/{document-model,editor-runtime,rendering,
history,ai-runtime,input-system,interaction-runtime,profiles}.md`, `documents` under `document/`, and the
`ai/` set.

**Prior finding that constrains this work.** `docs/research/architecture/tldraw-history-source.md`
(§Important Edge Cases) establishes that **selection is recorded but not replayed**, and that **cancel is
destructive bail, not suppressed recording**. Spool's stated history direction is the opposite: eager
interaction commit, cancel creates no entry. Where this document discusses history (§22) it assumes Spool's
stated direction and does not revisit it.

---

## 3. Existing Spool architecture assumptions (context, not questions)

These are treated as fixed inputs. This research does not reopen them.

| # | Assumption | Status |
|---|---|---|
| A1 | Persistent document state vs. editor runtime state are **type-separated** | Settled |
| A2 | Selection and camera are **not** persistent document content | Settled |
| A3 | A typed **semantic operation** system is the single mutation path, reached identically from human input, AI/MCP, keyboard/menu actions, and plugins | Settled direction, implementation open |
| A4 | History: eager interaction commit, transient mutation during interaction, one semantic transaction per successful interaction, cancel restores start state and creates no entry, history is semantic/operation-oriented, **tldraw's capture-time/bail model is not to be copied blindly** | Settled direction |

`SOURCE-CODE FACT` — the current prototype reality, read from `app/src/canvas.rs`:
`Document` (`:431-436`) holds `objects: Vec<DesignObject>`, `next_id`, `next_names`,
`layer_structure_revision` — **no selection, no camera**. `History` (`:425-428`) is
`{ undo: Vec<DocumentCommand>, redo: Vec<DocumentCommand> }` with `record` (`:648-680`) dropping no-op
changes and clearing redo; `undo`/`redo` (`:687-745`) pop one `DocumentCommand` and replay `before`/`after`
fields. `CommandOperation` (`:379-385`) is a closed 5-variant enum: `Geometry`, `Style`, `Text`, `Insert`,
`Delete`. Gestures record at commit (`:1910-1913`, `:1917-1920`). `Interaction::restore`
(`:1007-1019`) rewrites geometry from gesture snapshots and touches **no history at all** — because nothing
has been recorded yet.

**INFERENCE.** The prototype already exhibits the *behavioural* property Spool wants (one undo step per
committed interaction; cancelled gestures leave no entry) via eager commit, and it does so with a **closed
operation enum and no marks**. That is the opposite architectural bet from tldraw's open, diff-capture,
mark-delimited model — and it is the shape an HTML/CSS document would have to plug into. This is a
meaningful asset, not a liability: §19 shows the HTML/CSS question is mostly about the **document**, not
about history.

---

## 4. HTML document capabilities — what HTML gives for free

### 4.1 Natively present, specified, and tooled

`OFFICIAL DOCUMENTATION` — WHATWG HTML Living Standard; MDN Element reference; W3C ARIA.

| Capability | Mechanism | Assessment for a design document |
|---|---|---|
| **Hierarchy** | DOM nesting | **Free and exact.** Arbitrary depth is permitted. |
| **Node identity (authored)** | `id` attribute | Free but **insufficient** — see §23. |
| **Grouping** | wrapper element + `class` | Free. But a `<div>` wrapper is a **presentational** node, and a design editor needs a *clipping/bounds* container, which is a CSS concern. |
| **Semantic naming** | `class`, `data-*`, custom attributes | Free. |
| **Text** | text nodes, `contenteditable` | Free, and semantically correct: text is text. |
| **Images** | `<img src>` + `alt` | Free. Accessibility semantics come free. |
| **Vector graphics** | inline `<svg>` | Free and well-specified (§6). |
| **Links** | `<a href>` | Free. |
| **Accessibility** | `alt`, `aria-*`, `role`, native semantics of `<button>`/`<nav>`/`<main>` | Free, and **better than any proprietary model** — a design editor that emits `<img alt="">` for an image object gets accessibility as a side effect. |
| **Attributes** | arbitrary `data-*` | Free extension point (see §25). |
| **Metadata** | `<meta>`, `<title>`, `lang` | Free for document-level. |
| **Forms** | `<form>`, `<input>`, `<label>` | Free; relevant only if Spool supports interactive design. |

### 4.2 The one structural problem: HTML has no *frame* concept

`INFERENCE`. In a design editor, the dominant container is the **frame**: an element with authored, fixed
bounds, that **clips** its children, and whose children are positioned relative to it. HTML supplies
nesting; it supplies clipping via `overflow: hidden`; it supplies authored bounds via explicit `width`/
`height`. That is three separate mechanisms that must all be applied together to get one design concept.

`SOURCE-CODE FACT` — the existing corpus already established why this is not cosmetic:
`docs/research/architecture/document-model.md` §5 records Figma needing **two** container types because
"hug container cannot clip", and Affinity needing **three** flatten scopes.

`INFERENCE` — the HTML encoding of a Spool frame would be something like:

```html
<div class="spool-frame" style="position: relative; width: 1440px; height: 900px; overflow: hidden">
  …
</div>
```

with the `position/width/height/overflow` quartet repeated per frame. This is *verbose* and *implicitly
coupled* — a user who edits `overflow` breaks the container semantics — but it is **expressible**, and it is
one `class`. The alternative reading is that a frame should be a *tag* (`<spool-frame>`), which buys
readability and costs: unknown-element handling, and the loss of `display:none`/accessibility behaviour that
real tags give you for free.

`PROPOSED SPOOL DESIGN` — record both encodings as candidates in §25 (custom element tag vs. convention
class), and note that this is a decision with a real readability tradeoff, not a detail.

### 4.3 What HTML does *not* give that a design document needs

| Missing | Closest HTML/CSS mechanism | Assessment |
|---|---|---|
| **Frame** | `<div>` + `overflow` + explicit size | Workaround; verbose (§4.2) |
| **Component definition + instance** | `<template>` (`content`), or custom element, or a repeated class | `<template>` is *static SVG template* semantics, not a React-style component with props. See §11. |
| **Component instance with property overrides** | nothing | **No native mechanism.** |
| **Variant / state** | `:hover`, `:focus`, `[data-state]` attribute selectors | Attribute-selector-driven variants are genuinely workable and worth serious consideration. |
| **Prototype connection** | `<a href>` + fragment | Partial; prototyping is multi-state and time-based, links are not. |
| **Layer ordering** | DOM order / `z-index` | Workable. `z-index` is a *local stacking context* concept and does not compose the way a design tool's layer list implies. |
| **Blend group** | `isolation: isolate` on a wrapper | Partial — a real blend group requires the wrapper to establish a stacking context and the children to be its only participants. See §7. |
| **Clip path (arbitrary)** | `clip-path: path(...)` | Supported (§7). |
| **Constraint** | nothing | **No concept in CSS.** See §7. |

---

## 5. CSS capabilities — what CSS gives for free

### 5.1 The capability matrix the brief asks for

Legend: **N** = native CSS concept · **W** = expressible via a documented workaround · **P** = partial
(needs more than one mechanism, or loses information) · **—** = no representation.

| Design capability | HTML/CSS support | Native / workaround / impossible | Problems | Spool implication |
|---|---|---|---|---|
| **Frames** | W | `<div>` + `position:relative` + `width`/`height` + `overflow:hidden` | Four properties must agree; editing any one breaks frame semantics | Frame must be a *class convention* or a *tag*; the editor must own the invariant (§25) |
| **Groups** | W | wrapper element, no bounds of its own | A group has *derived* bounds; an HTML element has none. A wrapper's box is determined by its children, which is closer to "hug" than to "group" | Group vs. frame must remain two distinct types; HTML does not merge them |
| **Arbitrary nesting** | **N** | DOM depth unbounded | Depth is an authoring-quality problem, not a validity one | None |
| **Absolute positioning** | **N** | `position:absolute; inset/left/top` | None | Direct |
| **Relative positioning** | **N** | `position:relative` + offsets | Establishes a containing block; side effect on descendants | Contained |
| **Flexbox** | **N** | `display:flex` | Full-featured; a complete layout engine exists in CSS | A design editor must implement flex *layout* itself to show layout guides, or embed a browser |
| **Grid** | **N** | `display:grid` | Same as flex; two coexisting layout models | Same |
| **Constraints** | — | **Impossible natively.** Nearest: `margin:auto`, flex `align/justify`, and container-query units. None expresses "keep this pinned to the bottom-right and grow the gap" | Figma's constraints are a *responsive rule*; CSS's mechanisms are *layout algorithms* | **Needs a Spool-side convention** or Spool declines constraints (§25) |
| **Responsive layouts** | **N** | media queries, container queries, `@container`, container query units (`cqi`/`cqw`) | Container queries make *component* responsiveness expressible without viewport coupling — a genuine asset | Spool can express responsive design natively; this is a *capability gain* over a pure coordinate model |
| **Typography** | **N** | `font-family/size/weight/line-height/letter-spacing/word-spacing/text-*`, `font-variation-settings` | Variable-font axis handling is a real gap in all four products (corpus `document-model.md`); CSS names the axes directly (`opsz`, `wdth`, `wght`, `slnt`) | **Opportunity**: CSS gives a *better* variable-font model than the corpus found |
| **Fills** | **N** | `background-color`, `background-image`, gradients, `background-clip` | `background-clip:text` is not a Figma-like fill but is close | Direct |
| **Strokes** | **N** | `border`, `outline`, `box-shadow` spread, or SVG `stroke` | CSS has **no stroke geometry**. `border` is four rectangles with one width; an SVG `stroke` is a real stroke | Arbitrary stroke → SVG (§6) |
| **Gradients** | **N** | `linear-gradient`, `radial-gradient`, `conic-gradient`, colour interpolation methods, `color-mix()` | None material | Direct |
| **Shadows** | **N** | `box-shadow` (incl. inset, multiple) | `box-shadow` is a *box* shadow; Figma's drop shadow applies to arbitrary alpha. SVG `feDropShadow` is the general case | Contained |
| **Blur** | **N** | `filter: blur()`, `backdrop-filter` | Filter on a *layer*; backdrop-filter needs a backdrop — no alpha-mask equivalent outside SVG filters | Contained |
| **Masks** | **N** | `mask`/`mask-image`, CSS `mask` shorthand | CSS masks are images/gradients; an **arbitrary vector mask** needs SVG `mask` | Split (§6) |
| **Clipping** | **N** | `overflow:hidden/clip`, `clip-path` | `overflow` clips to the *box* only; arbitrary shapes need `clip-path` | Contained |
| **Transforms** | **N** | `transform: translate/rotate/scale/matrix/skew/3d` | None | Direct |
| **Rotation** | **N** | `transform: rotate()` | 2D and 3D both | Direct |
| **Opacity** | **N** | `opacity`, plus `fill-opacity`/`stroke-opacity` in SVG | None | Direct |
| **Blend modes** | **N** | `mix-blend-mode` + `isolation` | Requires a stacking context on the group; the *group boundary* is not a CSS object (§4.3) | Spool must own blend-group semantics |
| **Vector graphics** | **N** | inline SVG | SVG-in-HTML has **different** paint and layout semantics from CSS boxes | Needs a dual-pipeline renderer (§16) |
| **SVG** | **N** | `<svg>` with viewBox, `<path>`, `<g>`, `<use>`, `<defs>`, filters, gradients | Excellent coverage; `usvg` support is deliberately partial (§6, §24) | Direct, with limits |
| **Images** | **N** | `<img>`, `url()`, `image-set()` | External URL loading is a security surface (§24) | Direct, sandboxed |
| **Video** | W | `<video>`; no CSS-box model participation in layout | A design document's video object is a *poster + frame*, not a playing video | Export-only in most cases |
| **Audio** | W | `<audio>` | Same | Rare in design documents |
| **Reusable components** | P | `<template>` (static SVG template only), custom element, or repeated markup | **No props, no slots, no override.** `<template>` in HTML is a *parser/UA* mechanism for inert content, not a parameterised component | **Needs a Spool-side mechanism** (§25) |
| **Variants** | P | attribute selectors (`.btn[data-variant="ghost"]`) | Genuinely workable; the variant *axis* is authored as attributes | Reasonable; needs convention |
| **States** | **N** | `:hover`, `:focus`, `:focus-visible`, `:active`, `:disabled`, `:checked`; plus arbitrary `[data-*]` | Interaction states are first-class in CSS; **prototype states** are not | Reusable for hover/focus; prototyping still needs Spool |
| **Variables / design tokens** | **N** | Custom properties `--x`, `var()`, `@property` typed registration, `light-dark()` | Custom properties **inherit** and are resolved at computed-value time; they are a genuinely excellent token system with **types** via `@property` | **Opportunity**: better typed than the corpus's variable-model options |
| **Responsive breakpoints** | **N** | `@media`, `@container`, custom media | None material | Direct |
| **Animations** | **N** | `@keyframes`, `animation-*`, `transition-*`, `scroll-timeline` | Full-featured | Direct, and richer than most design tools' prototype animation |
| **Interactions (behaviour)** | P | `:hover`/`:active` + transitions; no scripted behaviour | **No JS-free interaction model** beyond pseudo-classes | Prototype needs Spool or JS |
| **Prototyping** | P | `<a href="#x">` links + transitions | Links model *navigation*, not timed multi-artboard flows | Needs Spool (§7) |
| **Accessibility metadata** | **N** | `alt`, `role`, `aria-*`, native semantics, `<title>` on SVG | **Better than any proprietary model** | Genuine asset |
| **Semantic metadata** | **N** | `data-*`, `<meta>`, `itemscope`/microdata, RDFa | Free extension points | See §25 |
| **Arbitrary visual objects** | W | any element with arbitrary CSS | **No user-defined object type.** CSS cannot express "a new kind of node with its own handles and snapping geometry" | **Needs Spool** (cf. corpus `ShapeUtil` finding) |
| **Effects not naturally web concepts** | — | blend *groups* with per-group compositing, 3D-ish transform hierarchies with perspective on non-3D content, true stroke geometry on arbitrary paths outside SVG, non-destructive boolean ops | Each requires either an SVG subtree or a Spool convention | Biggest single gap (§7) |

### 5.2 The capability gain, stated honestly

`INFERENCE`. Reading the "Native" and "Workaround" columns honestly, the surprising result is that **CSS
covers more of a modern design surface than most design tools' own document models do**. Responsive
breakpoints, container queries, typed design tokens (`@property`), blend modes, backdrop blur, and
transitions are all first-class in CSS and all awkward or absent in a pure coordinate-and-style object model.
The corpus's `document-model.md` lists variables, layout, constraints, and animation as *open Spool
questions*; CSS answers three of those four natively.

`INFERENCE`. This is the strongest **structural** argument for the hypothesis, and it is not a novelty
argument — it is a "you would otherwise have to build it anyway" argument. It is also, precisely, the
argument that is *most likely to be over-read*, because the difficulty is not in the *features*; it is in
the *ownership* of them (§12).

---

## 6. SVG capabilities

`OFFICIAL DOCUMENTATION` — SVG 2, W3C.

SVG supplies, natively: paths with full geometry (Bézier, arcs), fills with `fill-rule`, strokes with
`stroke-width`/`linecap`/`linejoin`/`miter`, gradients (linear/radial/mesh-ish), clipping and masking,
filters (`feGaussianBlur`, `feDropShadow`, `feComposite`, `feColorMatrix`, `feTurbulence`, `feDisplacementMap`),
`transform` hierarchies, `<use>`/`<defs>` reuse, `<symbol>`, viewBox-based scaling, text with
`textPath`, and a real accessibility surface (`<title>`, `role`, `aria-label`).

**Coverage assessment.** SVG is the correct answer for arbitrary vector geometry and for effects CSS cannot
express (true strokes, vector masks, complex filters). It is *not* the right answer for UI-ish layout —
SVG has no flow layout, and SVG-as-DOM does not participate in CSS flex/grid.

`SOURCE-CODE FACT` — usvg (the Rust SVG parser used by resvg, Linebender) documents its own limits:

> "Unsupported SVG features will be ignored"
> "CSS support is minimal"
> "Only *static* SVG features, e.g. no `a`, `view`, `cursor`, `script`, no events and no animations"

(`crates/usvg/src/lib.rs:30-46`)

and what it *does* resolve:

> "Recursive elements will be detected and removed"
> "Markers will be converted into regular elements."
> "`objectBoundingBox` will be replaced with `userSpaceOnUse`."

**INFERENCE.** Two consequences for Spool. First, **animated SVG is out of scope for a usvg-based runtime
pipeline** — an SVG object in a Spool document renders as a static image. Second, "recursive elements will be
detected and removed" is both a **feature** (billion-laughs protection, §24) and a **warning**: it means
round-tripping through usvg is *normalising*, not source-preserving. If Spool's SVG is canonical source, it
must be edited via a CST (tree-sitter has `tree-sitter-svg`), with usvg used only as a *renderer input
normaliser*, never as the on-disk round-tripper.

---

## 7. HTML/CSS limitations, and which ones actually bind

Most "limitations" lists are wrong because they list things CSS does not have while ignoring what CSS has
instead. This section lists only limitations that survive that test.

### 7.1 Binding limitations

**L1 — There is no constraint system.** `OFFICIAL DOCUMENTATION` — no CSS property expresses "when the
container resizes, pin this edge and grow the gap." The nearest mechanisms are `margin: auto`, flex
`align-items`/`justify-content`, and container-query units, each of which encodes *one* specific strategy
rather than a user-chosen responsive rule. Figma's constraints are a first-class, per-child property with
left/right/top/bottom/scale flags. **This is a real product gap, not an encoding gap.** `PROPOSED SPOOL
DESIGN`: either Spool declines constraints (and says so), or it invents a convention that is
Spool-specific — which is proprietary representation, and §25 requires justifying it.

**L2 — There is no blend-group object.** `mix-blend-mode` blends an element with its *backdrop* within its
stacking context; `isolation: isolate` creates that context. But a design-editor blend group is an explicit
object whose membership can be edited, which may be non-contiguous in layer order, and which composites as
a unit before the group composite applies. CSS expresses the *effect* and not the *object*.

**L3 — There is no arbitrary user-defined object type.** `INFERENCE` over the corpus: tldraw's `ShapeUtil`
interface exists precisely so "one new shape type is added once and serves six subsystems" (corpus
`architecture/rendering.md` Implication C, `architecture/document-model.md` Implication C). HTML+CSS has no
registration point for a new type. **Any Spool-only object type is proprietary, by necessity.** This is the
single most important structural limitation and it is not about CSS at all — it is about *extensibility*.

**L4 — Cascade ownership is not optional.** See §12. This is the deepest one.

**L5 — Stroke is not a CSS concept.** Arbitrary-geometry strokes live in SVG (§6). Fine, but it means the
renderer has two paint pipelines (§16).

**L6 — "Authored" vs. "computed" is not representable in CSS itself.** A design editor's inspector must show
*what the user wrote*, not *what the engine resolved*. CSS has no syntax for the former once it has been
computed — `getComputedStyle` returns resolved values, and there is no standard API returning authored
values (DevTools reconstructs them from `getMatchedStylesForNode`, §12). **This limitation is about tooling,
not about the format**, and it is surmountable by keeping the CST.

**L7 — Shorthands are lossy for round-tripping.** `SOURCE-CODE FACT` — CDP exposes `longhandProperties` on
`CSSProperty` ("Parsed longhand components of this property if it is a shorthand"), and lightningcss
actively merges longhands into shorthands during minification (`src/stylesheet.rs:97-103`, and the README's
"Combining longhand properties into shorthands where possible"). **INFERENCE**: a design editor must record
*which* longhand it changed and write that longhand, or the user's `margin: 0` becomes `margin-top: 0`. This
is solvable but is a concrete correctness requirement, and it means the CST's `declaration` node identity is
load-bearing.

**L8 — HTML has no fragment identity that survives moving.** §23.

### 7.2 Non-binding "limitations" (recorded so they are not re-litigated)

| Non-issue | Why |
|---|---|
| "HTML is verbose" | Verbosity is a *readability* cost, not a validity cost. Minification is optional; source is source. |
| "CSS has no variables like Figma" | False. Custom properties + `@property` typed registration + `color-mix()` + `light-dark()` is a *stronger* token system than Figma's 6 typed kinds, because types are enforceable by the UA. |
| "CSS can't do responsive" | False. `@media`, `@container`, and container query units. |
| "CSS animations aren't prototyping" | True but separable — see L-list; CSS covers motion, Spool covers flow. |
| "HTML has no state machine" | False for interaction states (pseudo-classes + attribute selectors). True for prototyping flow. |

---

## 8. Source-preserving editing — the mechanism, precisely

This section answers §Q4 of the brief at the level of *mechanism*, because it is the hinge of the whole
hypothesis. The brief's worked example:

```css
.hero { display: flex; gap: 24px; padding: 40px; }
```
visual edit `gap: 24px → 32px` must produce:

```css
.hero { display: flex; gap: 32px; padding: 40px; }
```

**and nothing else.**

### 8.1 tree-sitter provides exactly the required primitives

`SOURCE-CODE FACT` — `lib/include/tree_sitter/api.h`, tree-sitter (read at HEAD):

```
/**
 * Edit the syntax tree to keep it in sync with source code that has been edited.
 *
 * You must describe the edit both in terms of byte offsets and in terms of
 * (row, column) coordinates.
 */
ts_tree_edit(...)          // api.h:454-463

/**
 * If you are parsing this document for the first time, pass `NULL` for the
 * `old_tree` parameter. Otherwise, if you have already parsed an earlier
 * version of the document and the document has since been edited, pass the
 * previous syntax tree so that the unchanged parts of it can be reused.
 * This will save time and memory.
 */
ts_parser_parse(..., old_tree)   // api.h:284-291

/**
 * Compare an old edited syntax tree to a new syntax tree representing the same
 * document, returning an array of ranges whose syntactic structure has changed.
 *
 * ... Characters outside these ranges have identical ancestor nodes in both trees.
 *
 * Note that the returned ranges may be slightly larger than the exact changed areas,
 * but Tree-sitter attempts to make them as small as possible.
 */
ts_tree_get_changed_ranges(old_tree, new_tree)   // api.h:466-486

ts_node_edit(...)   // api.h:729 — "Edit the node to keep it in-sync with source code that has been edited."
```

**INFERENCE.** These four functions *are* the source-preserving-editing mechanism, and they are exactly
what a visual editor needs:

| Editor need | tree-sitter primitive |
|---|---|
| "Change only `gap`'s value" | `Node::byte_range` on the `value` node → splice those bytes |
| "I edited the file; update my tree without reparsing everything" | `ts_tree_edit` + `ts_parser_parse(old_tree)` |
| "Which parts of the document did the external edit actually change?" | `ts_tree_get_changed_ranges` → **precise invalidation set** |
| "I moved a subtree; shift its recorded offsets" | `ts_node_edit` |

The last row is the one that makes §Q10 (external file editing) tractable: tree-sitter gives you a
**minimal, principled invalidation set for free**, computed by the parser itself rather than guessed.

### 8.2 Round-trip fidelity is a property of the grammar, and it holds

`SOURCE-CODE FACT` — `tree-sitter-html/grammar.js:13-30`:

```js
name: 'html',
extras: $ => [ $.comment, /\s+/ ],
externals: $ => [
  $._start_tag_name, $._script_start_tag_name, $._style_start_tag_name,
  $._end_tag_name, $.erroneous_end_tag_name, '/>',
  $._implicit_end_tag, $.raw_text, $.comment,
],
```

`INFERENCE`. Three properties follow. **Comments are named nodes**, so they survive and can be moved
deliberately. **Whitespace is `extras`**, stored as the padding before each node's start byte, so
byte offsets map 1:1 onto the source and *any* untouched region is reproduced by copying bytes verbatim.
**`raw_text`** covers `<script>` and `<style>` bodies verbatim.

`SOURCE-CODE FACT` — `tree-sitter-html/src/scanner.c` + `tag.h` implement an external scanner with
`IMPLICIT_END_TAG`, void-tag handling (`tag_is_void`, `END_OF_VOID_TAGS`), and an explicit implicit-end set
covering `LI, DT, DD, P, COLGROUP, RB, RT, RP, OPTGROUP, TR, TD, TH, option, …` (`tag.h:349-379`).

**Important qualification.** `INFERENCE` — this is a **pragmatic subset** of the HTML5 tree-construction
algorithm, not the algorithm. It does not implement foster parenting, the full insertion-mode machinery, or
the full fix-up table. For a *design editor authoring its own well-formed output* that is entirely
sufficient and arguably preferable (it does not invent elements the user did not write). For *importing
arbitrary real-world HTML* (§14) it is materially weaker than html5ever, and that tension is real.

`SOURCE-CODE FACT` — `tree-sitter-css/src/node-types.json` confirms the CSS CST has the nodes a design
editor needs: `declaration` (with `property_name`, value variants incl. `plain_value`, `float_value`,
`integer_value`, `string_value`, `color_value`, `grid_value`, `binary_expression`, `call_expression`, and
`important`), `rule_set`, `block`, `selectors`, `class_selector`, `id_selector`, `tag_name`,
`attribute_selector`, `import_statement`, `media_statement`, `supports_statement`, `keyframes_statement`.

**Verdict on §Q4.** `INFERENCE` — **feasible, with a named implementation**: tree-sitter-html +
tree-sitter-css give lossless CSTs, byte-exact splices, incremental reparse, and parser-computed change
ranges. The brief's example is a ~30-line function: locate the element by `id`, resolve the rule, find the
`declaration` whose `property_name` is `gap`, splice its `value` node's byte range. The remaining problem is
**not writing the edit**; it is **deciding which rule to edit** (§12).

### 8.3 What the "obvious" Rust parsers cannot do

`SOURCE-CODE FACT` — html5ever's own README:

> "html5ever uses callbacks to manipulate the DOM, therefore **it does not provide any DOM tree
> representation**."

`SOURCE-CODE FACT` — markup5ever's `TreeSink` trait (`markup5ever/interface/tree_builder.rs:117-298`) method
list, in full: `ns`, `local_name`, `finish`, `parse_error`, `get_document`, `elem_name`, `create_element`,
`create_comment`, `create_pi`, `append`, `append_based_on_parent_node`, `append_doctype_to_document`,
`mark_script_already_started`, `pop`, `get_template_contents`, `same_node`, `set_quirks_mode`,
`append_before_sibling`, `add_attrs_if_missing`, `associate_with_form`, `remove_from_parent`,
`reparent_children`, `is_mathml_annotation_xml_integration_point`, **`set_current_line`**, …

`create_element(&self, name, attrs, flags) -> Handle` — no position. `append(&self, parent, child)` — no
position. The **only** positional information the sink receives is `set_current_line(&self, line_number: u64)`.

`SOURCE-CODE FACT` — same file, `NodeOrText` doc comment: *"Adjacent sibling text nodes are merged into a
single node, so the sink may not want to allocate a `Handle` for each."*

**INFERENCE.** html5ever is **structurally incompatible with source preservation**, and by design: it is a
specification-faithful HTML5 *tree builder*, whose job is to produce the *correct DOM*. Correct DOM requires
inserting implied elements, merging adjacent text, and discarding the distinction between authored and
inherited. It is the right tool for *import correctness* and the wrong tool for *canonical source*. Spool
must not plan to use it as the canonical-source parser. (It remains a legitimate second parser for
import-time normalisation, §14.)

`SOURCE-CODE FACT` — lightningcss (Parcel's Rust CSS toolchain), `src/rules/mod.rs:119-127`:

```rust
pub struct Location {
  /// The index of the source file within the source map.
  pub source_index: u32,
  /// The line number, starting at 0.
  pub line: u32,
  /// The column number within a line, starting at 1 for first the character of the line.
  /// Column numbers are counted in UTF-16 code units.
  pub column: u32,
}
```

and `src/stylesheet.rs:105-120`: `to_css()` returns `pub code: String`.

**INFERENCE**. lightningcss keeps a location, but it is **line + UTF-16 column with no byte offsets, no end
position, and no range** — insufficient to splice a declaration's value. And `to_css()` **re-serialises the
whole stylesheet**, which is precisely the outcome the brief forbids. lightningcss is a *compiler* (minify,
vendor-prefix, lower, CSS Modules, browserslist targets), built for bundlers. Using it as Spool's
on-disk round-tripper would rewrite every file on every save. **Do not.**

---

## 9. AST / CST / parser technologies — evaluation

| Technology | Language | Lossless? | Source ranges? | Incremental? | Round-trip edit? | Verdict for Spool |
|---|---|---|---|---|---|---|
| **tree-sitter-html / -css** | C core, Rust bindings | **Yes** (byte-exact) | **Yes** — `byte_range`, `Point{row,column}` | **Yes** — `ts_tree_edit` + `parse(old_tree)` | **Yes** — splice bytes | **The mechanism.** Only candidate that satisfies the brief's requirement |
| **html5ever / markup5ever / tendril** | Rust | **No** — normalises, merges text | **No** — `set_current_line` only | No | No | **Import normaliser only.** Spec-faithful DOM builder, explicitly not a tree |
| **lightningcss / cssparser / selectors** | Rust | No | Line+column only | No | No (`to_css()` rewrites) | **Value/type engine only.** Excellent typed property values; not a persistence layer |
| **oxc** | Rust | No (printer) | `oxc_span` exists | No (not documented) | **No** — "Today, comments are not printed" | **Value engine for JS/TS if React is ever canonical.** See §11 |
| **scraper / kuchiki** | Rust (html5ever wrappers) | No | No | No | No | Same as html5ever. Inherits its limits |
| **SWC (Rust)** | Rust | No | Yes | Partial | No | Mature; dominated by oxc on speed |
| **Biome** | Rust | No (formatter is canonicalising) | Yes | No | No (formatting is its purpose) | Useful for *lint/format*; formatting destroys author intent |
| **DOM (browser)** | — | No (`outerHTML` is a serialisation) | n/a | n/a | No | **Not a persistence layer.** See §14 |

`INFERENCE` — the ecosystem splits cleanly. **Compiler-grade parsers** (html5ever, lightningcss, oxc, Biome)
produce *semantic* results and normalise. **Editor-grade parsers** (tree-sitter) produce *syntactic* results and
preserve. Spool needs editor-grade for canonical source and can optionally use compiler-grade as a *second*
stage. Assuming one library does both is the most likely first mistake.

`PROPOSED SPOOL DESIGN` — a two-stage pipeline where stage 1 is a CST and stage 2 is a *typed* value model:

```
bytes → tree-sitter CST (lossless, ranges) → typed value model (property → typed value)
      ↑                    ↑                        ↓
      └──── splice ────────┘                  layout / cascade engine (Spool)
```

This gets tree-sitter's preservation *and* lightningcss-style typed property values without either library
having to do both.

---

## 10. HTML/CSS project structures

The brief's Models A–D, assessed on the axes that actually matter.

| | A: one `index.html` | B: html + one css | C: pages/components/styles | D: hybrid chunked |
|---|---|---|---|---|
| **Human diff quality** | Poor (one enormous file) | OK | **Best** | Best |
| **Agent context size** | **Terrible** — cannot send a 50 MB file | Poor | **Best** — targeted `read_file` | Best |
| **Git merge quality** | Terrible | OK | **Best** | Best |
| **Editing locality** | Poor | OK | **Best** | **Best** — chunked by page/component |
| **Parse-on-open cost** | One huge parse | One huge parse | Parallel parses; only `index.html` needed to start | **Best** — manifest first |
| **File-watch granularity** | Worst (any change = full reparse) | Poor | **Good** — per-file | **Best** |
| **Component reuse** | Not possible | Not possible | **Native** | Native |
| **Cyclical-reference risk** | None | None | **Real** — `navbar.html` ↔ `hero.html` | Real, needs cycle detection |
| **Implementation complexity** | Lowest | Low | Medium | **Highest** |

`OFFICIAL DOCUMENTATION` — Unreal's World Partition gives a shipping-engine precedent for the D-shaped
answer, and specifically addresses the source-control dimension:

> "The World Partition system works by storing your world in a single persistent Level file and subdividing
> the space into streamable grid cells using a configurable runtime grid." *(dev.epicgames.com, World
> Partition in Unreal Engine)*

> "Since Actors are saved to their own individual files using the One File Per Actor feature, **you do not
> need to check out the Level file from source control to make changes to the Actors in the world.** This
> frees up the Level file for others on your team." *(same)*

`OBSERVED BEHAVIOR` — the same page documents a `-SCCProvider=(None,Perforce…)` option for world-partition
conversion, i.e. that *source-control integration is an explicit, named concern* even at Epic's scale.

`INFERENCE`. Models A and B should be rejected on agent-context grounds alone: the brief's own requirement is
that an agent must be able to inspect a project *without* reading the whole thing. A single file makes that
impossible. Model C is the minimum viable. Model D's extra machinery — chunking, a manifest, cycle detection —
is exactly the machinery the scalability target (§15, File 3) requires anyway, so the marginal cost of D over
C is lower than it appears.

`PROPOSED SPOOL DESIGN` — record Model D as the target and Model C as the mandatory floor, with the explicit
note that **D's chunk boundaries must be semantic (page / component), not spatial**, so that a human's mental
model and an agent's `read_file` granularity agree.

---

## 11. Single-file vs multi-file

Answered with the table above. Three additional findings:

`INFERENCE`. **Component reuse has a genuine HTML answer that is better than `<template>`**: repeated markup
with a shared `class`, plus a custom element that upgrades the behaviour. `<template>` itself is *not* the
answer — `OFFICIAL DOCUMENTATION` HTML `<template>` is inert parsed content used by the UA (shadow roots,
cloning); it has no parameters. A Spool component system needs either (a) a build-time expansion convention,
(b) a custom element with attributes as props, or (c) a runtime custom-element registry. All three are
Spool-specific, which §25 must justify.

`INFERENCE`. **Module resolution, imports, and `@import` order** introduce a *dependency graph* that HTML/CSS
otherwise does not have, and that graph is the hard part of incremental external editing (§File 3, §Q10):
`@import` in CSS establishes a load-order-sensitive cascade, so "which file do I need to reparse when file X
changed" is a transitive-closure question. `lightningcss` exposes `analyze_dependencies` returning
`Option<Vec<Dependency>>` (`src/stylesheet.rs:116-119`) — evidence that dependency analysis is a real,
separately-solved problem, not a detail.

---

## 12. HTML/CSS vs JSON/XML/SVG/SQLite/hybrid — the comparison the brief requires

### 12.1 Format comparison

Scores are qualitative and evidence-anchored, not a winner.

| Criterion | HTML/CSS | JSON | XML | SVG | SQLite | Hybrid dir (D) |
|---|---|---|---|---|---|---|
| Human readability | **High** — self-describing tags | Med — key names only | Med | High for vectors, poor for docs | **Very low** | High |
| LLM familiarity | **Highest** (INFERENCE — models are trained overwhelmingly on this; see §18) | High | High | High | Low | High |
| Agent editability | **High** — a human-readable diff is a reviewable edit | Med | Med | Med | Low | **High** |
| Deterministic edits | **High** — a declaration's byte range is exact | Med — needs a JSON CST with ranges | Med | High | Med | High |
| Precise patches | **High** | Med | Med | High | Med | High |
| **Git diff quality** | **High** — one declaration per line; appends are pure adds | **Low near boundaries** | Med | High | **N/A (binary)** | High |
| **Mergeability** | High for declaration-level edits; same-rule edits conflict | **Poor** — see §12.2 | Med | High | N/A | High, if chunked by concern |
| Schema evolution | **High** — unknown attributes/elements are tolerated | **Poor** — strict schemas reject; lax schemas drift | Med | Med | Med | High |
| **Parsing speed** | High (tree-sitter: concrete, no error-recovery search) | **Highest** (SIMD-friendly) | High | High | High (native) | High per-chunk |
| **Incremental parsing** | **Yes** — §8.1 | Yes, but needs a CST; most JSON parsers are AST-only | Yes | Yes | Yes | **Yes, per chunk** |
| **Partial loading** | **Yes, per file** — §11 | Poor (one doc) | Poor | Poor | **Yes, by row** | **Yes, per chunk** |
| Partial writing | Yes, per file | Poor | Poor | Yes | **Yes, by row** | Yes, per chunk |
| Random access | Poor without an index | Poor | Poor | Poor | **Excellent** | Needs an index |
| **Validation** | **Parsing-optional; semantics weak** | **Excellent** — JSON Schema | Good — DTD/XSD | XSD | Excellent | Good |
| Extensibility | High (`data-*`, custom elements) | Med | High | Med | Med | High |
| Compatibility | **Universal** — every browser, every tool, every AI | Universal | Universal | Universal | Needs SQLite | Universal |
| Ecosystem | **Largest in existence** | Large | Large | Large | Large | Large |
| **Asset handling** | Native (`img`, `url()`, `<image>`) — **and a security surface** (§24) | Paths only | Paths only | Native | Blobs | Native |
| Performance at 1M+ objects | **Parse: yes. Runtime: only with a native runtime, never a DOM** (§15) | **Excellent** — this is JSON's one decisive win | Good | Good for vectors only | **Excellent** | Excellent, if chunked |

### 12.2 The one place JSON is decisively worse, with the mechanism

`OBSERVED BEHAVIOR` — JSON's merge problem is *positional*, and it has a specific cause:

> "I run into JSON file merge conflicts almost every day, and a shocking percentage of the time it's due to me
> and another person attempting to add a new line at the end of the file… you aren't just adding a value to
> the end of the file—you're also modifying what was previously the final line"
> — *Avoid JSON file merge conflicts*, sophiabits.com, citing the `cal.com` codebase, which works around it
> with a sentinel `"ADD_ABOVE_HERE_TO_AVOID_MERGE_CONFLICTS": ""` line at the bottom.

The quoted diffs are the evidence:

```
3c3,4      ← appending to an object: rewrites the last line
< "valueTwo": "two"
---
> "valueTwo": "two",
> "valueThree": "three"

3a4        ← inserting in the middle: a clean single-line add
> "valueThree": "three",
```

`INFERENCE`. JSON has **no trailing delimiter**, so the closing brace of a container is fused to its last
member. CSS and HTML both have statement-level newlines, so appending is a pure line addition. This is a
real, mechanistic, reproducible advantage for HTML/CSS on the merge axis, and it is *specific* — it does not
generalise to "text is easier".

`INFERENCE` — the honest counterweight: HTML/CSS has its **own** merge hazard that JSON does not. Two people
editing the *same rule* conflict at the same granularity, and moving a rule (which changes cascade order)
conflicts with any edit inside it. A CSS file with 200 rules has a cascade whose meaning depends on order, so
reordering is a semantically-loaded edit. JSON objects are **unordered by spec**, so key order carries no
meaning and cannot be loaded. That is a genuine structural advantage of JSON: *it has no cascade order to
get wrong.*

### 12.3 Persistence comparison, summarised

`INFERENCE`. HTML/CSS wins human/agent/Git axes; JSON wins validation, random access, and load speed; SQLite
wins everything operational and loses everything human. **No format wins all axes**, which is the actual
finding, and it is why File 2 presents five options rather than a verdict.

---

## 13. React / TSX / Tailwind comparison

### 13.1 The five options

| Option | Assessment |
|---|---|
| **A. Canonical source** | **Not supported by evidence.** See below. |
| **B. Import/export target** | **Viable and cheap.** HTML/CSS or any document model → JSX/Tailwind on export. One-way. |
| **C. Optional project mode** | **Plausible, expensive.** A second dialect to keep coherent with A. |
| **D. Runtime representation** | **No.** React is not a representation; it is a program that produces one. |
| **E. Plugin/adapter** | **Viable.** A Tailwind-class-to-design-token compiler, a JSX emitter, a dev-server. |

### 13.2 Why A is not supported by the evidence

Four independent reasons, each evidence-backed.

1. **Source preservation is not available.** `SOURCE-CODE FACT` — oxc's own documentation: *"Today,
   comments are not printed. It will be supported thanks to oxc-parser #13285."* The most modern Rust
   TSX parser does not round-trip comments. A design editor whose canonical source is a file whose comments
   and formatting are destroyed on every save is not source-preserving. (tree-sitter-typescript would
   preserve them — which means TSX-as-canonical means committing to tree-sitter's TSX grammar *plus* an
   entire JS/TS semantic layer, not oxc.)
2. **The value is a function, not data.** `INFERENCE` — `<Button variant="ghost" disabled={x}>` evaluates.
   A visual editor that edits `variant` must know which branch ran, what the props were, and whether the
   render was pure. Every one of those is a JS execution question, which means the editor needs a JS
   runtime to answer "what am I editing". Compare: editing HTML changes *text*.
3. **It drags in the module/build world.** `SOURCE-CODE FACT` — oxc is a *parser*, and its repository
   description explicitly lists `oxc_resolver` for module resolution as a *separate tool*. Module resolution,
   `node_modules`, package exports maps, and bundler behaviour are a second unsolved problem, not a
   feature. `INFERENCE`: making TSX canonical makes Spool a build system. Build reproducibility is listed in
   the brief as a cost to investigate; the evidence says the cost is severe.
4. **Conditional rendering destroys static structure.** `INFERENCE` — `{items.map(...)}` produces a
   *runtime* tree. A visual design document needs a *static* tree so that a node has a stable identity
   before any code runs. Every one of the brief's requirements — stable identity (§23), deterministic
   patches (§File 4), Git merges, sub-500 ms external-edit updates — degrades to "run the code and diff the
   result" under TSX.

### 13.3 But the LLM-familiarity argument survives, in a different form

`INFERENCE`. The genuine finding is not "TSX is a good format" — it is that **agents are better at
*emitting* JSX/HTML than at emitting a novel schema**, and that this is a *training-distribution* fact, not
a format-quality fact. It can be captured without paying for TSX-as-canonical by making the **agent's output
format** HTML/CSS even when the *human's* format is something else. That is Option E + a normalisation step,
and it is materially cheaper than Option A.

### 13.4 Tailwind

`INFERENCE`. Tailwind is a *utility-class dialect* of CSS. Its canonical form is still CSS text, so it is
**format-compatible** with the hypothesis — but it makes the cascade problem worse in one specific way: a
utility class **encodes a property in its name** (`p-4` → `padding: 1rem`), so "what rule caused this pixel"
becomes "which of 400 utility classes matched", and the edit target is a *class name*, not a declaration.
It also makes `gap: 24px → 32px` become a *class rename* (`gap-6` → `gap-8`), which is a cross-cutting
find-and-replace with an unbounded blast radius and no source-level guarantee. **Tailwind as canonical source
is the worst case for a visual editor.**

---

## 14. Existing website import

### 14.1 The structural problem, stated correctly

`INFERENCE` — the naive framing is "parse their HTML, keep their CSS". The real problem is that **a live
DOM is not a document**.

`SOURCE-CODE FACT` — CDP's `DOM.getOuterHTML` is described as: *"Returns node's HTML markup."* The DOM
`Element.style` attribute, per CSSOM, reflects the **declarations set on the element**, which the engine
populates from matching rules, and inline `style` is what the *script* wrote, not what the *author* wrote.
`OFFICIAL DOCUMENTATION` — CSSOM defines `style` as a `CSSStyleDeclaration`; `getMatchedStylesForNode` exists
precisely because the inline attribute alone does not explain the rendering (§12).

`INFERENCE` — therefore, for a script-rendered site, the only faithful capture is:

```
browser renders URL
  → snapshot the computed result (DOM + getComputedStyle + matched rules)
  → reconstruct authored-looking CSS from the computed styles
  → emit a NEW, normalised, human-readable stylesheet
```

This is **normalisation**, and it is **lossy in a specific way**: authored structure (which rule, which
class, which media query, what intent) is *destroyed* and must be *invented*. What survives is the *result*.

### 14.2 What can, cannot, and must be normalised

| Input | Direct? | Notes |
|---|---|---|
| Static HTML, external CSS | **Mostly** | Closest to the target. Messy but authored. |
| HTML with `<style>` blocks | Yes | Parse with tree-sitter-css; source preserved |
| HTML with inline `style` | Yes, but **should be lifted** to a class | Otherwise "visual edit" writes inline styles forever |
| **Tailwind** | **Normalise** | Utility classes → semantic classes + a token layer, or keep utilities and lose inspector clarity |
| **React/Next** | **Export-then-import** | Cannot be read from source (runtime output); import the built HTML+CSS, or run SSR |
| **CSS-in-JS** | **Normalise** | The *runtime-injected* stylesheet can be snapshotted via CDP `CSS.getMatchedStylesForNode`; the source is unobtainable |
| Remote assets | **Download** | And then they are Spool assets (§24 threat model) |
| Web fonts | Download + **licence check** | `@font-face` + licensing is a legal surface, not a technical one |
| `canvas`, `video`, WebGL | **Screenshot** | Cannot be structurally represented. Must become a raster asset. |
| Pseudo-elements `::before/::after` | **Cannot** | CSSOM pseudo-element styles are not DOM nodes. `getMatchedStylesForNode` returns `pseudoElements` as a *separate channel* — proof they are first-class-but-separate. |
| JS-driven DOM mutations | **Cannot** | Only the final state is observable |
| Shadow DOM | **Needs piercing** | `DOM.getDocument` has a `pierce` parameter — evidence this is special-cased even in Chrome |
| `<template>` contents | Needs explicit handling | Inert content |

`INFERENCE`. The honest verdict: **import is feasible and valuable, and it produces a *normalised Spool
project*, not the original website.** That is a legitimate and marketable outcome — "open any site, get an
editable design" — provided Spool is explicit that the result is a *reconstruction*. It is **not** evidence
that the original HTML/CSS is a good canonical format, because the import has already thrown the authored
layer away.

### 14.3 Is browser-DOM → runtime → source-preserving editing realistic?

`INFERENCE`. Yes, **in that order only**: browser DOM → Spool runtime → *authored* Spool source. Never
browser DOM → authored Spool source directly, because the DOM's `style` attributes are already
computed-flavoured and would bake specificity accidents into the source.

---

## 15. Browser / DOM architecture — and why it is the wrong runtime

### 15.1 The DOM is not a scene graph

`INFERENCE`. A design editor needs: a retained scene representation, a spatial index for hit-testing and
culling, dirty-region tracking, GPU batching, and object identity. A browser DOM provides a *retained tree*
and *style*, and then takes all control of layout, painting, compositing, and hit-testing. You cannot
interpose. The corpus already records why Figma built its own:

> "All browsers provide a high-performance GPU compositor but the web doesn't have any way of hooking into
> the rendering algorithm and changing how [it works]" — *ENGINEERING DISCLOSURE*, Figma, "Building a
> professional design tool on the web", 2015 (via corpus `architecture/rendering.md`)

### 15.2 The scale evidence

`OBSERVED BEHAVIOR` — `docs/research/architecture/rendering.md` §"Performance options", from tldraw's
`sdk-features/performance.mdx`:

| Option | Default |
|---|---|
| `maxShapesPerPage` | **4000** |
| `debouncedZoomThreshold` | 500 |

`INFERENCE`. tldraw is the most architecturally sophisticated editor in the Spool corpus — hierarchical state
chart, reactive store, documented culling, LOD, geometry caching, instrumentation. **Its default page cap is
4,000 shapes.** Spool's target is 10⁶–10⁷. That is a **three-to-four-order-of-magnitude gap** against the
best-documented comparable, and it must be treated as the central engineering fact of this document, not a
detail.

`OFFICIAL DOCUMENTATION` — Lighthouse / web.dev, *How large DOM sizes affect interactivity*:

> "According to Lighthouse, a page's DOM size is excessive when it exceeds **1,400 nodes**. Lighthouse will
> begin to throw warnings when a page's DOM exceeds **800 nodes**."

and on mechanism:

> "When interactions modify the DOM … the work necessary to render that update can result in very costly
> layout, styling, compositing, and paint work."
> "The principal way to reduce DOM size is to reduce DOM depth."

**Precise statement of what this means** (`INFERENCE`, and it is important not to over-read): 1,400 is an
**advisory latency threshold for interaction responsiveness**, not a capacity limit. Browsers will hold a
million nodes. The threshold exists because style recalculation and layout cost grows super-linearly with
tree size and selector complexity. **The relevant lesson for Spool is the mechanism, not the number:** the
browser's own performance guidance is that *the more nodes you have, the more expensive every change is* —
which is the opposite of the property Spool needs at 10⁶ objects.

`OFFICIAL DOCUMENTATION` — the same page names the browser's own escape hatches: `content-visibility`
("effectively a way to lazily render off-screen DOM elements") and CSS containment ("isolate rendering work
to a DOM subtree"). `INFERENCE` — the web platform needed a dedicated property to approximate what a game
engine gets for free from chunked streaming. That is a strong signal about what Spool must build natively.

### 15.3 What the DOM *is* good for in Spool

`INFERENCE`. Not nothing: (a) it is the reference semantics for what a computed style means, so a
`getComputedStyle`-equivalent oracle is valuable in tests; (b) `DOM.getNodeForLocation(x, y)` is a
well-specified hit-test contract worth matching; (c) a headless browser is a legitimate *oracle* for import
and for CSS-layout conformance testing. All three are **testing and import uses**, not runtime uses.

---

## 16. Game-engine and large-scene architecture

### 16.1 The transferable pattern is chunked streaming over a persistent source

`OFFICIAL DOCUMENTATION` — Unreal Engine World Partition:

| Concept | Mechanism | Spool analogue |
|---|---|---|
| Single persistent level | "storing your world in a single persistent Level file" | The project directory as one logical document |
| Spatial subdivision | "subdividing the space into streamable grid cells using a configurable runtime grid" | Chunks indexed by page/region |
| Streaming sources | Streaming components with a `Loading Range`; priority resolution when cells intersect multiple sources | The camera viewport; the editor window |
| `Cell Size` | Configurable (`CellSize=51200` in the shipped conversion ini) | Chunk granularity as a tuning knob |
| **One File Per Actor** | Each actor in its own file | One file per page/component |
| Data Layers | Named visibility layers | Page visibility / artboard sets |
| HLOD | "Hierarchical Levels of Detail" | Reduced-detail representation at low zoom |
| Editor streaming toggle | `Enable Streaming` in World Settings | Explicit load-everything mode for export/print |

`INFERENCE`. The single most transferable sentence in that page is: **"you do not need to check out the
Level file from source control to make changes to the Actors in the world."** That is the mergeability
requirement stated by a shipping engine, and it maps exactly onto the brief's requirement that a human's
visual edit and an agent's file edit must not contend.

### 16.2 Figma's renderer: the strongest available evidence that native 10⁶+ is achievable

`ENGINEERING DISCLOSURE` — Figma, *"Figma Rendering: Powered by WebGPU"* (Sep 2025), via a detailed
secondary summary of the official post (techfeed.io, 2025-09-24; the official page's body did not render in
retrieval, so this is second-hand and flagged as such):

- Figma's renderer is **C++**. It is compiled with **Emscripten to WebAssembly for the browser** and to
  **x64/arm64 natively for server-side and native environments**.
- Figma integrated **Dawn**, the WebGPU implementation Chromium uses, so **web and native share one
  foundation**.
- The graphics interface was redesigned from global-state binding (`context->bindVertexBuffer(...);
  context->draw()`) to **explicit argument passing** (`context->draw(vertexBuffer, framebuffer, {texture},
  material, …)`).
- A **custom shader processor** translates legacy GLSL to WGSL, combined with **`naga`**, the Rust
  shader-compiler crate.
- Draw calls are **batched into a uniform buffer and submitted once** (`encodeDraw(...)` × N, then `submit()`).
- Because Windows GPU/driver faults cause **device loss mid-session**, Figma implemented **dynamic
  WebGPU → WebGL fallback during a session**.
- Roadmap: **compute shaders** for blur, **MSAA**, and `RenderBundles` to cut CPU overhead.

`INFERENCE`. Three lessons for Spool. (1) The same renderer can serve web and native — which means "native
Rust + HTML/CSS source" is not a compromise; the runtime can be the *same class of artefact* Figma ships
natively. (2) **Compute shaders for blur** is the disclosed direction for expensive per-pixel effects, which
is exactly Spool's blur/backdrop-filter problem. (3) **Device-loss fallback is not optional** at this
performance tier; Spool on GPUI will need an equivalent.

### 16.3 GPU/ECS/scene-graph comparison (brief requirement)

| Approach | What it is | Suitability for Spool | Evidence |
|---|---|---|---|
| **Browser DOM** | Retained tree; browser owns layout/paint | **Rejected** as runtime | §15 |
| **Scene graph** (Figma-named) | Persistent scene nodes between document and GPU | **Required** | `ENGINEERING DISCLOSURE`; corpus `rendering.md` Implication A |
| **Retained renderer** | Own paint list, own culling, own invalidation | **Required** | Same |
| **ECS** (Bevy-style) | Data-oriented systems over archetypes | **Partial** — right for *effects and render state*; wrong for the document, which is a hierarchy with identity | INFERENCE |
| **Game-engine scene arch** (UE/Godot) | Chunked streaming + HLOD + spatial index | **Required for scale** | §16.1 |
| **Dirty-region + instancing** | Track damage; batch draws | **Required** | Figma's `encodeDraw`/`submit` (§16.2) |

`INFERENCE`. The corpus's existing conclusion — "Document → Scene graph / paint tree → UI toolkit (or GPU)",
`architecture/rendering.md` Implication A — is *unchanged* by the HTML/CSS hypothesis. If anything it is
strengthened: HTML/CSS as source makes the scene graph *more* necessary, because the persistent form is now
a tree of text that must be compiled into a non-textual runtime representation.

---

## 17. Large-document techniques (indexed)

`INFERENCE` — the technique set, and what each buys. This is the input to File 3.

| Technique | Mechanism | Required at 10⁶? |
|---|---|---|
| **Chunked persistent storage** | Split source into independently-parseable, independently-written units (UE grid cells / One File Per Actor) | **Yes** |
| **Lazy/streaming load** | Load only what the viewport needs; unload what it does not | **Yes** |
| **Spatial index** | R-tree / BVH / quadtree for hit-test, cull, marquee | **Yes** — the corpus already requires it (`rendering.md` Implication D) |
| **View-frustum culling** | Only submit visible nodes; tldraw's `getCulledShapes()` | **Yes** |
| **LOD** | Cheaper representation at low zoom (UE HLOD; tldraw's documented LOD list) | **Yes** |
| **Dirty tracking** | Per-node dirty bits so a change costs O(subtree) | **Yes** — Blink's 4 bits/node (§19) is the reference |
| **Incremental invalidation** | Changed-ranges from a parser beats guessing | **Yes** — `ts_tree_get_changed_ranges` (§8.1) |
| **GPU instancing / batching** | One draw call per material, many instances | **Yes** |
| **Texture atlases** | Fewer binds; Figma batches uniform buffers | **Yes** |
| **Persistent cache / content addressing** | Hash-addressed derived artefacts | **Strongly recommended** — the startup story (§8 of the brief's Q9) |
| **Background parsing** | Parse off the UI thread; show visible region first | **Yes** |
| **Memory mapping** | Avoid a copy for read-only chunk access | **Yes** |

---

## 18. Agent editing

### 18.1 The LLM-familiarity claim, stated with its actual epistemic status

`INFERENCE` (explicitly *not* a fact, per the brief's own example). LLMs are extensively trained on HTML,
CSS, JSX, and Tailwind, and on the surrounding documentation and Q&A. This *plausibly* reduces the
domain-specific schema knowledge an agent must acquire before it can edit a Spool project correctly. It is
**not** a guarantee of correctness, not a guarantee of minimal diffs, and not a property of the format — it
is a property of the training distribution. Two counterpoints that are equally inferences:

- `INFERENCE`: an agent that knows HTML/CSS will confidently produce *valid* CSS that is *wrong for the
  design intent*, because it cannot see the canvas. Familiarity buys syntax, not judgement. The corpus
  already records the product-level consensus here: generation **into existing structure** beats generation
  from nothing, in all four products.
- `INFERENCE`: familiarity cuts both ways — it also means an agent will reach for idioms (utility classes,
  `!important`, deep specificity, `z-index` escalation) that are *hostile* to a visual editor. §12 and §13.4.

### 18.2 The three agent paths, and whether they converge

The brief asks whether semantic operations, source editing, and MCP can converge on one operation/history
system. `INFERENCE` — they converge **only if** there is a single normalisation step that turns all three
into the same validated operation, and only if that step is *lossless enough* to round-trip source edits.

```
MCP tool call ─┐
File edit ─────┼─→ NORMALISE ─→ VALIDATE ─→ semantic operation ─→ history ─→ runtime ─→ source
Script ────────┘                    ↑
                            (external source edits must be
                             diffed into operations, not applied raw)
```

The hard cell is **file edit**. An external edit is a *text* delta, not an *operation*. To make it participate
in Spool's operation/history system it must be **inverted into operations** — which requires knowing what the
text delta *means*, which requires the cascade engine (§12). `ts_tree_get_changed_ranges` gives the *where*;
§19 gives the *what*.

### 18.3 MCP as an integration surface

`OFFICIAL DOCUMENTATION` — Model Context Protocol specification, revision `2026-07-28`, `/server/tools`:

- Tools are **model-controlled**: *"Tools in MCP are designed to be model-controlled, meaning that the
  language model can discover and invoke tools automatically."*
- *"For trust & safety and security, there SHOULD always be a **human in the loop** with the ability to deny
  tool invocations."*
- A tool has `inputSchema` (JSON Schema, default 2020-12) and an **optional `outputSchema`**; when an
  `outputSchema` is provided, *"Servers MUST provide structured results that conform to this schema"*.
- Backwards compatibility: *"a tool that returns structured content SHOULD also return the serialized JSON in
  a TextContent block."*
- Tool **annotations are explicitly untrusted**: *"clients MUST consider tool annotations to be untrusted
  unless they come from trusted servers."*
- `InputRequiredResult` / `elicitation/create` supports **multi-round-trip** interaction.
- Tools may return **`resource_link`** or embedded `resource` items with `file://` URIs — i.e. MCP's own
  vocabulary includes "here is a file in the project".
- Servers SHOULD return tools in a **deterministic order** to improve *"LLM prompt cache hit rates"*.

`INFERENCE`. Three consequences. (1) MCP is a *transport*, not a document model; adopting it says nothing
about the representation. (2) The spec's own human-in-the-loop requirement aligns with Spool's semantic
operation + history design (A3/A4) — an MCP tool call is naturally one operation, and therefore naturally one
history entry, which is the "one semantic transaction per agent action" answer to the brief's Q17. (3) Because
MCP resources are `file://`-addressable and tools can return them, **MCP and file editing are the same
mechanism** at the protocol level — reinforcing that source editing is not a side channel but a first-class
agent surface. Detailed design in File 4.

---

## 19. File watching and incremental external updates

`INFERENCE` — the mechanism, assembled from the evidence already gathered:

```
watcher event (path, mtime, size)
  → is this path in the project graph?              [§11 dependency graph]
  → read only that chunk                             [chunked storage]
  → ts_tree_edit(old_tree, edit)                    [§8.1]
  → ts_parser_parse(old_tree)                       [§8.1  incremental]
  → ts_tree_get_changed_ranges(old, new)            [§8.1  precise invalidation]
  → for each changed range:
        re-resolve affected selectors               [§19 cascade]
        mark dirty nodes                             [4 bits/node, Blink §19]
  → recompute cascade for dirty subtrees
  → re-render dirty regions
```

`INFERENCE` — feasibility hinges on three things, each of which is achievable and each of which is *work*:

1. **Identity survives the edit.** §23. Without it, node handles are lost and selection dies.
2. **Change ranges are small.** `ts_tree_get_changed_ranges` returns ranges that "may be slightly larger
   than the exact changed areas" — good, but a change near the top of a 50,000-line file can still return a
   wide range. `INFERENCE`: chunking (§11) bounds this, which is the real argument for chunked storage.
3. **Selection and camera survive.** Both are editor runtime state (A1/A2), and both are addressable by
   stable id, so they survive by construction *if and only if* (1) holds.

**Save races.** `INFERENCE` — three distinct races must be handled and none is solved by HTML/CSS:
(a) Spool writes a file while an agent writes it (last-writer-wins → lost update; needs optimistic
concurrency via content hash); (b) the watcher fires for Spool's own write (needs self-write suppression by
hash, not by path); (c) an agent edits while Spool has the same node dirty (needs a conflict surface). All
three belong in File 4, not here.

---

## 20. History and source control

### 20.1 Does HTML/CSS help or hurt Spool's stated history direction?

`INFERENCE`. It **helps**, in one specific and important way. Spool's direction (A4) is eager commit with a
semantic operation per interaction. An operation that says "set `gap` on `#hero` from 24px to 32px" maps onto
exactly one CST splice, so the inverse operation is exactly one inverse splice, and `⌘Z` is a **source
splice** rather than a runtime state restoration. That is arguably *better* than tldraw's model, because the
history entry and the source change are the same object.

`INFERENCE` — but this only holds if history entries record **source ranges**. A history entry that records
"node X had property P = V" must *also* record "at range R of file F". If the file has since been edited
externally, R is stale. So **history is entangled with external-edit reconciliation**, which tldraw never
had to face because its document is not external-editable. This is a genuinely new architectural obligation
created by the HTML/CSS hypothesis, and it is not in the brief's list of costs. `PROPOSED SPOOL DESIGN` —
record the *semantic* change in history and reconstruct the source range at undo time, rather than storing
the range; the cost is that undo after an external edit may be non-minimal, which is acceptable and honest.

### 20.2 The specific undo question from the brief

> Human edits canvas → Spool changes HTML/CSS. Agent edits HTML/CSS → Spool detects. Human presses undo.
> What exactly is undone?

`INFERENCE` — a precise answer requires naming three distinct cases, which the brief's framing conflates:

| Case | What undo should do | What it costs |
|---|---|---|
| Undo after only visual edits | Undo the operation = one source splice. Exact. | Nothing |
| Undo after only external edits | **Undefined.** Are external edits in Spool's history at all? | Requires a decision: are external edits history entries? |
| Undo spanning both | Needs interleaving external and internal entries | Requires a single ordering |

`INFERENCE` — the honest recommendation shape (not a decision): **external edits should be admitted to
history as operations too**, at a coarser granularity (one transaction per watcher batch), or the history is
two systems that cannot explain each other. This has a precedent-shaped cost: the corpus records that
"undo, version history, and branching are three systems" in Figma, and that "undo and version history are
separate systems" is a 2-of-2 convergence. `PROPOSED SPOOL DESIGN` — record the distinction explicitly rather
than letting it emerge.

### 20.3 "Agent makes 15 changes — 15 entries or 1 transaction?"

`INFERENCE` — this is not a format question, and Spool has already decided the principle: A4 says one
*successful interaction* is one transaction. An agent turn is the agent's equivalent of an interaction, and
MCP gives the boundary for free (`tools/call` is one request). So: **one MCP tool call = one history
transaction**, regardless of how many source splices it performs. That is `PROPOSED SPOOL DESIGN` and it
requires the normaliser (§18.2) to emit a *batch*, not a single operation — which is a real requirement on the
operation API that does not currently exist in the prototype.

### 20.4 Merges

`OBSERVED BEHAVIOR` — §12.2 gives the JSON mechanism. For HTML/CSS the corollary is:

| Conflict | Frequency | Mitigation |
|---|---|---|
| Same declaration, both sides | Low | Auto-resolvable |
| Same rule, different declarations | Low | Auto-resolvable at declaration granularity |
| Same file, different regions | Low | Git handles it |
| **Same rule, one side reorders** | **High** | Conflict; cascade order is semantically load-bearing |
| Different files, same shared class | **Medium** | Conflict; this is the cost of multi-file |
| Element moved between files | Conflict | — |

`INFERENCE`. The multi-file model *trades* one JSON-era conflict class (trailing-line) for another (shared
class across files). Neither is free. The mitigation for both is **chunking by concern so that a shared class
lives in exactly one file** — which is the same conclusion §16.1 reached from the scalability side. Two
independent axes pointing at the same design is the strongest structural signal in this document.

---

## 21. React/TSX as canonical — see §13. Verdict: not supported.

## 22. Identity

### 22.1 Why HTML `id` is insufficient

`INFERENCE`. `id` is authored, human-meaningful, unique-per-document *by convention only*, and — critically —
**it is what humans and agents will naturally collide on**. Three concrete failure modes:

1. **Duplicate ids are legal-ish and widespread.** No browser enforces uniqueness; tooling varies. An
   imported page can arrive with duplicates.
2. **`id` is a styling hook, not a key.** Designers and CSS both target `#id`. Coupling identity to a styling
   selector means a designer renaming `#hero` to `#landing` silently breaks every agent's saved reference.
3. **Copy/paste duplicates it.** Duplicating a subtree with ids is a guaranteed collision. Every design
   editor's duplicate must mint new identity.

### 22.2 What identity has to survive

| Event | Requirement |
|---|---|
| Move in the document | identity unchanged |
| Copy/paste | new identity for the copy, same for the original |
| Component instance | instance identity distinct from definition identity |
| **Source edit by an agent** | identity unchanged if the agent did not touch the id |
| **External file edit** | identity unchanged (this is the whole point of §8.1's change ranges) |
| **Git merge** | identity must not collide; must survive both sides |
| Save/reload round trip | identity persisted, not regenerated |
| Undo/redo | identity unchanged (a redo must not mint a new id) |

### 22.3 How small can the metadata layer be?

`PROPOSED SPOOL DESIGN` — the smallest layer that satisfies §22.2 is **one generated, immutable, opaque
attribute per node**, plus one per project. Nothing else. Concretely, in preference order:

| Option | Mechanism | Pros | Cons |
|---|---|---|---|
| 1 | `data-spool-id="<uuid>"` | Standard, inert, survives any HTML tool, ignored by CSS selectors | 30+ bytes per node in source |
| 2 | `id="<uuid>"`, human names in a class or `data-name` | Zero extra bytes | Couples identity to styling; breaks §22.1(2) |
| 3 | A sidecar `.spool/ids.json` mapping (hash of path) → id | Keeps source pristine | **Fragile**: any text edit changes the hash; Git-hostile |
| 4 | A CSS custom property `style="--spool-id: …"` | Attribute-free visually | Bloats `style`; interacts with shorthand handling |

`INFERENCE` — **option 1 is the only one that satisfies every row of §22.2 without a bespoke mechanism**,
because `data-*` attributes are inert to CSS, ignored by renderers, preserved by every HTML tool, and
mergeable in a line-based way. Its cost is source verbosity, and that cost is real: at 10⁶ nodes a
`data-spool-id` on every node is roughly 30 MB of pure identity. `PROPOSED SPOOL DESIGN` — the resolution is
that **§15's target means not every node is ever in source at once**; chunking bounds it, and a project that
*is* 10⁶ nodes of authored HTML is a different product from a project that *streams* 10⁶ runtime objects. That
distinction is the crux of File 2's Option D and File 3.

**The honest finding on §Q19 / "what must remain Spool-specific".** At minimum: (a) node identity, (b) style
ownership convention, (c) component definitions, (d) prototype connections, (e) project manifest. That is
**five** things, not one — and (b) is the expensive one, because it is not data, it is a *rule about who owns
which declaration*. §25.

---

## 23. Security

### 23.1 The threat model HTML/CSS introduces

`INFERENCE` — by making external source canonical, Spool takes on the threat surface of a browser:

| Vector | Mechanism | Severity |
|---|---|---|
| `<script>` | Executes in any preview | **Critical** |
| `<iframe>` / `<object>` / `<embed>` | Nested browsing context | **Critical** |
| `javascript:` URLs | Event handler / navigation | High |
| Event handler attributes (`onload=`) | Inline JS | High |
| `<link rel=stylesheet href="https://…">` | Remote fetch; exfiltration channel | High |
| `url(https://…)` in CSS | Remote fetch on render | High |
| `@import url(https://…)` | Same | High |
| `<img src="https://…">` | Tracking pixel | Medium |
| Malicious SVG | `<script>` inside SVG, `<foreignObject>`, XXE | High |
| External entities (XXE) | XML parsing | High |
| Path traversal in `href`/`src` | Local file read | High |
| Zip bombs / billion laughs | Decompression, entity expansion | Medium |
| CSS `@font-face` src | Remote font fetch | Medium |
| CSS `image-set()` / huge `url()` | Memory exhaustion | Medium |

### 23.2 What Rust gives for free

`SOURCE-CODE FACT` — usvg, `crates/usvg/src/lib.rs`:

```rust
#![forbid(unsafe_code)]
```

and its documented behaviour: *"Recursive elements will be detected and removed"*; *"Only static SVG
features, e.g. no `a`, `view`, `cursor`, `script`, no events and no animations"*; built on `roxmltree`.

`INFERENCE`. `#![forbid(unsafe_code)]` plus a Rust XML parser removes memory-safety classes of bug
entirely, and usvg's design removes the entire SVG-scripting surface **structurally** rather than by
filtering. That is stronger than sanitisation: there is nothing to sanitise because the feature is absent
from the output model.

### 23.3 The one sharp default, and the missing budget

`SOURCE-CODE FACT` — usvg's `ImageHrefResolver` (`crates/usvg/src/parser/image.rs:23-46`):

> "This type can be useful if you want to have an alternative `xlink:href` handling to the default one. For
> example, you can **forbid access to local files (which is allowed by default)** or add support for
> resolving actual URLs (**usvg doesn't do any network requests**)."

`SOURCE-CODE FACT` — `crates/usvg/src/parser/options.rs:13-100`: `Options` contains `resources_dir`,
`dpi`, `font_family`, `font_size`, `languages`, `shape_rendering`, `text_rendering`, `image_rendering`,
`default_size`, `image_href_resolver`, `font_resolver`, `fontdb`, `style_sheet`. **There is no
`max_nodes`, no `max_dimension`, no `max_bytes`, no `timeout`, and no recursion budget.**

`INFERENCE` — two concrete conclusions:

1. **Opening an untrusted SVG with usvg defaults reads local files.** `<image xlink:href="file:///…">` is
   resolved by default. Spool must install a restrictive `ImageHrefResolver` on the *import* path. This is a
   real, specific, checkable finding.
2. **There is no DoS budget in the library.** A hostile SVG with millions of nodes, or a
   `data:` raster of enormous dimensions, is parsed in full. Spool must impose its own budget *above* usvg,
   because the library will not.

### 23.4 The architectural answer to §Q20

`PROPOSED SPOOL DESIGN` — the architecture that makes "open an untrusted website project" ≠ "execute code",
without needing a browser sandbox:

| Layer | Policy |
|---|---|
| **Spool's own renderer** | Never interprets HTML semantics beyond what it needs: **no `<script>`, no event-handler attributes, no `<iframe>`/`<object>`/`<embed>` execution** — these are *rejected at parse time*, not sanitised |
| **Preview** | Only if a browser is embedded, and then in an isolated process. Better: Spool's own renderer *is* the preview, so there is no second engine to sandbox. |
| **Network** | Default-deny. A resolver abstraction that only resolves `data:` and `assets/`-relative paths. Any `http(s):` reference becomes an explicit, user-visible "remote asset" requiring consent. |
| **Filesystem** | Canonicalise and confine every resolved path to the project root. No absolute paths, no `..`. |
| **Budgets** | Node count, byte count, raster dimensions, parse wall-clock — all capped per project, per import. |
| **Assets** | Downloaded once, stored as content-addressed blobs, never referenced live. |
| **Scripts** | If Spool ever executes project JS, it runs in a **separate process** with no filesystem, no network, and a wall-clock budget. If it never does, the entire class disappears. |

`INFERENCE`. The strong version of this answer is: **Spool's own renderer is the sandbox.** Because Spool
renders from its own runtime rather than from a browser, there is no JS engine to escape from, and the whole
"preview isolation" problem reduces to "don't implement `<script>`". That is a genuine security *advantage*
of the native-runtime half of the hypothesis, and it should be weighed against the cost of the source-preserving
half.

---

## 24. Relevant implementation technologies (consolidated)

| Need | Candidate | Status | Evidence |
|---|---|---|---|
| HTML CST | `tree-sitter` + `tree-sitter-html` | **Recommended** | §8 |
| CSS CST | `tree-sitter` + `tree-sitter-css` | **Recommended** | §8.2 |
| HTML5 import normalisation | `html5ever` + `markup5ever` | **Recommended for import only** | §8.3 |
| Typed CSS property values | `lightningcss` (value model only, no `to_css`) | **Viable** | §9 |
| Selector parsing/matching | `selectors` crate (Servo/Mozilla), also used by lightningcss | **Viable** | lightningcss README |
| SVG parse (import) | `usvg` (resvg) | **Recommended, with a restrictive `ImageHrefResolver`** | §23.3 |
| SVG render | `resvg` | Recommended for raster/offscreen paths | §23.2 |
| JS/TS parse (if ever) | `oxc` + `oxc_resolver` | Viable; **not source-preserving** | §13.2 |
| TSX CST (if ever) | `tree-sitter-typescript` | Viable for preservation | INFERENCE |
| Spatial index | R-tree (`rbus`/`geo`) or hand-rolled BVH | Required; corpus already requires one | corpus `rendering.md` |
| File watching | `notify` + a content-hash check | Required | §19 |
| Serialisation | hand-written CST splicer (bytes → `String`) | **Do not use a serialiser** | §8.3 |
| MCP server | `rmcp` or hand-rolled JSON-RPC over stdio | Required | §18.3 |
| Compression | `zstd` for binary cache | Recommended | INFERENCE |

`INFERENCE` — the single most important line in that table is **"Do not use a serialiser"**. If any stage in
Spool's write path can re-emit a document from an AST, source preservation is lost, and every earlier
guarantee in §8 evaporates. The write path must be **byte splicing into the original buffer**, full stop.

---

## 25. What must remain Spool-specific — and whether standard mechanisms suffice

This is §Q19, answered as a table with the *reason* for each proprietary element, as required.

| # | Capability | Can standard HTML/CSS carry it? | Smallest Spool addition | Justification |
|---|---|---|---|---|
| 1 | **Stable node identity** | **No** — `id` is a styling hook and collides (§22) | one `data-spool-id` per node | §22.3; irreversible otherwise |
| 2 | **Style ownership convention** | **No** — this is a *rule*, not data (§12) | a class/attribute convention + a resolver | §12; the expensive one |
| 3 | **Component definitions + instance overrides** | **No** — `<template>` has no props (§11) | a naming convention + an instantiation rule | §7 L-list |
| 4 | **Prototype connections** | **Partly** — `<a href>` models navigation, not timed flows | an attribute or manifest entry | §5 matrix |
| 5 | **Project manifest** | **No** — `<meta>` is per-document, not per-project | `.spool/manifest.json` | chunking + config (§10) |
| 6 | **Constraints** | **No** (§7 L1) | either a convention or an explicit non-goal | §7 L1 |
| 7 | **Blend-group membership** | **No** (§7 L2) | a class + sibling rule | §7 L2 |
| 8 | **Design tokens** | **Yes** — custom properties + `@property` | **nothing** | §5.2; a *gain* |
| 9 | **Accessibility** | **Yes, fully** | nothing | §4.1; a *gain* |
| 10 | **Assets** | **Partly** — `img`/`url()` reference, do not carry metadata | an assets manifest | §12.1 |
| 11 | **Responsive breakpoints** | **Yes, fully** | nothing | §5.1; a *gain* |
| 12 | **User-defined object types** | **No** (§7 L3) | a registration mechanism | §7 L3; mirrors corpus `ShapeUtil` |

`INFERENCE` — the honest count is **five things Spool genuinely cannot avoid** (1, 2, 3, 5, 12) and **two
more that are capability gaps requiring a decision** (6, 7). Of those, exactly one is a *data* addition
(identity). The rest are **rules**. `PROPOSED SPOOL DESIGN` — the architectural claim this supports is
narrower and more defensible than "HTML/CSS is canonical":

> HTML/CSS can carry Spool's document **data**. It cannot carry Spool's document **semantics**, and the
> semantics must live in a Spool-owned convention layer. The size of that layer is a design question, not a
> format question — and it is the layer the brief's "minimal Spool metadata" option is really asking about.

`INFERENCE` — three of the four worst entries in that table (2, 6, 7) exist for the *same* reason: **a design
editor wants direct manipulation and CSS wants indirection**. That is one root cause, not three gaps, and
File 2 treats it as the central design tension.

---

## 26. Open questions this research could not answer from evidence

1. **How large can a single tree-sitter HTML/CSS CST get before incremental reparse degrades?** No published
   benchmark found. `INFERENCE`: chunking sidesteps it, but the chunk-size ceiling is unknown.
2. **tree-sitter-html's divergence from the HTML5 tree-construction algorithm** (§8.2) is unquantified for
   real-world malformed input. Needs a corpus test.
3. **What is the actual cost of a `ts_tree_get_changed_ranges` result for a large file?** The header says
   ranges "may be slightly larger than the exact changed areas" but gives no bound.
4. **A cascade engine in Rust at design-editor scale** — selector matching, `@layer`, `@scope`,
   `!important`, container queries, custom-property substitution with cycles, `:has()` — has no obvious
   existing crate. This is the largest *unresearched implementation risk*.
5. **Whether `lightningcss`'s typed value model can be consumed without its serialiser.** Its public API is
   built around `to_css()`; a value-only API may not exist.
6. **Figma's WebGPU renderer in detail** — the official post's body did not render; §16.2 is second-hand via a
   detailed Japanese summary. Corpus gap **G5 is partially closed, not closed**.
7. **No product in the corpus uses HTML as a document**, so there is no production precedent at all. Every
   "editors do X" claim in this document is about *export* or *import*, never about canonical persistence.
8. **LLM edit quality on HTML/CSS vs. a bespoke schema** is unmeasured. §18.1 is an inference about training
   distribution, not a measurement.
9. **`@scope`, `@layer`, `:has()`, and container queries all widen blast radius**, and their interaction with
   the invalidation-sets approach is not characterised here.
10. **Whether identity via `data-spool-id` survives aggressive third-party HTML tooling** (formatters,
    minifiers that rename ids, CMS importers). No evidence gathered.

---

## 27. Evidence gaps

| Gap | Why it matters | Suggested method |
|---|---|---|
| **Cascade engine cost in Rust** | Largest implementation risk (§26.4). Blocks Options B/C/D in File 2 | Prototype: selector matching + invalidation sets over a 10⁶-rule synthetic corpus |
| **CST scale ceiling** | Determines chunk size, which determines everything in File 3 | Benchmark tree-sitter on 1–100 MB HTML/CSS |
| **Real-world HTML quality** | Decides how much normalisation import needs | Run tree-sitter-html vs html5ever over a large real-world corpus; diff the trees |
| **Agent edit quality** | Tests §18.1's central inference | Give models the same task in HTML/CSS vs. a JSON schema; measure diff minimality and validity |
| **Round-trip fidelity under real editors** | Tests §8's central claim | Programmatically edit 10⁵ nodes; assert byte-identical outside splices |
| **Figma's renderer internals** | Corpus G5 | Re-attempt the official post; the body is client-rendered |
| **Git merge rates** | Quantifies §20.4 | Synthetic merge harness on generated projects |

---

## 28. References

**Specifications and official documentation**

1. WHATWG HTML Living Standard — https://html.spec.whatwg.org/multipage/
2. W3C CSS Syntax Module Level 3 — https://www.w3.org/TR/css-syntax-3/
3. W3C CSS Cascading and Inheritance Level 4/5 — https://drafts.csswg.org/css-cascade/
4. W3C CSS Conditional Rules (container queries, `@scope`) — https://drafts.csswg.org/css-conditional-5/
5. W3C SVG 2 — https://www.w3.org/TR/SVG2/
6. W3C ARIA / ARIA in HTML — https://www.w3.org/TR/html-aria/
7. WebCGM / `content-visibility` guidance — https://web.dev/articles/dom-size-and-interactivity
8. Chrome DevTools Protocol, `CSS` domain — https://chromedevtools.github.io/devtools-protocol/tot/CSS/
9. Chromium Blink, *CSS Style Invalidation in Blink* — https://chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/core/css/style-invalidation.md
10. Chromium, *RenderingNG deep-dive* — https://developer.chrome.com/docs/chromium/blinkng
11. Unreal Engine, *World Partition in Unreal Engine* — https://dev.epicgames.com/documentation/en-us/unreal-engine/world-partition-in-unreal-engine
12. Model Context Protocol specification, revision 2026-07-28, `/server/tools` — https://modelcontextprotocol.io/specification/2026-07-28/server/tools
13. Lighthouse, *Avoid an excessive DOM size* — https://developer.chrome.com/docs/lighthouse/performance/dom-size

**Source code read directly (unversioned HEAD unless noted)**

14. tree-sitter — `lib/include/tree_sitter/api.h` (`ts_tree_edit`, `ts_parser_parse`,
    `ts_tree_get_changed_ranges`, `ts_node_edit`)
15. tree-sitter/tree-sitter-html — `grammar.js`, `src/scanner.c`, `src/tag.h`, `src/node-types.json`
16. tree-sitter/tree-sitter-css — `src/node-types.json`
17. servo/html5ever — `README.md`; `markup5ever/interface/tree_builder.rs`
18. parcel-bundler/lightningcss — `src/rules/mod.rs`, `src/stylesheet.rs`, `src/parser/options.rs`,
    `src/declaration.rs`, `README.md`
19. linebender/resvg — `crates/usvg/src/lib.rs`, `crates/usvg/src/parser/options.rs`,
    `crates/usvg/src/parser/image.rs`
20. oxc-project/oxc — https://oxc.rs/docs/guide/usage/parser.html

**Engineering disclosure**

21. Figma, *Figma Rendering: Powered by WebGPU*, Sep 2025 — https://www.figma.com/blog/figma-rendering-powered-by-webgpu/
    (body did not render in retrieval; details in §16.2 are from https://techfeed.io/entries/68d31867d0e17253abfbe736,
    a detailed secondary summary — labelled second-hand at point of use)
22. Figma, *Building a professional design tool on the web*, 2015 — quoted via the existing Spool corpus
23. tldraw, `sdk-features/performance.mdx` and `culling.mdx` — quoted via the existing Spool corpus

**Third-party / observed**

24. *Avoid JSON file merge conflicts*, sophiabits.com (2023), citing the `cal.com` codebase — https://sophiabits.com/blog/avoid-json-file-merge-conflicts

**Existing Spool corpus consulted (read-only, not modified)**

25. `docs/research/README.md`; `architecture/{document-model,editor-runtime,rendering,history,ai-runtime,input-system,interaction-runtime,profiles}.md`; `creative/export.md`; `ai/*`; `document/*`; `matrices/*`.
