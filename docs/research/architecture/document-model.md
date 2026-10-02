# Architecture Extraction: Document Model

This document extracts recurring architectural concepts from the product evidence. For each concept it
answers: which products expose an equivalent, what problem it solves, whether it is persistent or transient,
whether it participates in history, whether it affects rendering/input/AI, and what evidence supports it.

It does **not** prescribe a Spool architecture. Candidate implications are recorded with `Decision: TBD`.

## Evidence summary table

| Concept | Figma | tldraw | Affinity | Canva | Evidence class |
|---|---|---|---|---|---|
| **Document** | File | Store | Document | Design | DOCUMENTED |
| **Page** | Yes | Yes (record) | Yes (pages/spreads) | Yes (page stack) | DOCUMENTED |
| **Node/Object** | Layer | Shape (record) | Layer | Element | DOCUMENTED |
| **Shape** | Per-type | `type` + `props` discriminator | Per-type | Element kind | DOCUMENTED |
| **Container** | Frame, Section, **Group** | Frame, Group | Container layer | Group | DOCUMENTED |
| **Tool** | Yes | Root state node | Yes | **No persistent tool** | DOCUMENTED |
| **Interaction state** | No published chart | **Hierarchical state chart** | Tool + context toolbar | Modal | DOCUMENTED (tldraw) |
| **Selection** | Set + nesting depth | Instance state + focused group | Set | Set | DOCUMENTED |
| **Camera** | Yes | Yes | Canvas zoom | Per view mode | DOCUMENTED |
| **History** | Undo + version history + branches | **Marks + diffs + bail + squash** | Undo | Undo + versions + Trash | DOCUMENTED |
| **Operation** | Implicit | `Editor#*` methods + `run()` | Scripting API | Actions | DOCUMENTED |
| **Transaction** | Implicit | `editor.run()` + history modes | "single undoable action" | Implicit | DOCUMENTED |
| **Binding** | Prototype connections | **First-class record** | — | Element-anchored comments | DOCUMENTED |
| **Style** | Styles + **variables** | Named palette | Styles + presets | Text styles + Brand Kit | DOCUMENTED |
| **Asset** | Uploads + libraries | **Asset records + resolver** | Resources + presets | Uploads + folders | DOCUMENTED |
| **Component** | Full 5-part system | **None** | **None** | **None** (templates) | DOCUMENTED |
| **Layout** | Absolute + constraints + **auto layout** | **None** | Constraints | Relative position | DOCUMENTED |
| **Constraint** | Yes (5 per axis) | None documented | Yes (container) | None | DOCUMENTED |
| **Snap engine** | 3 settings | **BoundsSnaps + HandleSnaps** | **Presets + construction snapping** | Not documented | DOCUMENTED |
| **Input system** | Key handling + keyboard box selection | **Event dispatch + re-dispatch** | Configurable shortcuts | Fixed shortcuts | DOCUMENTED |
| **Renderer** | **Scene graph + WebGPU** | Per-shape components + culling | Unknown | Unknown | ENGINEERING-DISCLOSED / UNKNOWN |
| **Spatial index** | Unknown | Documented | Unknown | Unknown | DOCUMENTED (tldraw) |
| **AI operation** | Design layers via agent | Typed actions + driver | MCP command API | Conversational | DOCUMENTED |
| **Prototype** | Triggers + actions + animation | None | None | Presentation only | DOCUMENTED |

## Concept-by-concept extraction

### 1. Document

**Equivalents:** Figma File, tldraw Store, Affinity Document, Canva Design. **All four.**

**Problem solved:** Provides a persistence, identity, sync, and ownership boundary.

**Persistent?** Yes. **In history?** It contains history. **Affects rendering?** Yes. **Input?** No.
**AI?** Yes — it is the context scope.

**Evidence:** DOCUMENTED everywhere. tldraw adds the **record-scope** distinction (document / session /
presence), which is the most precise published statement of what "document state" means.

[INFERRED] A document must be independently serialisable, independently syncable, and independently
addressable — otherwise none of collaboration, versioning, or export works.

