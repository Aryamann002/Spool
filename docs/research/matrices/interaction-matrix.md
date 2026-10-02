# Interaction Matrix

> §33. Per-interaction behaviour across products, with precision about modifiers and context.
> **No ranking.** Where behaviour varies by tool or context, that is stated rather than averaged.
>
> Spool column is [SOURCE-CODE] against the working tree — it is the prototype, not a target.
> Legend: **✔** · **◐** partial · **—** absent · **?** undocumented

---

## 1. Pointing and clicking

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Click empty canvas | clears selection | clears selection | clears selection | clears selection | ✔ clears |
| Click object | select | select | select | select | ✔ topmost by reverse linear hit test |
| Double-click object | **descend into group** / edit text | **edit text** / focus group | **enter Node Tool** on paths | **direct-select** (`⌘F2`) | **edit text only** |
| Triple-click | selects a paragraph | ? | ? | ? | — |
| Shift-click | add to selection | add to selection | add to selection | add to selection | ✔ |
| Cmd/Ctrl-click | remove from selection | subtract | ? | ? | — |
| Alt/Option-click | **select the one behind** | **select the one behind** | **cycle through overlaps** | ? | — |
| Cmd/Ctrl-click (deep) | **deep-select into group** | — | — | — | — |
| Click during a gesture | joins / changes gesture target | re-dispatched by `target` | ? | ? | priority: pan→text→tool→handle→move→marquee |
| Click on empty during gesture | cancels | cancels | cancels | cancels | starts marquee |
| Click count source | platform | platform | platform | platform | `MouseDownEvent::click_count` available |

[INFERRED] **Double-click means three different things across the corpus** — descend a level, edit
text, or enter a node-editing mode. This is the single largest interaction ambiguity in the corpus and
it is resolved differently in every product.

---

## 2. Dragging and transforming

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Drag selected object | move | move | move | move | ✔ |
| Drag unselected object | select + move | select + move | ? | ? | ✔ |
| Shift-drag | **constrain axis** | constrain axis | — | — | **—** |
| Alt-drag | **duplicate** | **bail to preview** | — | — | **—** |
| Cmd/Ctrl-drag | ? | constrain 45° | **mirror / shear** (with selection box) | — | **—** |
| Shift+Alt-drag | — | — | — | — | — |
| Drag a resize handle | resize | resize | resize | resize | ✔ 8 handles, **single selection only** |
| Shift while resizing | **preserve aspect** | preserve aspect | — | — | **—** |
| Alt while resizing | resize about centre | — | **3-way corner split** | — | **—** |
| Shift+Alt while resizing | aspect + centre | — | — | — | **—** |
| Drag a rotate handle | rotate | rotate | rotate | rotate | **— (no rotate)** |
| Shift while rotating | 15° increments | — | ✔ | — | — |
| Drag below threshold | treated as a click | treated as a click | ? | ? | ✔ `DRAG_THRESHOLD = 4.0` |
| Drag outside canvas | continues | continues | continues | continues | pointer capture; loss path unverified |
| Drag a locked object | no-op | no-op | no-op | no-op | — no lock concept |

[INFERRED] **`Alt`-drag is the sharpest divergence**: duplicate in Figma, *bail the gesture* in tldraw,
nothing in Affinity or Canva. A profile cannot be defined without choosing here, because "duplicate"
and "bail" are mutually exclusive semantics for one keystroke.

---

## 3. Marquee and bulk selection

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Drag on empty canvas | marquee | **lasso** | selection box | marquee | ✔ marquee |
| Containment rule | ◐ varies by context | ◐ | **Shift = full containment** | ? | contains |
| Intersection rule | ◐ | ◐ | **Ctrl = partial intersection** | ? | — |
| Cycle selection-box shape | — | — | ✔ `.` | — | — |
| Marquee while a tool is active | tool gesture | tool gesture | tool gesture | n/a (no tools) | tool gesture |
| Select-all | `⌘A` | ✔ | ✔ | ✔ | ✔ (text action) |
| Select-all-in-direction | — | — | — | ✔ `⇧W/A/S/D` | — |

[INFERRED] Affinity is the only product that documents both marquee rules explicitly, and they map to
modifiers rather than to settings. That is a small but important design decision: **marquee mode is a
gesture modifier, not a preference**, in Affinity; elsewhere it is ambiguous.

---

