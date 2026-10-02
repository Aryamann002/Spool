# Interaction: Snapping

Snapping is where the four products diverge most sharply and where the most transferable specification
exists. tldraw's documentation is the most complete public description of a snap engine in any of the
four products.

## Cross-product summary

| Aspect | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Default state | **On** (preference) | **Off** | **On** (toolbar) | Not documented |
| Enable/disable modifier | `Ctrl/Cmd` temporarily disables | `Ctrl/Cmd` **snap** when off; **disables** when snap mode on (inverted) | **`Alt`/`Option` suspends snapping** | Unknown |
| Persistent toggle | Preferences ▸ Snap to settings | Preferences ▸ `isSnapMode` | Toolbar ▸ Snapping | Unknown |
| Snap-to-pixel-grid | Yes, separate setting; works when grid is invisible | `isGridMode` (separate system) | **Force Pixel Alignment** + **Move by whole pixels** | Unknown |
| Tolerance model | Not documented | **`options.snapThreshold` screen px ÷ zoom** | **Screen tolerance** per preset | Not documented |
| Presets | Individual on/off toggles | Two flags | **7 named presets** | n/a |
| Object targets | Object bounding boxes, centres, outermost points | Snap points (bbox corners + centre, overridable) | Bounding boxes + midpoints + shape key points + object geometry | Guides + rulers |
| Gap snapping | Not documented | **Yes — gap centring + gap duplication** | **Yes** — "Snap to gaps and sizes" | Not documented |
| Geometry (vertex) snapping | Yes, vector mode only | Handle outline snapping | **Yes** — object vertices and intersections | Not documented |
| Guide snapping | Yes | Not documented (no persistent guides system documented) | **Yes** — guides, margins, spreads, baseline grid | **Yes** — `⇧R` rulers+guides, `⌥⌘;` lock |
| Guides are colour-coded | No | No | **Yes** — red H, green V, yellow key point, blue third plane, orange projection grid | Not documented |
| Distance labels on guides | No | No | **Yes** — "labels which report the distance between the snapping objects" | Not documented |
| Per-layer opt-out | No | `canSnap()` | **Yes — "Exclude From Snapping"** with a symbol on the layer row | Not documented |
| Snappable candidate scoping | Not documented | Common ancestor walk, skip selected, skip offscreen | **4 modes** — Candidate List (+Maximum), Immediate layers, Immediate layers+children, All layers | Not documented |
| Snapping during move | Yes | Yes | Yes | Not documented |
| Snapping during resize | Yes | Yes (per-handle rules) | Yes | Not documented |
| Snapping during rotate | Not documented | Handle snapping | 15° increments | Not documented |
| Snapping during draw | Yes | Yes (handle snapping) | **Yes** — vector warp nodes obey snapping | No drawing tools |
| Snapping during node edit | Yes ("Snap to geometry") | Yes | **Yes** — with separate construction-snapping rules | n/a |

## tldraw: the reference snap specification

All DOCUMENTED from `sdk-features/snapping.mdx` unless noted.

### Two independent systems

```
editor.snaps.shapeBounds  → BoundsSnaps   edges, centres, corners, gaps
editor.snaps.handles      → HandleSnaps   handles → outlines and key points
```

A third, separate system: **grid snapping**, controlled by `isGridMode`.

### Zoom-normalised tolerance

> "The snap threshold is `editor.options.snapThreshold` screen pixels (default 8), scaled by the current
> zoom level. At 100% zoom, shapes snap when within 8 pixels. At 200% zoom, the threshold becomes 4 canvas
> units (still 8 screen pixels)."

[INFERRED] The tolerance is expressed in **screen space and divided by zoom**. A world-space tolerance
would make snapping inert when zoomed out and hyperactive when zoomed in. This is the single most
important transferable rule in the snapping domain.

### Candidate filtering

`getSnappableShapes()`:

1. Start from the **selection's common ancestor**.
2. Walk **down** the shape tree.
3. Skip: selected shapes ("you don't snap to what you're dragging"), shapes outside the viewport, and
   shapes whose `ShapeUtil#canSnap()` is false.
4. **Frames are included** as snap targets.
5. For **groups**, recurse into children and snap to them — **but not to the group itself**.

[INFERRED] The asymmetry "recursing into groups but not snapping to the group" makes sense: a group's
bounds are derived and change whenever any child moves, so it is a poor snap target. Frames have real
bounds and are good targets. This is a design rule that follows directly from the hierarchy model.

### Per-shape customisation

Two hooks with distinct meanings:

