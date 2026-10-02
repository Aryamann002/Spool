# Architecture Extraction: Interaction Runtime

## The published model: a hierarchical state chart

tldraw is the only product that documents its interaction runtime. It is the reference for this domain.

[DOCUMENTED — `sdk-features/tools.mdx`]

> "Tools in tldraw define how the editor responds to user input. **Each tool handles one interaction mode**:
> selecting shapes, drawing, panning the canvas. **The editor has a single active tool at any time and routes
> all input events through it.** When you click the hand icon in the toolbar, the editor transitions from the
> select tool to the hand tool, and the canvas starts responding to drags by panning."

> "You implement tools as state machines using the `StateNode` class. Multi-step interactions map onto child
> states: **when you resize a shape with the select tool, the tool moves through `idle`,
> `pointing_resize_handle`, and `resizing`.**"

### Anatomy

```
root                       ← contains tools
└── select                 ← tool (branch node)
    ├── idle               ← leaf
    ├── pointing_canvas    ← leaf
    ├── pointing_shape     ← leaf
    ├── brushing           ← leaf
    ├── translating        ← leaf
    ├── resizing           ← leaf
    └── rotating           ← leaf
```

| Node type | Role |
|---|---|
| **Root** | Contains tools |
| **Branch** | Has child states |
| **Leaf** | Does the work |

[DOCUMENTED]

### Lifecycle

- `onEnter` when active; `onExit` when inactive. [DOCUMENTED]
- Transitions are explicit: `this.parent.transition('pointing_shape', info)`. [DOCUMENTED]
- Transitions carry payloads, visible to **both** the old `onExit` and the new `onEnter`. [DOCUMENTED]
- Transitions can target direct children by id, or deeper by dot notation
  (`'crop.pointing_crop_handle'`). [DOCUMENTED]

### Event dispatch

[DOCUMENTED — the full handler set]

```
onPointerDown  onPointerMove  onPointerUp  onLongPress  onDoubleClick
onRightClick   onMiddleClick
onKeyDown      onKeyUp        onKeyRepeat
onWheel        onCancel       onComplete   onInterrupt   onTick
```

> "A state that doesn't implement a handler **skips the event**, and the event still continues down to the
> active child state, so a parent and its child can both respond."
> [DOCUMENTED]

[INFERRED] This is an **event bubbling** model: unhandled events fall through to children. A tool can put
global behaviour in its branch node (e.g. "show the transform HUD") and specific behaviour in leaves.

### Event targets and re-dispatch

[DOCUMENTED]

> "Event info objects include a `target` property indicating what the user interacted with: `canvas`,
> `shape`, `handle`, `selection`, or `overlay`. **The canvas dispatches every pointer event with `target:
> 'canvas'`. The select tool's idle state hit-tests the pointer position and re-dispatches the event to
> itself with a more specific target**, then transitions to the matching child state."

[INFERRED] **Hit-testing is part of the state transition, not a precondition.** This has four consequences:

1. Each tool can hit-test differently (the lasso and measure tools both have their own examples).
2. The event vocabulary is small and fixed (5 targets).
3. A custom tool can respond to a shape without the core knowing anything about it.
4. Testing can dispatch directly to a target.

### Tool lock

[DOCUMENTED]

> "Tool lock keeps the current tool active after completing an action. Normally, tools like geo, arrow, or
> note **return to the select tool** after creating a shape. With tool lock enabled, the tool stays active so
> you can create multiple shapes without reselecting the tool each time."
>
> "**Tool lock is not enforced by the state machine. Custom tools check `isToolLocked` themselves** when
> deciding where to go after completing their action."

[INFERRED] The separation of flag (instance state) and policy (tool) is deliberate but leaky: every custom
tool author must remember to check the flag. A cleaner design would put the transition in the base tool class.

### Static tool configuration

[DOCUMENTED]

| Property | Meaning |
|---|---|
| `id` | Unique identifier |
| `initial` | Initial child state id |
| `children()` | Child state constructors |
| `isLockable` | Whether the toolbar shows the lock toggle (default `true`) |
| `useCoalescedEvents` | Receive coalesced pointer moves (default `false`; always off on iOS) |
| `trackPerformance` | Opt in to interaction-window performance measurement |

---

## Reconstructing the model for the other products

[INFERRED — the products do not publish state charts, but behaviour implies states.]

### Figma (inferred)

```
select
├── idle
│   ├── click-on-object → (selects parent) → idle
│   ├── cmd-click → (deep select) → idle
│   ├── double-click → descend scope
│   ├── drag-on-handle → resizing
│   ├── drag-outside-bounds → rotating
│   └── drag-empty → marquee
└── text-edit / vector-edit modes
```

