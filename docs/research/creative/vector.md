# Creative: Vector Editing

See `interaction/drawing.md` for tool lifecycle and interaction. This document is about the **data model**
required for serious vector editing.

## Why this matters for Spool

Spool's stated ambition is "eventually capable of more than UI mockups". Affinity is the reference for
that, and Affinity's vector model implies four requirements that Spool's prototype lacks entirely:

1. Per-node types, including an **auto-fit** kind.
2. A **fill rule** (winding mode).
3. **Node-level sub-selection** with its own transform box.
4. Discrete, named **path surgery** operations.

Plus one capability that is easy to underestimate:

5. **Boolean operations** (union, subtract, intersect, exclude) require robust polygon clipping.

## Affinity's node model — the reference

[Fully DOCUMENTED — "Node Tool"]

### Node types

| Type | Documented behaviour |
|---|---|
| **Sharp** | "converts the selected node to a sharp node (for cusped corners)" |
| **Smooth** | "converts the selected node to a smooth node (for a Bézier curve)" |
| **Smart** | "converts the selected node into a smart node (for a **best-fitting curve**)" |

[INFERRED] **"Smart" implies a solver, not a flag.** Converting a node to Smart and having it re-fit means
the editor must retain the neighbouring nodes' geometry and recompute handles to minimise curvature
discontinuity. This is the single most algorithmically demanding requirement in professional vector editing,
and it is why most tools don't offer it.

### Path surgery

| Operation | Documented behaviour |
|---|---|
| **Split Curve** | "adds a new node at the midpoint of one of the curve segments of the currently selected node, depending on the current curve orientation (indicated by a red-line indicator)" |
| **Reverse Curve** | "changes this curve orientation" |
| **Break Curve** | "opens the shape at the selected node" |
| **Close Curve** | "joins the start and end nodes to create an enclosed shape" |
| **Smooth Curve** | "modifies a line or shape, **by adding and removing nodes**, to create a rounder and softer curve effect" |
| **Join Curves** | "connects two separate curves together to make one curve" |

[INFERRED] Six distinct operations, each a separate undoable transformation. Note that **Split Curve is
orientation-dependent** — the "midpoint of one of the curve segments" depends on whether the subpath is
closed and on winding direction. This is a real semantic requirement: a path must record its direction,
not just its node set.

### Node-space transforms

| Feature | Documented behaviour |
|---|---|
| **Transform Mode** | "creates a bounding box around the selected nodes, allowing them to be transformed as a group" |
| **Enable Transform Origin** | "displays a movable transform origin about which the selection box can be rotated" |
| **Selection Box From Curves** | "the selection box encompasses and includes all curves that extend outside the array of currently selected nodes" |
| **Hide Selection while Dragging** | "the selection box is temporarily hidden when transforming […] The selected behavior persists across all objects unless it is manually switched" |

[INFERRED] "Transform Mode" is a **second selection level**: a set of nodes *within* one object, with its
own bounding box, its own transform origin, and its own snapping. Spool's selection model would need to
extend to this.

### Curve orientation and winding

| Feature | Documented behaviour |
|---|---|
| **Show curve orientation** | "displays the segment leading up to the end node in red to help visualize the direction of the closed shape's outline (**used for winding fill mode**); the drawing direction is away from the red-line indicator" |

[INFERRED] Affinity has at least **two fill modes** selected by a "winding fill mode" setting — the
non-zero rule and the even-odd rule. The UI exposes the *reason* for winding (direction) as an aid, which
is a well-designed teaching affordance.

### Node snapping within the tool

Documented as independent of the global snapping options:

- **Align to nodes of selected curves** — "aligns any moving node you drag to any other node on the same or
  a different curve"
- **Snap to geometry of selected curves** — "snap moving node to the same or different curve's path or node"
- **Snap all selected nodes when dragging** — "snap multiple selected nodes […] to a 'target' node on any
  selected curves"
- **Align handle positions using snapping options** — control handles obey the *global* snapping criteria
  (grid, guide, object geometry, key points, spread, margin)