| Hook | Effect |
|---|---|
| `canSnap()` | Removes the shape as a snap target **entirely** |
| `getBoundsSnapGeometry(shape)` → `{ points: [...] }` | Customises which points the shape offers. Default: bbox corners + centre. `{ points: [] }` drops point snapping **but keeps it as a gap target** |

[INFERRED] The gap-target behaviour of `{ points: [] }` is a deliberate nuance: a shape can be a spacing
reference without being an alignment reference.

### Translation snapping

`snapTranslateShapes({ lockedAxis, initialSelectionPageBounds, initialSelectionSnapPoints, dragDelta })`
returns `{ nudge }` — the adjustment to apply to the delta.

- `lockedAxis: 'x' | 'y' | null` constrains to one axis.
- **When multiple shapes align at the same distance, the system displays all of them.**

[INFERRED] "Compute a nudge, let the caller apply it" separates *detection* from *application*. This
makes the snap engine pure and testable, and it means snapping composes with other constraints
(aspect lock, axis lock, modifiers) by simply adding nudges.

### Resize snapping — per-handle rules

- **Corner handle** → snaps both axes using *that corner*.
- **Edge handle** → snaps only the **perpendicular** axis, using **both corners on that edge**.
- **Aspect ratio locked** → the **dominant snap axis determines both**.

[INFERRED] "Which snap point is relevant depends on which handle moved" is the part most hand-rolled
snap engines get wrong. An edge handle moves only one coordinate, so only the perpendicular-axis
constraint is meaningful — and it is anchored at whichever corner is closer.

### Gap snapping — two distinct behaviours

| Behaviour | Rule |
|---|---|
| **Gap centring** | Centres the selection within a gap *larger than itself*, with equal spacing on both sides |
| **Gap duplication** | Repeats an existing gap on the opposite side — if two shapes have a 100px gap, a third snaps to create another 100px gap |

- Gaps computed **separately per axis**.
- A gap exists when two shapes **do not overlap on one axis** and **do overlap on the perpendicular axis**.
- "When several gaps have matching lengths, the indicators show all of them together."

[INFERRED] Gap duplication is the mechanism behind "distribute by dragging" — it is how a user achieves
equal spacing manually without invoking a distribute command. It is a genuinely useful affordance that
neither Figma nor Affinity documents.

### Handle snapping

`getHandleSnapGeometry(shape)` → `{ outline: Geometry2d | null, points, getSelfSnapOutline(),
getSelfSnapPoints() }`.

> "By default, handles cannot snap to their own shape. **Moving the handle would change the snap target
> and create a feedback loop.**"

Two handle snap types:

| `snapType` | Behaviour |
|---|---|
| `'point'` | Snap to the single nearest location. Checks snap points first, then falls back to the nearest point on any outline |
| `'align'` | Align to nearby snap points on x and y **independently**, with a snap line in each direction |

The older `canSnap` boolean on handles is deprecated; if both are set, `canSnap` wins and point snapping
is used.

[INFERRED] The self-snap opt-in (`getSelfSnapOutline` / `getSelfSnapPoints`) is an explicit escape hatch
for the feedback-loop problem, gated on "remains stable regardless of handle position". This is a
correctly-reasoned exception with a stated precondition.

### Snap indicators

Two kinds, both rendered as SVG overlays:

- **`points`** — lines connecting aligned points. "When several snap points align on the same axis, they
  appear as one **continuous line**."
- **`gaps`** — spacing between shapes with measurement lines at each gap; all matching gaps shown together.

Redundancy elimination: "The manager drops redundant gap indicators: if every gap in one indicator
already appears in a larger indicator for the same direction, only the larger one is kept."

Indicators are cleared automatically when dragging stops.

[INFERRED] Two rendering-quality rules that are invisible until violated:
1. Collinear snap lines must merge into one line, or a 3-object alignment draws 3 overlapping lines.
2. Gap indicators must be deduped by containment, or a 5-object drag draws 8 nested measurement bars.

### Grid snapping

Separate: `isGridMode`. [DOCUMENTED — `sdk-features/instance-state.mdx`]

## Figma's snap settings

[Fully DOCUMENTED — "Adjust alignment, rotation, position, and dimensions"]

Three settings in Preferences:

1. **Snap to geometry** — "Used **only in vector edit mode**. When this setting is on, clicking and
   dragging a vector point will align it to other vector points."
2. **Snap to objects** — "Align the centers and outermost points of different objects."
3. **Snap to pixel grid** — "Align objects to an underlying grid to prevent misaligned pixel errors when
   exporting elements. **The pixel grid does not need to be visible for this setting to work.**"

Modifiers:

- `Ctrl/Cmd` **temporarily disables** Snap to geometry and Snap to objects.
- `Ctrl/Cmd` **temporarily disables** Snap to pixel grid — "but make sure you're in vector edit mode and
  zoomed in to the canvas."
- A documented UX trap: "If you've toggled on `Ctrl+click` opens right click menus, click and hold the
  object **before** using Control to temporarily disable snapping. This prevents accidentally opening the
  secondary menu."

[INFERRED] That last note is a genuine, documented conflict: on macOS, `Ctrl+click` opens the context menu,
so `Ctrl` cannot simultaneously be a snap-suppression key. Affinity uses `Alt` for this reason. This is a
concrete argument for choosing `Alt/⌥` as the snap-suppression modifier in Spool.

Snap-to-pixel-grid independent of grid visibility is a deliberate decoupling: **pixel snapping is about
export fidelity, not about seeing a grid.** That separation is worth copying.

Pixel grid itself is `⇧'` (Mac) / `CapsLock '` (Win); snap-to-pixel-grid toggle is `⇧⌘'`.

Snap settings "are applied across your Figma Design files". [DOCUMENTED]

## Affinity: the most configurable snap system

[Fully DOCUMENTED — "Snapping"]

### Targets

Snapping applies to "images, strokes, lines, shapes and selection areas" and aligns them to "grid lines,
guides, margins, artboards or spreads, or any combination of these. You can also snap to **object bounding
boxes, key points on shapes, and object geometry** when those targets are available."

Additionally: "**Text can also snap to the baseline of other text** (the first line only for text frames)
and artistic text objects can snap to the height of previously created artistic text."

[INFERRED] Text-baseline snapping is a professional-tool capability with no equivalent in the other three
products, and it depends on having real shaped text metrics.

### Colour-coded guides

| Colour | Meaning |
|---|---|
| **Red line** | The snapped item aligns **horizontally** with the target |
| **Green line** | The snapped item aligns **vertically** with the target |
| **Yellow node** | Aligns to shape key points (often centres) or object geometry |
| **Blue line** | Aligns to the third plane when using a triangular projection grid |
| **Orange line** | Aligns horizontally or vertically if a projection grid is active |

Smart guides "include **labels which report the distance between the snapping objects** (measured in the
document's set units)."

[INFERRED] Colour-coding the axis is a small decision with a large clarity payoff — a single glance tells
you whether you are aligning horizontally or vertically without reading the line orientation.

### Presets (7)

| Preset | Purpose |
|---|---|
| Page layouts | Snapping to placed guides, margins, spreads |
| Page layouts with objects | Above + object-to-object alignment |
| Object creation | "Simple object-to-object alignment to bounding boxes and their midpoints, plus for aligning some shapes to key points" |
| Curve drawing | "The setup for non-geometric use (e.g., drawing with the pen tool)" |
| UI design | "For UI/web design for pixel accuracy when using snapping to fixed guides and grid" |
| Pixel work | "For pixel-only brush work where vector-based object snapping is not needed" |

[INFERRED] Affinity has a **UI design** preset that disables curve snapping and enables pixel accuracy.
That is, in effect, a shipped interaction profile for exactly Spool's target user. It also means snapping
behaviour is *per-workflow*, not global.

### Candidate scoping (4 modes)

| Mode | Rule |
|---|---|
| **Candidate List** | Limits candidates to the number you set; creating a new object, selecting or hovering designates it as a candidate. Only active candidates can be snapped to |
| **Immediate layers** | Candidates limited to the current layer, sibling layers, or the immediate parent |
| **Immediate layers and children** | Current layer + its subordinate children |
| **All layers** | No limit |

With Candidate List: **Maximum** limits active candidates; new candidates replace older ones
chronologically. **Show snapping candidates** highlights active candidates with a purple halo.

[INFERRED] This is the most important performance mechanism in snapping: an explicit bound on candidate
count, plus a recency policy. Without a bound, snapping cost is O(all objects) per pointer move. Figma
and tldraw bound this implicitly (viewport + selection) rather than explicitly.

### Individual options

**Page snapping (group)**: Snap to grid; Snap to baseline grid (Layout Studio); Snap to guides; Snap to
spread; Include spread mid points; Snap to margin; Include margin mid points.

**Object snapping (group)**:
- **Only snap to visible objects**
- **Snap to object bounding boxes**
- **Include bounding box mid points** (vertical/horizontal centre of a target)
- **Snap to gaps and sizes** — "arrows represent matched gaps between snapping candidates and matched
  horizontal and/or vertical sizes"