---

### 2. Page

**Equivalents:** All four. tldraw: a record; Figma: a container; Affinity: pages/spreads; Canva: a page stack.

**Problem solved:** Partitions a document into addressable regions; bounds the working set.

**Persistent?** Yes. **In history?** Creation/deletion/ordering are. **Affects rendering?** Yes.
**Input?** Yes — page navigation. **AI?** Yes — the working context is usually a page.

**Evidence:** DOCUMENTED. Notable: tldraw documents `currentPageId` as **instance state**, not document state.
Canva documents explicit page navigation (`⌥⌘G` Go to page) and page break (`⌘⏎`).

[INFERRED] **The current page is editor state; the pages themselves are document state.** Getting this wrong
makes page navigation undoable or makes the camera shared between collaborators.

---

### 3. Node / Object

**Equivalents:** Figma Layer, tldraw Shape record, Affinity Layer, Canva Element.

**Problem solved:** The atomic addressable unit.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** Yes. **Input?** Yes (hit testing).
**AI?** Yes — the operation target.

**Evidence:** DOCUMENTED. Note the naming caution: "layer" in Figma and Affinity includes groups and
containers, so it is a UI word, not a model word.

---

### 4. Shape (the kind discriminator)

**Equivalents:** Figma per-type layers; tldraw `type` + `props`; Affinity per-type layers; Canva element kinds.

**Problem solved:** How a node knows how to draw itself, hit-test itself, extract text from itself, and
validate its own properties.

**Persistent?** The discriminator yes; the behaviour no. **In history?** The discriminator yes.
**Affects rendering?** **Yes, entirely.** **Input?** **Yes, entirely.** **AI?** **Yes** (`getText`,
`canSnap`, `getGeometry` are AI-relevant hooks).

**Evidence:** tldraw SOURCE-CODE/DOCUMENTED — a `ShapeUtil` subclass implements `getGeometry`,
`getBoundsSnapGeometry`, `getHandleSnapGeometry`, `canSnap`, `canCull`, `canBind`, `canTabTo`, `getText`,
`component`, `getIndicatorPath`.

[INFERRED] **This is the concept with the highest ratio of architectural leverage to apparent complexity.**
A single behavioural interface per shape kind serves the renderer, the hit tester, the snap engine, the
culler, the layers panel, the exporter, and the AI context builder simultaneously.

---

### 5. Container

**Equivalents:** Figma Frame + Section + Group; tldraw Frame + Group; Affinity Container layer; Canva Group.

**Problem solved:** Scoping coordinates, clipping, ordering, and interaction.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** Yes (clip, order, coordinate transform).
**Input?** **Yes** — selection targeting. **AI?** Yes (reparenting, layout inference).

**Evidence:** Figma DOCUMENTED — groups have derived bounds and cannot take constraints; frames can.
tldraw DOCUMENTED — groups are not snap targets but their children are; frames are snap targets.

[INFERRED] **Two container kinds, with different properties, are the norm in every product that has them.**
A single "group" concept is the most common modelling error.

---

### 6. Tool

**Equivalents:** All except Canva.

**Problem solved:** Defines which interaction grammar is active.

**Persistent?** **No** (editor state). **In history?** No. **Affects rendering?** Only the cursor.
**Input?** **Yes, entirely.** **AI?** Yes — tool lock and tool identity affect what a driver can do.

**Evidence:** tldraw DOCUMENTED (root state node, `setCurrentTool`, tool lock). Canva DOCUMENTED absence —
creation is a one-shot command.

[INFERRED] **Tool persistence (does the tool stay active after use?) is a real product decision** and the
products disagree: Figma's shape tools stay active, tldraw's return to select unless locked, Affinity's
tools persist, Canva has no tools.

---

### 7. Interaction state

**Equivalents:** tldraw publishes a **hierarchical state chart**. Figma publishes an implicit mode stack.
Affinity uses tools + context toolbars. Canva uses modals and one-shot commands.

**Problem solved:** Modelling multi-phase interactions explicitly rather than as ad-hoc boolean flags.

