# Interaction: Transformation

Cross-product research on move, resize, rotate, scale, and their modifier semantics.

## The transformation vocabulary

Different products use different words for the same concepts. Precise vocabulary matters because it
determines how many distinct primitives Spool's transform model needs.

| Concept | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Move | Position X/Y | `translateShapes` | Move Tool | Position + arrow keys |
| Resize | W/H fields, corner/edge drag | `resizeShapes` + handle | Corner handle drag | `⌘`+Arrow per edge |
| Proportional resize | `Shift`-drag / `Lock aspect ratio` | `Shift` / `isAspectRatioLocked` | `⇧`-drag corner | **`F2` mode toggle** |
| Resize from centre | `Alt`-drag | `Alt` (option) drag | **`⌘`-drag corner** | Not documented |
| Rotate | Rotation field / outside-corner drag | `rotateShapesBy`, `rotateSelection` | `⌃`-drag corner (about opposite corner) | `⌥,` / `⌥.` keys |
| Rotate from origin | **Yes** — `⌥R` reveals a draggable origin | Rotation is about the selection centre | `⌃`-drag corner (opposite corner) | Not documented |
| Scale (scale tool) | **Yes — a separate tool** that scales layers *and* font sizes | Ratio-locked resize | Not documented | Not documented |
| Flip | `⇧H` / `⇧V` | Not documented | Not documented | Not documented |
| Skew / shear | Not documented | Not documented | **`⌃`-drag = mirror shearing** | Not documented |
| Corner radius | Per-corner radius in styles | Not documented | Not documented | Not documented |

## Position semantics

### Figma [DOCUMENTED]

- X/Y are the **top-left corner of the layer's bounds**.
- **Rotated layers keep their original unrotated top-left as the X/Y reference.** "If you rotate a layer
  in the canvas, Figma will base the X and Y co-ordinates of that layer on the original top-left corner
  of the layer's bounds."
- Numeric fields accept inline arithmetic: `- + * / ^ ( )`, either before or after the existing value.

[INFERRED] "X/Y is the unrotated local origin, not the visual top-left" is a deliberate choice: it makes
X/Y a *stable property of the object* rather than a function of its rotation, which keeps numeric editing
predictable. Spool currently stores `position` + `size` with no rotation, so this is a future decision.

### tldraw [DOCUMENTED]

Shapes have `x`, `y`, `rotation`, and `props.w`/`props.h`. Origin semantics are handled by the geometry
layer: `getShapePageBounds()` returns page-space AABB; `getSelectionRotatedPageBounds()` returns the
selection's rotated box.

### Affinity [DOCUMENTED]

Affinity has no documented X/Y convention (professional tools tend to expose transforms in a transform
panel instead). **Unknown.**

## Aspect-ratio lock — three different models

| Model | Product | Behaviour |
|---|---|---|
| **Modifier** | Figma | `⇧`-drag corner preserves ratio; `Ctrl`-drag *temporarily disables* an already-locked ratio; the temporary state is **not persisted** on release |
| **Persistent flag** | Figma | "Lock aspect ratio" is a per-layer boolean in the Layout/Auto layout section |
| **Mode toggle** | Canva | **`F2`** — "Toggle between proportional and single-axis resize" |
| **Parameter** | tldraw | `isAspectRatioLocked` passed to resize; part of the interaction state, not a layer flag |

**Figma's rule has a subtlety worth extracting:**

> "When resizing a layer from the canvas with aspect ratio locked, you can temporarily disable the setting
> by holding ⌃ Control. When resizing a layer from the canvas with aspect ratio unlocked, you can
> temporarily enable the setting by holding ⇧ Shift. **Once you release the key, the aspect ratio will
> no longer be locked on the layer.**"
> [DOCUMENTED]

[INFERRED] The layer property is authoritative; the modifier is a *gesture-local override* that is
discarded on release. This is different from Figma's *ignore constraints* behaviour:

> "Sometimes you may want to resize a frame or layer, without using the constraints that are applied to
> it. You can temporarily ignore any constraints applied to a layer by holding down a modifier key.
> Mac: Hold down Command when you resize / Windows: Hold down Ctrl when you resize."
> [DOCUMENTED]

Same gesture, two different meanings depending on which property you started from. This is a subtle,
real design.

