# Architecture Extraction: Editor Runtime

The editor runtime is the layer that owns *transient* state and mediates between input, the document, and
the renderer. It is the layer tldraw documents most completely, so it carries most of the evidence.

## The published decomposition

From tldraw's documentation, the editor is organised into these areas:

[DOCUMENTED — `docs/editor.mdx` capability table]

| Area | Topics |
|---|---|
| **Data** | Signals, Store, Shapes, Bindings, Pages, Assets |
| **Interaction** | **Tools**, Selection, **Input handling**, Events |
| **View** | **Camera**, Coordinates |
| **State** | Instance state, Visibility, **History**, Side effects |
| **Configuration** | User preferences, Readonly mode, Locked shapes |
| **Output** | Image export |

[INFERRED] This is a **four-layer** decomposition: Data / Interaction / View / Output, with State and
Configuration as cross-cutting concerns.

## Runtime concepts

### 1. The Editor as a façade

> "By design, the editor's surface area is very large. **Almost everything is available through it.**"
> [DOCUMENTED — tldraw `docs/editor.mdx`]

[INFERRED] A single façade object that owns everything, with no privileged internal API. Every capability is
a public method. This is the opposite of Figma's model, where the plugin API is a curated subset.

**Evidence class:** SOURCE-CODE/DOCUMENTED (tldraw).

**Spool relevance:** A façade makes an AI operation API, a scripting API, and a driver all expressible in
the same terms, because they are all clients of the same façade.

---

### 2. Reactive state (signals)

> "The editor's state is reactive. Methods like `Editor#getSelectedShapeIds()` or
> `Editor#getCurrentPageShapes()` return values that automatically update when the underlying data changes."
> [DOCUMENTED — `docs/editor.mdx`]

> "The SDK uses reactive signals instead of React's built-in state management. Signals automatically track
> dependencies and update only the parts of your application that actually depend on changed data."
> [DOCUMENTED — `sdk-features/performance.mdx`]

[INFERRED] **Fine-grained reactivity replaces "rebuild everything on change".** Spool's prototype rebuilds
the whole layers projection on structural change and does an O(N) sweep on selection change — PHASE15
documents both costs precisely.

**Critical detail — identity stability:**
> "Both are reactive, and `getCulledShapes()` returns **the same `Set` instance while its contents are
> unchanged**, so it's cheap to read in `track` components or `useValue`."
> [DOCUMENTED — `sdk-features/culling.mdx`]

[INFERRED] Referentially-stable containers are what make fine-grained reactivity usable from a per-frame
render loop. A signal that returns a new `Vec` every frame defeats the purpose.

**Spool relevance:** GPUI's model is *entity-level caching with upward dirty propagation*, which PHASE15
documents as structurally unable to do per-row retention. A fine-grained dependency system would be a
different answer — but this is a large investment, not a clear win.

**Decision: TBD — requires architecture review.**

---

### 3. Instance state (the transient state bag)

[DOCUMENTED — `sdk-features/instance-state.mdx`]

> "Instance state is the **per-tab state** that tracks your current session. It includes which page you're
> viewing, whether the grid is visible, if debug mode is on, and transient interaction state like the current
> cursor.
> **Unlike document state (shapes, pages, assets), instance state belongs to a single browser tab and isn't
> synced between collaborators.**"

Fields documented: `isGridMode`, `isDebugMode`, `isFocusMode`, `isPenMode`, `isToolLocked`, `isReadonly`,
`currentPageId`, `cursor`, `isFocused`, `screenBounds`, `devicePixelRatio`, `isCoarsePointer`,
`selectedShapeIds`.

[INFERRED] **This is the clearest published instance of the research brief's §36 question.** The
document/editor state split is enforced by a *type and a store*, not by convention.

**Spool relevance:** Spool's `CanvasView` mixes all of these into one struct:
`camera, document, history, selection, pan, interaction, marquee, tool, space_held, hitbox, focus_handle,
text_edit`. [OBSERVED in `app/src/canvas.rs`]

That is workable but it means there is no way to ask "what is the persistent state?" — because the
separation does not exist in the type system.

**Decision: TBD — requires architecture review.**

---

### 4. Side effects (record lifecycle hooks)

> "Side effects are lifecycle hooks that run when records are created, updated, or deleted. You can use them
> to intercept and modify records, validate changes, …"
> [DOCUMENTED — `sdk-features/side-effects.mdx`]

