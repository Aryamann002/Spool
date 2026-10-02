# Interaction Profile Evidence — Spool (prototype)

> **What this file is.** A factual record of what the Spool prototype's interaction repertoire
> actually is today, expressed in the same categories as the four product profiles, so that a future
> `spool` profile can be authored against a baseline and the gaps measured.
>
> **What this file is not.** A target. Nothing here is a recommendation, and per §44 the prototype is
> treated as experimental. Several observations below are *corrections* to earlier notes in this
> corpus, and are marked.
>
> All claims are [SOURCE-CODE] against the working tree at `app/`.

---

## 1. Summary

Spool today has the **Figma-shaped skeleton** — shape tools with `V/F/R/O/T`, a `Frame` concept, a
canvas with `⇧1` zoom-to-fit — with **two parallel keyboard paths**, **no snapping**, **no hierarchy**,
**no rotate**, and **two declared-but-unimplemented tools**.

Its interaction repertoire, stated as a profile:

- **Tool vocabulary:** 7 declared, 5 implemented.
- **Contexts:** 1 implicit (an in-flight `Interaction`), plus a text-editing session. **No focus-mode
  ladder.** [CORRECTION to earlier notes in this corpus, which implied a selection-scope model: there
  is none — `Selection { selected: Vec<ObjectId> }` has no depth and no scope container.]
- **Modifiers during transforms:** **none.** Move and resize read `event.modifiers` only to the extent
  that `space_held` (left-button pan) is derived from a key flag; there is no `⇧`-constrain,
  no `⌥`-duplicate, no aspect-preserve.
- **Navigation:** space-drag / middle-drag pan, `⌘`|`Ctrl`+wheel zoom, `⇧1` fit.
- **Snapping:** none.
- **History:** undo/redo implemented, bound via the raw-key path, not the action path.
- **Selection:** click, `⇧`-click add, marquee, click-empty clears. No depth, no behind, no `Tab`.

[INFERRED] The honest characterisation is: **a linear, single-depth, no-modifier prototype with
Figma-shaped naming.** The naming creates an expectation the behaviour does not yet meet — which is
worth recording explicitly, because a profile is partly a promise.

---

## 2. Tool vocabulary

[SOURCE-CODE] `app/src/shell.rs:5-13`:

```rust
const TOOLS: [(&str, &str, &str, canvas::Tool); 7] = [
    ("↖", "Select", "V", canvas::Tool::Select),
    ("▱", "Frame",  "F", canvas::Tool::Frame),
    ("□",  "Rectangle", "R", canvas::Tool::Rectangle),
    ("○",  "Ellipse", "O", canvas::Tool::Ellipse),
    ("⌁", "Pen",     "P", canvas::Tool::Pen),
    ("T",  "Text",    "T", canvas::Tool::Text),
    ("◌", "Comment", "C", canvas::Tool::Comment),
];
```

[SOURCE-CODE] `Tool::creates_object()` returns `None` for **Pen** and **Comment**. Both are selectable,
both appear in the toolbar, and neither does anything.

[INFERRED] **This is the clearest example in the whole corpus of a profile promising what the document
model cannot deliver.** A Pen tool requires a path model (`creative/vector.md`); a Comment tool
requires comments to be document objects with their own identity and thread model. Neither exists.
Shipping them in the toolbar makes the tool list a claim the document cannot honour.

**Persist-after-use:** [SOURCE-CODE] shape tools do not revert to Select — `set_tool` is only called
from the toolbar and from the shortcut path, never on commit. This **diverges from every product in
the corpus** and would be the most immediately noticeable difference to a Figma user.

**Missing entirely vs the product profiles:** line, arrow, slice, hand/pan, scale, gradient, crop,
image, and every Affinity vector tool.

---

## 3. Contexts and focus modes

[SOURCE-CODE] There are exactly two contexts:

1. **Text editing** — `TextEditState { id, original_text, editing_text, selected_range,
   selection_reversed, marked_range, pointer_anchor }`. Entered by double-click or by the Text tool's
   creation gesture; exited by `Esc` (`cancel_text_edit`).