**Figma's aspect-ratio + min/max rule [DOCUMENTED]:** "If a layer with aspect ratio locked has an aspect
ratio of 1:2, setting a minimum height of 100px will automatically set a minimum width of 200px."
[INFERRED] Derived clamps are computed, not stored.

**Figma's instance rule [DOCUMENTED]:** "The aspect ratio setting is not available on child layers of
component instances. The aspect ratio can be adjusted from their respective main components." Another
case where instance children are not freely editable.

## Rotation

### Figma [DOCUMENTED]

- Default origin: the **horizontal and vertical centre of the current selection**.
- Positive = counter-clockwise toward 180°; past 180° it counts down to −180°.
- `⇧` snaps to **15° increments**.
- Effects are **not** rotated.
- Per-object origin via `⌥R` / `Alt R` then dragging the target.
- Dev Mode shows `transform: rotate(-90deg)` for a 90° Figma rotation — **the CSS sign convention is
  inverted**.
- Flip H/V use the CSS `matrix()` property; "Once you have applied a flip transformation to a selection,
  Figma will continue to use the matrix transformation CSS property, even if you later apply rotation."

[INFERRED] The rotate/flip/matrix interaction tells us Figma stores an *affine transform matrix* per
object, with rotation and flip expressed in it, and derives the UI angle from the matrix. Spool currently
has **no transform at all** beyond `position` + `size`, which means rotation, flip, scale-tool, and
skew are all still ahead.

### tldraw [DOCUMENTED]

- Rotation is a shape property.
- `getSelectionRotation()` returns the shared rotation, or `0` when shapes differ.
- Rotate about the selection's rotated bounds centre.
- The **driver** exposes `rotateSelection(radians)` for scripted/AI use.

### Affinity [DOCUMENTED]

- `⇧`-drag a rotation handle → 15° increments.
- `⌃`-drag a corner handle (Mac) / LMB-then-RMB (Win) → **rotate around the opposite corner**.
- `⌃`-drag with a selection box → **mirror shearing**.

[INFERRED] Affinity has *three* distinct corner/handle drag behaviours keyed on modifier: proportional
resize, resize-from-centre, rotate-about-opposite-corner. Figma has *two* (proportional, from-centre).
This is a real interaction expressiveness difference.

### Canva [DOCUMENTED]

- Rotate left/right small: `⌥,` / `⌥.`
- Rotate left/right large: `⌥⇧,` / `⌥⇧.`

[INFERRED] Rotation is keyboard-first, not drag-first — consistent with a template-driven product where
elements usually need small corrections, not free placement.

## Resize-from-centre

| Product | Binding |
|---|---|
| Figma | `Alt`-drag corner |
| tldraw | `Alt` (Option) drag — documented via `isResizingFromCenter` in the snapping API |
| Affinity | **`⌘`-drag corner** |
| Canva | Not documented |

[INFERRED] Figma and tldraw use `Alt`; Affinity uses `⌘`. On macOS `⌥` is `Alt` and `⌘` is Command, so
this is a genuine cross-platform inconsistency between two Mac-native tools. **Spool must pick one.**

## Nudge

| Product | Small | Large | Configurable? |
|---|---|---|---|
| Figma | Arrow (default 1) | `⇧`+Arrow (default 10) | Yes — "small nudge" and "big nudge" values, resolution-independent |
| tldraw | `nudgeShapes` | — | Method-level |
| Affinity | Arrow | `⇧`-Arrow | **Yes — "Nudge distances are customizable", under Settings ▸ Tools** |
| Canva | Arrow | `⇧`+Arrow | Not documented |

[INFERRED] "Configurable nudge distance as a first-class preference" appears in Figma *and* Affinity, both
with the same two-tier model. This is a standardised convention. Note Figma's arrow-key-**pans** when
nothing is selected — a context switch on the same keys.

## Alignment

| Product | Bindings | Scope rule |
|---|---|---|
| Figma | `⌥W/A/S/D`, `⌥V`, `⌥H` | One object → aligns to **its parent**. Multiple → aligns **to each other** (or to their respective parent frames / selected instance layers). `⇧`+click aligns as a group to the parent frame |
| tldraw | Actions `align-left`, `align-center-horizontal`, `align-right`, `distribute-horizontal`, `distribute-vertical` | Actions "do nothing unless shapes are selected and the select tool is active" |
| Affinity | Scriptable; not documented in shortcuts | — |
| Canva | Contextual; `⇧⌘J` justify elements-level | — |