> "To keep data internally consistent, like **cleaning up bindings when a shape is deleted**, use side
> effects instead of side effects are lifecycle hooks that can intercept and modify records during
> operations."
> [DOCUMENTED — `sdk-features/store.mdx`]

> "Updating meta with side effects. To keep meta up to date as shapes change, register a side effect that
> runs before each shape update: `editor.sideEffects.…`"
> [DOCUMENTED — `docs/shapes.mdx`]

Documented uses:
- **Selection consistency**: ancestor–descendant filtering and focused-group management happen in the
  `instance_page_state` after-change side effect. [DOCUMENTED — `sdk-features/selection.mdx`]
- **Binding cleanup** on delete. [DOCUMENTED]
- **Shape meta maintenance** on update. [DOCUMENTED]
- **Derived state** (e.g. geometry caches).

[INFERRED] **Side effects are the mechanism that keeps derived state correct without every mutation site
having to know about it.** For Spool this is the natural home for: cleaning up a deleted object's children,
invalidating a spatial index, recomputing derived bounds, and maintaining a layers projection.

**Spool relevance:** Spool has no side-effect mechanism; `LayersProjection::synchronize` recomputes by
comparing a revision counter. [OBSERVED in `app/src/layers.rs`]

**Decision: TBD — requires architecture review.**

---

### 5. Readonly and locked modes

[DOCUMENTED — tldraw has `sdk-features/readonly.mdx` and `sdk-features/locked-shapes.mdx`]

- Readonly mode: "only actions with `readonlyOk` are bound". [DOCUMENTED in `sdk-features/actions.mdx`]
- Locked shapes: excluded from bulk operations; `selectLockedShapes` option allows click/brushing while
  still protecting from edits. [DOCUMENTED — `sdk-features/selection.mdx`]

[INFERRED] **Readonly and locked are different axes**: readonly is per-user/per-document (collaboration);
locked is per-object (authoring). Figma's view-only mode is the readonly case.

---

### 6. User preferences vs. instance state

[DOCUMENTED — `sdk-features/user-preferences.mdx`: "Cross-instance settings like dark mode"]

Documented preference-backed behaviours: `isSnapMode`, dark mode, grid toggle, nudge distances.

[INFERRED] Three levels of state, all documented:

| Level | Scope | Examples |
|---|---|---|
| **Document** | Persisted + synced | Objects, pages, assets, bindings |
| **Session / instance** | Per tab, not synced | Selection, camera, current page, tool, grid visibility |
| **User preference** | Cross-instance | Snap mode, dark mode, nudge distance |

[INFERRED] **Interaction profiles would sit at the preference level** (see `profiles/`), which is the only
level where "how the user wants to work" belongs.

---

### 7. Visibility

[DOCUMENTED — `sdk-features/visibility.mdx`]

A separate visibility system, distinct from `canCull`.

[INFERRED] **Visibility (user intent: hide this) is not culling (renderer decision: this is off-screen).**
Confusing them is a common bug: a hidden object should not appear in exports or the layers panel in the same
way a culled one does not.

---

### 8. Performance instrumentation as a runtime service

[DOCUMENTED — `sdk-features/performance.mdx`]

> "For programmatic monitoring (telemetry or in-app dashboards), the editor exposes `PerformanceManager` at
> `editor.performance`. Subscribe to events and you'll get aggregated frame-time stats from real
> interactions, **with no overhead when no listeners are attached**."
>
> Events: `interaction-end` (with `fps`, `p95FrameTime`, LoAF attribution), `camera-end`,
> `shapes-created`, `shapes-updated`, `shapes-deleted`, `frame`.
>
> "Custom tools opt into interaction tracking by setting `StateNode#trackPerformance` on the state node
> class. When the state is entered, the manager starts a tracking window; when it exits, it emits
> `interaction-start` / `interaction-end` **with the state path**. Built-in interactions like
> `select.translating` and `draw.drawing` already track."

[DOCUMENTED]

[INFERRED] **Measuring performance per interaction state rather than per UI component is the key idea.**
It ties telemetry to semantics: "translate is slow" is actionable; "Widget 47 is slow" is not.

**Spool relevance:** Spool has `diagnostics::count` / `diagnostics::record` and a benchmark harness
(`canvas_workloads.rs`, `canvas_benchmarks.rs`, `measure_runtime.py`), but instrumentation is sprinkled
inline rather than tied to interactions. [OBSERVED]

