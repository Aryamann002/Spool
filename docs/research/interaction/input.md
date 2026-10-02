# Interaction: Input System

Input is the layer that turns physical events into semantic editor operations. It is where Spool's
ambition for configurable interaction profiles lives or dies.

## The mapping problem

Every product solves the same problem: many physical inputs → few semantic operations.

```
Physical          Semantic                              Document effect
─────────────     ────────────────────────────────       ──────────────────────
left drag         translate / brush / marquee           depends on active tool
right click       context menu                          none
middle drag       pan camera                            none
shift + drag      constrain                             geometry
alt + drag        duplicate / from-centre               geometry + new objects
space + drag      pan (temporarily)                     none
wheel             zoom or scroll                        none
two-finger        scroll / zoom                         none
pen               draw with pressure                    shape
⌘                 deep-select / bypass snap             varies
key               action                                varies
```

[INFERRED] The three-layer structure is common to all mature editors:

1. **Device layer** — normalises pointer/keyboard/touch/stylus/gesture into a small set of semantic events.
2. **Interaction layer** — the state chart / tool decides what a semantic event *means* right now.
3. **Operation layer** — a semantic operation mutates the document and creates a history entry.

The mistake is collapsing layers 2 and 3, or letting layer 1 leak device specifics into tools.

## Device support across products

| Device | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Mouse | Yes | Yes | Yes | Yes |
| Trackpad | Yes (two-finger scroll; `⇧`+wheel horizontal) | Yes (pinch, wheel) | Yes | Yes |
| Stylus | Not documented as distinct | **`isPenMode` — "becomes true once a direct-display stylus (e.g. Apple Pencil) is used"** | **Yes — full pressure brush** | Not documented |
| Touch | Yes (tap/hold → Select Multiple) | **`isCoarsePointer`** — "true for touch input" | Yes | **Primary input on mobile** |
| Middle mouse | Yes | **`onMiddleClick` handler on every state node** | Not documented | Not documented |
| Wheel | Yes | **`onWheel` handler** | Not documented | Native |
| Pinch | Yes | **Yes — pinch events flow through `editor.dispatch`** | Not documented | Native |
| Right click | Context menu | **`onRightClick` handler** | Not documented | Context menu |
| Drag & drop | Yes | **`sdk-features/drag-and-drop.mdx`** — a documented module | Yes | Yes |

### tldraw's documented input state [DOCUMENTED — `sdk-features/instance-state.mdx`]

```ts
instance.screenBounds      // viewport dimensions
instance.devicePixelRatio  // display scaling factor
instance.isCoarsePointer   // true for touch input
instance.isPenMode         // true once a direct-display stylus is used
instance.isFocused         // whether the editor has focus
instance.cursor            // cursor type and rotation
```

[INFERRED] `isCoarsePointer` and `isPenMode` are the two most important device flags for an editor,
because they change hit-test slop, handle sizes, and gesture availability. Note `isPenMode` is
**sticky** — once a stylus has been seen, it stays on. That is a deliberate design decision (once you
start drawing with a pencil you probably keep using it) that Spool would have to decide independently.

### tldraw's input event set [DOCUMENTED — `sdk-features/tools.mdx`]

Every state node can implement:

```
onPointerDown  onPointerMove  onPointerUp
onLongPress    onDoubleClick
onRightClick   onMiddleClick
onKeyDown      onKeyUp        onKeyRepeat
onWheel        onCancel       onComplete    onInterrupt
onTick
```

[INFERRED] Three of these are worth calling out because they are *lifecycle* rather than *input* events:

- `onCancel` — the escape hatch for pointer capture loss, system gestures, or explicit cancel.
- `onComplete` — the interaction succeeded.
- `onInterrupt` — the interaction was aborted by something other than the user.

tldraw separates "cancel" and "interrupt" even though both end the interaction. That distinction matters:
a user-initiated Escape should *bail* history; a system interruption may need to *commit*. Collapsing them
is a common source of "sometimes my drag gets recorded, sometimes it doesn't" bugs.