- **Perform construction snapping** — control handle constraints:
  - inline with adjacent node
  - to 90° from inline
  - to reflected angle with adjacent control handle
  - parallel to adjacent control handle
  - 90° to parallel control handle
  - to logical triangle

[INFERRED] **Construction snapping is a different algorithm from object proximity snapping.** It operates
on angular relationships between the moving handle and *adjacent* segments/handles. It requires knowing the
subpath topology and the direction of travel. It is entirely orthogonal to the snap-manager design in
`interaction/snapping.md`.

## tldraw's handle model

[DOCUMENTED — `sdk-features/handles.mdx`]

`TLHandle` is a **record separate from the shape's geometry**, carrying `x`, `y`, `type`, and `snapType`.

[INFERRED] Two architectural consequences:

1. **Handles can be snap targets**, which is what makes arrow endpoints snap to shape outlines. And
   self-snapping is opt-in via `getSelfSnapOutline()` / `getSelfSnapPoints()` precisely because a handle
   that is part of the geometry it snaps to creates a feedback loop.
2. **Handles are mutable independently of the geometry cache**, so a handle drag is a narrow invalidation.

Affinity appears to treat handles as intrinsic to the path. Both work; the record approach is more flexible
and more expensive to serialise.

## Figma's vector model

[DOCUMENTED — very little at the data level]

- Vector shapes are created with the Pen tool and editable in **vector edit mode** (entered by
  double-click).
- "Snap to geometry: **Used only in vector edit mode**" — confirms a distinct mode.
- "Snap to pixel grid" applies "when created, moved or modified" in vector mode.
- `⌘/Ctrl`-dragging disables snap-to-geometry.
- Boolean operations (union/subtract/intersect/exclude) exist as a command over a selection.
- "Convert to Curves" is an **Affinity** command; Figma's equivalent is implicit.
- Figma's internal vector representation (node kinds, handles, fill rule) is **not publicly documented.**

[INFERRED] Figma's vector feature set is sufficient for UI illustration but is not the node-editing depth
Affinity offers. The boolean operations are the notable addition.

## Canva: no vector editing

[DOCUMENTED absence] Canva's creation tools are text, rectangle, line, elbowed line, circle, sticky note,
emoji. Vector editing is not offered. **Shape Generator** (AI custom shapes) is the substitute.
[DOCUMENTED]

## The data model a serious vector editor needs

Derived from the evidence above. [INFERRED throughout]

```
Path {
    subpaths: [ Subpath ]
}

Subpath {
    closed: bool
    direction_reversed: bool          // or implied by node order
    fill_rule: NonZero | EvenOdd       // per shape, not per subpath
    nodes: [ Node ]
}

Node {
    kind: Corner | Smooth | Auto       // "Sharp" / "Smooth" / "Smart"
    position: Vec2                     // in shape-local space
    in_handle: Option<Vec2>            // relative to position
    out_handle: Option<Vec2>
}

Shape {
    geometry: Geometry                 // enum { Rect, Ellipse, Path, Polygon, Star, … }
    fill: Paint
    stroke: Stroke { paint, width, cap, join, miter, dash }
    transform: Transform               // position, size, rotation, flip
}
```

Required supporting capabilities:

| Capability | Why |
|---|---|
| **Path bounds** (tight, not AABB-of-controls) | hit testing, snapping, culling |
| **Flattening** (curve → polyline at a given tolerance) | hit testing, boolean ops, export, LOD |
| **Point-at-arc-length** | text-on-path, trimming |
| **Offsetting** (stroke → outline, and shape → expanded outline) | stroke-to-path, boolean prep |
| **Curve fitting** (points → smoothed path) | pencil tool, "Smart" node conversion |
| **Self-intersection detection** | fill rules, boolean robustness |
| **Winding / fill-rule evaluation** | rendering and boolean correctness |
| **Boolean ops** (union, subtract, intersect, exclude, divide) | compositing shapes |

