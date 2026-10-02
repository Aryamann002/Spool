# Interaction: Selection

Cross-product research on selection semantics. See `matrices/interaction-matrix.md` for the raw grid.

## Why this is a foundational domain

Selection is the bridge between *pointer position* and *semantic intent*. Almost every downstream system
depends on it: transformation operates on the selection, the inspector reads the selection, history
records selection changes, the layers panel renders the selection, and an AI agent's context is
(selection + document).

## The critical distinction: selected vs. editing vs. active tool

This was called out as extremely important in the research brief, and the evidence supports treating
these as **three separate pieces of state**.

| State | What it means | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|---|
| **Active tool** | Which interaction grammar is currently interpreting input | `Tool` (Move, Frame, Pen, Text, …) | Root state node id (`select`, `draw`, `hand`, …) | Current tool (Node, Pen, Move, …) | **No persistent tool** — creation is a one-shot command |
| **Selected** | Which objects are the subject of the next operation | Set of layer ids (+ a *nesting depth*) | `selectedShapeIds` in instance state | Current selection | Selected elements |
| **Editing** | Which object has entered a sub-mode with its own input grammar | Text edit mode; vector edit mode; shape-create drag | `editingId` — the shape whose handles are live | Node Tool active on a curve | Text editing |

### Evidence

**Figma** implements "editing" as an *implicit mode* revealed by modifier and click-count:
- click → select parent; double-click → descend one level; `Enter` → descend; `⇧Enter` → ascend; `Esc` →
  ascend/deselect. [DOCUMENTED]
- A double-click on a vector object enters **vector edit mode** — proven by the existence of a
  vector-mode-only preference: "Snap to geometry: *Used only in vector edit mode*." [DOCUMENTED]
- Click-to-create text places the object and enters edit mode in one gesture. [DOCUMENTED]

**tldraw** makes "editing" explicit: the editor holds an *editing shape* distinct from the selection, and
the culling system treats "the shape being edited" as never-cullable — "so users can always see what
they're working with". [DOCUMENTED — `sdk-features/culling.mdx`]

**Affinity** makes it a *tool* rather than a mode: the Node Tool is a tool you switch to (`A`), and it
operates on whatever is selected. [DOCUMENTED]

[INFERRED] Three plausible implementations:

| Approach | Consequence |
|---|---|
| Modal (Figma) | Simplest state; awkward for nested cases; selection depth becomes a hidden mode variable |
| Tool-based (Affinity) | Explicit; but forces tool switching for every sub-mode |
| Separate editing state (tldraw) | Cleanest separation; costs one more piece of editor state |

Spool's prototype currently collapses these: `Interaction` is one enum, `Selection` is a plain id list,
and text editing is a separate `Option<TextEditState>`. It has no notion of a selection *depth*.

## Selection is document state or editor state?

| Product | Where selection lives | In history? | Synced? | Persisted? |
|---|---|---|---|---|
| Figma | Editor state (Layers panel is a view) | Unknown | Yes (presence) | No |
| tldraw | **Instance state** (session scope) | **Yes** — with `record-preserveRedoStack` | Yes (presence records) | Session snapshot |
| Affinity | Editor state | Unknown | No | No |
| Canva | Editor state | Unknown | Yes | No |

tldraw's decision is the most explicitly documented:

> "Selection changes are recorded in history without clearing the redo stack."
> "We use `record-preserveRedoStack` when selecting shapes. This way you can undo, select some shapes,
> copy them, and then redo back to where you were. The selection goes on the undo stack, but existing
> redos aren't cleared."
> [DOCUMENTED — `sdk-features/history.mdx`]

[INFERRED] This is a subtle and correct requirement: selection is *not* a document mutation, so it should
not clear the redo stack — but the user should still be able to undo their last *selection change* if
they selected the wrong thing. That requires **selection to be undoable independently of document
changes**. Very few products document this.

## Selection scope and depth

### Figma: implicit scope stack [DOCUMENTED]

- Click selects the **parent** by default.
- Double-click / `Enter` descends one level.
- `⇧Enter` ascends one level.
- `Esc` clears or ascends.
- `Cmd/Ctrl`-click bypasses the parent rule and selects the actual hit object ("deep select").
- Marquee with `Cmd/Ctrl` selects nested objects rather than parents.
- Figma explicitly warns that selecting a child without its parent is possible and "something to be
  mindful of, especially when selecting objects to move them".

[INFERRED] Figma's model is: `selection: Set<Id>` + `nestingDepth: usize` (relative to whatever is hit).
The "select parent by default" rule is then "when hit-testing, climb to the ancestor at
`nestingDepth` levels up".

### tldraw: focused group [DOCUMENTED]

- `focusedGroupId` is explicit editor state.
- Selecting children of a group focuses that group.
- While a group is focused, clicks select shapes inside it, not the group.
- Clearing the selection **leaves the focused group in place**.
- `selectAdjacentShape` is scoped: "If the selection is inside a group or frame, only siblings in that
  container are considered."
- `selectParentShape()` / `selectFirstChildShape()` are explicit APIs.

