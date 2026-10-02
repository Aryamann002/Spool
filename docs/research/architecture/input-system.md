# Architecture — Input System

> Research note. Layer 2 of three: **what the products' architectures appear to require.**
> Layer 1 (what the products do) lives in `docs/research/interaction/input.md` and `keyboard.md`.
> Layer 3 (what Spool should do) is *not* decided here.

Scope: the machinery that turns platform events into semantic editor operations — pointer
normalisation, capture, modifiers, chords, context-scoped action dispatch, focus, text input,
IME, and accessibility routing.

This note contains one finding that materially changes cost estimates elsewhere in the corpus:

> **GPUI already ships a complete, JSON-user-editable action + keymap + context-predicate system.**
> Spool's "action registry" is not a from-scratch build. It is an adoption decision plus a naming
> and ownership layer.

---

## 1. The layered shape every product needs

[INFERRED] All four products, and any mature editor, factor input into at least these stages.
The stage boundaries matter because they are where "a Figma-like shortcut" and "a Figma-like
drag" become separable concerns.

| # | Stage | Responsibility | Spool equivalent today |
|---|---|---|---|
| 0 | Platform source | OS delivers `NSEvent` / `RawInput` / DOM event | `gpui_platform` (opaque) |
| 1 | Pointer normalisation | button, position, modifiers, click count, device kind | `MouseDownEvent`, `MouseMoveEvent`, `ScrollWheelEvent`, `PinchEvent` |
| 2 | Capture | decide which element owns a drag, and release it | `Window::capture_pointer(HitboxId)` |
| 3 | Modifier state | current held modifiers, chord state | `Modifiers`, `Keystroke` |
| 4 | Keyboard dispatch | keystroke/chord → action, scoped by context | `Keymap` + `DispatchTree` + `Action` |
| 5 | Event routing | action/event → the view that should handle it | `on_action`, `DispatchPhase`, focus tree |
| 6 | Text input | UTF-16 ranges, replacement, IME composition | `EntityInputHandler` |
| 7 | Interaction state | which tool state is live; consume or fall through | `Interaction` enum (Spool) |
| 8 | Semantic operation | `MoveObjects`, `DeleteSelection`, `SetText` | direct document mutation |

The critical architectural seam is **between 5 and 7**. Below 7 everything is a mechanical
translation. Above 7, the interaction state machine decides meaning.

[OBSERVED] Spool currently collapses stages 5–8: `begin_left_interaction` in `app/src/canvas.rs`
does its own priority resolution (`pan → text edit → tool → resize handle → move → marquee`) and
then mutates the document directly.

---

## 2. GPUI's action and keymap system (SOURCE-CODE — vendored `gpui` at rev `397cbc84`)

This is the highest-value section in the note because it is directly inspectable and it is the
framework Spool is built on.

### 2.1 Actions are a global, named, JSON-constructible registry

[SOURCE-CODE] `crates/gpui/src/action.rs`:

```rust
pub trait Action: Any + Send {
    fn boxed_clone(&self) -> Box<dyn Action>;
    fn partial_eq(&self, action: &dyn Action) -> bool;
    fn name(&self) -> &'static str;
    fn name_for_type() -> &'static str where Self: Sized;
    fn build(value: serde_json::Value) -> Result<Box<dyn Action>> where Self: Sized;
    fn action_json_schema(...) -> Option<JSONSchema> { None }
    fn deprecated_aliases() -> &'static [&'static str] { &[] }
    fn deprecation_message() -> Option<&'static str> { None }
    fn documentation() -> Option<&'static str> { None }
}
```

- `actions!(namespace, [Name, …])` declares unit-struct actions; names are `namespace::Name`.
- `register_action!(T)` allows a hand-written impl for parameterised actions.
- `ActionRegistry` is a `HashMap<&'static str, ActionData>` plus a `TypeId → name` reverse map,
  populated at `App` construction via `inventory::collect!(MacroActionBuilder)`.