- **Snap to shape key points** — "e.g. the center of a rectangle or ellipse, when moving nodes"
- **Snap to object geometry** — "vertices and intersections when drawing new objects, resizing objects, or
  moving nodes, rather than only bounding boxes or shape key points. Vertices are object corners or
  intersections, such as the points of a star"
- **Snap to pixel selection bounds** — "For example, using the Flood Select Tool, a pixel selection drawn
  over image 'edges' … will expose those edges for snapping"

**Explicit scoping note:** "Some object snapping options apply only during specific actions, such as
drawing new objects, resizing objects, or moving nodes with the Node Tool. **They do not affect moving
objects with the Move Tool.**"

[INFERRED] That last sentence is an admission of a genuine design tension: a single snap system cannot
serve object movement, resizing, drawing, and node editing with the same options. Affinity resolves it by
scoping options per action. Spool will face the same fork.

Also: "**Snapping always snaps to the currently set measurement unit.**"

### Force pixel alignment

Two separate settings:
- **Force pixel alignment** — "vector content will snap to full pixels when created, moved or modified. If
  this option is off, vector content can occupy partial pixels."
- **Move by whole pixels** — "allows you to constrain the movement of vector objects, nodes and handles to
  whole pixels"

[INFERRED] Two settings rather than one: creation/move snapping (automatic) vs. a *constraint* on the
gesture (explicit). Figma collapses these into one "snap to pixel grid" preference.

### Construction snapping (node editing only)

Documented as independent of the global snapping options:

- Inline with adjacent node
- To 90° from inline
- To reflected angle with adjacent control handle
- Parallel to adjacent control handle
- 90° to parallel control handle
- To logical triangle

Plus, within the Node Tool's own settings:
- **Align to nodes of selected curves** — align a moving node to any node on the same or a different curve
- **Snap to geometry of selected curves** — snap a moving node to the same or a different curve's path
- **Snap all selected nodes when dragging** — snap multiple selected nodes to a "target" node
- **Align handle positions using snapping options** — control handles obey the *global* snapping criteria

[INFERRED] Construction snapping is a **different algorithm** from object snapping: it operates on
**angular relationships between adjacent segments**, not on proximity to other objects. This is a
genuinely separate subsystem that only matters if Spool supports Bézier node editing.

## Canva

Canva documents guides and rulers (`⇧R` to show, `⌥⌘;` to lock) but **no snap settings, no tolerance, no
object snapping**. [DOCUMENTED absence]

[INFERRED] In a bounded, template-driven page where elements are positioned via the Position panel and
aligned via explicit tools, freehand object snapping is less necessary. Whether it is absent or simply
undocumented is **Unknown**.

## Snapping vs. alignment vs. distribute — the conceptual separation

All three products distinguish these, and conflating them is a common design error.

| Concern | Question answered | tldraw | Figma | Affinity |
|---|---|---|---|---|
| **Snap** | "While dragging, what nearby thing should this align to?" | `BoundsSnaps`, `HandleSnaps` | Preferences ▸ Snap to | Toolbar ▸ Snapping |
| **Align** | "Make these selected things line up." | `align-*` actions | `⌥W/A/S/D` | Contextual |
| **Distribute** | "Make the gaps between these things equal." | `distribute-*` actions | Distribute / Tidy up | "Snap to gaps and sizes" (partly) |

[INFERRED] Affinity blurs the boundary by putting gap snapping inside the snap system. That is arguably
correct for professional tools (drag-to-distribute is faster than a command) but it makes the snap engine
substantially more complex and couples it to gap detection over all candidates.

**Spool consideration:** separating them keeps the snap engine small and fast; coupling them gives a
faster professional workflow. This is a genuine architectural fork, and the research brief's emphasis on
"interaction profiles" suggests it may want both — snap-to-gaps on in an "Affinity"-like profile.

## Snapping and the interaction state machine

tldraw's snapping functions are **called by tools**, not by the core:

```ts
const snapData = editor.snaps.shapeBounds.snapTranslateShapes({...})
const snappedDelta = Vec.Add(delta, snapData.nudge)
```

and

```ts
const snapData = editor.snaps.handles.snapHandle({ currentShapeId, handle })
```

[INFERRED] Snapping is a **pure library the interaction layer calls**, returning a nudge. This is the
right shape because:
- It makes snapping testable without a UI.
- It composes: `final_delta = drag_delta + snap_nudge + constraint_adjustments`.
- The same snap engine serves translate, resize, and handle drag with different inputs.
- Indicators are a *side-effect* of the snap manager, not of the tool.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **No snapping whatsoever.** `const CULL_PADDING`, `DRAG_THRESHOLD`, `RESIZE_HANDLE_HIT_RADIUS` are the
  only tolerance constants.