[INFERRED] **Boolean operations are the hardest item in this document.** Robust polygon clipping requires
handling: self-intersections, degenerate edges, coincident segments, holes, and numerical robustness.
Every serious vector library either implements a well-tested clipper (e.g. a Greiner–Hormann or
Martinez–Rueda variant) or delegates to a system library. This is a multi-week component with a long tail
of edge cases, and Figma, Affinity, and Illustrator all ship it.

[INFERRED] **Fill rule is not optional.** Without it, a self-intersecting star fills incorrectly, and a
compound path with a hole fills solid.

## Strokes

| Aspect | Affinity | Figma |
|---|---|---|
| Stroke paint | Yes (separate from fill; `X` swaps) | Yes (separate) |
| Stroke width | Yes; `[`/`]` adjust by percentage, `⇧[`/`⇧]` by absolute value | Yes; number variables can drive "stroke weight: all, top, bottom, left, right" |
| Alignment | Yes ("Edit stroke settings — change stroke style, width, and **alignment**") | Per-side weights documented via variables |
| Cap / join / miter | Standard; not documented in pages read | Not documented in pages read |
| **Contour** | The research brief lists "contour"; **not found in the Affinity help index** | Not documented |
| **Stroke → outline** | Implied by boolean/convert operations; not documented | "Outline stroke" appears in the Figma tool set |
| **Mirror shearing** | `⌃`-drag with a selection box | Not documented |

[INFERRED] **Per-side stroke weights** (Figma variable target list includes "all, top, bottom, left, right")
imply strokes are four independent weight values that default to being equal. That is more expressive than
a single width and cheap to store.

## Compound paths and masks

| Concept | Product |
|---|---|
| Compound path (multiple contours, one object) | Affinity (Boolean ops, Join Curves) |
| Boolean union/subtract/intersect/exclude | Affinity, Figma |
| **Clipping mask as a separate object** | Affinity (`⇧C`); SVG `<clipPath>` |
| Clipping as a container property | Figma (Frame Clip content), tldraw |
| Boolean groups | Figma (a frame holding operands + a boolean operation property) — internal form **Unknown** |
| Shape builder (add/erase across overlapping shapes) | Affinity |
| Vector flood fill (fill areas created by overlapping shapes and open curves) | Affinity |

[DOCUMENTED]

[INFERRED] **Non-destructive booleans vs. destructive booleans** is a real fork:
- **Destructive** (Figma): apply the operation, operands are consumed, result is one object. Simple undo
  (one entry), simple document, loses the ability to change the operation later.
- **Non-destructive** (Affinity's clipping mask; Figma's boolean group): keep operands and the operation,
  re-evaluate. More flexible, but requires re-evaluating geometry when any operand changes — a cache
  invalidation problem.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **No path data at all.** `ObjectType { Frame, Rectangle, Ellipse, Text }`.
- `Geometry { position, size }` — a box. Every object is a rectangle.
- `Tool::Pen` is declared; `Tool::creates_object()` returns `None` for it. **Declared but unimplemented.**
- Rendering (`render_object`) draws rects and ellipses only.
- Hit testing (`DesignObject::contains`) is **AABB containment only** — no per-pixel or per-path test.
- `ObjectStyle { fill, stroke }`; `Stroke { color, width }` — **no cap, join, dash, alignment, or
  per-side width.**

[INFERRED] Everything in this document is ahead of the prototype. That is appropriate — the prototype is
explicitly a prototype — but it means the *object model* decision (§ `document/objects.md`) determines
whether paths can be added later without a migration.

## Candidate architectural implication

**Evidence:**

1. Node editing needs **per-node types including an auto-fit kind**, which implies a curve solver.
   [Affinity DOCUMENTED]
2. Node editing needs a **sub-selection level** with its own transform box and origin.
   [Affinity DOCUMENTED]
3. Node editing needs **orientation/winding** recorded and surfaced. [Affinity DOCUMENTED]
4. Path surgery is a set of **six discrete named operations**, and Split Curve depends on orientation.
   [Affinity DOCUMENTED]