PHASE15 shows the pattern working: `layers_projection_rebuild`, `row_presentation_updates`,
`layers_row_construction`, `canvas_notify` are named counters. [OBSERVED in `app/PHASE15.md`]

**Decision: TBD — requires architecture review.** Instrumenting by interaction state would be a natural
extension of what already exists.

---

### 9. Migration system

[DOCUMENTED — `sdk-features/persistence.mdx`, `sdk-features/store.mdx`]

- Snapshots carry a schema version.
- `editor.store.migrateSnapshot(oldSnapshot)` migrates without loading.
- Per-shape `TLPropsMigrations` and per-record-type `MigrationSequence` with `{ id, up, down }`.
- Custom record types get their own migration sequence.

[INFERRED] **Schema migration is a first-class concern in an open-source product whose files are shared.**
It constrains the object model: ids must be stable, records must be self-describing, and properties must be
open to extension.

**Spool relevance:** Spool has **no serialisation at all** — `Document::default()` constructs a hard-coded
scene. [OBSERVED] So migrations are entirely ahead, and the object model decision will constrain them.

**Decision: TBD — requires architecture review.**

---

### 10. Ticks

[DOCUMENTED — `sdk-features/ticks.mdx`]

> "Frame-synchronized updates for animations and continuous interactions."

Every state node can implement `onTick`. The driver emits a tick after each input method and offers
`forceTick(count)` for multi-frame work.

[INFERRED] **Ticks are how a state machine runs work that spans frames** (drag smoothing, freehand fitting,
animated selection). It is the cleanest way to keep per-frame work inside the interaction model rather than
in a global render callback.

---

## Runtime state classification

From `architecture/document-model.md`, the runtime owns:

| State | Kind | Documented owner |
|---|---|---|
| Selection | Instance/session | tldraw DOCUMENTED |
| Focused group / nesting depth | Instance/session | tldraw, Figma |
| Active tool / current state | Instance/session | All four |
| Pointer state / drag gesture | Transient | All four |
| Camera (pan, zoom, viewport) | Instance/session | All four |
| Text caret / range / IME | Transient | All four |
| Hover state | Transient | All four |
| Snap indicators / temporary guides | Transient | Figma, tldraw, Affinity |
| Tool lock | Instance/session | tldraw DOCUMENTED |
| Grid / debug visibility | Instance/session | tldraw DOCUMENTED |
| Conversation history | Session | tldraw DOCUMENTED |
| Task hints / style profile | User, local | Affinity, Canva DOCUMENTED |
| Screen bounds / DPR / coarse pointer | Device | tldraw DOCUMENTED |

## Spool prototype: the current runtime

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

```rust
pub struct CanvasView {
    camera: Camera,          // transient
    document: Document,      // persistent
    history: History,        // runtime
    selection: Selection,    // transient
    pan: Option<PanGesture>, // transient
    interaction: Interaction,// transient
    marquee: Option<MarqueeGesture>, // transient
    tool: Tool,              // transient
    space_held: bool,        // device
    hitbox: Rc<Cell<Option<CanvasHitbox>>>, // device
    focus_handle: Option<FocusHandle>,      // device
    text_edit: Option<TextEditState>,       // transient
    workload_started/running,               // debug
}
```

Observations:

1. **One struct holds everything.** There is no persistent/transient type distinction.
2. **`Document` is embedded in `CanvasView`.** A `CanvasView` *is* a document plus an editor. There is no
   separation between "the document" and "the thing that edits it".
3. **`History` is owned by the view**, so history is per-view and cannot be shared or serialised.
4. **`Interaction` is a good state machine** — `None / PotentialMove / Moving / PotentialResize / Resizing /
   PotentialCreate / Creating`. This is the tldraw pattern in miniature.
5. **`space_held` is a device flag stored as plain state**, with no mechanism to reset it on focus loss
   (no `.on_focus`/`.on_blur` listener is registered anywhere despite an existing `FocusHandle`).
6. **An action registry exists but is nearly empty**: 12 actions in one `spool_text` namespace, all
   text-editing, 16 bindings all with `context: None`, compiled in rather than loaded, and a second
   raw `on_key_down` match path in `shell.rs` running in parallel. No side effects, no ticks, no
   performance instrumentation keyed to interactions. See `architecture/input-system.md` §3, which
   corrects the earlier description of this area as having "no action registry" at all.