2. **In-flight interaction** — `Interaction { None, PotentialMove, Moving, PotentialResize, Resizing,
   PotentialCreate, Creating }` with `Interaction::restore()` on Escape.

[SOURCE-CODE] `Escape` in `shell.rs:1348-1368` does four things in one handler: cancel the
manipulation, close the AI/share/export/zoom overlays, and if nothing was cancelled, clear the
selection.

[INFERRED] That single `Escape` handler is a **flattened** version of what the corpus treats as a
chart traversal (Figma `Esc` = up one scope; tldraw `Esc` = up one chart node; Affinity = leave Node
Tool). The behaviour is defensible; the structure — one handler doing four unrelated jobs — is what
`architecture/interaction-runtime.md` identifies as the gap.

[SOURCE-CODE] **No focus-mode ladder.** No `⌘F1/F2/F3`, no selection depth, no `focusedGroupId`
analogue. Clicking selects whatever the topmost hit test returns.

[SOURCE-CODE] No `.on_focus` / `.on_blur` listener is registered anywhere, although a `FocusHandle`
exists and is focused and tracked. Consequence: **no hook exists to reset transient modifier state or
cancel an in-flight drag on focus loss.** See `architecture/input-system.md` Implication C.

---

## 4. Modifier semantics by operation

[SOURCE-CODE] The complete inventory of modifier-sensitive behaviour:

| Operation | Modifier | Behaviour |
|---|---|---|
| Pan | — | middle-drag always pans |
| Pan | `Space` held | left-drag pans (`space_held` flag) |
| Zoom | `⌘` or `Ctrl` + wheel | zoom about the cursor |
| Zoom to fit | `⇧1` | `fit_canvas` |
| Resize | `⇧` | **not implemented** — `⇧` does not constrain |
| Move | `⇧` | **not implemented** |
| Duplicate | `⌘D` / `Ctrl+D` | `duplicate_selection` |
| Undo / redo | `⌘Z` / `⌘⇧Z` / `Ctrl+Y` | |

[SOURCE-CODE] **During move and resize, `event.modifiers` is not consulted at all.**
[INFERRED] Consequently there is no `⇧`-constrain-to-axis, no `⌥`-drag-copy, no
`⇧`-preserve-aspect, no `⌥`-resize-about-centre — the four most-copied transform conventions in the
corpus, all absent.

[SOURCE-CODE] `space_held` is a plain `bool` field on `CanvasView`, set and cleared by the *shell*
view's raw `on_key_down`/`on_key_up` handlers through a `WeakEntity` update.

---

## 5. Creation semantics

[SOURCE-CODE]

| Tool | Click | Drag |
|---|---|---|
| Rectangle / Ellipse / Frame | creates with a default size | creates with a drag-defined size |
| Text | enters a text session with auto-sizing | creates a drag-defined text box |
| Pen / Comment | declared only | declared only |

[INFERRED] The click-vs-drag split **does** match the Figma convention recorded in
`profiles/figma.md` §5. That is the single strongest piece of evidence that the prototype's
interaction instincts are aimed at the right target.

[SOURCE-CODE] Text line height is hard-coded at `18.0` (`gpui_px(18.0 * zoom)`). No font metrics, no
font selection, no wrapping model beyond what the platform text input provides. `creative/typography.md`.

---

## 6. Navigation

[SOURCE-CODE]

| Action | Method |
|---|---|
| Pan | middle-drag, space-drag |
| Zoom | `⌘`/`Ctrl` + wheel, about the cursor |
| Zoom range | `MIN_ZOOM 0.1`, `MAX_ZOOM 4.0` |
| Zoom to fit | `⇧1` |
| World bounds | hard-coded `WORLD_BOUNDS (764, 688)`, `WORLD_CENTER (382, 344)` |

[INFERRED] `WORLD_BOUNDS` being hard-coded means the canvas is **effectively bounded** while
*behaving* like an infinite one. This is a divergence worth recording: every product in the corpus is
either explicitly infinite (tldraw, Figma) or explicitly bounded (Canva), and an accidental bound is
neither.

---

## 7. Snapping configuration