- **Duplicate registration panics at `App` creation** — the registry is validated eagerly.
- Because `Action::build` takes `serde_json::Value`, **an action can be reconstructed from JSON**.
  That is the mechanism by which a user-editable keymap file works. It is not a Spool feature; it
  is already there.
- Actions may carry data: `#[derive(Action)] pub struct SelectNext { pub replace_newest: bool }`.

Two sentinel actions exist for rebinding:
- `NoAction` — "unbind the keybinding this is associated with, if it is the highest precedence match."
- `Unbind(SharedString)` — "unbind later bindings for the same keystrokes when they dispatch the named action."

### 2.2 Keybindings carry keystrokes *and* a context predicate

[SOURCE-CODE] `crates/gpui/src/keymap/binding.rs`:

```rust
pub struct KeyBinding {
    action: Box<dyn Action>,
    keystrokes: SmallVec<[KeybindingKeystroke; 2]>,   // ← up to 2 ⇒ chords are supported
    context_predicate: Option<Rc<KeyBindingContextPredicate>>,
    meta: Option<KeyBindingMetaIndex>,               // ← source precedence for user overrides
    action_input: Option<SharedString>,              // ← raw JSON from the keymap
}
```

`KeyBinding::new(keystrokes, action, context: Option<&str>)` is the common constructor and
**panics on parse error**.

### 2.3 Contexts are a stack, predicates are a small language

[SOURCE-CODE] `crates/gpui/src/keymap/context.rs` and `key_dispatch.rs`:

- `DispatchTree::push_node()` / `pop_node()` build a **per-frame tree of dispatch nodes**; each node
  carries a `KeyContext` (a list of `key` / `key=value` entries), a `FocusId`, and a `ViewId`.
- `KeyBindingContextPredicate` is an explicit little language:

  | Predicate | Meaning |
  |---|---|
  | `Identifier(s)` | context contains `s` |
  | `Equal(a, b)` | context has `a == b` |
  | `NotEqual(a, b)` | context has `a != b` |
  | `Descendant(p, c)` | `p` above `c` in the element tree — written `p > c` |
  | `Not(p)` / `And(..)` / `Or(..)` | `!`, `&&`, `\|\|` |

  Example from the source doc: `StatusBar && mode == visible`, and `StatusBar > mode == visible`.

[INFERRED] This is structurally the same mechanism as tldraw's `StateNode` path (a tool state is a
node; a key binding may be scoped to a subtree) and as Canva's `⌘F1/F2/F3` focus model (contexts
that change what keystrokes mean). Spool would be expressing an interaction state machine in a
language the framework already has.

### 2.4 Resolution, precedence, chords, and undo of a pending chord

[SOURCE-CODE] `Keymap::bindings_for_input(input, context_stack) -> (SmallVec<KeyBinding>, bool)`:

- Bindings are scanned in reverse registration order and ranked by **(context depth descending,
  registration index descending)**. Documented rule: *"Precedence is defined by the depth in the tree
  (matches on the Editor take precedence over matches on the Pane, then the Workspace, etc.). … In
  the case of multiple bindings at the same depth, the ones added to the keymap later take
  precedence. User bindings are added after built-in bindings so that they take precedence."*
- `match_keystrokes` returns `Option<bool>`: `None` = no match, `Some(true)` = **pending** (a longer
  binding could still match), `Some(false)` = final match. This is how `g v` works without `g`
  firing first.
- `dispatch_key` returns bindings to execute, the new pending set, and **`to_replay`** — keystrokes
  that were held pending and are no longer matched, to be replayed into the focused view.
  `flush_dispatch` does the same on a timeout.
- `possible_next_bindings_for_input` exists specifically so a UI can offer chord completion.

### 2.5 Action delivery is two-phase over a hierarchical tree

[SOURCE-CODE] `Window::on_action(TypeId, Fn(&dyn Any, DispatchPhase, &mut Window, &mut App))` and
`Context::on_action(TypeId, window, Fn(&mut T, &dyn Any, DispatchPhase, …))`.

- `DispatchPhase` implies **capture and bubble** phases.
- `on_action_when(condition, …)` registers a listener for exactly one frame — the idiomatic way to
  declare "this action applies *right now*".
