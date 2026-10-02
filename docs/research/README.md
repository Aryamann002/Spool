# Spool Research Corpus — Synthesis

> **This is a research synthesis, not a design.** It contains no Spool architecture and no build
> plan. Where the evidence points somewhere, the point is recorded as an implication with
> `Decision: TBD — requires architecture review.` and the decision is deferred to a later phase.
>
> Evidence labels used throughout: `DOCUMENTED` · `OBSERVED` · `ENGINEERING-DISCLOSED` ·
> `SOURCE-CODE` · `THIRD-PARTY` · `INFERRED` · `PROPOSED`.

## How to read this corpus

```
docs/research/
├── README.md              ← this file: the synthesis
├── products/        (4)   one file per product, §41 format
├── interaction/     (8)   behaviour: selection, transformation, snapping, text,
│                            drawing, navigation, keyboard, input
├── document/        (8)   objects, hierarchy, components, styles, layout, assets,
│                            bindings, variables
├── creative/        (5)   vector, raster, typography, animation, export
├── ai/              (5)   generation, editing, structured-generation, agents, context
├── architecture/    (8)   what the products' architectures appear to require
├── profiles/        (5)   per-product interaction repertoires, including Spool's
└── matrices/        (6)   cross-product comparison
```

**49 files. ~16,600 lines.** Read order for a new contributor: this file, then
`architecture/*.md`, then the relevant `matrices/` row, then the specific behaviour note.

Three layers are kept separate everywhere in this corpus:

| Layer | Where it lives | Meaning |
|---|---|---|
| **(a) What the products do** | `products/`, `interaction/`, `document/`, `creative/`, `ai/`, `matrices/` | Evidence-backed behaviour |
| **(b) What their architecture appears to require** | `architecture/` | Evidence-backed or explicitly marked inference |
| **(c) What Spool should do** | *nowhere in this corpus* | Deferred to the next phase by instruction |

---

## Executive Summary

Nine findings carry most of the weight.

**1. The user-visible contract of a design editor is more standardised than the architectures
underneath it.** One continuous gesture is one undo step, in all four products. Escape cancels then
exits. `⇧1` zooms to fit. `⌘D` duplicates. `⌘F1/F2/F3` is the focus ladder in both products that
have one. Spool already matches the undo contract. The divergences between products are almost
entirely in *mechanism*, not in *behaviour* — which means "feel familiar" is cheap and "extend
generally" is not.

**2. There is no such thing as "the design editor document model."** Four products, four
incompatible answers to the same questions. Figma: Frame is the universal container, three coexisting
layout systems, variables referencing values. tldraw: a flat reactive record store with scoped
records, a shape-type policy interface, and no layout engine at all. Affinity: paths first, studios
as capability gates. Canva: bounded pages, templates, brand values copied rather than referenced.
**The one concept with no carrier anywhere is a reuse primitive that three of the four do not have at
all.**

**3. tldraw is the only product whose interaction architecture is inspectable, and it is the most
instructive source in the corpus.** Its `StateNode` chart, unhandled-event fallthrough to children,
`target` re-dispatch, per-tool static config (`useCoalescedEvents`, `isLockable`,
`trackPerformance`), three history capture modes, and `ShapeUtil` policy hooks are directly
transferable abstractions. It is also the *only* product with a documented extensible object model —
which is the closest existing analogue to Spool's stated "VS Code for visual design" positioning.

**4. Two of the strongest architectural patterns arrived from opposite directions.** tldraw's live
mutation required *bail* and *squash* to feel like commit-on-release. Affinity's "generate structured
design into an existing document" guidance and tldraw's viewport-bounded AI context are the same
conclusion reached from product and architecture respectively: **generation into existing structure
beats generation from nothing.** Both also independently imply that the document must be separable
from the view.

**5. The corpus has larger holes than it has disagreements.** Undocumented in all four products:
missing-asset behaviour, missing-font behaviour, variable fonts, reparenting coordinate preservation,
per-operation AI approval, whether partial AI output is visible, and provenance. Any of these will be
invented by whoever implements first. Inventing them consciously, early, is worth more than copying
a convention.

**6. Interaction profiles are architecturally reasonable, and the industry has already shipped a
version of the idea.** Affinity's **Studios** change which tools and panels exist; Affinity's **7
snapping presets** change which rules are active; Affinity has full shortcut customisation. Figma has
16 keyboard layouts. tldraw makes tool registration the host's decision. **But no product exposes
modifier semantics or gesture behaviour as configuration** — which is the part people imagine when
they imagine "profiles". A realistic profile changes *what exists and which keys reach it*, not *how a
gesture interprets a modifier*.

**7. Spool's framework already provides most of the hard keyboard infrastructure.** GPUI ships a
named action registry, JSON-constructible actions, multi-key chords, a context-predicate language with
a descendant operator, source-prioritised user overrides, availability queries for menu generation,
and chord-completion queries. Spool uses 12 of them, all text-editing, all with `context: None`, in
parallel with a second raw key path that owns undo, delete, duplicate, tools, digits and `space`.
**The action registry is an adoption decision, not a build.**

**8. Spool's interaction gaps correlate with document-model gaps, not handler gaps.** Rotate, corner
radius, opacity, hierarchy, reparenting, snapping — all absent — but so are the document fields that
would express them. Of the 19 conventions checked in `profiles/spool.md`, the 6 that match are exactly
the ones that required a document model rather than a gesture handler. **A profile feature with no
document representation cannot be built by writing a gesture handler**, which is why two tools
(`Pen`, `Comment`) are currently selectable and inert.

**9. Spool's AI critical path does not go through model integration.** Of seven prerequisites for an
AI runtime, six are absent — but two of them, *offscreen rendering* and *caller-chosen history capture
boundaries*, are not AI features at all. They are general editor capabilities that AI happens to
require first. [INFERRED] Planning the AI roadmap as "AI work" will mis-sequence it.