**Persistent?** No. **In history?** Only indirectly (marks). **Affects rendering?** Only the affordances.
**Input?** **Yes, it is the input model.** **AI?** Yes — tools are addressable objects.

**Evidence:** tldraw DOCUMENTED — `idle`, `pointing_canvas`, `pointing_shape`, `brushing`, `translating`,
`resizing`, `rotating`; transitions carry payloads; a state that doesn't handle an event lets it fall to its
child; targets are re-dispatched.

[INFERRED] The `PotentialX → X` pattern that Spool's prototype already implements
(`Interaction::PotentialCreate` / `Creating`, `PotentialResize` / `Resizing`, `PotentialMove` / `Moving`)
is the same idea as tldraw's `pointing_* → active`. Spool has arrived at the correct abstraction by
accident.

---

### 8. Selection

**Equivalents:** All four.

**Problem solved:** The operand of every operation.

**Persistent?** **No** — editor/session state in every product. **In history?** tldraw: **yes**, with a
distinct history mode. **Affects rendering?** Yes (handles, bounds). **Input?** **Yes.** **AI?** **Yes** —
the default scope of an agent operation.

**Evidence:** DOCUMENTED. tldraw's `selectedShapeIds` in instance state; Figma's parent-by-default click
plus depth; Affinity's `⌥`-click cycle; Canva's `⇧WASD` directional selection.

---

### 9. Camera

**Equivalents:** All four.

**Problem solved:** The mapping between world and screen.

**Persistent?** **No.** **In history?** **No** — no product documents camera in undo. **Affects rendering?**
**Yes, entirely.** **Input?** **Yes, entirely.** **AI?** Partly (zoom-to-selection is an agent-useful op).

**Evidence:** DOCUMENTED. tldraw additionally documents `getEfficientZoomLevel()` — a *separate*, debounced
zoom used for rendering decisions.

[INFERRED] **Two zooms, not one.** The true camera zoom (for coordinates) and a stable render zoom (for LOD
and stroke widths) are different values with different lifetimes.

---

### 10. History

**Equivalents:** All four, with radically different designs.

**Problem solved:** "What does the user perceive as ONE undoable action?"

**Persistent?** The stack is session state; **versions are document-adjacent server state**. **In history?**
It is history. **Affects rendering?** Indirectly (culling must account for shapes created by undo).
**Input?** Yes (bindings). **AI?** **Yes** — agents must batch and abort.

**Evidence:**

| Product | Model | Documentation quality |
|---|---|---|
| Figma | Undo + named versions + branching | High on the product, **none on the mechanism** |
| tldraw | **Marks + diffs + three capture modes + bail + squash** | **Excellent** |
| Affinity | `⌘Z`; scripting applies "often as a single undoable action" | Mechanism undocumented |
| Canva | Undo + 1,000 attributed versions + Trash | High on versions, none on undo |

[INFERRED] **The concept that must be extracted is not "undo" but "mark".** tldraw's published primitives:

| Primitive | Problem solved |
|---|---|
| **Mark** | Defines a stopping point → one undoable action |
| **`record`** mode | Normal capture; clears redo |
| **`record-preserveRedoStack`** | Selection changes must be undoable without destroying redo |
| **`ignore`** | Cursor position, transient writes |
| **Bail** | Discard an interaction without polluting redo (Escape; mid-gesture mode change) |
| **Squash** | Many steps → one, retroactively (crop mode: fine-grained during, coarse after) |

---

### 11. Operation

**Equivalents:** tldraw (`Editor#*` methods), Affinity (scripting API), Canva (actions), Figma (Plugin API).

**Problem solved:** The unit of semantic change; the thing an agent calls.

**Persistent?** No (an operation is code). **In history?** **Yes** — an operation is what produces a history
entry. **Affects rendering?** Via the document. **Input?** Tools call operations.
**AI?** **Yes — operations are the agent's tools.**

**Evidence:** tldraw `docs/ai.mdx` (typed action schemas) and `sdk-features/actions.mdx` (~100 actions);
Affinity's scripting API reference at affin.co/affinity-sdk.