## 4. Navigation and camera

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Space-drag | pan | pan | pan | ? | ✔ |
| Middle-drag | pan | pan | pan | ? | ✔ |
| Two-finger scroll | pan | pan | pan | pan | ✔ |
| Wheel alone | vertical pan (page-based) | pan | pan | pan | ✔ |
| Cmd/Ctrl + wheel | zoom | zoom | zoom | ? | ✔ (gated on `control \|\| platform`) |
| Pinch | zoom | zoom | ◐ | ? | `PinchEvent` exists |
| Zoom anchor | cursor | cursor | cursor | cursor | ✔ |
| Zoom to fit | `⇧1` | `⇧1` | ✔ | `⌥⌘0` | ✔ `⇧1` |
| Zoom to selection | `⇧2` | `⇧2` | ✔ | — | **—** |
| Zoom to 100% | `⇧0` | `⇧0` | ✔ | `⌘0` | **—** |
| Modifier reset on focus loss | ✔ implied | ✔ implied | ✔ implied | ✔ implied | **— no `on_blur` registered** |

---

## 5. Keyboard verbs

| Verb | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Undo | `⌘Z` | `⌘Z` | `⌘Z` | `⌘Z` | ✔ **via raw key path** |
| Redo | `⌘⇧Z` | `⌘⇧Z` | `⌘⇧Z` | `⌘Y` / `⇧⌘Z` | ✔ both |
| Delete | `⌫` / `⌦` | ✔ | ✔ | ✔ | ✔ **via raw key path** |
| Duplicate | `⌘D` | ✔ | `⌘D` | `⌘D` | ✔ `⌘D` |
| Group | `⌘G` | — | `⌘G` | ✔ | **—** |
| Ungroup | `⇧⌘G` | — | `⌘J` | ✔ | **—** |
| Move into group | — | — | `⌥⌘G` | — | — |
| Move out of group | — | — | `⌥⇧⌘G` | — | — |
| Flatten | — | — | ✔ 3 scopes | — | — |
| Send backward / forward | ✔ | ✔ | ✔ | ✔ | **—** |
| Arrow nudge | ✔ | ✔ | ✔ | ✔ | ✔ 1 unit |
| Shift+Arrow nudge | ✔ larger | ✔ | ✔ | ✔ | ✔ |
| Precise (10× / 0.1×) | ✔ (`⌥`) | ✔ | ✔ | ✔ panel | **—** |
| Select all | `⌘A` | ✔ | ✔ | ✔ | ✔ (text action only) |
| Enter (descend) | descend | ? | — | `⌘F3` | **—** |
| Escape (cancel) | cancel → up a scope | **up one chart node** | cancel / leave Node Tool | cancel | ✔ flat handler |
| Quick actions | `/` (layers) | ? | `/` (limited layouts) | `/` or `⌘E` | **—** |

[INFERRED] **Escape's structure is the clearest single discriminator in the corpus.** Figma, tldraw and
Affinity all treat it as *up one level of a hierarchy* — selection scope, state chart, or tool mode.
Canva treats it as *cancel*. Spool's handler does both, plus closing four overlays, in one branch.

---

## 6. Tool lifecycle

| Question | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Tools persist after use? | shape tools **no**; Text **yes**; Hand/Scale **yes** | shape tools **no**; Text **yes** | varies; Personas persist | **no tools exist** | **shape tools do not revert** |
| Tool lock | — | double-click tool | — | — | — |
| Tool double-click | — | ✔ lock | — | — | — |
| Tool keyboard shortcut | `V F R O L T C …` | ✔ | ✔ | one-shot commands | ✔ `V F R O T P C` |
| Tools needing a model | pen, connectors, tables | none | Node, Pen, Brush | none | **Pen declared, unimplemented** |
| Tool shown but inert | — | — | — | — | **Pen, Comment** |

[INFERRED] Figma and tldraw agree on persist-after-use, including the Text exception. Spool diverges
on the shape tools. This is a one-line difference with an outsized effect on how the product feels.

---