[INFERRED] Figma's "align one object to its parent / many objects to each other" rule is a single
dispatch on selection size, and it is the model almost every tool copies.

## Distribute vs. tidy up — Figma's clearest documentation of a subtle distinction

[Fully DOCUMENTED — "Adjust alignment, rotation, position, and dimensions"]

| | **Distribute** | **Tidy up** |
|---|---|---|
| What | Equalise spacing within the selection's original bounds | Reorganise the selection into rows/columns |
| Outermost objects | Retain position | Repositioned |
| Anchor | The selection's existing bounds | **The top-left corner of the selection** |
| Axes | One at a time | Both, for 2D selections |
| Overlap requirement | None | 1D selections must overlap on the other axis |
| Result | Equal gaps | Equal gaps **and** alignment |

Both report a "space between" value that is the **mode** (most common gap), not the mean. With pixel snap
on, Figma allows 1px of rounding; with it off, distribute can produce fractional gaps like 7.5.

tldraw exposes both as **actions** (`distribute-horizontal`, `distribute-vertical`) and does not document
a separate tidy-up. tldraw's Substack notes indicate tidy-up/smart-selection was removed in favour of
explicit arrange actions. [THIRD-PARTY]

Canva has `⌥⇧T` "Tidy up". [DOCUMENTED] Affinity's snapping can "generate guides or layout helpers from
selection bounds" via scripting. [DOCUMENTED]

[INFERRED] **Distribution should be modelled as at least two distinct operations**, because "equalise
spacing in place" and "reflow into a grid" produce visibly different results and users can tell the
difference.

## Modifier behaviour — consolidated

This is the single most useful table for Spool. Every row is DOCUMENTED from the source named.

| Gesture | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Plain drag selected object | Move | Translate | Move | Move |
| `⇧`-drag (object) | — | Axis-lock / 15° / etc. | **Constrain to H / V / diagonal** | — |
| `⌥/Alt`-drag | Duplicate / resize from centre | Clone / resize from centre / ignore constraints | — | — |
| `⌘/Ctrl`-drag | Temporarily disable snapping; ignore constraints | — | **Resize from centre**; mirror shear with selection box | — |
| `⌃`-drag | — | — | **Rotate about opposite corner**; partial-intersection marquee | — |
| `⇧`-drag corner | Aspect lock | Aspect lock | Aspect lock | — |
| `⇧`-drag rotation | 15° snap | 15° snap | 15° snap | — |
| `F2` | — | — | — | **Toggle proportional resize** |
| Modifier released mid-drag | Behaviour reverts; aspect lock not persisted | **Bails accumulated changes and re-applies in the new mode** | Unknown | Unknown |

[INFERRED] **The tldraw mid-drag modifier change is the most robustly specified behaviour in this
research.** From `sdk-features/history.mdx`:

> "We use bailing while cloning shapes. A user can switch between translating and cloning by pressing or
> releasing the alt (option) key during a drag. When this changes, we bail on the changes since the
> interaction started, then apply the new mode's changes."

This means the gesture is a **pure function of (start state, current pointer, current modifiers)** — there
is no accumulated incremental state to reconcile. It is also why the "bail" primitive exists. Spool's
prototype mutates the document incrementally on every pointer move (`apply_move`), which cannot support
this cleanly without reworking.

## Numeric transformation

- Figma: inline arithmetic in numeric fields (`- + * / ^ ( )`). [DOCUMENTED]
- tldraw: `driver.translateSelection(50, 0)`, `rotateSelection(π/4)`, `resizeSelection({scaleX: 2},
  'bottom_right')` — scripted/agent-addressable. [DOCUMENTED]
- Affinity: scripting can "align, transform, rename, restyle" selected objects. [DOCUMENTED]
- Spool's prototype: `shell.rs` has `position_fields` / `geometry_field_pair` showing X/Y/W/H — display
  only at present. [OBSERVED in source]

[INFERRED] **A numeric transform operation that accepts expressions is a prerequisite for both good UX
and AI addressability.** tldraw's driver form is especially important because it operates on *the current
selection* rather than on ids, which makes it exactly the shape an agent would want.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- `DesignObject { position: Point<f32>, size: Size<f32> }` — **position + size only. No rotation, no
  transform matrix, no scale, no flip, no skew, no corner radius.**