[INFERRED] **The operation registry is the single most reusable abstraction in this research.** It serves the
keyboard, the menus, the toolbars, the scripts, and the agent. Spool currently has none.

---

### 12. Transaction

**Equivalents:** tldraw `editor.run()`, Affinity's "single undoable action", tldraw's streaming agent.

**Problem solved:** Grouping many writes into one atomic, undoable, coherent change.

**Persistent?** No. **In history?** **Yes — it is the unit history records.** **Affects rendering?** Batching
notifications. **Input?** No. **AI?** **Yes** — streaming agent output must be one transaction.

**Evidence:** tldraw DOCUMENTED — `editor.run(fn, { history, ignoreShapeLock })`; nested calls keep the outer
mode; store listeners are already batched per animation frame.

---

### 13. Binding

**Equivalents:** tldraw bindings; Figma prototype connections; Canva element-anchored comments.

**Problem solved:** A persistent, non-containment relationship.

**Persistent?** **Yes.** **In history?** Yes. **Affects rendering?** Yes (arrows re-route). **Input?** Yes
(creating a binding is a tool). **AI?** **Yes** — "connect these", "make this sticky".

**Evidence:** tldraw DOCUMENTED in full — record shape, directionality, lifecycle hooks including
**isolation**, bookkeeping rules, incremental index.

[INFERRED] **The isolation/deletion distinction is the extractable concept**, not the record shape. Deletion is
one of several causes of separation.

---

### 14. Style

**Equivalents:** All four, with different depths.

**Problem solved:** Reusable appearance.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** **Yes.** **Input?** No. **AI?** **Yes** —
a style is a high-value target ("restyle everything matching Button/Primary").

**Evidence:** Figma's two-layer split (styles = composites, variables = raw values, aliasable, modal);
Affinity's styles + effects + presets; Canva's text styles + Brand Kit + Brand Controls; tldraw's palette.

---

### 15. Variable / token

**Equivalents:** **Figma only, by name.** Canva's Brand Kit is the constraint-based equivalent.

**Problem solved:** A single source of truth for a raw value, resolvable per context.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** **Yes — every read path must resolve.**
**Input?** Indirectly (snap tolerance, nudge distances are preferences, not variables). **AI?** **Yes — the
agent's vocabulary.**