- `is_action_available(action, target)` and `available_actions(target)` exist, plus
  `bindings_for_action(action)` and `highest_precedence_binding_for_action(action, context_stack)`.
  [INFERRED] Menus can therefore be **generated from the keymap**, with disabled items computed from
  live availability — the same relationship Figma documents between its context menus and its
  shortcut list.
- `focus_path`, `dispatch_path`, `focus_contains(parent, child)` expose the hierarchy explicitly.

### 2.6 Keystroke syntax and cross-platform modifiers

[SOURCE-CODE] `crates/gpui/src/platform/keystroke.rs`:

```
[secondary-][ctrl-][alt-][shift-][cmd-][fn-]key[->key_char]
```

- `secondary` resolves to `platform` on macOS and `control` everywhere else. **[INFERRED] one
  binding string therefore works on both platforms without duplicating it** — this is the same
  abstraction Figma calls a keyboard *layout* and that `docs/research/interaction/keyboard.md`
  records as "16 layouts, no user rebinding".
- `cmd`, `super`, and `win` all set `modifiers.platform`.
- Display glyphs exist: `shift` → `⇧`, `alt` → `⌥`, `platform` → `⌘`.
- `Modifiers { control, alt, shift, platform, function }` — there is **no separate Super/Windows/
  AltGr slot**, and no `caps_lock` on the struct (it lives on `ModifiersChangedEvent`).

[SOURCE-CODE] `crates/gpui/src/platform/keyboard.rs`:

```rust
pub trait PlatformKeyboardLayout { fn id(&self) -> &str; fn name(&self) -> &str; }
pub trait PlatformKeyboardMapper {
    fn map_key_equivalent(&self, keystroke: Keystroke, use_key_equivalents: bool) -> KeybindingKeystroke;
    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>>;  // only used on macOS
}
```

[INFERRED] A keyboard-layout abstraction already exists at the framework level. `docs/research/
profiles/` therefore does not need to invent one; it needs to decide how far to use it.

### 2.7 Pointer events — and what they do not carry

[SOURCE-CODE] `crates/gpui/src/interactive.rs`:

| Event | Fields |
|---|---|
| `MouseDownEvent` | `button: MouseButton`, `position: Point<Pixels>`, `modifiers`, `click_count: usize`, `first_mouse: bool` |
| `MouseUpEvent` | `button`, `position`, `modifiers`, `click_count` |
| `MouseMoveEvent` | `position`, `pressed_button: Option<MouseButton>`, `modifiers` (+ `.dragging()` == left held) |
| `ScrollWheelEvent` | `position`, `delta: ScrollDelta`, `modifiers`, `touch_phase: TouchPhase` |
| `PinchEvent` | `position`, `delta` (sign: positive = zoom in) |
| `ModifiersChangedEvent` | `modifiers`, caps-lock state |

`ScrollDelta` is `Pixels(Point<Pixels>)` or `Lines(Point<f32>)`, with `.pixel_delta(line_height)`.

**Not present on any pointer event** [SOURCE-CODE, by absence]:
- `pointer_type` (mouse / pen / touch)
- `pressure`
- tilt / altitude / barrel button / rotation
- a coalesced-event flag
- a multi-button bitmask (only one `pressed_button`)

[INFERRED] Consequences: Spool cannot currently distinguish a stylus from a finger, cannot read
pressure, cannot request coalesced high-frequency move events for freehand drawing, and cannot do
palm rejection. Those are exactly the inputs Affinity's brush/pencil ecosystem depends on. This is
a **framework-level constraint**, not a Spool design choice, and it interacts with the eventual
UI-technology decision recorded in `rendering.md`.

### 2.8 Text input and IME are a first-class framework contract

[SOURCE-CODE] `crates/gpui/src/platform.rs`, `InputHandler` — deliberately 1:1 with
`NSTextInputClient`, with the AppKit method named in each doc comment:

| Method | AppKit counterpart |
|---|---|
| `selected_text_range` | `selectedRange()` |
| `marked_text_range` | `markedRange()` |
| `text_for_range` | `attributedSubstring(forProposedRange:)` |
| `replace_text_in_range` | `insertText(_:replacementRange:)` |
| `replace_and_mark_text_in_range` | `setMarkedText(_:selectedRange:replacementRange:)` |
| `unmark_text` | `unmarkText()` |
| `bounds_for_range` | `firstRect(forCharacterRange:)` — **used to position the IME candidate window** |
| `character_index_for_point` | `characterIndexForPoint:` |
| `set_selected_text_range` | reverse data flow from system UI |
| `paste(ClipboardItem)` | platform-initiated paste |
| `accepts_text_input` | whether to accept inserted text |
| `prefers_ime_for_printable_keys` | see below |
| `text_input_editable_range` | clamps multi-step IME gestures to an editable window |
| `text_input_configuration` | autocorrect / autocapitalize / suggestions / `enterkeyhint` |

All ranges are **UTF-16 code units**. [INFERRED] Spool's text model must therefore use UTF-16
offsets at the boundary even if it stores UTF-8 internally — a real constraint on any future
rich-text or variable-font work.

Two methods deserve emphasis for Spool:

- **`prefers_ime_for_printable_keys`** — doc: *"Returns whether printable keys should be routed to
  the IME before keybinding matching when a non-ASCII input source (e.g. Japanese, Korean, Chinese
  IME) is active. This prevents multi-stroke keybindings like `jj` from intercepting keys that the
  IME should compose."* Spool's toolbar shortcuts are single letters (`T`, `R`, `L`, `C`, `S`).
  [INFERRED] When an IME is active, those letters must reach the IME, not the toolbar. The framework
  provides the hook; whether Spool uses it is a decision.
- **`accepts_text_input` / `ElementInputHandler::new(bounds, view)`** — the handler is bound per
  element *during paint*, and `bounds_for_range` receives the element bounds so hit-testing results
  can be clamped to the visible text element.

[SOURCE-CODE] `crate::input::EntityInputHandler` is the view-side adapter of the same surface,
bridging `Window::handle_input` to a `Context<Self>`.

### 2.9 Focus

[SOURCE-CODE] `FocusHandle` + `Window::focus` / `track_focus` / `is_focused` / `focus_contains`.
Focus is a separate axis from dispatch depth: a dispatch node is a *position in the element tree*,
a `FocusId` is *what the user is typing into*, and `DispatchTree` keeps both.

[INFERRED] Figma's `⌘F1` select / `⌘F2` direct-select / `⌘F3` text-edit triple is a **three-state
focus ladder**, not three shortcuts. Modelling it as three *contexts* on one focus handle is a
reasonable [PROPOSED] reading, and is exactly what `KeyBindingContextPredicate` expresses.

---

## 3. Spool prototype: observed input state (SOURCE-CODE)

### 3.1 What exists

`app/src/canvas.rs:10`:

```rust
gpui::actions!(
    spool_text,
    [Backspace, Delete, Left, Right, SelectLeft, SelectRight,
     SelectAll, Home, End, Paste, Copy, Cut]
);
```

`app/src/main.rs` registers **16 `KeyBinding::new` calls**, every one with `context: None`.

[SOURCE-CODE] The canvas registers 12 `on_action(cx.listener(Self::text_*))` handlers on the
viewport element. Each handler branches internally and calls `window.play_system_bell()` when not
editing text (e.g. `fn text_backspace`).

### 3.2 Corrections to earlier assumptions in this corpus

The earlier `editor-runtime.md` note described Spool as having "no action registry". That is
**wrong and is corrected here**: an action registry exists and is GPUI's. The accurate statement is
narrower and more interesting.

### 3.3 Accurate gaps