[SOURCE-CODE] **None.** No snap manager, no tolerance, no indicators, no guides, no rulers, no grid
that participates in interaction.

[SOURCE-CODE] A zoom-adaptive grid *is* rendered (24–48 screen px, `dot_size = (1.5 * zoom).clamp(1.0,
2.0)`) but it is decorative — nothing snaps to it.

[INFERRED] Relative to the product profiles this is the single largest interaction gap, and per
`architecture/profiles.md` §2.3 it is also the axis on which one product (Affinity) has already
shipped a profile-like preset system. See `interaction/snapping.md` and `architecture/rendering.md`.

---

## 8. Selection model summary

[SOURCE-CODE]

| Concept | Spool today |
|---|---|
| Selected | `Selection { selected: Vec<ObjectId> }` — a flat set, no depth, no scope |
| Hit test | `objects.iter().rev().find(contains)` — linear, topmost-first |
| Z-order | vector index; `ObjectId` constants `LANDING=1, EDITOR=2, FEATURES=3, MOBILE=4` |
| Additive | `⇧`-click |
| Marquee | left-drag on empty canvas |
| Behind | not implemented |
| Depth | **not implemented** |
| Traversal | not implemented |
| Groups | not implemented |
| Layers panel | flat `LayerRow` list; `LayersProjection` with an O(changed) presentation diff |

[INFERRED] The **flat, index-based z-order with hard-coded page object ids** is a significant
constraint. `LANDING`/`EDITOR`/`FEATURES`/`MOBILE` are *page names used as object identities*, which
means the four "pages" in `shell.rs`'s `PAGES` array are not a page model at all — they are four
disjoint object ranges sharing one document. This is a prototype shortcut with architectural
consequences for `document-model.md`.

---

## 9. History perception

[SOURCE-CODE] `History { undo: Vec<DocumentCommand>, redo }` with before/after **value snapshots**;
`DocumentCommand` wraps `CommandOperation::{Geometry(Vec<GeometryChange>), Style, Text,
Insert(Vec<ObjectPlacement>), Delete}`; `record()` filters no-op diffs and clears redo; `Insert`/`Delete`
carry the z-order index. One `TextChange` per text session = one undo step.

[SOURCE-CODE] Undo/redo **are reachable** — but only through `shell.rs`'s raw key path
(`shortcut_action` → `ShortcutAction::Undo|Redo`), **not** through GPUI actions. This is a
[CORRECTION]: earlier notes in this corpus described Spool as having no undo binding. It has one; it
simply lives in the wrong path. See `architecture/input-system.md` §3.

[SOURCE-CODE] Selection is **not** captured in history. Neither is the camera. No marks, no bail, no
squash. See `architecture/history.md` for the ten-row gap table.

---

## 10. Accessibility state

[SOURCE-CODE] Two concrete, verified gaps:

1. **No `prefers_ime_for_printable_keys` implementation.** The tool shortcuts `v f r o t p c` are
   single printable letters handled in `shell.rs`'s raw `on_key_down`. [INFERRED] With a Japanese,
   Korean or Chinese IME active, those keystrokes should compose text, not switch tools. GPUI provides
   the hook specifically to prevent this; Spool does not use it.
2. **Tool shortcuts guard `!platform && !control && !shift` but not `alt`.** [SOURCE-CODE]
   `shortcut_action` in `shell.rs:63-74`. [INFERRED] `⌥V` selects the Select tool. Minor, but it shows
   the guard conditions are ad hoc.

[SOURCE-CODE] Positively: the `EntityInputHandler` implementation is correct — UTF-16 ranges, a
`marked_range` for IME composition, and `prefers_ime_for_printable_keys` simply left at its default.
The *text* path is more accessible than the *toolbar* path.

---

## 11. Distance from each product profile