**Evidence:** Figma DOCUMENTED in full. Canva DOCUMENTED ("gets to know your brand's fonts, colours, and
rules").

[INFERRED] **This is the concept whose absence most limits AI generation.** A model cannot respect a design
system it cannot reference.

---

### 16. Asset

**Equivalents:** All four.

**Problem solved:** Referencing external content without embedding it.

**Persistent?** **Yes.** **In history?** Yes. **Affects rendering?** **Yes — including resolution LOD.**
**Input?** No. **AI?** **Yes — generation produces assets.**

**Evidence:** tldraw DOCUMENTED (`AssetRecordType`, `TLAssetStore.resolve`, `steppedScreenScale`);
Canva DOCUMENTED (reference-counted deletion); Figma/Affinity DOCUMENTED at product level.

---

### 17. Component / instance

**Equivalents:** **Figma only.** Canva uses templates; Affinity uses styles + scripts; tldraw uses custom
shape types.

**Problem solved:** Structural reuse with live update propagation.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** **Yes** (instances render their main
component's tree). **Input?** **Yes** (instance children are not freely editable). **AI?** Yes.

**Evidence:** Figma DOCUMENTED in full. Explicit absence in the other three.

[INFERRED] **The extractable concept is the override overlay**, not the component concept per se:
`Instance = { reference, property_overrides_by_path, hidden_children, variant }`. Everything else (variants,
properties, swap) is a specialisation.

---

### 18. Layout

**Equivalents:** Figma's full model; Affinity's constraints; Canva's relative positioning; **none in tldraw**.

**Problem solved:** Automatic positioning and sizing of children relative to a container.

**Persistent?** Yes (it is document state). **In history?** Yes. **Affects rendering?** **Yes.**
**Input?** **Yes** (resize triggers relayout). **AI?** **Yes — layout inference.**

**Evidence:** Figma DOCUMENTED in full (flows, wrap, grid, hug/fill/fixed/min/max, ignore-auto-layout).
tldraw's absence is as informative as Figma's presence.

[INFERRED] **Two container kinds are structurally required**: a content-sized container cannot clip. And a
child inside a flow container has three states (participate / opt out / absent).

---

### 19. Constraint

**Equivalents:** Figma, Affinity. Not Canva, not tldraw.

**Problem solved:** Declaring how a child responds when its container resizes.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** Yes. **Input?** **Yes** (resize
propagation). **AI?** Yes.

**Evidence:** Figma DOCUMENTED — five values per axis, with `Scale` as a percentage-based mode.

[INFERRED] Constraints are **five small functions per axis**, not a general layout engine. That makes them
cheap to implement and surprisingly powerful — arguably the best value-per-complexity in the layout domain.

---

### 20. Snap engine

**Equivalents:** Figma (3 settings), tldraw (two systems + grid), Affinity (presets + construction snapping),
Canva (unknown).

**Problem solved:** Aligning to nearby geometry during a drag.

**Persistent?** The *configuration* yes; the *snap result* no (transient). **In history?** The configuration
yes; the snapped result is **part of the resulting geometry**, so it is implicitly in history.
**Affects rendering?** Yes (indicators). **Input?** **Yes, entirely.** **AI?** Yes.

**Evidence:** tldraw DOCUMENTED at the highest level of any product — screen-px ÷ zoom tolerance, per-handle
rules, gap centring and duplication, indicator merging and dedup, self-snap opt-out, candidate filtering.

[INFERRED] The extractable concepts: **snap returns a nudge** (pure), **candidates are bounded**, **tolerance
is zoom-normalised**, **indicators need merging**, **options are action-scoped**.

---

### 21. Input system

**Equivalents:** All four, with tldraw the only one documenting the abstraction.

**Problem solved:** Mapping physical events to semantic events, with hit-testing inside the state machine.

**Persistent?** No. **In history?** No. **Affects rendering?** Indirectly. **Input?** **Yes.**
**AI?** **Yes** — the driver.

**Evidence:** tldraw DOCUMENTED — event handler set, `target` vocabulary, re-dispatch, `isCoarsePointer`,
`isPenMode`, coalesced events, activation conditions.

---

### 22. Renderer

**Equivalents:** Figma (scene graph + WebGL → WebGPU); tldraw (per-shape components + culling + LOD); Affinity
and Canva **unknown**.

**Problem solved:** Turning a document into pixels, efficiently.

**Persistent?** No (derived). **In history?** No. **Affects rendering?** **Yes.** **Input?** No.
**AI?** **Yes — export is a renderer read.**

**Evidence:** Figma ENGINEERING-DISCLOSED (scene graph data model, WebGL, WebGPU migration, C++/TypeScript
client); tldraw DOCUMENTED (eight named techniques, LOD specifics, culling identity stability).

[INFERRED] **The extractable concept is the separation of a paint/scene representation from both the
document and the UI toolkit.** Spool currently has neither separation.

---

### 23. Spatial index

**Equivalents:** tldraw (documented); Figma/Affinity/Canva **unknown**.

**Problem solved:** Fast viewport culling, hit testing, and candidate queries.

**Persistent?** No (derived). **In history?** No. **Affects rendering?** **Yes** (culling).
**Input?** **Yes** (hit tests). **AI?** **Yes** (context summaries, snapping).

**Evidence:** tldraw DOCUMENTED — "The editor maintains a spatial index", "The bindings index is a computed
value that updates incrementally. Lookups are fast and never scan all records."

[INFERRED] A spatial index is required by culling, snapping, selection, marquee, and AI context. It is one of
the few structures that five subsystems need.

---

### 24. AI operation

**Equivalents:** tldraw typed actions; Affinity MCP; Canva conversational; Figma agent.

**Problem solved:** Letting a model act on the document without giving it UI access.

**Persistent?** No. **In history?** **Yes — operations are recorded like user edits.** **Affects rendering?**
Via the document. **Input?** No (commands) or yes (driver). **AI?** **Yes.**

**Evidence:** tldraw DOCUMENTED (typed action schemas, sanitisation, streaming); Affinity DOCUMENTED (MCP
capability toggles, single-undo scripts); Canva DOCUMENTED (six context sources).

[INFERRED] **An AI operation should be a document operation, not a privileged call.** tldraw's driver is the
exception that proves the rule: it simulates *input*, which runs the real code paths.

---

### 25. Prototype graph

**Equivalents:** Figma only. Canva has presentation mode; Affinity and tldraw have none.

**Problem solved:** Connecting frames into a navigable flow with triggers and transitions.

**Persistent?** Yes. **In history?** Yes. **Affects rendering?** No (editor-time it is invisible).
**Input?** Yes (play mode). **AI?** Yes ("add interactions").

**Evidence:** Figma DOCUMENTED — trigger + action + animation per interaction; the full action list reveals a
runtime with navigation stack, overlay stack, scroll position, variable state, variant state, and media time.

---

### 26. Version history

**Equivalents:** Figma (named versions + branches), Canva (1,000 attributed versions), tldraw (snapshots for
persistence), Affinity (unknown).

**Problem solved:** Long-horizon recovery and attribution.

**Persistent?** Server-side, yes. **In history?** It *is* a separate history. **Affects rendering?** No.
**Input?** No. **AI?** Partly (a generation should be a version).

**Evidence:** Figma and Canva DOCUMENTED in detail; **neither documents the undo mechanism**, which confirms
these are distinct systems.

---

## Candidate architectural implications

### Implication A — an operation registry is the keystone abstraction

**Evidence:** tldraw's `actions` registry drives keys, menus, toolbars, the shortcuts dialog, scripts, and
the agent. Affinity's scripting API is a document-command surface. Canva's `/` quick-actions palette is an
action lookup.

**Why it matters:** It is the only abstraction that serves keyboard, menus, toolbars, scripts, agents, and
toolbars simultaneously. Spool's registry is GPUI's, but only 12 text actions are declared, every
binding passes `context: None`, and a second raw `on_key_down` path in `shell.rs` bypasses it.

**Approaches:**
- **A. Keep ad-hoc bindings.** Blocks profiles, blocks agents, blocks testability.
- **B. An action registry** (id, label, kbd, handler) as the single invocation surface.
- **C. B + a separate tool state chart.** Matches tldraw's split (tools handle continuous input; actions
  handle discrete commands).

**Tradeoffs:** B is cheap and enabling. C is what tldraw does and is a clean separation. A is a trap.
Note: per `architecture/input-system.md` Implication A, B's *mechanism* already exists in GPUI — the
work is naming, ownership and content, not construction.

**Decision: TBD — requires architecture review.**

---

### Implication B — history needs marks, bail, and squash, not just before/after pairs

**Evidence:** tldraw DOCUMENTED in full. Spool's prototype already implements before/after diffs
(`GeometryChange`, `StyleChange`, `TextChange`, `ObjectPlacement`) and already collapses a drag into one
entry — so it has the *diff* but not the *mark*.

**Why it matters:** Bail is required for Escape-during-drag (Spool currently mutates the document live and
restores geometry, which works but is not a bail) and for mid-gesture modifier changes. Squash is required
for interactions that want fine-grained undo during and coarse undo after.

**Approaches:**
- **A. Keep before/after diffs; add explicit transaction open/close/abort.** Minimal.
- **B. Full tldraw semantics**: marks, three capture modes, bail, squash.

**Tradeoffs:** A covers Escape; B additionally covers selection-in-history, transient writes, and post-hoc
squashing.

**Decision: TBD — requires architecture review.**

---

### Implication C — one shape kind interface serves six subsystems

**Evidence:** tldraw's `ShapeUtil`. Each hook maps to a subsystem: `getGeometry` (hit test, bounds, culling),
`getText` (AI context), `canSnap`/`getBoundsSnapGeometry` (snapping), `canCull` (rendering),
`getHandleSnapGeometry` (snapping handles), `canBind` (bindings), `getIndicatorPath` (hit-test outlines),
`component` (rendering).

**Why it matters:** It makes the object model extensible without touching the engine.

**Approaches:** See `document/objects.md` (A enum / B discriminator+registry / C hybrid).

**Decision: TBD — requires architecture review.**

---

### Implication D — renderer independence is required by export and AI verification

**Evidence:** Figma's scene graph (ENGINEERING-DISCLOSED); tldraw's per-shape component + export API
(DOCUMENTED); the fact that tldraw's agent uses `getSvgString`/`toImage` to verify its own output.

**Why it matters:** Export, headless rendering, AI verification, and thumbnails all need to render without a
window. Spool's prototype renders directly to GPUI elements.

**Decision: TBD — requires architecture review.** This is arguably the highest-leverage architectural
question in the research, because it also determines whether export is feasible at all.

---

## Persistent vs. transient state — the classification

From the research brief's §36. Evidence-backed classification:

| Persistent (document) | Evidence |
|---|---|
| Objects/nodes and their properties | All four |
| Hierarchy (parent) | All four except Canva (shallow) |
| Z-order | All four |
| Styles / variables | Figma, Affinity, Canva; tldraw (palette) |
| Pages | All four |
| Assets | All four |
| Layout mode / constraints | Figma, Affinity |
| Components / instances / variants | Figma |
| Prototype links / flows | Figma |
| Bindings | tldraw; Figma (prototype connections) |
| Guides / grids / margins / baseline grid | Affinity, Figma, Canva |
| Slices / export configs | Figma, Affinity |
| Colour modes / theme selection | Figma (document-level); tldraw (theme, instance) |

| Transient (editor/session) | Evidence |
|---|---|
| Selection | All four |
| Focused group / nesting depth | tldraw, Figma |
| Active tool / current state node | All four except Canva |
| Pointer state / drag state | All four |
| Camera | All four |
| Text caret / selection range / IME composition | All four |
| Temporary guides / snap indicators | Figma, tldraw, Affinity |
| Hover state | All four |
| Tool lock flag | tldraw (instance state) |
| Grid visibility / debug flags | tldraw (`isGridMode`, `isDebugMode`) |
| Conversation history | tldraw (session scope) |
| Task hints / style profile | Affinity (local memory); Canva ("learns with you over time") |

[INFERRED] **The cleanest published rule is tldraw's three record scopes**: `document` (persisted + synced),
`session` (local, optionally persisted), `presence` (synced, not persisted). Every transient item in the
table above is session or presence scoped in tldraw's model.

## Open questions

1. Enum-per-kind or discriminator + registry for the object model?
2. Is there an operation registry, and is it shared with scripts and agents?
3. Does history use marks/bail/squash or simple transactions?
4. Is there a paint/scene representation between the document and GPUI?
5. Is there a spatial index, and what owns its invalidation?
6. Which state is `session` and which is `document`, explicitly and by construction rather than by habit?
7. Does the object model support stable subtree paths (a prerequisite for instances and for AI context
   addressing)?

## Sources

This document synthesises the four product files in `docs/research/products/`, the domain files in
`docs/research/document/` and `docs/research/interaction/`, and the AI files in `docs/research/ai/`.
Primary sources are listed in each of those files. The most load-bearing sources are:

- tldraw `apps/docs/content/sdk-features/{store,tools,selection,snapping,history,actions,performance,culling,
  bindings,shapes,instance-state}.mdx` and `apps/docs/content/docs/{ai,driver,mermaid,llm-docs}.mdx`
  (https://github.com/tldraw/tldraw/tree/main/apps/docs/content)
- Figma Help Centre articles listed in `products/figma.md`
- Figma Blog engineering posts listed in `products/figma.md`
- Affinity Help Centre articles listed in `products/affinity.md`
- Canva Help Centre articles listed in `products/canva.md`
