# Interaction: Drawing

Drawing covers freehand input, shape construction, and the tools that produce *paths* rather than
*rectangles*. This is the domain where the products separate most sharply, and where Spool's stated
ambition ("more than UI mockups") is most at risk.

## Tool inventory

| Tool | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Pen (Bézier) | Yes | Not documented (no built-in pen in the SDK core) | Yes | No |
| Pencil (freehand → vector) | Yes | **Yes (draw tool)** | Yes | No |
| Brush (pressure-sensitive raster) | No | Scribble has pressure/dynamics | **Yes** | No |
| Line | Yes | Via arrow + geo shapes | Yes | **Yes (`L`)** |
| Elbowed line | No | No | No | **Yes (`⌘\`)** |
| Arrow / connector | **Yes (cannot be created by keyboard)** | **Yes (ArrowShapeTool)** | Yes | No |
| Basic shapes | Rectangle, ellipse, line, polygon, star | `geo` shape with a `geo` prop | Full geometric set | `R` rectangle, `C` circle |
| Corner tool | No | No | **Yes** | No |
| Curve tool | No | No | **Yes** | No |
| Knife | No | No | **Yes** | No |
| Shape builder | No | No | **Yes** | No |
| Vector flood fill | No | No | **Yes** | No |
| Eraser | No | **Yes (a tool)** | Yes (pixel) | No |

[DOCUMENTED — tldraw docs index lists `draw-shape.mdx`, `scribble.mdx`, `eraser.mdx`,
`arrow-shape.mdx`, `geo-shape.mdx`; Affinity help index lists the tool pages; Canva shortcuts list
`L`, `⌘\`, `R`, `C`; Figma documents "lines, vector paths, connectors, and tables" as keyboard-excluded]

## The keyboard-creation boundary

Figma makes an explicit, revealing admission:

> "Objects that require multiple clicks cannot be inserted using keyboard controls. This includes
> **lines, vector paths, connectors, and tables.**"
> [DOCUMENTED — "Use Figma products with a keyboard"]

[INFERRED] Figma has drawn a hard line: anything requiring a click-drag-click sequence is out of scope for
its keyboard accessibility commitment. Spool, if it wants a Figma-parity keyboard story, must either
accept the same limit or solve keyboard-driven path creation — which is a hard problem and, per the Figma
team's own judgement, an expensive one.

tldraw's position is different and worth noting: **it has no built-in pen or polyline tool at all.**
`@tldraw/editor` ships zero tools; `tldraw` adds Select, Hand, Draw (freehand), and Arrow. Arrow is a
multi-point polyline with bindings. Everything else must be a custom shape.

[INFERRED] tldraw's position is a strong statement: **a general-purpose extensible editor can defer paths
entirely and remain genuinely general**, because custom shape types are a first-class extension point.
Spool could take the same path — build a strong geometry/transform core and treat path authoring as an
add-on.

## Freehand input representation

### tldraw

- The **draw tool** is `DrawShapeTool`. [DOCUMENTED]
- The **scribble** shape is a documented first-class shape type with its own module
  (`sdk-features/scribble.mdx`). [DOCUMENTED]
- The draw tool uses **`useCoalescedEvents: true`** — "Whether to receive the browser's coalesced pointer
  move events for higher-fidelity input, as the draw tool does (default `false`; always off on iOS)."
  [DOCUMENTED — `sdk-features/tools.mdx`]

[INFERRED] Two important facts here:
1. **Coalesced pointer events exist and are used for drawing.** Browsers deliver multiple position samples
   per frame for high-frequency input; using them produces a much denser stroke at the same cost. This is
   a real, transferable technique for native input stacks too (pointer prediction / coalesced deltas).
2. **iOS disables them.** So a drawing implementation must work with raw move events too — a stroke model
   that requires coalesced events is not portable.

### Affinity

Brush and Pencil are separate tools with different targets (pixel vs. vector). Pencil in Vector Studio
produces vector strokes; Brush in Pixel Studio produces raster strokes. The Vector Studio shortcuts page
documents Pencil's own shortcut. [DOCUMENTED at module level]

### Figma

Pencil produces vector strokes with a smoothing/fitting step. Figma does not document the fitting
algorithm. **Unknown.**

## Freehand is persistent document data, not transient input

| Product | Evidence | Inference |
|---|---|---|
| tldraw | `scribble` is a shape type with its own schema, in the store, syncable, undoable | Freehand points are an ordered array of (position, pressure, time) on the shape record |
| Affinity | Brush strokes are persistent layers; "Convert to Curves" converts pixel data to vector | Both raster and vector stroke forms are first-class |
| Figma | Freehand strokes are stroke-editable and node-editable | Stored as vector geometry after fitting |
| Canva | No drawing tools | n/a |

[INFERRED] **Freehand must be persistent**, which means:
- it participates in undo/redo,
- it is serialised and synced,
- it is exportable,
- it has a point-sampling model (time, pressure, or both),
- and it needs a **smoothing/fitting** step to become usable geometry.

A stroke record is therefore a *different shape of data* from a rectangle, and pretending otherwise is a
common source of late rework. Spool has no stroke representation today.

## The draw-tool state lifecycle

tldraw's documented creation pattern is the clearest available template:

```typescript
export class DrawingPointing extends StateNode {
    static override id = 'pointing'

    override onPointerMove(info) {
        if (this.editor.inputs.getIsDragging()) {
            this.parent.transition('drawing', info)
        }
    }

    override onPointerUp(info) {
        this.parent.transition('idle', info)
    }
}
```
[DOCUMENTED — `sdk-features/tools.mdx`]

[INFERRED] **A click with the draw tool creates nothing**: `pointing` round-trips to `idle` on pointer-up
without creating a shape. This differs from Figma, where a click with a shape tool creates a
default-sized shape. Both are defensible; tldraw's is arguably more correct for a *freehand* tool (a
click is not a stroke), while Figma's is more correct for a *shape* tool (a click is a
default rectangle).

The critical structural point is the **`pointing` state exists to disambiguate click vs. drag**. This is
the same `PotentialX`/`X` pattern Spool's prototype already implements as
`Interaction::PotentialCreate` / `Interaction::Creating`.

## Shape construction vs. shape editing

| Concern | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Create a shape | Drag or click | Drag (no click-create) | Drag | Drag or click |
| Edit existing path points | Double-click into vector edit mode | `handles` on shapes + tool states | **Node Tool** (`A`) with modes/transforms | Not available |
| Node types | Smooth/corner (implicit) | Per-handle `type` on `TLHandle` | **Sharp / Smooth / Smart** — explicit, per node, toolbar-selected | n/a |
| Multi-node selection | Yes | Yes | Yes; **Transform Mode** creates a bbox over selected nodes | n/a |
| Node-level transform origin | No | No | **"Enable Transform Origin"** — a movable origin the node bbox rotates about | n/a |
| Auto-fit curve node | No | No | **"Smart" node** — "converts the selected node into a smart node (for a **best-fitting curve**)" | n/a |
| Compound path | Boolean operations | No documented equivalent | Yes (Boolean: Add/Subtract/Intersect/Exclude; Join Curves) | No |
| Convert to curves | No | No | **`⌘⏎` Convert to Curves** | No |
| Merge / flatten | Flatten command | No | **`⌘E` Merge Down, `⇧⌘E` Merge Selected, `⌥⇧⌘E` Merge Visible** | Tidy up (aligns, not flattens) |

[DOCUMENTED]

### Affinity's node model in detail

From the Node Tool help page [Fully DOCUMENTED]:

**Node type conversion (context toolbar):**
- **Sharp** — "converts the selected node to a sharp node (for cusped corners)"
- **Smooth** — "converts the selected node to a smooth node (for a Bézier curve)"
- **Smart** — "converts the selected node into a smart node (for a best-fitting curve)"

**Path surgery:**
- **Split Curve** — "adds a new node at the midpoint of one of the curve segments of the currently
  selected node, depending on the current curve orientation (indicated by a red-line indicator)"
- **Reverse Curve** — changes which segment is indicated
- **Break Curve** — "opens the shape at the selected node"
- **Close Curve** — "joins the start and end nodes to create an enclosed shape"
- **Smooth Curve** — "modifies a line or shape, by adding and removing nodes, to create a rounder and
  softer curve effect"
- **Join Curves** — "connects two separate curves together to make one curve"

**Node-space transforms:**
- **Transform Mode** — "creates a bounding box around the selected nodes, allowing them to be transformed
  as a group"
- **Enable Transform Origin** — "displays a movable transform origin about which the selection box can be
  rotated"
- **Selection Box From Curves** — "the selection box encompasses and includes all curves that extend
  outside the array of currently selected nodes"
- **Hide Selection while Dragging** — "The selected behavior **persists across all objects** unless it is
  manually switched."

**Curve orientation:**
- **Show curve orientation** — "displays the segment leading up to the end node in red to help visualize
  the direction of the closed shape's outline (**used for winding fill mode**)"

[INFERRED] Four architectural requirements are implied, all absent from Spool's prototype:
1. **Per-node types** with at least {corner, smooth, auto}. "Smart" implies the editor retains *other
   nodes' geometry* and refits — that is a solver, not a flag.
2. **A sub-selection of nodes within an object**, with its own bounding box — a second level of
   selection below the object level.
3. **An explicit winding/fill rule** (non-zero vs. even-odd), surfaced in the UI as "winding fill mode".
   Every serious vector editor needs this.
4. **Path surgery as discrete, named operations** (split, break, close, join, reverse, smooth), each of
   which is a distinct, undoable, AI-addressable transformation.

### tldraw's handle model

`TLHandle` carries `x`, `y`, `type`, and `snapType`. [DOCUMENTED — `sdk-features/handles.mdx`]

[INFERRED] Handles are **separate records from the shape's geometry**, which is what makes:
- snapping a handle (with self-snap opt-out) possible without a feedback loop,
- per-handle `snapType` meaningful,
- and handle-level operations (rotating, scaling a subset) expressible.

This differs from Affinity, where handles appear to be intrinsic to the path. The record-based approach
is more flexible and more snapshot-heavy; the intrinsic approach is simpler.

## Snapping during drawing

| Product | Documented |
|---|---|
| Affinity | "Snap to object geometry — snapping can target object **vertices and intersections** when **drawing new objects**, resizing objects, or **moving nodes**, rather than only bounding boxes or shape key points." **Vector warp nodes now obey snapping.** Also: construction snapping (angular, adjacent-segment-relative). |
| tldraw | `HandleSnaps` connects handles to outlines and key points. `getHandleSnapGeometry` with `outline`, `points`, `getSelfSnapOutline()`, `getSelfSnapPoints()`. `snapReferenceHandleId` controls which handle Shift-angle snapping measures from. |
| Figma | "Snap to geometry: **Used only in vector edit mode**." Pixel grid snapping applies "when created, moved or modified". |
| Canva | No drawing |

[INFERRED] Affinity is the only product that documents snapping during **drawing**, and it does so by
specifying that some snap options are **action-scoped**: "Some object snapping options apply only during
specific actions, such as drawing new objects, resizing objects, or moving nodes with the Node Tool.
**They do not affect moving objects with the Move Tool.**"

This is a direct, documented statement that a single snap configuration cannot serve all four actions. It
implies a snap engine with **per-action option profiles**.

## Eraser semantics

| Product | Model |
|---|---|
| tldraw | **A tool** (`sdk-features/eraser.mdx`). Switching to the eraser is a tool change, i.e. a state-chart transition. |
| Affinity | Pixel-oriented; a Pixel Studio eraser. |
| Figma | No eraser documented for vector content. |
| Canva | None. |

[INFERRED] tldraw's "eraser is a tool" choice means erasing goes through the same state machine as drawing
and therefore participates in the same history marks. An "eraser modifier" (holding a key while drawing)
would need a different design.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- `Tool { Select, Frame, Rectangle, Ellipse, **Pen**, Text, **Comment** }`.
- **`Tool::creates_object()` returns `None` for `Pen` and `Comment`** — they are declared but **not
  implemented**. Pen is a placeholder.
- `creation_geometry(start, current)` builds an axis-aligned `WorldRect` with a `MIN_OBJECT_SIZE` floor of
  20. **All four creating tools (Frame, Rectangle, Ellipse, Text) share one axis-aligned creation path.**
- No click-to-place default-size path.
- `ObjectType { Frame, Rectangle, Ellipse, Text }` — **no path, no line, no arrow, no polygon, no
  compound shape.**
- `Interaction::{PotentialCreate, Creating}` with `CreationPreview { object_type, geometry }`.
- Escape correctly cancels creation with no history entry (test:
  `escape_cancels_creation_without_history_or_selection_changes`). [OBSERVED]
- No pressure input, no coalesced-event equivalent, no freehand data, no node model.

## What is missing, in dependency order

1. **Path data type.** A `Path { subpaths: Vec<Subpath { nodes, closed }> }` with per-node
   `{ position, in_handle, out_handle, kind }`. This is the single largest missing primitive.
2. **Path geometry**: bounds (AABB and tight), hit testing, point-at-distance, flattening, offsetting.
3. **Path↔shape conversion**: rectangle/ellipse → path, and back. ("Convert to Curves", `geo` shapes with a
   `geo` discriminator — tldraw's design, where *all* simple shapes are one type with a variant prop.)
4. **Node-level editing** with three node kinds and the surgery operations.
5. **Fill rule** (non-zero / even-odd).
6. **Freehand capture**: coalesced/high-frequency input, stroke storage, smoothing/fitting.
7. **Boolean operations**: union/subtract/intersect/exclude/divide. This requires a robust polygon
   clipping implementation — a substantial and notoriously bug-prone component.
8. **Arrows/connectors** as a first-class object with bindings.
9. **Snapping during draw and node edit.**

## Candidate architectural implication

**Evidence:**

1. Freehand is **persistent, serialised, undoable document data** with a point-sampling model and a
   fitting step. [tldraw, Affinity DOCUMENTED]
2. High-frequency input benefits from coalesced pointer samples; the implementation must also work without
   them. [tldraw DOCUMENTED, including the iOS caveat]
3. Professional node editing requires **per-node types including an auto-fit ("Smart") node**, which
   implies a curve-fitting solver. [Affinity DOCUMENTED]
4. Professional node editing requires **node-level sub-selection with its own transform box and origin**.
   [Affinity DOCUMENTED]
5. Every serious vector editor needs an explicit **fill rule**. [Affinity DOCUMENTED as "winding fill
   mode"]
6. Path surgery (split / break / close / join / reverse / smooth) is a set of **discrete named
   operations**, not ad-hoc editing. [Affinity DOCUMENTED]
7. Handles-as-records enable self-snap opt-out and per-handle snap behaviour; intrinsic handles are
   simpler. [tldraw DOCUMENTED]
8. A single snap configuration cannot serve move, resize, draw, and node-edit; options must be
   action-scoped. [Affinity DOCUMENTED]
9. **tldraw ships a genuinely general editor with no path-authoring tool**, relying on custom shape types.
   [DOCUMENTED]
10. tldraw models all simple shapes as **one shape type with a `geo` discriminator**, not one type per
    shape. [DOCUMENTED — `sdk-features/shapes.mdx`, record example `type: 'geo', props: { geo: 'rectangle' }`]

**Why it matters:** Paths are the hardest data structure in a design document. Once paths exist, they
affect hit testing, bounds, snapping, text-on-path, export, hit-test performance, and the operation API
all at once. Getting the representation wrong is expensive.

**Potential Spool approaches:**

- **A. No paths. Rectangles and ellipses only; treat Figma-style vectors as out of scope for now.**
  Honest, shippable, but caps Spool at "UI mockup tool" indefinitely.
- **B. Path as a first-class object type** with node/handle data, node editing, and path surgery, but
  **no boolean operations** initially. A large but tractable step; gets logos and icons working.
- **C. B + booleans + compound paths.** The full professional vector model. Requires polygon clipping.
- **D. tldraw's model**: one `Shape` type with a `kind` discriminator and a `props` map, where all simple
  shapes are `kind: 'geo'`. Reduces the type explosion and makes custom shapes cheap.
- **E. Extensibility-first**: build the geometry/transform core and a `ShapeKind` trait, then ship path
  authoring as a pluggable shape type later (tldraw's path).

**Tradeoffs:**

| | Effort | Ceiling |
|---|---|---|
| A | Trivial | UI mockups only |
| B | Large | Logos, icons, illustrations; no boolean compositing |
| C | Very large (clipping is bug-prone) | Professional vector |
| D | Small | Structural; reduces long-term type count |
| E | Medium | Preserves optionality; risks never shipping B |

[INFERRED] **D is nearly free and strictly reduces future cost** — one `Shape` type with a discriminator
instead of N types with a match statement in every visitor. It is the one recommendation here that is
clearly positive-EV regardless of which path is chosen.

[INFERRED] **E is the honest answer for Spool's stated ambition.** The brief says Spool should eventually
be capable of more than UI mockups, but also says not to design prematurely. Option D + deferring the
path-type decision preserves both.

**Decision: TBD — requires architecture review.**

## Open questions

1. Does Spool need paths at all for its first general-purpose milestone? If not, is the object model
   extensible enough to add them later without a migration?
2. One shape type with a discriminator (tldraw) or one type per shape (Figma)?
3. Are nodes stored as handles-as-records or intrinsic to a path?
4. Is `Smart` (auto-fit) node editing required, and does it imply a curve solver?
5. Are boolean operations in scope ever? They are the hardest item in this document.
6. Does freehand store time+pressure, and is the fitting algorithm a fixed rule or user-adjustable?
7. Is the eraser a tool or a modifier?

## Sources

- Figma: "Use Figma products with a keyboard" (/360040328653); "Select keyboard layout" (/5665442977431);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914); "Guide to components in Figma"
  (/360038662654)
- tldraw: `sdk-features/handles.mdx` ⭐, `sdk-features/draw-shape.mdx`, `sdk-features/scribble.mdx`,
  `sdk-features/eraser.mdx`, `sdk-features/arrow-shape.mdx`, `sdk-features/geo-shape.mdx`,
  `sdk-features/shapes.mdx`, `sdk-features/snapping.mdx`, `sdk-features/tools.mdx`,
  `sdk-features/geometry.mdx`, `sdk-features/default-shapes.mdx`, `sdk-features/shape-clipping.mdx`
- Affinity: "Node Tool" (/tools-tools-node/) ⭐; "Snapping" (/design-aids-snapping/); "Keyboard shortcuts
  for general editing" (/workspace-shortcuts-editing/); "About Studios" (/workspace-about-studios/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`Tool`, `Tool::creates_object`, `ObjectType`, `creation_geometry`,
  `Interaction::{PotentialCreate, Creating}`)