**A note on two corrections.** Two claims in earlier notes in this corpus were wrong and have been
corrected in place rather than silently amended: Spool **does** have an action registry (GPUI's), and
Spool **does** have working undo/redo bindings (in the raw-key path, not the action path). See
`profiles/spool.md` §12 and `architecture/input-system.md` §3.2.

---

## What We Learned From Figma

### The depth model is the product

[DOCUMENTED] Figma's selection is not just a set. It is a set plus an implicit `nestingDepth`:

- a click selects the **parent** at the current depth,
- double-click or `Enter` **descends** one level,
- `⇧Enter` **ascends**,
- `⌘`-click **deep-selects**,
- `⌥`-click selects **behind**.

[INFERRED] Almost every Figma convention is downstream of this. "Why is my click selecting the whole
frame?" has one answer: depth. It is also why Figma can have a three-key focus ladder and a single
click-to-select and both feel right.

### Frame is the universal container, and that is load-bearing

[DOCUMENTED] Frame spans artboards, auto layout, components, prototypes, export, and developer handoff.
Two container types exist because two things are incompatible: **Group** has *derived* bounds and
cannot take constraints or layout; **Frame** has authored bounds, clips, accepts children, and accepts
constraints. "Hug container cannot clip, so a second container type is required" is a model fact
surfacing as a UI fact.

### Three layout systems coexist deliberately

[DOCUMENTED] Absolute positioning, constraints, and auto layout all live in Figma, with an explicit
"Ignore auto layout" escape hatch, and a child's sizing mode constrains what the parent can do.
[INFERRED] This is not incoherence; it is migration. Figma served absolute positioning for a decade
before auto layout existed and did not break existing files. Any Spool layout decision should assume
the same pressure will arrive.

### Creation gesture determines text behaviour

[DOCUMENTED] Click with the Text tool → auto-width box. Drag → fixed-width, wrapping box. Click with
Rectangle → fixed 100×100. Drag → drag-defined. [INFERRED] The most economical text model in the
corpus: the *gesture* picks the *behaviour* and no setting is ever touched.

### Variables reference; Brand Controls copy

[DOCUMENTED] Figma has 6 typed variable kinds, collections, groups, and modes, driving paint,
typography, layout metrics, visibility, variant selection, and motion timing. Canva's Brand Controls
**copies** the value into each element.

[INFERRED] Two products, two opposite answers to "how do you keep a design on-brand". Figma's failure
mode is indirection and edit-time surprise. Canva's is drift. Neither is wrong; they suit different
users. Spool will have to pick or invent a third.

### Undo, version history, and branching are three systems

[DOCUMENTED] Figma has undo, a version history with named/attributed/timestamped versions, **and
branching** from any historical point. They are distinct concepts with distinct semantics.

### Engineering disclosures are unusually specific

[ENGINEERING-DISCLOSED] C++/TS client; a named **scene graph**; WebGL → WebGPU (Sep 2025); the
"Keeping Figma Fast" performance-test framework (2023); the 2015 custom-renderer rationale.
[DOCUMENTED] 16 keyboard layouts and **no user rebinding** — a deliberate refusal that is itself a
data point for the profiles question.

[OBSERVED] **There is no minimap.**

---

## What We Learned From tldraw

This is the highest-value source in the corpus, because it is the only one whose architecture is
inspectable.

### Interaction is a hierarchical state chart with fall-through

[SOURCE-CODE] `StateNode` instances form a chart. Each node may handle any event; **unhandled events
fall through to children**. Transitions carry payloads delivered to the outgoing node's `onExit` and
the incoming node's `onEnter`. A node can **re-dispatch to a `target`** — one of `canvas`, `shape`,
`handle`, `selection`, `overlay` — so "the shape handles it, otherwise the canvas does" needs no
manual routing.

[INFERRED] Fall-through plus re-dispatch is the single most transferable mechanism in the corpus. It
is why tldraw scales to dozens of tools without a routing table, and it is why its tool list can be
empty by default without the editor being unusable.

### Tools are configuration, not code

[SOURCE-CODE] Each tool declares a static table: `id`, `initial`, `children`, `isLockable`,
`useCoalescedEvents`, `trackPerformance`. Tools **check** `isToolLocked` rather than being forced to
honour it — documented tool-lock leakage is a known consequence.

### The store is scoped

[SOURCE-CODE] Records live in one of three scopes: **`document`**, **`session`**, **`presence`**.
Selection is in instance state. Ancestor filtering is a side effect, not a rule. `focusedGroupId`
gives scope; `selectAdjacentShape` traverses by **geometric scoring** (distance + off-axis term), not
tree order.

[INFERRED] The three scopes were not invented for tidiness — they were forced by multiplayer. Spool
has no presence and would be choosing the taxonomy without the pressure that produced it. That is
worth recording, not worth copying blindly.

### `ShapeUtil` is the most reusable abstraction found

[SOURCE-CODE] Per-shape-type hooks: `canSnap`, `getBoundsSnapGeometry`, `getHandleSnapGeometry`,
`canCull`, `canBind`, `canTabTo`, `getText`, `getGeometry`, `component`.

[INFERRED] This single interface serves six subsystems — snapping, culling, binding, traversal, text,
and rendering — so a new shape type is added once. It is the pattern that makes a general-purpose
document model tractable, and it is the clearest answer in the corpus to "how do we stop every feature
from becoming a switch statement."

### History is public

[SOURCE-CODE] Diffs against **marks**; **bail** (`Alt`-switch, `Escape`); **squash** (`crop`); three
capture modes via `editor.run(fn, { history: 'record' | 'ignore' | 'squash' })`.

[INFERRED] **tldraw is the only product whose history boundary is a caller-chosen parameter.** That is
what makes it usable by tools, plugins and agents, and it is the one history capability Spool's AI and
extensibility ambitions both require.

### Notable absences

[SOURCE-CODE] **No built-in tools. No layout engine. No components. No pen tool. No node editing.**
The most architecturally sophisticated editor in the corpus has none of the content features the
mainstream product is known for.

### The AI path is the cleanest in the corpus

[SOURCE-CODE] A model emits a **simplified `_type` format**; it is sanitised; it passes through one
total conversion function; results stream. Context = screenshot + simplified in-view shapes + clusters
+ selection + recent actions + chat history. Verification = **export and inspect**. `@tldraw/driver`
synthesises real pointer/keyboard/wheel/pinch events through `editor.dispatch`.

---

## What We Learned From Affinity

### Studios are the shipped profile precedent

[DOCUMENTED] Vector, Pixel, Layout, Canva AI, Slice, Retouching, Color Grading, Typography,
Compositing, Astrophotography, Scripting — and **fully customisable**.

[INFERRED] A Studio is a **capability gate plus a panel layout plus a default tool**. It is *not*
primarily an interaction profile. That decomposition matters: Affinity treats "what exists" and "where
it lives" as one decision and leaves gestures alone.

### Snapping is where profiles already exist as a shipped feature

[DOCUMENTED] 7 named presets including **UI design** and **Pixel work**; 4 candidate-scoping modes plus
a Candidate List; colour-coded guides with **numeric distance labels**; per-layer **"Exclude From
Snapping"**; **"Force Pixel Alignment"**; **construction snapping** as a *different algorithm*;
some options action-scoped.

[INFERRED] Four ideas hide under "snapping": a preset bundle (a profile, scoped); a per-layer opt-out
(a **document field**, not a preference); distance labels (the snap **result must carry data**, not
just a nudge); and a second algorithm selected by mode. This directly contradicts tldraw's
minimal pure-nudge design, and the two are **not obviously combinable**.

### Paths first

[DOCUMENTED] Node Tool (`A`) as a **mode**; Sharp/Smooth/**Smart** nodes; Split/Break/Close/Smooth/
Join/Reverse; movable origin; Selection Box From Curves; winding fill mode. **Every created shape is
immediately editable as a path** — the opposite of Figma's "a rectangle until converted".

[INFERRED] This is a document-model decision expressed as a tool. A profile promising Affinity-class
expressiveness without a path model would be a menu of unimplemented tools.

### Modifier semantics are state-dependent — and shipped

[DOCUMENTED] `⌥`-click **cycles** overlapping objects (not "select behind"); `⇧`-marquee requires full
containment, `⌃`-marquee accepts partial intersection; **corner-handle modifiers split three ways**;
`⌃`-drag with a selection box active **mirror/shears** rather than moves; `.` cycles selection-box
shape; `⌥⌘G` / `⌥⇧⌘G` move in/out of a group; three flatten scopes.

[INFERRED] `⌃`-drag meaning two different things depending on an interaction state is modifier-as-mode
where the mode *is* the state machine. **It is shipped in a commercial product**, and it is exactly
the case `architecture/profiles.md` §5.3 names as the obstacle to faithful profile emulation. No
keymap configuration can express it.

### Scripting, MCP, and an unusually candid AI guidance

[DOCUMENTED] A JavaScript scripting API (beta) whose operations are *"often as a single undoable
action"*; a local MCP server for Claude Desktop with **per-capability approval** and privacy toggles;
workflow→script promotion; and the guidance that **"work with existing documents… designing from
scratch generally produces poor results."**

[INFERRED] Per-capability approval required first *defining what the capabilities are*. Approval is
downstream of naming. And "often as a single undoable action" is a candid admission that the scripting
API lacks a rigorous transaction boundary.

---

## What We Learned From Canva

### Approachability achieved by removing choices

[DOCUMENTED] **No persistent tool.** Every creation action is a one-shot command: `T R L C S ⌘\ ⇧;`.
There is no tool state to be in, so there is no tool lifecycle to get wrong.

[INFERRED] A "general-purpose environment" copying this would lose the very thing the other three
products treat as essential. The lesson is not "use one-shot commands"; it is "know what you gave up".

### The best-documented accessibility model in the corpus

[DOCUMENTED] `⌘F1` select / `⌘F2` direct select / `⌘F3` text edit, published **with an a11y rationale**.
Plus `F2` proportional resize as a **mode** rather than a modifier; a position panel with relative
modes so every transform is achievable without a chord or a drag; and **directional multi-select**
(`⇧W/A/S/D`) which gives a non-pointer path to "select everything to the right of here".

[INFERRED] Two of these cost Spool almost nothing — directional multi-select is a selection
operation, and the position panel is a panel. They are the highest-value conventions in the corpus per
unit of architectural cost. But note the structural catch: with persistent tools, a screen-reader user
must be told about *both* the focus mode and the tool state, which no product in the corpus currently
documents.

### Governance by copy

[DOCUMENTED] Brand Kit + Brand Controls **copy values** rather than referencing them. No variables, no
components. [INFERRED] The deliberate opposite of Figma, and it is why Canva templates stay editable
and predictable.

### Deep, attributed, unbounded version history

[DOCUMENTED] **1000 attributed versions** with avatars and **no time limit**; **30-day** Trash; asset
deletion blocked by reference.

[INFERRED] Compare Figma (branching, time-limited) — a different trade: retrieve-anything versus
branch-from-anywhere. A local-first Spool could approximate Canva's policy cheaply via autosave
rotation, which is worth knowing before designing a versioning subsystem.

### AI with named context sources

[DOCUMENTED] Canva AI 2.0 names **six context sources** and supports **scheduled tasks**; Magic Layers
converts a flat image into an editable layout; Magic Edit and Magic Resize operate on the document.

[INFERRED] Scheduled tasks are the strongest argument in the corpus that the operation layer must work
**without a window** — a scheduled AI task runs while the user is not watching.

---

## Common Design-Editor Conventions

Converged across products, and therefore the ones Spool should feel familiar for regardless of profile.

| Convention | Convergence | Evidence |
|---|---|---|
| **One continuous gesture = one undo step** | **4 of 4** | `matrices/history-matrix.md` §1 |
| **Selection and camera are never in history** | **4 of 4** | `matrices/history-matrix.md` §4 |
| **Escape cancels, then exits, then deselects** | **4 of 4** | `interaction-matrix.md` §5 |
| **Undo and version history are separate systems** | 2 of 2 that have both | `document-model.md` |
| Shape tools revert to Select after creation | Figma + tldraw verbatim | `profiles/figma.md`, `profiles/tldraw.md` |
| **The Text tool is the exception** — it stays active | Figma + tldraw verbatim | same |
| Click vs drag changes creation semantics | 3 of 4 | `interaction-matrix.md` §8 |
| `⇧`-constrain during move/resize | Figma, tldraw, Affinity | `interaction-matrix.md` §2 |
| `⇧`-arrow coarse nudge, `⌥`-arrow fine nudge | 3 of 4 | `keyboard-matrix.md` §5 |
| `⌘Z` / `⌘⇧Z`, `⌘D`, `⌘A`, `+`/`-` zoom | 3–4 of 4 | `keyboard-matrix.md` |
| `⇧1` zoom-to-fit | Figma + tldraw verbatim; 4 of 4 in some form | `keyboard-matrix.md` §6 |
| `⌘F1/F2/F3` focus ladder | Figma + Canva verbatim, **both accessibility leaders** | `keyboard-matrix.md` §2 |
| Space-drag and middle-drag pan | Figma, tldraw, Affinity | `interaction-matrix.md` §4 |
| Zoom about the cursor | 4 of 4 | same |
| Modifiers constrain the gesture, `⌘` changes the target | Figma's consistent rule | `interaction-matrix.md` §2 |
| Every drag has a click/drag threshold | tldraw + Spool explicit; universal in practice | `DRAG_THRESHOLD = 4.0` |
| **No minimap anywhere** | **4 of 4 absent** | `feature-matrix.md` §1 |

---

## Important Differences

Genuine disagreements, with the likely reason for each. **No ranking.**

| # | Disagreement | Figma | tldraw | Affinity | Canva | Likely why |
|---|---|---|---|---|---|---|
| 1 | **`Alt`-drag** | duplicate | **bail the gesture** | — | — | Mutually exclusive semantics for one key; the single largest profile blocker |
| 2 | **Double-click means** | descend a level | edit text / focus group | enter Node Tool | direct select | Each product's hierarchy model defines what "deeper" is |
| 3 | **Escape is** | up one scope | up one chart node | leave Node Tool | cancel | Chart-vs-state distinction is invisible to users |
| 4 | **Marquee** | ambiguous | **lasso** | rect, `⇧` contain / `⌃` intersect | rect | Freeform whiteboards vs precise design |
| 5 | **Layout engine** | 3 coexisting systems | **none** | Layout Studio | relative panel only | Migration pressure vs scope |
| 6 | **Reuse primitive** | full components | **none** | **none** | **none** | Different markets; tldraw/Affinity/Canva never needed it |
| 7 | **Variables** | 6 typed kinds | none | none | none — values *copied* | Reference vs copy is a governance philosophy, not a feature |
| 8 | **Vector paths** | pen only | **none** | **first-class** | none | Affinity is a vector editor; the others are not |
| 9 | **Snapping result** | ? | **pure nudge** | **colour + distance labels** | ? | The two richest documented designs are incompatible in shape |
| 10 | **Snap tolerance unit** | ? | **screen px ÷ zoom** | ? | ? | Only tldraw states one |
| 11 | **Persistent tools** | yes | yes | yes + Personas | **none** | Accessibility vs tool expressiveness |
| 12 | **Keyboard rebinding** | **no** (16 layouts) | host-defined | **full** | **no** | Products pick a side on user control |
| 13 | **Camera in shared state** | ◐ | **yes** | — | — | Whiteboard-as-link vs document-as-file |
| 14 | **Version branching** | **yes** | no version history | ? | **no** (1000 deep instead) | Retrieve-anything vs branch-from-anywhere |
| 15 | **Figma has no rebinding; Affinity has full rebinding** | | | | | Both are deliberate; neither is a bug |
| 16 | **Selection traversal** | tree order | **geometric scoring** | layer panel | `Tab` | Whiteboard navigation vs document navigation |
| 17 | **Tool persistence after use** | reverts | reverts | varies | n/a | Persona model |
| 18 | **Page model** | pages | pages | pages | **bounded page stack + 4 view modes** | Canva treats the canvas as one *presentation* of a document |

---

## Emerging Architectural Patterns

Concepts that appear in more than one product, extracted per §35.

| # | Concept | Products exposing it | Problem solved | Persistent / transient | In history | Affects render | Affects input | Affects AI |
|---|---|---|---|---|---|---|---|---|
| 1 | **Document** | 4 | canonical state | **P** | ✔ | ✔ | ✔ | ✔ |
| 2 | **Node / Object with stable id** | 4 | addressability | **P** | ✔ | ✔ | ✔ | ✔ **essential** |
| 3 | **Page** | 4 | bounded scope | **P** | ✔ | partial | partial | ✔ |
| 4 | **Container** | 4 (two kinds in Figma) | clipping, layout, scoping | **P** | ✔ | ✔ | ✔ | ✔ |
| 5 | **Tool** | 4 (absent in Canva) | a mode of creation | **T** | ✘ | ✔ | ✔ | ✔ |
| 6 | **Interaction state** | 4 | gesture phase | **T** | ✘ | ✔ | ✔ | ◐ |
| 7 | **Selection** | 4 | the object of operations | **T** | **✘ all 4** | ✔ | ✔ | **✔ essential** |
| 8 | **Camera** | 4 | viewport | **T** (tldraw: also in links) | **✘ all 4** | ✔ | ✔ | ✔ (context) |
| 9 | **History / Transaction** | 4 | reversibility | **T** (log) | *is* history | ✘ | ✔ | **✔ essential** |
| 10 | **Operation** | tldraw, Affinity; Figma/Canva implicit | intent vs mutation | **T** | ✔ | via result | via invocation | **✔ essential** |
| 11 | **Binding** | Figma, tldraw, Affinity | a live relation | **P** | ✔ | ✔ | ◐ | ✔ |
| 12 | **Style** | 4 | appearance | **P** | ✔ | ✔ | ✔ | ✔ |
| 13 | **Asset** | 4 | external resource | **P** + ref | ✔ | ✔ | ◐ | ✔ |
| 14 | **Component / Instance** | **Figma only** | reuse | **P** | ✔ | ✔ | ✔ | ✔ |
| 15 | **Layout / Constraint** | Figma, Affinity, Canva(◐) | adaptive arrangement | **P** | ✔ | ✔ | ✔ | ✔ |
| 16 | **Snap engine** | Figma, tldraw, Affinity | precision | **T** (+ per-layer opt-out in Affinity) | ✘ | ✔ | ✔ | ✔ |
| 17 | **Input system** | 4 | device → meaning | **T** | ✘ | ✘ | **is** input | ✘ |
| 18 | **Renderer** | 4 | state → pixels | **T** | ✘ | **is** render | ✘ | **✔ (context/export)** |
| 19 | **Spatial index** | tldraw, Figma(disclosed) | hit test, culling | **T** derived | ✘ | ✔ | ✔ | ◐ |
| 20 | **Action registry** | tldraw, Affinity; Canva `/` | one invocation surface | **T** | ✘ | ✘ | **is** input | **✔ essential** |
| 21 | **Instance/session state** | **tldraw only, named** | state tiers | **T** | ✘ | ✔ | ✔ | ◐ |
| 22 | **AI operation** | tldraw, Affinity (MCP) | agent vocabulary | **T** | ✔ | via result | ✘ | **is** AI |
| 23 | **Prototype graph** | Figma, Canva(◐) | interaction model | **P** | ✔ | ✘ | ◐ | ✔ |
| 24 | **Version history** | Figma, Affinity, Canva | long-horizon time | **separate store** | *is* history | ✘ | ✘ | ✔ |

**Read this table for the "essential for AI" column.** Nine of the concepts an agent needs are
already needed by a human editor. Only #22 is new. [INFERRED] That is the strongest argument in the
corpus for building the editor properly first and treating the AI runtime as a caller.

**Pattern: the concepts that generalise are interfaces, not enums.** tldraw's `ShapeUtil`, its store
scopes, its tool config table, its action gating — all are interfaces or records that a third party
fills in. Every product's content features are enums. [INFERRED] For "VS Code for visual design",
this is the single most actionable observation in the corpus: **what makes tldraw general-purpose is
that its extension points are types, not lists.**

---

## Persistent Document State

What must survive closing the file, per the corpus.

| Category | Contents | Evidence |
|---|---|---|
| **Structure** | objects, identity, z-order, hierarchy, parent links | all 4 |
| **Pages** | page membership and order | all 4 |
| **Geometry** | position, size, rotation, corner radius | all 4 |
| **Appearance** | fill, stroke, opacity, effects, blend mode | all 4 |
| **Typography** | text content, font, size, line height, spacing | all 4 |
| **Reuse** | components, instances, variants, libraries | Figma |
| **Values** | variables, collections, modes | Figma |
| **Layout** | constraints, auto layout, sizing modes | Figma, Affinity |
| **Relations** | bindings, connectors | Figma, tldraw, Affinity |
| **Assets** | references + metadata (**not** necessarily bytes) | all 4 |
| **Prototype** | triggers, actions, transitions, flows | Figma, Canva |
| **Guides** | guides, grids, rulers | **Affinity** |
| **Per-object flags** | `Exclude From Snapping`, locked, hidden | **Affinity** (snapping) |

[INFERRED] Two rows are routinely mis-classified:

- **Guides and per-object snapping opt-out are document state in Affinity, not session state.** The
  natural assumption is that guides are a view aid. Affinity's are content.
- **Assets are references, not bytes.** That is why all four have an undocumented missing-asset
  problem, and why the answer belongs in the document model rather than in a renderer.

---

## Transient Editor State

What must not survive, per the corpus.

| Category | Contents | Notes |
|---|---|---|
| **Selection** | selected ids, scope, depth | ✘ in history, **4 of 4** |
| **Camera** | offset, zoom, viewport | ✘ in history, **4 of 4** |
| **Hover** | hovered id | never in history |
| **Tool** | active tool / mode | never in history |
| **Gesture** | drag, marquee, resize, in-progress creation | cancelled, not committed |
| **Snap preview** | candidates, indicators, nudge | never in history |
| **Text session** | caret, selection range, IME composition | committed as one step |
| **Transient modifier flags** | space-held, `Alt`-bail state | **Spool's is unresettable** |
| **Panels** | open/closed, widths, layout | never in history |
| **Presence** | cursors, selections of others | tldraw: its own store scope |

[INFERRED] The tiering itself matters. tldraw names three levels — `document`, `session`, `presence` —
and that taxonomy was forced by multiplayer, not invented for clarity. Spool has no presence and would
be choosing a taxonomy without the pressure that produced it.

[INFERRED] One category is unique to Spool as a *defect*: `space_held` is a transient device flag with
**no defined reset path**, because no focus/blur hook is registered despite a `FocusHandle` existing.
Every product's escape hatch assumes transient flags can be cleared.

---

## Interaction State Machines

### The evidence

[SOURCE-CODE] **tldraw**: a hierarchical `StateNode` chart. Handlers per event; unhandled events fall
through to children; transitions deliver payloads to `onExit` and `onEnter`; re-dispatch to a
`target` (`canvas` / `shape` / `handle` / `selection` / `overlay`); per-tool static config including
`useCoalescedEvents` and `isLockable`.

[INFERRED] **Figma**: a chart implied by `nestingDepth` — idle → hovered → selected → editing →
dragging → resizing → rotating → marquee — with double-click as a descent transition and `Enter`/`⇧Enter`
as depth transitions. Not published as a chart, but every documented behaviour is consistent with one.

[INFERRED] **Affinity**: a chart implied by Studio → Persona → Tool → sub-mode. `A` toggles Node Tool;
Node Tool has corner/smooth/smart node sub-states; Transform Mode is a persistent numeric mode.

[INFERRED] **Canva**: the smallest chart in the corpus, because there are no tools. Idle → focus mode
(×3) → text session. It exists because focus mode has to be *something*.

### The `PotentialX → X` pattern

[SOURCE-CODE] tldraw names the pre-commit gesture state explicitly; Spool reproduces the shape:

| Gesture phase | tldraw | Spool |
|---|---|---|
| Idle | idle | `Interaction::None` |
| Armed, threshold not crossed | *(idle — threshold checked inside the tool)* | `PotentialMove`, `PotentialResize`, `PotentialCreate` |
| Committed | moving / resizing / creating | `Moving`, `Resizing`, `Creating` |
| Cancel | `Esc` → parent node, bail to mark | `Interaction::restore()` ✔ |
| Complete | commit → parent node | commit ✔ |
| Interrupt | **not a separate path** | **not modelled** |

[SOURCE-CODE] `DRAG_THRESHOLD = 4.0` with a pure `drag_threshold_crossed(start, current)` predicate.
[INFERRED] This is a good pattern and one of the few places the prototype is already ahead of a
naive reading.

### The cancel / complete / interrupt triad

[SOURCE-CODE] tldraw distinguishes three exits. **Complete** commits. **Cancel** restores to the parent
state. **Interrupt** — a gesture terminated by something outside the interaction, e.g. losing pointer
capture — is the one exit with no explicit handler in the corpus.

[INFERRED] Every product handles complete and cancel. **Nobody documents interrupt.** Spool has pointer
capture with no observable release-on-loss path, so an interrupted `Moving` state is a live
corruption candidate.

### The priority chain is the weakest link in Spool's design

[SOURCE-CODE] `begin_left_interaction` resolves by fixed priority:
`pan → text edit → tool → resize handle → move → marquee`.

[INFERRED] A fixed chain cannot express *"the shape handles this, otherwise the canvas does"*, which is
exactly what tldraw's fall-through plus `target` re-dispatch expresses. It also cannot express Affinity's
`⌃`-drag-means-mirror-when-a-selection-box-is-active, because that depends on interaction state rather
than on a fixed ordering. See `architecture/interaction-runtime.md` for the reconstructed charts and
the five candidate implications.

---

## Operations / Transactions

### The distinction

[INFERRED] Every product distinguishes **intent** from **mutation**, whether or not it names the
concept:

| Product | How the distinction shows |
|---|---|
| tldraw | ✔ **`Action` registry** + `editor.run(fn, {history})` — explicit, public, caller-chosen |
| Affinity | ✔ **scripting API** — object commands; local MCP server exposes them per capability |
| Figma | ◐ plugin API; MCP Server; "actions" in prototype |
| Canva | ✔ `/` and `⌘E` quick-actions palette |
| **Spool** | ✘ **no operation layer**; gestures mutate `Document` directly |

### Why this is the keystone

[INFERRED] The operation registry is the only abstraction that serves **keyboard, menus, toolbars,
scripts, plugins, and agents** simultaneously. It is also the only place where the corpus's two hardest
requirements meet:

- **Profiles** need a *named invocation surface* to rebind. `architecture/profiles.md` Implication A.
- **AI** need a *named vocabulary* to target. Affinity had to invent one to build per-capability
  approval. `architecture/ai-runtime.md` Implication A.

[INFERRED] Everything that makes Spool "general-purpose" points at the same missing concept.

### What an operation layer does *not* need to be

[INFERRED] It does not need to be a trait object with a dynamic vtable, nor a serialisable command
language, nor a plugin ABI. The minimal shape that unblocks keyboard + menus + agents is:

- a **name** (stable, namespaced),
- a **declared set of preconditions** (so availability can be queried, as GPUI's `is_action_available`
  already supports),
- **one apply function** that is the *only* sanctioned way to mutate the document.

That third property is the whole discipline. Everything else is negotiable.

---

## History

### What is settled

[INFERRED] **The user-visible contract is universal and Spool already matches it**: one continuous
gesture = one undo step; a text session = one step; a multi-selection change = one step; no-op
changes filtered; redo cleared on a new change; **selection and camera excluded in 4 of 4.**

### Where the products actually diverge

| Axis | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Representation | ? internal | **diffs + marks** | ? | ? | **before/after snapshots** |
| Bail (revert to mark, keep gesture) | discard | ✔ `Alt` / `Esc` | discard | discard | discard |
| Squash | implicit | ✔ `crop` | ? | ? | **✘** |
| Caller-chosen scope | ? internal | ✔ **`editor.run`** | ◐ "often" | ? | **✘** |
| Selection in history | ✘ | ✘ | ✘ | ✘ | ✘ ✔ |
| Version history | ✔ + **branching** | — | ✔ | ✔ **1000, unbounded, attributed** | **✘** |
| History owned by | document | store | document | document | **the view** |

[INFERRED] The mechanism debate (snapshots vs diffs) is about **cost at scale**, not behaviour. tldraw's
choice is forced by its reactive store; Spool's is currently unforced and unmotivated — which means it
is still cheap to change.

### Spool's structural gaps

[SOURCE-CODE] `History` is **owned by `CanvasView`**, so it cannot be shared, serialised, or tested
alone. `undo()` writes to the document directly — safe by construction today, fragile the moment a
store abstraction exists. No marks, no bail, no squash, no pause, no caller-chosen scope.

[INFERRED] The two that block other work are **caller-chosen scope** (blocks agents and plugins) and
**ownership** (blocks everything). The rest are cheap.

---

## Rendering

### Disclosed engineering

[ENGINEERING-DISCLOSED] **Figma**: C++/TS client; a named **scene graph**; WebGL → **WebGPU (Sep
2025)**; the "Keeping Figma Fast" performance-test framework (2023); the 2015 rationale for a custom
renderer.

[SOURCE-CODE] **tldraw**: eight named performance techniques, including viewport **culling** (never
cull selected or editing shapes), **`getEfficientZoomLevel`** with a 500 ms debounce,
`steppedScreenScale`, `textShadowLod 0.35`, `maxShapesPerPage 4000`, and a `PerformanceManager` keyed by
state path. Plus the **identity-stability rule**: changing a shape's props replaces its React element
unless identity is preserved.

### The central question: document → scene graph, or document → elements?

[INFERRED] 2 of 2 — tldraw's disclosure and Figma's — support a **transformed** representation. Both
build a separate scene graph from the document. Spool renders document objects directly into GPUI
elements.

[INFERRED] The separation buys five things: (1) partial invalidation without re-deriving document state;
(2) headless rendering for **export and AI context**; (3) a **level-of-detail** policy independent of the
document; (4) a place for derived caching (geometry, text shaping, image scaling); (5) decoupling the
persistent hierarchy from the framework's view hierarchy — which §39 of the brief explicitly warns
about.

### Zoom-derived quantities are four, not one

[SOURCE-CODE] tldraw derives from zoom independently: cull set, efficient zoom level (debounced),
stepped screen scale (for images), and text shadow LOD. Spool derives: cull padding (`CULL_PADDING =
64.0` world units), grid density (24–48 screen px), dot size, line height.

[INFERRED] Four distinct quantities per frame is normal, not a smell. The absence of a debounced
"efficient zoom" is the notable gap, because it is what keeps culling stable while the user is
continuously zooming.

### The GPUI constraint

[SOURCE-CODE] From `app/PHASE15.md`: entity granularity, upward dirty propagation, and
`window.refreshing` forcing nested cached views to re-render mean **per-object render isolation is not
achievable in GPUI as configured**.

[INFERRED] Therefore a paint tree would be an **export and AI feature, not a performance feature**. This
is a finding about the framework, not about Spool's architecture, and it should inform the eventual
UI-technology decision rather than trigger premature optimisation.

---

## Input

### The finding that changes cost estimates

[SOURCE-CODE] **GPUI already ships a complete action and keymap system**, verified in the vendored
source at rev `397cbc84`:

| Capability | Present? | Where |
|---|---|---|
| Named action registry, `actions!` macro, JSON-constructible actions | ✔ | `action.rs` |
| Duplicate-registration panic at `App` creation | ✔ | `ActionRegistry` |
| Multi-key chords, with pending state and `to_replay` | ✔ | `keymap.rs` |
| Per-frame hierarchical **dispatch tree** | ✔ | `key_dispatch.rs` |
| **Context stack** + a predicate language (`>`, `&&`, `\|\|`, `!=`, `!`) | ✔ | `keymap/context.rs` |
| Source-prioritised user overrides; `NoAction` / `Unbind` | ✔ | `keymap.rs` |
| **Availability queries** for menu generation and disabled items | ✔ | `is_action_available` |
| Chord-completion queries for a cheat sheet | ✔ | `possible_next_bindings_for_input` |
| Two-phase action delivery (capture + bubble) | ✔ | `DispatchPhase` |
| `secondary-` modifier: one string, both platforms | ✔ | `keystroke.rs` |
| Platform keyboard **layout** + **mapper** abstraction | ✔ | `platform/keyboard.rs` |
| Full IME contract: UTF-16 ranges, composition, candidate-window bounds | ✔ | `platform.rs` |
| `prefers_ime_for_printable_keys` | ✔ | `platform.rs` |

[INFERRED] **Spool's action registry is an adoption and ownership decision, not a construction
project.** Every keyboard-adjacent implication elsewhere in this corpus — user remapping, context-scoped
shortcuts, generated menus, chord commands, keyboard cheat-sheet — is already *mechanism*.

### What GPUI does *not* have

[SOURCE-CODE, by absence] on any pointer event: `pointer_type` (mouse/pen/touch), `pressure`, tilt,
a coalesced-event flag, a multi-button bitmask.

[INFERRED] Consequence: Spool cannot currently distinguish stylus from finger, cannot read pressure,
and cannot request coalesced high-frequency moves for freehand. Those are exactly the inputs Affinity's
brush and pen ecosystem needs — a **framework-level constraint** that belongs in the eventual
UI-technology decision.

### Spool's actual state

[SOURCE-CODE] Two parallel keyboard paths that do not know about each other:

- **Path A** — GPUI actions: 12 `spool_text` actions, 16 bindings, **all with `context: None`**. Covers
  clipboard and caret only.
- **Path B** — a raw `on_key_down` string matcher in `shell.rs`: undo, redo, delete, duplicate, all
  seven tool shortcuts, `space`, `⇧1`, Escape.

| Consequence | Evidence |
|---|---|
| `backspace`/`delete` bound in **both** paths | `main.rs:15-16`, `shell.rs:64` |
| User remapping would remap only half the surface | structural |
| Tool guards omit `alt` | `shell.rs:65-73` |
| **Tool shortcuts will fight a CJK IME** — `prefers_ime_for_printable_keys` unused | `shell.rs` |
| `cmd-`/`ctrl-` duplicated by hand instead of `secondary-` | `main.rs:21-30` |
| `space_held` has no focus-loss reset | no `on_focus`/`on_blur` registered |

[INFERRED] Two items are cheap, concrete, and verifiable: adopt `secondary-` and implement
`prefers_ime_for_printable_keys`. Both are one-line-ish changes with disproportionate correctness value.

### Pointer and device

[SOURCE-CODE] Pointer capture is `Rc<Cell<Option<CanvasHitbox>>>` + `window.capture_pointer`, with no
observable release-on-loss path. Pan is `Middle || (Left && space_held)`, hard-coded. Zoom is gated on
`control || platform` — the same dual-write smell as the `cmd`/`ctrl` bindings.

---

## Layout

[INFERRED] **The area with the least convergence in the corpus.**

| Product | Approach | Structural consequence |
|---|---|---|
| Figma | Auto layout + constraints + absolute, coexisting, with an escape hatch | Three systems and a migration path; a child's sizing mode constrains its parent |
| tldraw | **None** | A shape's geometry is authoritative; no layout pass exists |
| Affinity | Layout Studio + constraints | Studio-gated capability |
| Canva | Relative position panel only | Panel-first precision, no engine |
| Spool | **None** (absolute only) | Same as tldraw |

[INFERRED] Three observations worth carrying:

1. **tldraw, the most architecturally sophisticated product, has no layout engine.** Layout is not a
   prerequisite for a serious editor.
2. **"Hug container cannot clip"** in Figma is the clearest evidence in the corpus that a container
   model's requirements are genuinely incompatible and force type-level decisions.
3. **Figma's three coexisting systems are a migration artefact.** Any Spool layout decision should
   assume the same pressure arrives — which argues for designing an escape hatch now rather than a
   unified model later.

---

## Components

[INFERRED] **Three of four products have no reuse primitive at all.** Duplication is a copy.

| Concept | Figma | Others |
|---|---|---|
| Component definition | ✔ | **—** |
| Instance = reference + property overrides | ✔ | **—** |
| Variants / variant properties | ✔ | **—** |
| Nested components | ✔ | **—** |
| Libraries, local and remote | ✔ | **—** |
| Publishing | ✔ | **—** |
| Detach instance | ✔ | **—** |
| Reuse of any kind | ✔ | ✘ |

[DOCUMENTED] Figma's instance model in particular: a reference to a component, a **property overlay**
addressed by **paths into nested layers**, a **hidden-children mask**, a variant selection; children are
locked; an instance swap is required when the variant's structure differs.

[INFERRED] The overlay-by-path detail is the important one: overrides are not stored on the instance's
objects but as **paths from the instance root**. That is what lets an instance survive edits to its
source. It is also the hardest part of the feature and the part nobody else has solved.

[INFERRED] **There is no incremental path to reuse from "duplicate a copy".** If Spool wants components,
it must design them from first principles, because no product in the corpus offers a stepping stone.
This is a genuine open strategic question, not an implementation task.

---

## Assets

[DOCUMENTED] Images, SVG, fonts, video, stock libraries, templates, embedded-vs-linked, replacement
without reflow, and reference-blocked deletion — all four products support the basics.

[INFERRED] The interesting rows are the undocumented ones:

| Question | Status |
|---|---|
| **What happens when an asset is missing?** | **undocumented in all 4** |
| **What happens when a font is missing?** | **undocumented in all 4** |
| Are assets embedded or linked? | ✔ linked (Figma, tldraw) |
| Can deletion be blocked by a reference? | ✔ Canva (documented) |
| Variable-font axis handling | **undocumented in all 4** |

[INFERRED] Missing-asset behaviour is the largest pure gap in the corpus. A dangling reference is a
first-class document state that must be *renderable* (placeholder), *serialisable* (round-trippable),
*undoable*, and *AI-addressable* (an agent must be able to see that it is broken). Every product has this
state; none documents it. **The answer belongs in the document model, not in a renderer.**

---

## AI

### What is settled by the market

[INFERRED] **Output form: native document objects.** Not pixels. Not HTML/CSS. Figma's Design Agent,
tldraw's shape conversion, Affinity's Studio generation, and Canva's Magic Layers all produce or
convert to editable structure. HTML/CSS appears only as a *developer-handoff export*.

[INFERRED] **Generation into existing structure beats generation from nothing.** Affinity documents it
explicitly; tldraw's architecture (focused shapes, viewport-bounded context) and Canva's template-first
model imply it. Three routes, one conclusion.

[INFERRED] **Agent changes route through ordinary undo in every product.** Nobody distinguishes AI
history from human history. That convention is nearly free — provided the agent calls operations rather
than mutating state.

### What the corpus has not solved

| Capability | Any product? |
|---|---|
| Preview / staged application | **no** |
| Dry-run before commit | **no** |
| Invariant checking after apply | **no** |
| Per-operation approval | Affinity only (per-*capability*) |
| Diff shown to the user | no |
| Provenance / "this was AI-generated" | **undocumented in all 4** |
| Are partial AI results visible? | undocumented |
| Contention between an agent and a live user | undocumented |

[INFERRED] **This is open space, not a gap Spool is behind in.** And preview is the one item where a
persistent document with an operation layer would pay off automatically: staging is free when operations
return a patch, and expensive when they mutate a view.

### The three properties that make an agent a participant

[INFERRED] Not the operation vocabulary — three properties:

| | Property | Requires |
|---|---|---|
| **P1** | **Addressable identity** | stable, ideally typed ids — tldraw's branded `shape:abc` |
| **P2** | **Observable outcome** | a query surface **and** an offscreen render for screenshots |
| **P3** | **Reversibility** | history capture boundaries, not per-operation undo spam |

Plus an implied fourth: **P4, bounded blast radius** — Affinity's per-capability approval.

[INFERRED] P1–P3 are satisfied by an operation layer and by nothing else.

### Spool readiness

| Prerequisite | Present? |
|---|---|
| A document model an operation could target | ◐ flat, unserialisable, no hierarchy |
| **A named operation vocabulary** | ✘ (12 text actions) |
| **Caller-chosen history capture boundaries** | ✘ |
| **Offscreen render** for screenshots and verification | ✘ |
| Model-facing restricted vocabulary + sanitisation | ✘ |
| Any AI surface | ✘ (`ai_inspector()` is chrome) |

[INFERRED] **Two of the six are not AI features at all.** They are general editor capabilities —
rendering headlessly, and a caller-chosen history boundary — that AI happens to require first.
Planning the AI roadmap as "AI work" will mis-sequence it.

---

## Potential Interaction Profiles

[INFERRED] Evidence-only. **No profile is proposed or recommended here.**

### What the products actually expose

| | Invocation | Modifier semantics | Tool vocabulary | Panels/layout | Gestures |
|---|---|---|---|---|---|
| Figma | 16 layouts, no rebinding | fixed | fixed | fixed | fixed |
| tldraw | host-defined registry | host-defined | host-defined | host-defined | host-defined |
| Affinity | **fully customisable** | fixed | **by Studio** | **by Studio** | fixed |
| Canva | fixed | fixed | fixed | fixed | fixed |

**No product exposes modifier semantics or gesture behaviour as configuration.** This is the single
most important negative finding in the corpus, and it is the part of "profiles" that people most often
assume exists.

### The five separable axes

| Axis | Kind of configurability | Who varies it |
|---|---|---|
| A. Keymap | preference | user (Affinity ✔, Figma partly) |
| B. Tool set | **profile identity** | Affinity Studios; tldraw host |
| C. Contexts / focus modes | **profile identity** | Canva `⌘F1/F2/F3`; GPUI `KeyContext` |
| D. Snapping rule bundle | preference | **Affinity's 7 presets** |
| E. Modifier semantics / gestures | **architecture — nobody** | no product |

[INFERRED] A realistic profile varies **A–D** and not E. That is a much smaller promise than
"choose Affinity and Alt-drag mirrors", and it is the one the evidence supports.

### The shipped precedent

[DOCUMENTED] **Affinity's snapping presets** are an interaction profile, scoped to one subsystem,
shipped in a shipping product: 7 named bundles including *UI design* and *Pixel work*.

[INFERRED] This is the cleanest available proof that the concept is reasonable, and it suggests the
first Spool profile attempt should be narrow — a snapping preset, or a keymap plus a tool set — rather
than a whole-editor emulation.

### What a profile must never reach

[PROPOSED, from `architecture/profiles.md` Implication B] A profile must not be able to change:

- the document model,
- the operation set,
- **history capture granularity**,
- rendering.

If a profile can change capture scope, "one undoable action" stops being a property of the operation
and becomes a property of the session, and undo tests become profile-dependent. **No product exposes
history granularity to users**, which is weak evidence that this is right.

### Per-profile reproduction cost

| Profile | Tier 1 (cheap, high fidelity) | Tier 2 (needs document work first) |
|---|---|---|
| **figma** | 10 conventions — all gesture-level | 3 — auto layout coexistence, depth model, tool exclusions |
| **tldraw** | 8 — but Alt-bail needs history marks | 4 — lasso, geometric `Tab`, bail, `ShapeUtil` hooks |
| **affinity** | 6 — Studios, presets, guides, customisation | 6 — **all need a path model** |
| **canva** | 7 — but #1 contradicts persistent tools | 3 — templates, Brand Controls, Magic Layers |
| **spool** | baseline | see `profiles/spool.md` §11 |

[INFERRED] The recurring pattern: **almost every Tier 2 item requires a document model Spool does not
have.** A profile offered before that model exists would be a menu of unimplemented tools — which is
exactly what `Pen` and `Comment` already are.

---

## Major Architectural Questions For Spool

Questions Spool must answer **before implementation continues**. Each is stated with the evidence that
makes it a question rather than a preference. None is answered here.

| # | Question | Why it is a question | Evidence |
|---|---|---|---|
| **Q1** | **Is the document separable from the view that edits it?** | Everything else follows. Owned-by-a-view means: not openable twice, not testable headlessly, not renderable to a file, not passable to an agent. | `History` and `Document` are fields of `CanvasView`; two independent architecture notes converge on this |
| **Q2** | **What is the operation registry, and who owns it?** | It is the single abstraction serving keyboard, menus, toolbars, scripts, plugins, and agents. GPUI supplies the mechanism; Spool must supply namespace, ownership, content, and the elimination of the second key path. | `action.rs`; `architecture/input-system.md` Implication A |
| **Q3** | **Is interaction a state chart or a priority chain?** | A fixed priority chain cannot express "shape handles this, otherwise canvas", nor Affinity's state-dependent modifier meaning. | tldraw's fall-through + `target` re-dispatch; Spool's `begin_left_interaction` |
| **Q4** | **What are the object types, and how is the set extended?** | tldraw is the only product with a documented extensible object model. "VS Code for visual design" is a claim about this row. | `matrices/document-model-matrix.md` §4 |
| **Q5** | **Is hierarchy first-class, and is a group a different *type* from a frame?** | Figma needs two container types because derived bounds and constraints are incompatible. Affinity needs three flatten scopes. Both are type-level facts. | `document/hierarchy.md`, `profiles/figma.md` §5 |
| **Q6** | **How is undo represented — snapshots, diffs, or operations?** | Cheap to change today (the choice is currently unmotivated); expensive once history is load-bearing for agents and plugins. | `architecture/history.md` |
| **Q7** | **Is there a paint/scene representation between the document and GPUI?** | Decides export, AI context, offscreen verification, LOD, and whether the persistent hierarchy is coupled to the view hierarchy. | `architecture/rendering.md` |
| **Q8** | **Can the renderer draw headlessly?** | Blocks AI context (screenshots), AI verification, batch export, and preview. | `matrices/ai-matrix.md` §10 |
| **Q9** | **What is missing-asset behaviour?** | Undocumented in all four products. The state must exist in the document model regardless of who invents it. | `matrices/document-model-matrix.md` §8 |
| **Q10** | **Are variables references or copies — or a third thing?** | Figma references, Canva copies. Spool must pick or invent. Governs every future design-system feature. | `matrices/document-model-matrix.md` §6 |
| **Q11** | **Does Spool have components at all, and if so how?** | Three of four products have none; there is no incremental path from copying. A genuine strategic question. | `matrices/document-model-matrix.md` §5 |
| **Q12** | **Does Spool have a layout engine?** | tldraw, the most sophisticated product, has none. Figma's answer is three coexisting systems because of migration. | §Layout above |
| **Q13** | **What belongs in a profile, and can a profile touch history?** | GPUI makes A–D cheap. E (modifier semantics) is the hard case and nobody has solved it. | `architecture/profiles.md` |
| **Q14** | **Is the UI technology final?** | GPUI cannot deliver pressure, tilt, `pointer_type`, or coalesced events — all needed for Affinity-class vector work. | `architecture/input-system.md` Implication D |
| **Q15** | **What is the model-facing vocabulary, and how is untrusted output admitted?** | tldraw's sanitise-then-convert discipline is the pattern. Spool will have ≥5 external-input paths. | `architecture/ai-runtime.md` Implication B |

---

## Unresolved Questions

Genuinely open. Grouped by who could answer them.

### Undocumented in all four products

1. Missing-asset behaviour (placeholder? crash? silent drop?) — **Q9**
2. Missing-font behaviour and fallback metrics
3. Variable-font axis handling
4. Reparenting coordinate preservation — does a child keep its offset, its size, or its layout slot?
5. Per-operation AI approval (Affinity has per-*capability*)
6. Whether partial AI output is visible mid-stream
7. Provenance metadata for generated objects
8. Behaviour when a drag target is unmounted mid-gesture (pointer-capture loss)
9. Snap tolerance in Figma and Affinity
10. Click/drag thresholds in every product except Spool's explicit `4.0`

### Disagreements with no consensus to defer to

11. Latching vs re-reading modifiers mid-gesture (Figma latches; Affinity re-reads)
12. Rectangular marquee vs lasso
13. Tree-order vs geometric traversal
14. Reference vs copy for brand values
15. Branching vs deep-unbounded version history

### Questions about Spool specifically

16. Is the flat vector z-order with hard-coded `LANDING`/`EDITOR`/`FEATURES`/`MOBILE` ids a starting
    point or a constraint to be undone? It is currently the *only* thing distinguishing the four
    "pages".
17. How should a document record — or not record — the profile it was last edited under?
18. What happens to saved keymaps when a profile disables an action they reference?
19. Should `Pen` and `Comment` remain selectable-but-inert, or be removed until implemented?
    [PROPOSED] inert tools are a demonstrated pattern in Spool's own prototype and were found in no
    product in the corpus.

### Questions this research could not reach

20. Figma's internal interaction, document, and input architecture — no engineering source describes it.
21. Affinity's internal architecture — the scripting API and MCP server are the only windows.
22. Canva's interaction internals — documentation is outcome-oriented, not mechanical.
23. tldraw.com's actual tool list — the library ships none; the consumer decides.

---

## Research Gaps

What a second research pass should prioritise, in order.

| # | Gap | Why it matters | Suggested method |
|---|---|---|---|
| **G1** | **Figma's depth/scope model, exhaustively** | The single most-copied interaction idea, and never documented as a model | Careful behavioural observation, recorded as a state chart |
| **G2** | **Figma's drag-reparenting rule under Auto Layout** | Documented as behaviour, not as a rule; needed by any `figma` profile | Focused observation across container types |
| **G3** | **Affinity's three corner-handle modifier behaviours** | Exists and is documented as existing; not enumerated | Affinity help pages on resize modifiers |
| **G4** | **tldraw's marks/bail/squash, as source** | Highest-value transferable mechanism; a source read would settle it | `packages/editor` history + side-effects modules |
| **G5** | **Figma's WebGPU renderer, in detail** | The best-disclosed rendering engineering in the corpus | The Sep 2025 post plus the "Keeping Figma Fast" series |
| **G6** | **Canva's template document model** | Determines whether template-first is a document feature or a UI feature | Canva templates / app docs |
| **G7** | **Missing-asset and missing-font behaviour, all four** | Largest pure gap; whoever answers first sets the norm | Deliberate breakage testing |
| **G8** | **GPUI's retained-dispatch-node behaviour under entity churn** | Affects whether a Spool state machine can publish `KeyContext` | Local experiment on the vendored source |
| **G9** | **Affinity's scripting API operation list** | The closest thing in the corpus to a documented operation vocabulary | affin.co/affinity-sdk |
| **G10** | **tldraw.com's actual tool set and defaults** | Needed before any `tldraw` profile can be authored | Observation; `llms.txt` and the docs site |

[INFERRED] **G1, G2, G4 and G9 are the highest value**, because each closes a gap that blocks a
specific implementation decision rather than adding colour to a general picture.

---

## Recommended Next Investigation

[INFERRED] In priority order, with the reasoning.

### 1. Read tldraw's history and side-effect modules as source (G4)

[INFERRED] Marks, bail and squash are the highest-leverage transferable mechanisms found, and they
block three separate Spool questions: profiles (Alt-bail), agents (caller-chosen scope), and history
representation. A source read would settle all three.

### 2. Read Affinity's scripting API operation list (G9)

[INFERRED] It is the only documented operation vocabulary in the corpus, produced by a company whose
agent integration already exists. Even a partial list would give Spool's Q2 something concrete to react
to — including what a professional tool considers an *operation* rather than a setting.

### 3. A focused Figma interaction pass on depth and reparenting (G1, G2)

[INFERRED] These two are the difference between a `figma` profile that feels right and one that is
almost right in a way users notice immediately.

### 4. A local experiment on GPUI's dispatch tree (G8)

[INFERRED] Cheap, and it answers whether the framework's context model can carry an interaction state
machine — which is the hinge of `architecture/input-system.md` Implication E.

### 5. Deliberate-breakage testing for missing assets and fonts (G7)

[INFERRED] Closes the largest pure gap in the corpus, and the answer belongs in the document model, so
it must exist before the document model is designed rather than after.

### 6. Only then: the governing documents

[PROPOSED] The intended sequence is `ARCHITECTURE.md`, then `AGENTS.md`, `DESIGN.md`,
`PRODUCT REQUIREMENTS.md`, `PLAN.md` — with `ARCHITECTURE.md` separated from `AGENTS.md` because
*"how an agent must work in this repository"* and *"what the system actually is"* are different
documents with different lifetimes.

[INFERRED] The corpus points at the **order** those questions should be answered in: Q1 (document/view
separation) and Q2 (operation registry) gate almost everything else; Q4 (extensible object model) and
Q13 (profile scope) determine the long-term shape; Q11 (components) and Q12 (layout) are strategic and
should be answered deliberately rather than by default. **A `PLAN.md` written before Q1 and Q2 would
be a plan against an architecture that had not been chosen.**

[PROPOSED] One rule worth carrying forward, which this corpus supports without prescribing anything:

> **No feature is considered complete merely because it works. It is complete when its behaviour,
> document semantics, history semantics, interaction semantics, rendering implications, and
> architectural ownership are coherent with the rest of Spool.**

[INFERRED] Applying that rule to Spool's current prototype produces an unexpected result worth stating:
the six conventions the prototype already matches (`profiles/spool.md` §11) are precisely the six that
required **document** work rather than handler work. That is a small piece of evidence that the
prototype's instincts are aimed correctly, and a warning that the fastest-looking path — writing more
gesture handlers — is the one that will not compound.

---

## Document index

| Section | Files | Count |
|---|---|---|
| Synthesis | `README.md` | 1 |
| Products | `products/{figma,tldraw,affinity,canva}.md` | 4 |
| Interaction | `interaction/{selection,transformation,snapping,text-editing,drawing,navigation,keyboard,input}.md` | 8 |
| Document | `document/{objects,hierarchy,components,styles,layout,assets,bindings,variables}.md` | 8 |
| Creative | `creative/{vector,raster,typography,animation,export}.md` | 5 |
| AI | `ai/{generation,editing,structured-generation,agents,context}.md` | 5 |
| Architecture | `architecture/{document-model,editor-runtime,interaction-runtime,history,rendering,input-system,profiles,ai-runtime}.md` | 8 |
| Profiles | `profiles/{figma,tldraw,affinity,canva,spool}.md` | 5 |
| Matrices | `matrices/{feature,interaction,keyboard,document-model,history,ai}-matrix.md` | 6 |

**Total: 50 files.**

Every architecture note ends with `Candidate architectural implications` — each terminated by
`Decision: TBD — requires architecture review.` — followed by `Open questions` and `Sources`. Every
product and matrix file ends with `Sources` and, where applicable, an evidence-gaps section.

**This corpus contains no Spool architecture.** The architectural questions in *Major Architectural
Questions For Spool* are the deliverable; the answers belong to the next phase.