[INFERRED] tldraw separates two concerns Figma conflates: (a) *what* is selected, and (b) *the container
in which selection operations act*. Figma's `nestingDepth` implies the container; tldraw names it.

### Affinity: object selection only [DOCUMENTED]

`⌘↑` selects the parent layer. There is no documented depth-stack behaviour on canvas.

### Canva: flat [DOCUMENTED]

Grouping exists but no nesting-descent interaction is documented. Selection is element-level.

## The invariant: no ancestor and descendant both selected

tldraw enforces this automatically:

> "When the selection changes, the editor filters out any shape whose ancestor is also selected […] This
> prevents ambiguous situations where both a container and its contents are selected. The filtering
> happens in the `instance_page_state` after-change side effect."
> [DOCUMENTED]

Figma **does not** enforce this — the documentation warns against it instead.

[INFERRED] Two defensible positions:
- **Enforce** (tldraw): simpler downstream reasoning; prevents ambiguous transforms.
- **Warn** (Figma): more expressive; the user asked for both.

The decision affects what a transform over a mixed selection means.

## Marquee semantics

| Product | Default | Additive modifier | Partial-intersection mode | Notes |
|---|---|---|---|---|
| Figma | Fully-contained | `⇧`-click toggles; `⇧`-marquee selects *matching* objects | **Not documented** | `⌘`-marquee selects nested children |
| tldraw | "brushing" state | Documented at method level | Not documented | State named `brushing` |
| Affinity | Fully-contained | `⇧`-drag = add/remove on full containment | **`⌃`-drag (Mac) / LMB-then-RMB (Win)** = add on partial intersection | Two distinct modes |
| Canva | Documented at a high level | Element multi-select mode via `F8` | Not documented | `⇧WASD` directional multi-select instead |

[INFERRED] Affinity is the only product with a **user-selectable intersection mode**. For a
pixel-honest editor like Spool, whether a 2% overlap counts as "selected" is a genuine semantic choice,
not an implementation detail. Canva's `⇧WASD` avoids the question by never having a marquee-as-primary.

## Selecting behind / overlapped objects

| Product | Mechanism |
|---|---|
| Figma | `⌘/Ctrl`-click deep select; **Select layer menu** (right-click) listing every layer under the cursor in Layers-panel order, including hidden (when toggled) and **locked** layers with a padlock icon |
| tldraw | None documented |
| Affinity | **`⌥`-click cycles overlapped objects** |
| Canva | Not documented |

[INFERRED] Affinity's `⌥`-click cycle is the cheapest possible affordance and is worth adopting on
principle: one key, no menu, no modifier-memory. Figma's Select-layer menu is the most powerful
(disambiguates by name) but costs a menu. Both are reasonable; they answer different questions.

## Selection bounds

tldraw is the only product that documents the distinction explicitly:

| Bound | Method | Meaning |
|---|---|---|
| Axis-aligned page bounds | `getSelectionPageBounds()` | Smallest AABB containing all selected shapes, **including rotated ones** |
| Rotated page bounds | `getSelectionRotatedPageBounds()` | Respects the selection's **shared** rotation; falls back to AABB if rotations differ |
| Screen variants | `getSelectionScreenBounds()`, `getSelectionRotatedScreenBounds()` | Camera-applied |
| Shared rotation | `getSelectionRotation()` | `0` if shapes have different rotations |

[DOCUMENTED — `sdk-features/selection.mdx`]

Figma: rotates about "the horizontal and vertical centre of your selection" [DOCUMENTED], which implies a
composed selection centre exists; the exact bounds model is **Unknown**.

[INFERRED] "Rotated selection bounds with fallback to AABB" is the correct general answer, and it also
defines what a rotation handle should do when the selection is non-uniform.

## Selection traversal

| Product | Mechanism | Scope |
|---|---|---|
| Figma | `Tab` / `⇧Tab` (siblings), `Enter` (child), `⇧Enter` (parent) | Documented |
| tldraw | `selectAdjacentShape('next'\|'prev'\|'left'\|'right'\|'up'\|'down')` — "Cardinal directions score candidate shapes by distance and by how far they sit off the axis of travel, then pick the lowest score" | Siblings within the focused container; shapes whose `canTabTo()` is false are skipped |
| Canva | `Tab`/`⇧Tab` (elements), `⇧W/A/S/D` (closest in direction, user-facing) | Elements |
| Affinity | `⌥]` next layer, `⌥[` previous, `⌘↑` parent (Layers panel) | Layers panel |

[INFERRED] The tldraw scoring function ("distance AND off-axis distance, lowest score wins") is a
directly implementable rule and is the only precise public specification of "nearest in direction"
selection found. Worth adopting.

## Locked and hidden objects

| Product | Locked | Hidden |
|---|---|---|
| Figma | Cannot be left-clicked; **can** be selected via the Select layer menu, shown with a padlock icon | Not shown in the Select layer menu; must be un-hidden to select |
| tldraw | Excluded from bulk ops; selectable via explicit `select()`; `selectLockedShapes` option allows click/brushing | Independently controlled via a visibility system |
| Affinity | `⌃⌘L` Unlock All; lock per-layer | `⌃⌘H` Show All |
| Canva | `⌥⇧L` lock element | Not documented |