| # | Observation | Evidence |
|---|---|---|
| 1 | **Every binding has `context: None`.** `KeyContext` / `KeyBindingContextPredicate` are entirely unused. | `main.rs:15-30` |
| 2 | **All 12 actions live in one namespace, `spool_text`,** and are all text-editing actions. There is no namespace for document operations. | `canvas.rs:10-26` |
| 3 | **Bindings are compiled in, not loaded.** `KeyBinding::new` in Rust code ⇒ no user remapping, even though the framework supports it via `Action::build(json)`. | `main.rs` |
| 4 | **No undo/redo action exists**, despite `History` being implemented. | `main.rs`, `canvas.rs` |
| 5 | **`cmd-x` *and* `ctrl-x` are both registered** (likewise `-a`, `-v`, `-c`) — the manual workaround for what `secondary-` already expresses. | `main.rs:21-30` |
| 6 | **A second, parallel raw-keyboard path exists** in `shell.rs`, using `on_key_down` string `match` arms for tools, digits, `⌘1`-style modifiers and `space`. It bypasses the action system entirely. | `shell.rs:1288-1391` |
| 7 | **`space_held` is a plain `bool` on `CanvasView`**, mutated from the *shell* view's raw key handlers via a `WeakEntity` update. | `shell.rs:1373-1389`, `canvas.rs:1160,1592` |
| 8 | **No `.on_focus` / `.on_blur` listener is registered anywhere**, although a `FocusHandle` exists and is focused/tracked. So `space_held` has no focus-loss reset, and an in-flight drag has no cancel hook. | `canvas.rs:1205,1231,2350`; grep finds no `on_focus`/`on_blur` |
| 9 | **Zoom gating is `event.modifiers.control \|\| event.modifiers.platform`** — the same dual-write smell as #5. | `canvas.rs:2178-2185` |
| 10 | **Pan is `Middle \|\| (Left && space_held)`**, hard-coded, not data. | `canvas.rs:1601` |
| 11 | **Pointer capture uses `Rc<Cell<Option<CanvasHitbox>>>` + `window.capture_pointer`**, but no release-on-loss path is observable. | `canvas.rs:1823-1826` |
| 12 | `EntityInputHandler for CanvasView` implements text editing with UTF-16 ranges and an `marked_range` for IME — the framework contract is used correctly. | `canvas.rs` `TextEditState` |

[INFERRED] Item #6 is the most consequential. Two keyboard paths means key bindings will drift from
actual behaviour, and any user-remapping feature would only remap half the surface.

---

## 4. Cross-product input conventions

[INFERRED from §22/§23 research] Input mapping conventions that are already near-standardised, and
which therefore belong in the *core* editor rather than in a profile:

| Convention | Figma | tldraw | Affinity | Canva | Status |
|---|---|---|---|---|---|
| Space-drag pans | yes | yes | yes | not documented | near-standard |
| Middle-drag pans | yes | yes | yes | not documented | near-standard |
| Two-finger scroll pans | yes | yes | yes | yes | near-standard |
| Pinch zooms | yes | yes | limited | not documented | near-standard |
| `⌘`/`Ctrl` + wheel zooms | yes | yes | yes | not documented | near-standard |
| Escape cancels the current interaction | yes | yes | yes | yes | **universal** |
| Right-click opens a context menu | yes | yes | yes | yes | universal |
| Drag may start on an unselected object and select it | yes | yes | varies | yes | common |
| Modifiers are read at gesture **start** and latched | yes | yes | mixed | mixed | divergent — decide |

The last row is a genuine disagreement. Affinity re-reads modifiers mid-drag in several places
(⌃-drag with a selection box mirrors/shears rather than moving). [INFERRED] a latched-at-start
model is easier to reason about and to replay; a live model is more discoverable. There is no
consensus to defer to.

### 4.1 Accessibility as an input requirement

[DOCUMENTED] Canva documents `⌘F1` / `⌘F2` / `⌘F3` as an explicit focus ladder, which screen-reader
users are expected to rely on.
[INFERRED] A general-purpose editor must expose focus state as a *queryable, addressable* property,
not merely as a side effect of pointer clicks. Spool currently has a `FocusHandle` and
`is_focused(window)` but no exposed focus mode.

---

## 5. Coarse pointer and stylus: the one place Spool's framework is visibly behind