`onTick` is a per-animation-frame callback, which is how the draw tool does fitting and the hand tool
does easing.

## The pointer event model

### tldraw's dispatch and re-dispatch [DOCUMENTED]

> "Event info objects include a `target` property indicating what the user interacted with: `canvas`,
> `shape`, `handle`, `selection`, or `overlay`. **The canvas dispatches every pointer event with `target:
> 'canvas'`. The select tool's idle state hit-tests the pointer position and re-dispatches the event to
> itself with a more specific target**, then transitions to the matching child state: `pointing_shape` for
> a shape, `pointing_canvas` for empty canvas."

[INFERRED] This is the single most architecturally important input decision in the research. It means:
- The event pipeline does not require the hit-test; **the tool performs the hit-test as part of its state
  logic**.
- Different tools can hit-test differently (e.g. a custom lasso tool, a measure tool).
- The target vocabulary is fixed and small: `canvas`, `shape`, `handle`, `selection`, `overlay`.

### Pointer capture

Spool's prototype uses GPUI's `window.capture_pointer(hitbox.id)` and stores the hitbox id in an
`Rc<Cell<Option<CanvasHitbox>>>` [OBSERVED in source]. This is the correct native equivalent of
`setPointerCapture` and is necessary for drags that leave the view.

[INFERRED] Pointer capture is a *device-layer* concern and must be handled there, not in tools. Every tool
that can drag needs it, so making it automatic is correct.

### Coalesced input

tldraw's draw tool opts into `useCoalescedEvents` for "higher-fidelity input"; always off on iOS.
[DOCUMENTED]

[INFERRED] Drawing quality depends on sub-frame input samples. A native editor should check whether its
input stack exposes equivalent high-frequency samples (pointer prediction on macOS, coalesced moves on
Windows/Linux via the toolkit).

## Right-click

| Product | Documented behaviour |
|---|---|
| Figma | Right-click opens a context menu containing **Select layer** (submenu of every layer under the cursor, in Layers-panel order, including hidden and locked layers with a padlock icon) |
| tldraw | `onRightClick` is a normal state-node event |
| Affinity | Context menus documented (Flip, Paste Style, etc. — menu contents not read in this pass) |
| Canva | `⇧10` opens "More actions menu for a selected element"; tap-and-hold on touch |

[INFERRED] Right-click is context-menu in every product. In Figma it is also a **selection refinement**
mechanism (the Select layer submenu), which is why `⌘`-click deep-select and right-click-select-layer
coexist.

## Middle mouse

| Product | Documented |
|---|---|
| Figma | Pan (OBSERVED) |
| tldraw | `onMiddleClick` is an available handler on every state node — **the core provides the event, the tool decides the meaning** |
| Affinity | Not documented |
| Canva | Not documented |

[INFERRED] tldraw's approach is the right one: the device layer reports "middle button", and a tool that
cares (hand tool) acts on it. Hard-coding "middle = pan" in the input layer forecloses other uses.

## Space-drag

| Product | Documented |
|---|---|
| Figma | Pan (OBSERVED) |
| tldraw | **Hand tool instead** |
| Affinity | Not documented |
| Canva | Not documented |

Spool's prototype: `should_pan = button == Middle || (button == Left && self.space_held)` [OBSERVED in
`begin_pan`].

[INFERRED] Space-drag and a Hand tool are complementary, not alternatives. Space-drag is faster when
you are already using a tool; a Hand tool is discoverable and keyboard-reachable. **Spool should probably
have both.**

## Wheel, pinch, and trackpad

| Product | Documented |
|---|---|
| Figma | Wheel scrolls; `⌘`+wheel zooms (community); `⇧`+wheel scrolls horizontally |
| tldraw | `onWheel` handler; **pinch events flow through `editor.dispatch`**; the **zoom tool** (`select-zoom-tool` action) exists as an explicit mode |
| Affinity | Not documented |
| Canva | Native scrolling/zoom per view mode |