7. **`LayersView` reaches into `CanvasView`** via a `WeakEntity` — panels hold a weak reference to the
   editor. [OBSERVED in `app/src/layers.rs`]

[INFERRED] Item 2 is the most consequential. A document that is *owned by* a view is a document that cannot
be opened in two places, tested headlessly, rendered to a file, or passed to an agent. Every one of those is
required by the product direction.

## Candidate architectural implications

### Implication A — separate the document from the editor

**Evidence:** All four products have a document that is independent of any view. tldraw's `createTLStore()`
can be used standalone for headless/test scenarios; snapshots serialise the store without a window.

**Why it matters:** Required for export, headless rendering, AI verification, testing, multi-window, and
collaboration.

**Approaches:**
- **A. Keep as-is.** Every feature requiring headless operation is blocked.
- **B. Extract `Document` (with its own serialisation and snapshot API); `Editor` owns it.**
- **C. B + a store-like abstraction** with reactive queries, side effects, and indexes.

**Tradeoffs:** B is a mechanical refactor with large benefit. C is tldraw's full model and is a much larger
commitment (the reactivity and index machinery).

**Decision: TBD — requires architecture review.**

---

### Implication B — type-separate persistent from transient state

**Evidence:** tldraw's document/session/presence record scopes, enforced by type.

**Why it matters:** Without it, "what is saved" is not answerable, and the layers-panel invalidation problem
(Phase 14/15) has no principled fix.

**Approaches:** A — keep one struct; B — separate structs with an explicit editor-state type; C — B + a
typed store with scopes.

**Decision: TBD — requires architecture review.**

---

### Implication C — an action registry as the single invocation surface

See `architecture/document-model.md`, Implication A. Cost estimate revised downwards by
`architecture/input-system.md` Implication A: GPUI already provides the registry, JSON keymaps,
chords, context predicates, precedence and availability. What is missing is namespace, ownership,
content, and the elimination of the parallel raw-key path.

**Decision: TBD — requires architecture review.**

---

### Implication D — side effects for derived state

**Evidence:** tldraw DOCUMENTED. Spool's revision-counter approach works but does not scale to multiple
derived views.

**Why it matters:** Selection consistency, binding cleanup, spatial-index invalidation, and layers-panel
updates are all derived-state problems with the same shape.

**Decision: TBD — requires architecture review.**

---

### Implication E — instrumentation keyed to interaction states

**Evidence:** tldraw DOCUMENTED (`trackPerformance`, state paths, `fps`, `p95FrameTime`).

**Spool relevance:** The existing diagnostics already name operations. Extending them to interaction
lifecycles would make "which gesture is slow" answerable, which is exactly the question Phase 14/15 raised.

**Decision: TBD — requires architecture review.** Low effort, direct continuation of existing work.

## Open questions

1. Is `Document` separable from `CanvasView`?
2. Is there a typed distinction between persistent, session, and device state?
3. Is there a *usable* action registry? (GPUI's exists; Spool's has 12 text actions, no context
   predicates, no file loading, and a parallel raw-key path. See `architecture/input-system.md`.)
4. Are there side effects, or only revision counters?
5. Are there ticks, or is per-frame work done in `Render`?
6. Is there a headless serialisation/snapshot API?
7. Is there a performance manager keyed to interaction states?
8. Is there a schema migration path?

## Sources

- tldraw: `docs/editor.mdx` ⭐ (capability table), `sdk-features/instance-state.mdx` ⭐,
  `sdk-features/side-effects.mdx` ⭐, `sdk-features/signals.mdx`, `sdk-features/performance.mdx` ⭐,
  `sdk-features/ticks.mdx`, `sdk-features/culling.mdx` (identity stability), `sdk-features/readonly.mdx`,
  `sdk-features/locked-shapes.mdx`, `sdk-features/user-preferences.mdx`, `sdk-features/visibility.mdx`,
  `sdk-features/persistence.mdx` (migrations), `sdk-features/store.mdx` ⭐, `docs/shapes.mdx` (side effects),
  `sdk-features/selection.mdx` (selection side effect)
- Figma: "Select layers and objects" (/360040449873) (view-only selection presentation)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) (local task memory as user-scoped state)
- Spool prototype: `app/src/canvas.rs` (`CanvasView`, `Document`, `History`, `Interaction`, `space_held`),
  `app/src/layers.rs` (`LayersProjection`, `WeakEntity<CanvasView>`), `app/src/diagnostics.rs`,
  `app/PHASE15.md`