Evidence for the inferred states: click-parent / double-click-descend / `Enter` / `⇧Enter` / `Esc`
[DOCUMENTED]; "Snap to geometry: used only in vector edit mode" implies a distinct vector mode
[DOCUMENTED]; rotation is entered "just outside one of the layer's bounds" [DOCUMENTED].

### Affinity (inferred)

```
tool-per-mode (Node, Move, Pen, Pencil, …)
└── node tool
    ├── idle
    ├── node-selected (transform mode)
    │   ├── dragging-nodes
    │   └── transforming-selection-box
    └── curve-selected
```

Evidence: Transform Mode "creates a bounding box around the selected nodes" [DOCUMENTED]; Escape "cancel a
sizing, moving, or creating operation" [DOCUMENTED].

### Canva (inferred)

```
selection
├── click element
├── shift-click (add)
├── F8 multi-select mode
└──⇧WASD directional multi-select
```

No persistent tool. Creation is a one-shot command (`T`, `R`, `L`, `C`, `S`). [DOCUMENTED]

---

## The `PotentialX → X` pattern

[INFERRED from tldraw's `pointing_*` states + Spool's own code]

Every multi-phase interaction needs to disambiguate **click** from **drag**. Both products solve it the same
way: a "pointing" state that transitions on a threshold.

| | tldraw | Spool prototype |
|---|---|---|
| Move | `idle` → `pointing_shape` → `translating` | `PotentialMove` → `Moving` |
| Resize | `idle` → `pointing_resize_handle` → `resizing` | `PotentialResize` → `Resizing` |
| Create | `pointing` → `drawing` | `PotentialCreate` → `Creating` |
| Threshold | `editor.inputs.getIsDragging()` | `drag_threshold_crossed(start, current)` at 4px |

[OBSERVED in `app/src/canvas.rs`: `const DRAG_THRESHOLD: f32 = 4.0;` and
`fn drag_threshold_crossed(start, current) -> bool { dx*dx + dy*dy >= DRAG_THRESHOLD*DRAG_THRESHOLD }`]

[INFERRED] **Spool has independently arrived at the correct abstraction.** That is a positive finding: the
prototype's interaction model needs extension (more states, targets, transitions) rather than replacement.

---

## The cancellation/commit vocabulary

[DOCUMENTED — tldraw handler set]

| Handler | Meaning | Spool equivalent |
|---|---|---|
| `onCancel` | The interaction was aborted | `Interaction::restore()` on Escape |
| `onComplete` | The interaction succeeded | commit on pointer-up |
| `onInterrupt` | Aborted by something other than the user | **None** |

[INFERRED] **The cancel/complete/interrupt triad is the runtime's contract with history.**
- Complete → commit (one history entry).
- Cancel → **bail** (discard; do not push to redo).
- Interrupt → ambiguous: probably commit the partial state rather than lose the user's work.

[INFERRED] Spool currently has complete and cancel but not interrupt. A pointer-capture loss (window
minimised, a system gesture) currently has no defined behaviour.

---

## Gestures as pure functions

[DOCUMENTED — tldraw history]

> "We use bailing while cloning shapes. **A user can switch between translating and cloning by pressing or
> releasing the alt (option) key during a drag.** When this changes, we bail on the changes since the
> interaction started, then apply the new mode's changes."

[INFERRED] This establishes that:

```
result(start_state, current_pointer, current_modifiers, elapsed) → document state
```

is a **pure function**, with no accumulated incremental state. The cost is recomputing on every move; the
benefit is exact behaviour on modifier change and trivially correct undo.

[OBSERVED] Spool is **half** there: `resized_geometry(start_geometry, handle, delta)` is already pure and
recomputed per move, but `apply_move` applies deltas incrementally.

---

## The runtime's other jobs

| Job | Mechanism | Evidence |
|---|---|---|
| **Selection maintenance** | A side effect on selection change (ancestor filtering, focus) | tldraw DOCUMENTED |
| **Binding maintenance** | `BindingUtil` hooks + `onOperationComplete` | tldraw DOCUMENTED |
| **Snap indicators** | Snap manager side effect; cleared when dragging stops | tldraw DOCUMENTED |
| **Performance windows** | `trackPerformance` on state classes | tldraw DOCUMENTED |
| **Cursor** | `instance.cursor` set in `onEnter` | tldraw DOCUMENTED (measure tool example) |
| **Focus** | `focusHandle`; shortcuts gated on `isFocused` | tldraw DOCUMENTED |
| **Locks** | Per-shape lock checks in bulk operations | tldraw DOCUMENTED |
| **Readonly** | Actions gated by `readonlyOk` | tldraw DOCUMENTED |

---

## Spool prototype: the interaction runtime

From `app/src/canvas.rs` [OBSERVED in source]:

```rust
enum Interaction {
    None,
    PotentialMove(MoveGesture),
    Moving(MoveGesture),
    PotentialResize(ResizeGesture),
    Resizing(ResizeGesture),
    PotentialCreate(CreateGesture),
    Creating(CreateGesture),
}
```

**What is right:**
- The state enum exists and is explicit.
- Potential/active pairs implement the drag threshold.
- `Interaction::restore()` correctly reverts geometry on Escape.
- `Interaction::preview()` produces a creation preview.
- `begin_left_interaction` shows a clear priority order: pan → text edit → tool → resize handle → move →
  marquee.

**What is missing relative to tldraw:**
- No target vocabulary / re-dispatch (hit-testing happens inline in `begin_left_interaction`).
- No `onEnter`/`onExit` lifecycle.
- No transition payloads (state is stored in the gesture structs instead — workable).
- No `onComplete`/`onInterrupt` distinction.
- No tool lock.
- No tick.
- No parent/child hierarchy — it is a flat enum, so there is no shared tool-level behaviour.

**What is missing relative to the products generally:**
- No selection scope/depth (no hierarchy).
- No rotate state, no handle-drag state, no text-caret state in the enum (text is a separate
  `Option<TextEditState>`).
- No alt-drag / modifier-with-memory.
- No snapping interaction.

---

## Candidate architectural implications

### Implication A — the flat enum is sufficient at current scale; hierarchy is needed for growth

**Evidence:** tldraw's chart exists because it has ~10 tools × ~7 states each plus shared behaviour per tool.
Spool has 7 tools and 7 interaction states.

**Approaches:**
- **A. Keep the flat enum.** Extend as needed. Simple, greppable, no indirection.
- **B. Hierarchical state nodes** (tldraw). Enables per-tool shared behaviour, tool registration at runtime,
  and profile-specific tool substitution.
- **C. A hybrid**: a `Tool` enum (which owns its behaviour) + a per-tool interaction enum.

[INFERRED] **C is a good fit for Spool.** The profile system needs tools to be substitutable, and per-tool
shared behaviour (cursor, cancel, commit) needs somewhere to live — but a full hierarchical chart with
dot-notation transitions is more machinery than a 7-tool prototype needs.

**Decision: TBD — requires architecture review.**

---

### Implication B — target vocabulary + re-dispatch is the enabling structure

**Evidence:** tldraw DOCUMENTED (5 targets, re-dispatch from `idle`).

**Why it matters:** Enables (a) per-tool hit-testing, (b) a uniform `hitTest(point)` that AI and profiles can
call, (c) driver-based testing, (d) handle-aware input.

**Decision: TBD — requires architecture review.**

---

### Implication C — interactions must be pure functions of (start, pointer, modifiers)

**Evidence:** tldraw DOCUMENTED via the bail-on-modifier-change behaviour.

**Spool relevance:** `apply_move` is incremental; `apply_resize` is already pure. The move path needs to match.

**Decision: TBD — requires architecture review.**

---

### Implication D — instrument by interaction state

**Evidence:** tldraw DOCUMENTED (`trackPerformance`, state paths, `fps`, `p95FrameTime`).

**Spool relevance:** `diagnostics::count("canvas_notify", 1)` exists; interaction-window instrumentation would
be a direct extension.

**Decision: TBD — requires architecture review.** Low effort.

---

### Implication E — cancel / complete / interrupt

**Evidence:** tldraw DOCUMENTED.

**Spool relevance:** Interrupt is undefined today.

**Decision: TBD — requires architecture review.**

## Open questions

1. Flat enum or hierarchical states?
2. Is hit-testing inside the state machine (re-dispatch) or outside?
3. Do interactions re-derive from start on every move?
4. What happens on pointer-capture loss?
5. Is there tool lock, and where is the policy enforced?
6. Are interactions instrumented as performance windows?
7. How does a profile substitute or disable a tool?

## Sources

- tldraw: `sdk-features/tools.mdx` ⭐⭐⭐ (state chart, lifecycle, dispatch, targets, tool lock, static props),
  `sdk-features/input-handling.mdx`, `sdk-features/history.mdx` ⭐ (bail), `sdk-features/performance.mdx` ⭐
  (`trackPerformance`, interaction-end), `sdk-features/eraser.mdx`, `sdk-features/pen-mode.mdx`,
  `sdk-features/instance-state.mdx`, `docs/driver.mdx`
- Figma: "Select layers and objects" (/360040449873); "Adjust alignment, rotation, position, and dimensions"
  (/360039956914); "Use Figma products with a keyboard" (/360040328653)
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/); "Node Tool"
  (/tools-tools-node/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`Interaction`, `drag_threshold_crossed`, `begin_left_interaction`,
  `Interaction::restore`, `Interaction::preview`, `MoveGesture`, `ResizeGesture`, `CreateGesture`)