[INFERRED] Two distinct gesture classes must not be conflated:
- **Wheel** = discrete notched scroll (Windows/Linux) or smooth pan (macOS trackpads).
- **Pinch** = a trackpad gesture with a scale delta.

A design editor typically wants: wheel = scroll (or zoom with `⌘`), pinch = zoom, trackpad two-finger =
pan. Three products get this right; getting it wrong is the single most-feeling-wrong input issue in web
design tools.

[OBSERVED / UNVERIFIED] Spool's prototype has an `on_scroll_wheel` handler registered but its behaviour
(wheel zooms vs. scrolls, modifier handling, pinch) was **not verified in this research pass**. This is a
concrete gap.

## Input → operation composition

[INFERRED] The generalisable rule from all four products: **a drag should be a pure function of (start
state, current pointer position, current modifiers)**.

Evidence: tldraw's mid-drag modifier change bails accumulated changes and re-applies from the mark.
[Fully DOCUMENTED in `sdk-features/history.mdx`]

Consequences:
- Moving 600px is not 600 mutations to reconcile.
- Modifier changes mid-gesture are exact.
- Undo is trivially one entry.
- Thumbnail / preview rendering can re-run the gesture.

Spool's prototype partially satisfies this: `resized_geometry(start, handle, delta)` is already a pure
function of (start geometry, handle, delta), and `Interaction::restore()` correctly resets to start
geometry on Escape. [OBSERVED]

But `apply_move` is *incremental* — it applies a delta to the current position each frame rather than
recomputing from the start snapshot. [OBSERVED: `apply_move` reads `object.geometry.position + delta`]
This means a mid-drag modifier change cannot be implemented by "bail and re-apply" without first changing
`apply_move`.

## Profiles and input

[INFERRED] The input system is the *lowest* layer where an interaction profile must be able to intervene,
because every higher behaviour depends on it. A profile that changed only tool names or panel layout would
not change feel at all.

| What a profile could change | Which layer |
|---|---|
| Key bindings | Action registry |
| Which gesture pans vs. zooms | Device normalisation |
| Snap default and modifier | Interaction (tool) layer |
| Nudge distances | Operation parameters (preferences) |
| Tool set and toolbar | UI + action registry |
| Whether snapping is on by default | Preferences |
| Proportional-resize as modifier vs. mode | Interaction layer |
| Double-click meaning (descend vs. enter edit) | Interaction layer |