## 7. Text editing

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Enter text edit | double-click | double-click | in-place | double-click / `⌘F3` | ✔ double-click |
| Exit text edit | `Esc` | `Esc` | `Esc` | `Esc` | ✔ `Esc` |
| Select all text | `⌘A` | ✔ | ✔ | ✔ | ✔ |
| Caret movement | ←/→ | ✔ | ✔ | ✔ | ✔ |
| Word movement | `⌥←/→` | ✔ | ✔ | ✔ | ✔ via actions |
| Line start/end | `Home`/`End`, `⌘←/→` | ✔ | ✔ | ✔ | ✔ |
| Delete forward / back | `⌦` / `⌫` | ✔ | ✔ | ✔ | ✔ |
| IME composition | ✔ | ✔ | ✔ | ✔ | ✔ `marked_range` |
| IME before keybinding | ? | ✔ | ? | ? | **— hook unused** |
| Editing a text inside a group | ✔ | ✔ | ✔ | ✔ | ✔ |
| One undo step per session | ✔ | ✔ | ✔ | ✔ | ✔ |

---

## 8. Creation

| Interaction | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Click-to-create default size | ✔ | ✔ | ✔ | ✔ | ✔ |
| Drag-to-create explicit size | ✔ | ✔ | ✔ | ✔ | ✔ |
| Click-to-type auto-width | ✔ | ✔ | ✔ | ✔ | ✔ |
| Drag-for-fixed-width text | ✔ | ✔ | ✔ | ✔ | ✔ |
| Creation reverts to select | ✔ | ✔ | varies | n/a | **✘** |
| Escape cancels creation | ✔ discard | ✔ discard | ✔ discard | n/a | ✔ `Interaction::restore()` |
| Creating past the world edge | clamps / extends | extends | extends | n/a | hard-coded `WORLD_BOUNDS` |
| Drag out of a group | reparent (rule undocumented) | **creates a new group** | ? | ? | — |
| Overlap z-order | created on top | ✔ | ✔ | ✔ | ✔ appended |

---

## 9. Snapping during gestures

| Gesture | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Move snaps | ✔ | ✔ | ✔ | ◐ | **—** |
| Resize snaps | ✔ | ✔ | ✔ | ◐ | **—** |
| Rotate snaps | ✔ | — | ✔ | ? | **—** |
| Draw snaps | ✔ | ✔ | ✔ | ? | **—** |
| Node edit snaps | — | — | ✔ construction snapping | — | **—** |
| Snap disable modifier | ✔ | `Esc` | ✔ | ? | **—** |
| Snap to grid | — | ✔ | ✔ pixel | ? | **— (grid is decorative)** |
| Snap to guide | — | — | ✔ | ? | **—** |

---

## 10. History verbs during a gesture

| Situation | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Drag, then Escape | discarded | **bailed** (gesture survives Alt-switch) | discarded | discarded | ✔ restored |
| Drag, release | one undo step | one undo step (mark) | one undo step | one undo step | ✔ one snapshot |
| Text session | one step | one step | one step | one step | ✔ one `TextChange` |
| Multi-selection style change | one step | one step | one step | one step | ✔ one `Style` op |
| Undo mid-gesture | n/a | possible with bail | ? | ? | not modelled |

---

## 11. What the matrix exposes

[INFERRED] Six rows carry most of the architectural weight:

| Row | Why it matters |
|---|---|
| **Double-click** | Three meanings across four products. The clearest evidence that interaction needs a *context* model, not a verb table. |
| **`Alt`-drag** | Duplicate vs bail. Mutually exclusive semantics for one keystroke — a profile must choose. |
| **Escape** | "Up one level" vs "cancel". Spool's flat handler is the shape that makes both hard. |
| **Tool persistence** | Two products agree; Spool diverges on a one-line change. |
| **Marquee rules** | Only Affinity documents both. Everywhere else it is ambiguous. |
| **Modifier columns are mostly empty for Spool and Canva** | The corpus's transform conventions are gesture-handler work, and neither of those two has built them. |

[INFERRED] The empty cells in the Spool column are **not evenly distributed**: nearly all of them sit
in rows that require a document model (group, z-order, rotate, snap-to-guide) rather than a gesture
handler. See `profiles/spool.md` §13.

## 12. Sources

Drawn from the evidence cited in `docs/research/interaction/*.md` and `docs/research/products/*.md`:
Figma help ⭐⭐⭐; tldraw docs + source ⭐⭐⭐; Affinity help ⭐⭐; Canva help ⭐⭐;
Spool `app/src/canvas.rs`, `app/src/shell.rs`, `app/src/main.rs` (SOURCE-CODE).
Cross-references: `matrices/keyboard-matrix.md`, `matrices/feature-matrix.md`,
`architecture/interaction-runtime.md`, `profiles/*.md`.