| Capability | tldraw | Affinity | GPUI |
|---|---|---|---|
| `pointerType` on move/down | yes (`'mouse' \| 'touch' \| 'pen'`) | full stylus support | **absent** |
| `pressure` | yes | yes (brush, node handles) | **absent** |
| Coalesced events opt-in | `useCoalescedEvents` per tool | n/a | **absent** |
| Coarse-pointer flag in instance state | `isCoarsePointer` | n/a | **absent** |
| Device pixel ratio in instance state | `devicePixelRatio` | Retina handling | available elsewhere |

[SOURCE-CODE] tldraw's per-tool static config declares `useCoalescedEvents` per tool, which means
**freehand tools opt into higher-fidelity move streams and everything else opts out**. That is a
cheap and highly targeted optimisation pattern.

[INFERRED] Spool can be feature-complete for mouse+keyboard on GPUI today, and cannot be
stylus-complete without either framework changes or a different UI technology. Given Spool's stated
ambition to eventually exceed UI mockups (Affinity territory), this is a real input on the eventual
stack decision — not a detail.

---

## 6. Interaction-profile feasibility, stated as evidence only

[INFERRED] The evidence in §2 says the *keyboard* half of an interaction profile is largely free:
load a different JSON keymap, change context predicates, and the dispatch, precedence, chord,
completion, availability, and menu-generation machinery all follow.

The *pointer* half is **not** profile-shaped by anything found here:

- Pan-on-space and pan-on-middle are hard-coded in Spool (`canvas.rs:1601`).
- Affinity's modifier re-reads mid-drag are behavioural, not keymap-shaped.
- Affinity's **Studios** (`architecture/profiles.md`) show that *which tools exist* is the profile
  dimension products actually expose, not *how the same tool responds to modifiers*.

[PROPOSED-QUESTION] The interesting question is therefore not "can we reskin modifiers" but "which
tools exist, and what does each tool's state machine look like" — with the keymap as a thin,
already-solved layer on top. This file deliberately does not settle it; see
`architecture/profiles.md`.

---

## 7. Candidate architectural implications

> Format per §46. None of these is a decision.

### Implication A — Adopt GPUI's action/keymap system rather than building a parallel one

**Evidence:** [SOURCE-CODE] `action.rs` (`Action`, `ActionRegistry`, JSON `build`), `keymap.rs`
(precedence, chords, `NoAction`/`Unbind`), `key_dispatch.rs` (`DispatchTree`, `KeyContext`,
`DispatchPhase`, `is_action_available`, `possible_next_bindings_for_input`).

**Why it matters:** Every keyboard-adjacent candidate implication elsewhere in this corpus
(action registry, context-scoped shortcuts, user remapping, chord commands, generated menus,
keyboard cheat-sheet) is already *mechanism*; only *namespace, ownership, and content* are Spool's
problem. Building a parallel system would duplicate a maintained, tested one and split dispatch.

**Potential Spool approaches:**
- **A1.** Keep `spool_text` as-is; add a `spool` namespace as operations grow; adopt
  `secondary-` and delete the `cmd-`/`ctrl-` duplicates; move bindings to a JSON keymap file.
- **A2.** As A1, plus adopt `KeyContext` predicates from the start so that context-scoped bindings
  are the only way to add a shortcut.
- **A3.** Add a thin Spool-side layer over GPUI's registry for menu generation and a cheat sheet,
  leaving dispatch untouched.

**Tradeoffs:** A1 is the smallest diff and defers the context decision. A2 costs a naming exercise
now and prevents the "binding added with `None` context" pattern from spreading. A3 buys
discoverability and is the only one that gets `available_actions` used.

**Decision: TBD — requires architecture review.**

---

### Implication B — One keyboard path, not two

**Evidence:** [SOURCE-CODE] `shell.rs:1288-1391` uses raw `on_key_down` string matching for tools,
digits and `space` while `main.rs` uses `KeyBinding::new` for text actions. Both are live.

**Why it matters:** With two paths, no single table answers "what does this key do?", user remapping
cannot be correct, and the recorded keymap will lie. Interaction profiles — a stated Spool goal —
are impossible with a hard-coded parallel path.