| Convention | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Shape tools revert to Select | ✔ | ✔ | varies | n/a (one-shot) | **✘** |
| `⇧`-constrain during transform | ✔ | ✔ | ✔ | n/a | **✘** |
| `⌥`-duplicate on drag | ✔ | ✔ | — | n/a | **✘** |
| `⇧`-preserve aspect on resize | ✔ | ✔ | ✔ | via `F2` | **✘** |
| Click vs drag changes creation | ✔ | ✔ | ✔ | n/a | **✔** |
| Escape cancels then exits | ✔ (chart) | ✔ (chart) | ✔ | ✔ | **partial** (one flat handler) |
| Undo is one perceived action | ✔ | ✔ (marks/bail/squash) | ✔ ("often") | ✔ | **✔** for geometry/text |
| Zoom-to-fit | `⇧1` | `⇧1` | ✔ | `⌥⌘0` | **`⇧1`** |
| Space-drag pan | ✔ | ✔ | ✔ | not documented | **✔** |
| Middle-drag pan | ✔ | ✔ | ✔ | not documented | **✔** |
| Selection depth | ✔ | ✔ (`focusedGroupId`) | ✔ (`⌥`-cycle) | ✔ (3 focus modes) | **✘** |
| Select behind | `⌥` | `⌥` | `⌥`-cycle | — | **✘** |
| Snapping | ✔ | ✔ (screen-px ÷ zoom) | ✔ (7 presets) | — | **✘** |
| Focus-mode ladder | `⌘F1/2/3` | — | `A` mode | `⌘F1/2/3` | **✘** |
| One keyboard path | ✔ | ✔ | ✔ | ✔ | **✘ (two)** |
| User-remappable keys | ✘ | ✔ (host) | ✔ | ✘ | **✘** |
| All declared tools implemented | ✔ | ✔ | ✔ | ✔ | **✘ (2 of 7)** |

Count: **6 of 19 conventions currently match.** [INFERRED] The six that match are the *structural*
ones — the ones that require a document model rather than a gesture handler. That is the pattern worth
noting: Spool's gaps are almost entirely in the layers that need a document model, an interaction
state machine, or a keymap file, and almost none in the layers that need a handler.

---

## 12. Two corrections to earlier notes in this corpus

| Earlier claim | Correction | Evidence |
|---|---|---|
| "No undo/redo binding despite History existing" | **Undo/redo exist**, via `ShortcutAction::Undo\|Redo` in `shell.rs`'s raw key path — not via GPUI actions | `shell.rs:44-61, 1300-1340` |
| "No action registry" | **An action registry exists** (GPUI's) with 12 `spool_text` actions; it is simply unused by every non-text command | `canvas.rs:10-26`; see `architecture/input-system.md` §3.2 |

Both corrections point at the same underlying fact and are recorded rather than silently amended.

---

## 13. What a future `spool` profile would inherit

[INFERRED] If the `spool` profile is the default, then its content is whatever the editor's core
supports — which means the profile concept, for Spool, is mostly a statement that **the core is the
default profile**. The other four profiles are then the interesting ones, and the useful exercise is
measuring each against this baseline (as §11 does).

[INFERRED] The single highest-leverage observation from this file: **Spool's profile gaps correlate
with document-model gaps, not with gesture-handler gaps.** Rotate, corner radius, opacity, hierarchy,
and reparenting are all missing — but so are the document fields that would express them
(`architecture/document-model.md`). A profile feature that has no document representation cannot be
built by writing a gesture handler.

## 14. Sources

- Spool prototype (SOURCE-CODE): `app/src/main.rs` (16 `KeyBinding`s, all `context: None`),
  `app/src/canvas.rs` (`Tool`, `creates_object`, `Interaction`, `Selection`, `Document`,
  `History`, `DocumentCommand`, `Camera`, `MIN_ZOOM`/`MAX_ZOOM`, `WORLD_BOUNDS`, `DRAG_THRESHOLD`,
  `space_held`, `EntityInputHandler`), `app/src/shell.rs` (`TOOLS`, `PAGES`, `ShortcutAction`,
  `shortcut_action`, Escape handler, `⇧1`), `app/src/layers.rs`
- Related notes: `architecture/input-system.md` §3, `architecture/profiles.md`,
  `architecture/editor-runtime.md`, `architecture/document-model.md`, `architecture/history.md`,
  `architecture/rendering.md`, `interaction/*.md`, `matrices/interaction-matrix.md`