- `movement_delta(camera, start_world, screen)` returns a raw world-space delta.
- `apply_move` adds it directly; `resized_geometry` uses it directly.
- No `SnapManager`, no snap points, no snap indicators, no guides, no grid snap.
- A decorative background grid *is* drawn in `Render` (`grid_y as f32 * spacing` with zoom-adaptive
  spacing between 24 and 48 screen px) — visual only, not a snap target. [OBSERVED]

[INFERRED] The prototype's zoom-adaptive grid spacing loop is *exactly* the zoom-normalisation rule that
tldraw applies to snap tolerance. That instinct is already present and should be reused for snapping.

## Candidate architectural implication

**Evidence:**

1. Every product has an explicit snap system with a bounded candidate set. [DOCUMENTED]
2. Tolerance must be zoom-normalised (screen px ÷ zoom) for consistent feel. [tldraw DOCUMENTED; Affinity
   "screen tolerance"; Spool's own grid code]
3. Snap detection returns a **nudge** that the interaction layer applies. [tldraw DOCUMENTED]
4. Which snap points are relevant depends on the **handle** and the **axis**. [tldraw DOCUMENTED]
5. `Ctrl/Cmd` cannot be the snap-suppression modifier on macOS without conflicting with `Ctrl+click`
   context menus. [Figma DOCUMENTED]
6. Indicators require merging collinear lines and deduplicating by containment. [tldraw DOCUMENTED]
7. Pixel snapping is about **export fidelity**, not grid visibility. [Figma DOCUMENTED]
8. A single snap system cannot serve object-move, resize, draw, and node-edit equally; options must be
   scoped per action. [Affinity DOCUMENTED]
9. Construction snapping (angular relationships between adjacent Bézier segments) is a **different
   algorithm**. [Affinity DOCUMENTED]

**Why it matters:** Snapping touches the hot path of every drag. A naive O(all objects) scan will be felt
immediately, and the "invert all gestures to pure functions of (start, pointer, modifiers)" property that
tldraw relies on is easier to achieve with a pure nudge-returning snap engine.

**Potential Spool approaches:**

- **A. Object-bounds snapping only.** Edge/centre/corner alignment against a spatial index, plus pixel
  snapping. Covers 90% of UI work. Small, fast, testable.
- **B. A + gap snapping.** Adds centring and duplication. Matches Affinity; increases complexity and cost.
- **C. A + handles snapping.** Needed for arrows/connectors — a relational feature.
- **D. A + construction snapping.** Only if Spool ships Bézier node editing.
- **E. Per-action option scoping** (Affinity's model) vs. one global setting set.

**Tradeoffs:** A is the smallest thing that is genuinely good. B is what makes a professional tool feel
professional but is where most naive implementations go wrong. C and D are gated on features that do not
exist in the prototype (connections, node editing).

A profile system (§ `profiles/`) could plausibly put A in the default profile and B/C/D in a
"professional" profile — but the *engine* must be built with those hooks from the start.

**Decision: TBD — requires architecture review.**

## Open questions

1. Snap tolerance: a single screen-px value, or a grid that scales with zoom?
2. `Alt/⌥` or `Ctrl/⌘` as the suppression modifier? (macOS context-menu conflict argues for `Alt`.)
3. Is gap snapping in scope at launch? If yes, is it in the core or in a profile?
4. Does snapping return a nudge (pure) or mutate (impure)?
5. Do snap indicators merge collinear lines, and who is responsible for dedup?
6. Does the snap candidate set come from a spatial index, a recency list, or a layer filter?
7. Is snapping on by default? (Figma/Affinity: on. tldraw: off.)
8. Is snapping a per-interaction option set, or global?

## Sources

- tldraw: `sdk-features/snapping.mdx` ⭐ (primary), `sdk-features/instance-state.mdx`,
  `sdk-features/performance.mdx`, `sdk-features/tools.mdx`, `sdk-features/handles.mdx`,
  `sdk-features/geometry.mdx`, `sdk-features/shapes.mdx`
- Figma: "Adjust alignment, rotation, position, and dimensions" (/360039956914); "Apply constraints to
  define how layers resize" (/360039957734)
- Affinity: "Snapping" (/design-aids-snapping/); "Smart guides" (/design-aids-dynamic-guides/);
  "Node Tool" (/tools-tools-node/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`movement_delta`, `apply_move`, `resized_geometry`, grid drawing
  in `Render`)