**Potential Spool approaches:**
- **B1.** Move everything to actions + bindings; express tool activation and `space` as actions.
- **B2.** B1 plus a single `KeymapSource` enum (`BuiltIn | File(PathBuf)`) so a profile is just a
  different keymap file plus a set of available tool ids.
- **B3.** Keep a raw path only for genuinely raw things (text input while editing, IME), and
  document the exception.

**Tradeoffs:** B1 is mechanical. B2 is what makes profiles real. B3's exception list is small and
worth writing down explicitly, because "except text editing" is exactly the kind of carve-out that
otherwise grows without limit.

**Decision: TBD — requires architecture review.**

---

### Implication C — Make focus a queryable, resettable property; never a bare flag

**Evidence:** [SOURCE-CODE] `space_held: bool` on `CanvasView`, set from the shell view, with no
`.on_focus`/`.on_blur` listener registered anywhere despite an existing `FocusHandle`.

**Why it matters:** A modifier held when the window loses focus is the classic "stuck pan" bug.
Affinity, Figma and Canva all bind `Escape` to a state reset; none of them can afford a flag that
survives focus loss. Any transient input state — space held, a drag in progress, a text session —
needs a defined reset path.

**Potential Spool approaches:**
- **C1.** Register `on_focus`/`on_blur` on the canvas and reset transient modifier state on blur.
- **C2.** Track held keys as part of a per-view input state that is reconstructed from
  `ModifiersChangedEvent` plus focus transitions, so it cannot drift.
- **C3.** Treat "focus mode" as an explicit editor state (see §4.1 and Canva's `⌘F1/F2/F3`) and
  make modifier-as-mode read from it rather than from a raw device flag.

**Tradeoffs:** C1 is the bug fix. C2 is the invariant. C3 is the design that also solves
accessibility, at the cost of a real concept to define. C3 is the only option that makes
`space_held` *not exist*; the other two manage it.

**Decision: TBD — requires architecture review.**

---

### Implication D — Decide the stylus question before, not after, the creative-tooling work

**Evidence:** [SOURCE-CODE] GPUI pointer events carry no `pointer_type`, `pressure`, tilt or
coalesced flag. [DOCUMENTED] Affinity's vector and brush workflows depend on pressure.
[SOURCE-CODE] tldraw exposes all four and makes coalescing a per-tool opt-in.

**Why it matters:** Spool's ambition includes Affinity-class vector work. Pressure-based brushes,
tilted nibs, and palm rejection are not add-ons to that; they are inputs to it. If they are not in
the stack, the honest options are (a) no pressure-sensitive tools, or (b) a framework contribution.

**Potential Spool approaches:**
- **D1.** Accept mouse+keyboard scope for now; record the constraint and revisit at architecture review.
- **D2.** Contribute `pointer_type`/`pressure`/coalesced events upstream to GPUI as a prerequisite.
- **D3.** Design the document model so pressure can be *recorded* (as sampled points) without
  depending on live input, so stylus support is a later input-layer change rather than a
  document-model change.

**Tradeoffs:** D1 is honest and cheap. D2 is a maintenance liability outside Spool's control. D3
costs a small amount of speculative document-model surface, which is precisely what the brief warns
against — but it is the one option that does not create a later migration.

**Decision: TBD — requires architecture review.**

---

### Implication E — Split "which state am I in" from "which action fired"

**Evidence:** [SOURCE-CODE] Spool's `Interaction` enum owns both gesture phase (`PotentialMove`,
`Moving`, …) and tool (`creates_object()`), and `shell.rs` sets the tool directly through a
`WeakEntity` call. [SOURCE-CODE] GPUI dispatches named actions down a context tree and can report
availability per target. [SOURCE-CODE] tldraw separates a `Tool` (a state-chart root) from its
per-tool interaction states, and *checks* `isToolLocked` rather than being forced to honour it.