5. Construction snapping is **orthogonal** to object proximity snapping. [Affinity DOCUMENTED]
6. Handles-as-records (tldraw) enable handle snapping and self-snap opt-out; intrinsic handles are
   simpler. [DOCUMENTED + INFERRED]
7. **Fill rule (winding mode) is a first-class setting** in Affinity. [DOCUMENTED]
8. **Stroke weight can be per-side.** [Figma DOCUMENTED, via variable target list]
9. **Boolean operations require robust polygon clipping** — the hardest component in this document.
   [INFERRED]
10. Destructive vs. non-destructive booleans is a live architectural fork. [INFERRED]

**Why it matters:** Paths are the largest data structure in a design document. Once they exist they affect
hit testing, bounds, snapping, text layout, culling, export, and the operation API simultaneously.

**Potential Spool approaches for the vector stack:**

- **A. No vectors.** Rectangles and ellipses only. Fine for UI mockups; caps Spool's ambition.
- **B. Path as an object type + node editing + surgery**, no booleans, destructive fill rules fixed to
  non-zero. Reaches logos and icons.
- **C. B + booleans (destructive) + even-odd fill rule + per-side stroke.**
- **D. C + non-destructive clipping masks.**
- **E. B + custom-shape extensibility only** (tldraw's route): expose a geometry trait so third parties
  can add vector-backed shape types.

**Tradeoffs:**

| | Effort | Ceiling |
|---|---|---|
| A | Trivial | UI mockups |
| B | Large | Logos, icons, illustration (no compositing) |
| C | Very large (clipping) | Professional vector, flat |
| D | Large + invalidation complexity | Professional vector, editable |
| E | Medium | Depends on third parties |

[INFERRED] **A → E → B is the cheapest route to Spool's stated ambition**: it does not require Spool to
build a vector engine, and it makes Spool a platform on which one can be built — which is precisely the
"VS Code for visual design" thesis. **A → B is the route to owning the capability.**

The choice is a genuine strategic fork, not a technical one, and it belongs in a product decision rather
than an architecture review.

**Decision: TBD — requires a product and architecture review together.**

## Open questions

1. Does Spool own a vector engine, or does it expose a geometry extension point?
2. Path representation: handles intrinsic or as records?
3. Node kinds: {corner, smooth} only, or include auto-fit?
4. Is the fill rule per shape, per subpath, or not stored?
5. Are booleans destructive or non-destructive? (Or both, as in Figma/Affinity?)
6. Is stroke weight single or per-side?
7. Is there a text-on-path capability? (Not documented in any of the four products.)
8. Is there a stroke-to-path (outline stroke) operation? (Figma has "Outline stroke"; Affinity's equivalent
   is undocumented.)

## Sources

- Affinity: "Node Tool" (/tools-tools-node/) ⭐⭐ (node types, surgery, transform mode, selection box
  from curves, snapping, curve orientation/winding); "Snapping" (/design-aids-snapping/) (snap to object
  geometry, construction snapping); "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/)
  (convert to curves, clipping mask, merge, colour); "Getting started" index (/getting-started/) (tool list)
- tldraw: `sdk-features/handles.mdx` ⭐, `sdk-features/geo-shape.mdx`, `sdk-features/draw-shape.mdx`,
  `sdk-features/scribble.mdx`, `sdk-features/shape-clipping.mdx`, `sdk-features/snapping.mdx`,
  `sdk-features/geometry.mdx`, `sdk-features/shapes.mdx`
- Figma: "Adjust alignment, rotation, position, and dimensions" (/360039956914) (snap to geometry, vector
  edit mode); "Overview of variables, collections, and modes" (/14506821864087) (per-side stroke weight)
- Canva: "Canva AI 2.0" (/canva-ai/) (Shape Generator as a vector substitute)
- Spool prototype: `app/src/canvas.rs` (`ObjectType`, `Geometry`, `Tool::Pen`, `DesignObject::contains`,
  `Stroke`, `render_object`)