[INFERRED] Only the last two require changing behaviour rather than configuration. Everything else is
data. This is encouraging: an interaction-profile system may be *mostly* configuration, with a small
behavioural seam at the tool/state layer.

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/main.rs` [OBSERVED in source]:

- **Mouse**: left, middle, right (via GPUI events). `MouseButton::Middle` and `space_held` gate pan.
- **Keyboard**: 16 hard-coded `KeyBinding`s (see `keyboard.md`).
- **Text input**: `impl EntityInputHandler for CanvasView` — IME, `marked_range`, UTF-8/16 conversion.
- **Focus**: `Option<FocusHandle>`; `window.focus(focus_handle, cx)` on text-edit entry.
- **Pointer capture**: `Rc<Cell<Option<CanvasHitbox>>>` + `window.capture_pointer`.
- **Wheel**: `on_scroll_wheel` registered; **behaviour unverified**.
- **Click count**: `event.click_count >= 2` used for double-click into text edit.
- **Missing**: no stylus/pressure, no touch-specific handling, no coarse-pointer detection, no pinch, no
  right-click context menu, no middle-click-as-tool-event, no drag-and-drop, no coalesced input.

## Candidate architectural implication

**Evidence:**

1. The device layer must be separate from the tool layer. [tldraw DOCUMENTED — `onWheel`,
   `onMiddleClick`, `onRightClick` are per-state handlers]
2. Hit-testing belongs *inside* the interaction state machine, with re-dispatch by target.
   [tldraw DOCUMENTED]
3. `cancel` and `interrupt` are distinct lifecycle events. [tldraw DOCUMENTED]
4. `isCoarsePointer` and `isPenMode` are first-class editor state. [tldraw DOCUMENTED]
5. High-frequency input needs sub-frame sampling, with a fallback when unavailable. [tldraw DOCUMENTED,
   including the iOS caveat]
6. Interactions should be pure functions of (start, pointer, modifiers). [tldraw DOCUMENTED via bail]
7. Pointer capture is a device-layer concern. [INFERRED from all products]
8. Right-click is context menu everywhere; in Figma it is also a selection-refinement surface.
   [DOCUMENTED]

**Why it matters:** Interaction profiles (§ `profiles/`) require the input layer to be substitutable at the
*semantic* level, not just the binding level. And the driver-style agent approach (§ `architecture/ai-
runtime.md`) requires that synthetic input flows through exactly the same path as real input.

**Potential Spool approaches:**

- **A. Device events → enum variants → handlers.** What the prototype does. Simple; no target
  vocabulary; hit-testing lives in handlers.
- **B. A + a semantic event layer** (`PointerDown { pos, button, modifiers, click_count }`) that the
  interaction runtime re-dispatches with a `target`.
- **C. B + a `Target` enum** (`Canvas`, `Shape(id)`, `Handle(id, kind)`, `Selection`, `Overlay`) and an
  event that can be re-dispatched.
- **D. C + separate `Cancel` / `Interrupt` / `Complete` lifecycle signals.**
- **E. C + an input-source abstraction** so a synthetic driver can inject the same events.

**Tradeoffs:** A is cheapest. C and D are the enabling structure for both interaction profiles and
agent-driving, and C is a prerequisite for a clean "what is under the pointer" question that the
operation API will need.

[INFERRED] **C + E together are the single highest-leverage input change available to Spool**, because
they simultaneously enable (a) interaction profiles, (b) a tldraw-style driver for testing and agents,
and (c) a uniform "hit test this point" operation. The prototype's `hit_test` already exists as a
`Document` method — the missing piece is delivering that answer *as an event target* to the interaction
runtime.

**Decision: TBD — requires architecture review.**

## Open questions

1. Does GPUI expose both the physical key and the emitted character? (Required for tldraw's keyboard
   matching strategies.)
2. Does GPUI expose coalesced/high-frequency pointer samples?
3. Does GPUI expose pointer pressure and stylus type? (`isPenMode` equivalent.)
4. Does GPUI distinguish coarse (touch) from fine pointers?
5. What does the prototype's `on_scroll_wheel` actually do?
6. Does GPUI have pointer capture semantics equivalent to `setPointerCapture`?
7. Should the input layer be swappable per profile, or should only bindings be?

## Sources

- tldraw: `sdk-features/tools.mdx` ⭐ (event handler list, event targets, re-dispatch, coalesced events,
  tool lock), `sdk-features/instance-state.mdx` ⭐ (`isCoarsePointer`, `isPenMode`, `devicePixelRatio`),
  `sdk-features/input-handling.mdx`, `sdk-features/drag-and-drop.mdx`, `sdk-features/actions.mdx`
  (activation conditions), `sdk-features/history.mdx` (bail on modifier change), `docs/driver.mdx`
- Figma: "Select layers and objects" (/360040449873); "Use Figma products with a keyboard" (/360040328653);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914)
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/); "Add, duplicate, and delete elements"
  (/add-elements/)
- Spool prototype: `app/src/canvas.rs` (`begin_pan`, `capture_pointer`, `CanvasHitbox`, `on_scroll_wheel`,
  `click_count`, `EntityInputHandler`, `apply_move`), `app/src/main.rs`