**Why it matters:** With GPUI's `KeyContext` available, the tool can be expressed as a context
rather than as a field the shell mutates. Then `⌘R` (rotate) would be *unavailable* rather than
*silently doing nothing*, and a profile could change which tools exist simply by changing which
contexts are reachable.

**Potential Spool approaches:**
- **E1.** Keep the current enum; add context predicates that mirror it. Risk: two sources of truth.
- **E2.** Make the interaction state machine own a `KeyContext` and expose it to GPUI's dispatch
  tree, so bindings and states cannot disagree.
- **E3.** Adopt tldraw's shape: a tool per state-chart root with its own sub-states, surfaced to
  GPUI as contexts.

**Tradeoffs:** E1 is cheap and keeps a duplication risk. E2 removes the duplication but couples the
interaction machine to the framework's dispatch tree. E3 is the largest change and the closest to
`interaction-runtime.md`'s recommendation; it should not be adopted before that document's
open questions are answered.

**Decision: TBD — requires architecture review.**

---

## 8. Open questions

1. Is `KeyContext` per-frame (`DispatchTree`) or persistent? If per-frame, where does an
   interaction state machine publish its context without being a view?
2. What is the failure mode of `DispatchPhase` capture for an editor — should an outer scope ever
   *consume* an action an inner scope also handles?
3. Should a keystroke that is both a chord prefix and a command (`g`) be disambiguated by timeout?
   `flush_dispatch` implies yes; what timeout is right for a canvas?
4. How does GPUI's `secondary-` model interact with a design where `Ctrl` has a *different meaning*
   on macOS and Windows (e.g. `Ctrl`-click deep-selecting in Figma vs `⌘`-click)?
5. What is the correct UTF-16 boundary discipline for a text model that stores UTF-8?
6. Should `PinchEvent` and trackpad scroll be normalised into one continuous zoom gesture, or kept
   distinct? The two feel different and the products do not agree.
7. Is pointer capture released when the capture target is unmounted mid-drag? Not observable in the
   current source; a lost capture with a live `Moving` state is a plausible corruption path.
8. What is the accessibility focus model Spool will expose, and does it need to be user-visible
   (as Canva's `⌘F1/F2/F3` ladder is) or only machine-readable?
9. Does the eventual stack need pressure/tilt, and if so is that a blocker for the AI
   "generate a vector path" flow, which produces pressure data but consumes none?

## 9. Sources

- GPUI (SOURCE-CODE, vendored at rev `397cbc84de333eeb56849dc90782d365b37e60d7`):
  `crates/gpui/src/action.rs`, `keymap.rs`, `keymap/binding.rs`, `keymap/context.rs`,
  `key_dispatch.rs`, `input.rs`, `interactive.rs`, `platform.rs`, `platform/keystroke.rs`,
  `platform/keyboard.rs`, `window.rs` (`capture_pointer`, `on_action`, `on_key_event`,
  `on_modifiers_changed`)
- tldraw (SOURCE-CODE): `packages/editor/src/lib/hooks/useEvents.ts` (event kinds, `PointerInfo`),
  `packages/editor/src/lib/editor/Editor.ts` (`dispatch`, `inputs`, `run` capture modes),
  tool config table (`id`/`initial`/`isLockable`/`useCoalescedEvents`/`trackPerformance`),
  `packages/driver` (headless pointer/keyboard/wheel/pinch simulation), instance-state fields
  (`isCoarsePointer`, `devicePixelRatio`, `isToolLocked`)
- Figma (DOCUMENTED): keyboard shortcut pages, `⌘F1/F2/F3`, 16 keyboard layouts, no user rebinding;
  help.figma.com
- Affinity (DOCUMENTED): resource pages for "personas", keyboard shortcuts, snapping;
  affin.co/help ⭐⭐
- Canva (DOCUMENTED): Keyboard shortcuts, Canva AI help; canva.com/help ⭐⭐
- Spool prototype (SOURCE-CODE): `app/src/main.rs`, `app/src/canvas.rs` (actions, `space_held`,
  pan rule, zoom modifier check, pointer capture, `EntityInputHandler`), `app/src/shell.rs`
  (raw key handlers)