[INFERRED] "Locked is not selectable but is visible and listable" is a defensible split: locking protects
from accidental edits without making the object invisible to the user. Spool should decide whether locked
objects can be selected at all, and document the choice.

## View-only selection

Figma documents a distinct view-only presentation: solid purple selection box, dashed purple parent box,
layer name and parent component name shown in the Properties panel, with the parent component clickable.
[DOCUMENTED]

[INFERRED] This is a **third** selection concept beyond "selected" and "editing": *inspected*. Worth
noting because a collaboration feature will need it.

## Mixed / common values

- Figma: "When you select more than one layer, you can access **Selection Colors** […] update individual
  Fills, Styles and Strokes in a mixed selection." [DOCUMENTED]
- Spool's prototype already has this pattern: `common_fill`, `common_stroke`, `common_stroke_width` in
  `shell.rs` return `Option<Option<T>>` — outer `None` = mixed, inner `None` = unset. [OBSERVED in
  source]

[INFERRED] The `Option<Option<T>>` shape is a reasonable idiom: *mixed / unset / set*. Any inspector that
reports on a multi-selection needs three states, not two.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- `Selection { selected: Vec<ObjectId> }` — a flat id vector. **No depth, no scope, no traversal.**
- `click(target, additive)` — additive toggles, non-additive replaces; `None` target clears.
- `replace(ids)` and `add_all(ids)` — used by marquee and `⌘A`.
- `begin_left_interaction` computes `ClickSelection::{SelectOnly, Toggle}` from `⇧`.
- Marquee uses `initial_selection` + `additive` flag for accumulation. [OBSERVED]
- No `Cmd/Ctrl`-click deep select (no hierarchy to descend into).
- Selection changes do **not** currently enter history. [OBSERVED — `History::record` only takes
  `DocumentCommand`s]

## Candidate architectural implication

**Evidence:**

1. All four products maintain an editor-level selection that is separate from the document. [DOCUMENTED
   everywhere]
2. Figma and tldraw both maintain an additional *container scope* (`nestingDepth` / `focusedGroupId`).
   [DOCUMENTED]
3. tldraw records selection in history with a distinct history mode. [DOCUMENTED]
4. Affinity makes "editing" a tool; Figma makes it a mode; tldraw makes it separate state. [DOCUMENTED]

**Why it matters:** Selection is the operand of nearly every operation. If selection semantics change
after the operation API is designed, every operation must change with it.

**Potential Spool approaches:**

- **A. Flat set only.** Simplest. Cannot express "selecting inside this frame". Adequate while the
  document is flat.
- **B. Set + explicit scope container.** `Selection { ids, scope_container: Option<ObjectId> }`. Matches
  tldraw. Requires a hierarchy.
- **C. Set + implicit depth (Figma-style).** `Selection { ids, depth: usize }`. Compact but hides the
  container.
- **D. A separate `EditScope` state machine value** distinct from selection, covering "navigating into a
  container", "editing a text run", "editing a path".

**Tradeoffs:** B and D need hierarchy to be meaningful. A cannot later support nested selection without a
migration of every operation that reads selection. B is the most explicit but adds a field that is
redundant when depth is always 0.

**Decision: TBD — requires architecture review.** This is one of the highest-leverage decisions in the
research because it gates hierarchy, and hierarchy gates layout, components, and the operation API.

## Open questions

1. Should locked objects be selectable? [Figma: yes via menu; tldraw: configurable]
2. Should selection be undoable? [tldraw: yes, with preserve-redo; others: unknown]
3. What is the canonical "select behind" affordance?
4. Should Spool's marquee have a partial-intersection mode? [Affinity: yes, as a distinct binding]
5. Should selection persist per page, or reset when the page changes?

## Sources

- Figma: "Select layers and objects"; "Use Figma products with a keyboard"; "Select keyboard layout";
  "Adjust alignment, rotation, position, and dimensions"; "Guide to auto layout"
  (https://help.figma.com/hc/en-us/articles/360040449873, /360040328653, /5665442977431,
  /360039956914, /360040451373)
- tldraw: `sdk-features/selection.mdx`, `sdk-features/history.mdx`, `sdk-features/culling.mdx`,
  `sdk-features/instance-state.mdx`, `sdk-features/store.mdx`
  (https://github.com/tldraw/tldraw/tree/main/apps/docs/content)
- Affinity: "About Studios"; "Keyboard shortcuts for general editing"; "Node Tool"
  (https://www.affinity.studio/help/workspace-about-studios/, /workspace-shortcuts-editing/,
  /tools-tools-node/)
- Canva: "Canva keyboard shortcuts"; "Add, duplicate, and delete elements"
  (https://www.canva.com/help/canva-keyboard-shortcuts/, /add-elements/)
- Spool prototype: `app/src/canvas.rs` (`Selection`, `begin_left_interaction`, `MarqueeGesture`),
  `app/src/shell.rs` (`common_fill`, `common_stroke`, `common_stroke_width`)