- `ResizeHandle` — the full 8-handle enum (4 corners + 4 edges). Handles are only hit-tested when
  `selection.ids().len() == 1`. [OBSERVED]
- `resized_geometry(start, handle, delta)` — computes the new box; clamps to `MIN_OBJECT_SIZE` (20).
- `apply_resize` recomputes from the *start* geometry each move — i.e. it is already a pure function of
  (start, handle, delta). Good.
- `apply_move` applies a delta to each snapshot incrementally.
- `Interaction::{PotentialResize, Resizing}` with `Interaction::restore()` for Escape. Correctly
  restores start geometry.
- On commit, `geometry_command(document, &snapshots)` produces **one `GeometryChange` per object with
  before/after** — a single history entry per drag. This is correct and matches the industry norm.
- **Missing:** proportional resize, resize-from-centre, rotation, rotation handles, scale tool, flip,
  multi-object resize (`self.selection.ids()[0]` is used, so multi-select resize is not implemented),
  numeric field editing, alignment, distribute.

## Candidate architectural implication

**Evidence:**

1. Every product needs position + size as the base. [DOCUMENTED]
2. Every product with rotation has a **transform composition** that survives rotation, flip, and
   origin changes. Figma's CSS `matrix()` behaviour proves flip and rotation coexist in one matrix.
   [DOCUMENTED]
3. Modifier-with-memory requires either a gesture-local override (Figma) or bail-and-reapply (tldraw).
   [DOCUMENTED]
4. Aspect-ratio lock is both a layer flag and a gesture override in Figma. [DOCUMENTED]
5. Alignment has a selection-size dispatch (one → parent, many → each other). [DOCUMENTED]
6. Distribution has ≥2 distinct operations. [DOCUMENTED]

**Why it matters:** The transform representation is the hardest thing to change later. A
`position + size` model cannot express rotation without either (a) adding a rotation field and re-deriving
bounds everywhere, or (b) switching to a matrix, which changes hit testing, marquee, snapping, text
layout, and the layers panel simultaneously.

**Potential Spool approaches:**

- **A. `position + size + rotation`.** Simple; matches tldraw and Affinity. Rotation about a configurable
  origin requires origin storage. Flip can be expressed as ±1 scale or as a boolean.
- **B. 2D affine matrix + separate logical size.** Matches Figma's proven model. More expensive hit
  testing and text layout.
- **C. `position + size + rotation + transform_origin`.** Explicit; origin is a first-class editable
  property (Figma's `⌥R` target is editable and stored).

**Tradeoffs:** A is cheapest and covers most UI work; C is needed for Figma-parity rotation origin; B is
needed for skew/shear and for faithful CSS export. Skipping B now and adding it later means reworking
export, hit testing, and snapping.

**Decision: TBD — requires architecture review.** Do not add rotation as a bare `f32` without deciding
the transform representation first, because the decision is expensive to reverse.

## Open questions

1. Transform representation: position+size, or matrix?
2. Is rotation origin stored per object, or always the selection centre?
3. Should `Alt`/`⌥` or `⌘`/`Ctrl` be resize-from-centre? (Figma/tldraw vs Affinity disagree.)
4. Is partial multi-object resize required at launch?
5. Are expressions required in numeric fields?
6. Should modifier changes mid-drag re-derive from start (tldraw) or apply incrementally?

## Sources

- Figma: "Adjust alignment, rotation, position, and dimensions" (/360039956914); "Apply constraints to
  define how layers resize" (/360039957734); "Guide to auto layout" (/360040451373); "Adjust text
  dimensions and resizing" (/27378154668951); "Use Figma products with a keyboard" (/360040328653)
- tldraw: `sdk-features/snapping.mdx`, `sdk-features/history.mdx`, `sdk-features/shape-transforms.mdx`,
  `sdk-features/selection.mdx`, `docs/driver.mdx`, `sdk-features/shape-indexing.mdx`
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/); "Node Tool"
  (/tools-tools-node/); "Scripting in Affinity" (/scripting-in-affinity/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`ResizeHandle`, `resized_geometry`, `apply_move`, `apply_resize`,
  `geometry_command`, `Interaction::restore`, `hit_test_resize_handle`)
