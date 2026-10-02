# tldraw

## Overview

tldraw is an open-source infinite-canvas editor SDK. It is architecturally the most transparent of the
four reference products because the implementation is public and the documentation describes internal
abstractions by name. It is therefore the primary reference for **editor architecture** in this
research, and explicitly **not** a reference for product design.

Two packages matter:

- `@tldraw/editor` — "the core editor". "The `@tldraw/editor` package has **no built-in tools**." The
  root state has no tools; you supply them. [DOCUMENTED — `sdk-features/tools.mdx`]
- `tldraw` — the batteries-included product: the default tool suite, UI, actions, menus, i18n, themes.

> "By design, the editor's surface area is very large. Almost everything is available through it."
> [DOCUMENTED — `docs/editor.mdx`]

This is the opposite design posture from Figma: **there is no privileged internal API**. Every capability
is a public method.

## Product Philosophy

1. **Tools are states, not flags.** A tool is a top-level state in a hierarchical state chart.
   [DOCUMENTED]
2. **One reactive database for the whole document.** Shapes, pages, bindings, assets, and custom records
   are all "records" in one store. [DOCUMENTED — `sdk-features/store.mdx`]
3. **History is automatic.** "The history manager captures all user-initiated store changes
   automatically." [DOCUMENTED]
4. **Everything is overridable.** Tools, actions, UI components, and menu contents are all supplied via
   an `overrides` prop of functions that receive the defaults and return modified copies.
   [DOCUMENTED — `sdk-features/actions.mdx`, `sdk-features/tools.mdx`]
5. **Custom record types are a first-class extension point**, with validators, migrations, and scopes.
   [DOCUMENTED]
6. **Extensibility is a design requirement, not a plugin afterthought.** There is no plugin sandbox;
   instead, the SDK *is* the extension surface.

## Canvas Model

| Aspect | Behaviour | Evidence |
|---|---|---|
| Canvas shape | Infinite | DOCUMENTED |
| Top-level container | **Pages** (records), one active page at a time | DOCUMENTED |
| Spatial container | **Frame** — a shape that groups and clips children | DOCUMENTED (`sdk-features/frame-shape.mdx`) |
| Logical container | **Group** — a shape whose children can be focused | DOCUMENTED (`sdk-features/groups.mdx`) |
| Z-order | Per-shape **fractional string index**, relative to siblings | DOCUMENTED (`sdk-features/shape-indexing.mdx`) |
| Bounds | Per-shape page bounds, plus rotated bounds | DOCUMENTED |
| Culling | `getNotVisibleShapes()` / `getCulledShapes()`, spatial index, `display:none` | DOCUMENTED |
| Camera | `editor` camera object with zoom, bounds, and screen bounds | DOCUMENTED (`sdk-features/camera.mdx`) |
| Grid | `isGridMode` flag on the instance | DOCUMENTED |

### Fractional indexing — a documented architectural choice [DOCUMENTED]

> "Integer indices don't leave room to insert. […] Fractional indexing uses lexicographically sortable
> strings. You can always generate a new index between any two existing indices. […] reordering only
> updates the moved shapes, not every shape on the canvas."

Indices look like `'a1'`, `'a2'`, `'a1V'`. The leading letter encodes the integer-part length; the rest
is the fractional part; digits are base-62. Generated indices are **jittered** so that two collaborators
inserting at the same position simultaneously get *different* keys and both merges succeed.

[INFERRED] This is a collaboration-first design decision with three consequences that Spool should note:
1. Reordering is O(moved) rather than O(all siblings).
2. Z-order cannot be a simple array index without renumbering.
3. "Bring forward" must be *defined* for shapes that do not overlap. tldraw's answer is notable:
   "these methods only move shapes past other shapes whose page bounds overlap theirs. This makes
   keyboard shortcuts feel intuitive." [DOCUMENTED]

## Interaction Model

This is tldraw's defining contribution and the highest-value material in this research.

### The state chart [DOCUMENTED, `sdk-features/tools.mdx`]

> "The editor creates a root state that contains all tools as children. When an input event occurs, it
> flows down from the root through the currently active tool and its active child state."

Node types: **root** (contains tools), **branch** (has child states), **leaf** (does work).
Tools are typically branch nodes.

Example from the docs, the select tool's child states:

```
root
└── select                (tool, branch)
    ├── idle
    ├── pointing_canvas
    ├── pointing_shape
    ├── brushing
    ├── translating
    ├── resizing
    └── rotating
```

> "When you resize a shape with the select tool, the tool moves through `idle`, `pointing_resize_handle`,
> and `resizing`."

### Lifecycle [DOCUMENTED]

- `onEnter` runs when a state becomes active; `onExit` when it becomes inactive.
- Handlers: `onPointerDown`, `onPointerMove`, `onPointerUp`, `onLongPress`, `onDoubleClick`,
  `onRightClick`, `onMiddleClick`, `onKeyDown`, `onKeyUp`, `onKeyRepeat`, `onWheel`, `onCancel`,
  `onComplete`, `onInterrupt`, `onTick`.
- **A state that does not implement a handler skips the event** — the event continues down to the active
  child state. This means parent and child can *both* respond.
- Transitions are explicit: `this.parent.transition('pointing_shape', info)`.
- Transitions can target direct children by id, or deeper descendants with dot notation
  (`'crop.pointing_crop_handle'`).
- Transition payloads are visible to **both** the old state's `onExit` and the new state's `onEnter`.

### Event targets and re-dispatch [DOCUMENTED]

> "Event info objects include a `target` property indicating what the user interacted with: `canvas`,
> `shape`, `handle`, `selection`, or `overlay`. The canvas dispatches every pointer event with
> `target: 'canvas'`. The select tool's idle state hit-tests the pointer position and **re-dispatches the
> event to itself with a more specific target**, then transitions to the matching child state:
> `pointing_shape` for a shape, `pointing_canvas` for empty canvas."

[INFERRED] This is the key idea: **hit-testing is itself an interaction state transition.** The state
chart doesn't just react to raw events; it *reclassifies* them. This makes "what am I pointing at"
a first-class part of the interaction model rather than a side effect.

### Tool lock [DOCUMENTED]

Creation tools return to `select` after use. **Tool lock** keeps the tool active:

```ts
if (this.editor.getInstanceState().isToolLocked) {
    this.parent.transition('idle')          // stay in this tool
} else {
    this.editor.setCurrentTool('select')   // hand back
}
```

Critically: **"Tool lock is not enforced by the state machine. Custom tools check `isToolLocked`
themselves."** [DOCUMENTED] This is a deliberate, and arguably awkward, separation — the *policy* lives
in the tool, the *flag* lives in instance state.

### Static configuration on a tool [DOCUMENTED]

| Property | Meaning |
|---|---|
| `id` | unique identifier |
| `initial` | id of initial child state |
| `children()` | child state constructors |
| `isLockable` | whether the toolbar shows the lock toggle (default `true`) |
| `useCoalescedEvents` | receive browser coalesced pointer moves for higher-fidelity input (draw tool); always off on iOS |

### Performance tracking per state [DOCUMENTED]

> "Custom tools opt into interaction tracking by setting `StateNode#trackPerformance` on the state node
> class. When the state is entered, the manager starts a tracking window; when it exits, it emits
> `interaction-start` / `interaction-end` with the state path. Built-in interactions like
> `select.translating` and `draw.drawing` already track."

The emitted event carries `fps`, `p95FrameTime`, and Long Animation Frame attribution.

[INFERRED] Treating **the state path as the unit of performance measurement** is a strong idea: it ties
performance telemetry to interaction semantics rather than to UI components.

## Selection

Selection is implemented as **instance state**, not document state. [DOCUMENTED —
`sdk-features/selection.mdx`, `sdk-features/instance-state.mdx`]

> "The editor tracks selection through the `selectedShapeIds` array in the current page's instance
> state. This array holds the IDs of all currently selected shapes. Everything else about the selection
> (bounds, rotation, the selected shape records) is derived from it."

Two automatic rules, enforced by store side effects:

1. **Ancestor–descendant filtering.** "When the selection changes, the editor filters out any shape
   whose ancestor is also selected." `editor.select(groupId, childOfGroupId)` results in only `groupId`
   selected. The filtering "happens in the `instance_page_state` after-change side effect."
2. **Focused group management.** Selecting children of a group focuses that group; while a group is
   focused, clicks select shapes *inside* it rather than the group. If all selected shapes share a common
   group ancestor, that group becomes focused. Clearing the selection **leaves the focused group in
   place**.

[INFERRED] "Focused group" is the explicit form of what Figma implements implicitly with its
click-parent / double-click-descend scope stack. tldraw additionally names the sibling traversal
behaviour:

- `selectAdjacentShape('next' | 'prev' | 'left' | 'right' | 'up' | 'down')` — cardinal directions score
  candidates by distance and by how far off-axis they are, picking the lowest score; if inside a
  group/frame only siblings in that container are considered; shapes whose `canTabTo()` util returns
  `false` are skipped. Bound to Tab / Shift+Tab and Cmd/Ctrl+Arrow by default. [DOCUMENTED]
- `selectParentShape()` / `selectFirstChildShape()` — only act with exactly one shape selected; both
  zoom to the new selection if it is offscreen. Bound to Cmd/Ctrl+Shift+Up / Cmd/Ctrl+Shift+Down.
  [DOCUMENTED]

Selection bounds come in two flavours: **axis-aligned** (`getSelectionPageBounds`) and **rotated**
(`getSelectionRotatedPageBounds`), plus screen-space variants of both. Shared rotation is available via
`getSelectionRotation()`, returning 0 if shapes have differing rotations. [DOCUMENTED]

Locked shapes: excluded from bulk operations (`selectAll`, `deleteShapes`, duplicate) but *can* be
selected explicitly via `select()`. The `selectLockedShapes` option allows clicking/brushing locked
shapes while still protecting them from edits. [DOCUMENTED]

## Transformation

- `translateShapes`, `rotateShapesBy`, `nudgeShapes`, resize with handle, `resizeShapes` with scale and
  handle id. [DOCUMENTED at the editor-method level]
- Aspect-ratio locking and resize-from-centre are parameters to resize snapping
  (`isAspectRatioLocked`, `isResizingFromCenter`). [DOCUMENTED — `sdk-features/snapping.mdx`]
- **Alt/Option drag changes mode mid-interaction**, not just at press time. [DOCUMENTED — history docs:
  "A user can switch between translating and cloning by pressing or releasing the alt (option) key during
  a drag. When this changes, we bail on the changes since the interaction started, then apply the new
  mode's changes."]

[INFERRED] This is the correct model for modifier keys with memory. Re-deriving the gesture from
scratch on modifier change, and *bailing* rather than committing the intermediate state, is what makes
"release Alt mid-drag to convert a move into a clone" feel correct and stay undoable.

- Z-order: `bringToFront`, `bringForward`, `sendBackward`, `sendToBack`, all preserving relative order,
  and by default only moving past shapes whose bounds overlap. `considerAllShapes: true` bypasses.
  [DOCUMENTED]

## Snapping

The single most detailed snapping specification found in any of the four products. All DOCUMENTED from
`sdk-features/snapping.mdx`.

### Two independent systems

```
editor.snaps.shapeBounds   → BoundsSnaps   (edges, centres, gaps)
editor.snaps.handles       → HandleSnaps   (point-to-outline, point-to-point)
```

### Default polarity (opposite to Figma)

> "Snapping is off by default. Users hold Ctrl (Cmd on Mac) while translating, resizing, or dragging a
> handle to snap. Turning on snap mode inverts this: snapping is always on and holding Ctrl disables it."

Grid snapping is a *separate* system controlled by `isGridMode`.

### Threshold is zoom-normalised

> "The snap threshold is `editor.options.snapThreshold` screen pixels (default 8), scaled by the current
> zoom level. At 100% zoom, shapes snap when within 8 pixels. At 200% zoom, the threshold becomes 4
> canvas units (still 8 screen pixels)."

[INFERRED] Expressing tolerance in **screen** pixels and dividing by zoom makes snapping feel identical
at every zoom level. A world-space tolerance would make snapping useless when zoomed out and
over-eager when zoomed in.

### Candidate filtering [DOCUMENTED]

`getSnappableShapes()`: starting from the selection's common ancestor, walk down the shape tree,
skipping: selected shapes ("you don't snap to what you're dragging"), shapes outside the viewport, and
shapes whose `ShapeUtil#canSnap()` returns false. **Frames are included as targets.** For groups, the
system recurses into children and snaps to them **but not to the group itself**.

Shape-level opt-in/out:
- `canSnap()` → boolean-ish computed set; `false` removes the shape as a target entirely.
- `getBoundsSnapGeometry(shape)` → `{ points: [...] }`; default is bbox corners + centre; return
  `{ points: [] }` to keep gap-snapping but drop point-snapping.

### Bounds snapping operations [DOCUMENTED]

- **Translation**: `snapTranslateShapes({ lockedAxis, initialSelectionPageBounds,
  initialSelectionSnapPoints, dragDelta })` returns a `nudge` vector. "When multiple shapes align at the
  same distance, the system displays all of them."
- **Resize**: corner handles snap both axes using that corner; edge handles snap only the perpendicular
  axis using **both corners on that edge**; when aspect ratio is locked the **dominant snap axis
  determines both**.
- **Gap snapping** — two sub-behaviours:
  - *Gap centring*: centres the selection within a gap larger than itself, equal on both sides.
  - *Gap duplication*: repeats an existing gap on the opposite side — if two shapes have a 100px gap,
    a third snaps to create another 100px gap.
  - Gaps computed separately per axis; a gap exists when two shapes do not overlap on one axis and do
    overlap on the perpendicular axis.

### Handle snapping [DOCUMENTED]

`getHandleSnapGeometry(shape)` returns `{ outline: Geometry2d | null, points, getSelfSnapOutline(),
getSelfSnapPoints() }`.

> "By default, handles cannot snap to their own shape. Moving the handle would change the snap target
  and create a feedback loop."

Two snap types on the handle record:
- `snapType: 'point'` — snap to the single nearest location; checks snap points first, then the nearest
  point on any outline.
- `snapType: 'align'` — align to nearby snap points on x and y **independently**, with a snap line in each
  direction.

(the older `canSnap` boolean on handles is deprecated)

### Indicators [DOCUMENTED]

Two indicator kinds:
- `points` — lines connecting aligned points; multiple aligned points on one axis render as one
  continuous line.
- `gaps` — measurement lines at each gap; all matching gaps shown together.

The manager **drops redundant gap indicators**: if every gap in one indicator already appears in a larger
indicator for the same direction, only the larger is kept. Indicators clear automatically when dragging
stops.

[INFERRED] Indicator redundancy elimination is a rendering-quality concern that is easy to miss and very
visible when wrong. Worth treating as a first-class requirement of any snap engine.

## Tools

Built-in tools in `tldraw` package [DOCUMENTED — `docs/tools.mdx`]: Select, Hand, Draw (freehand),
Arrow. The core has none.

Documented creation patterns:
- A **no-child-state tool** implements pointer handlers directly (the "stamp" example: `onPointerDown`
  creates a shape).
- A **phased tool** declares `initial` + `children()`. The "pointing" state waits to disambiguate:
  `onPointerMove` checks `editor.inputs.getIsDragging()` and transitions to `drawing`; `onPointerUp`
  transitions back to `idle`. **A click therefore creates nothing** — it round-trips to idle.

[INFERRED] tldraw's shape tools do not have a click-to-place-default-size behaviour. That is a genuine
product difference from Figma, not an oversight.

Custom shape shapes (via `ShapeUtil`) expose optional per-type hooks that the core calls:
`canSnap()`, `canCull()`, `canBind()`, `canTabTo()`, `getBoundsSnapGeometry()`,
`getHandleSnapGeometry()`, `getText()`, `getGeometry()`. [DOCUMENTED]

## Text

- Text is a **shape type** with a `richText` property, not a separate subsystem. `toRichText(...)`
  converts a plain string. [DOCUMENTED — `docs/tools.mdx`, `sdk-features/rich-text.mdx`]
- `ShapeUtil#getText(shape)` extracts a shape's text as a plain string, returning `undefined` when there
  is none. [DOCUMENTED — used by the AI agent for structured context extraction]
- Text measurement has its own documented module. [DOCUMENTED — `sdk-features/text-measurement.mdx`]
- Editing text (caret, ranges, rich-text marks) is a dedicated shape/tool concern.
  [DOCUMENTED at the module level]

## Drawing

- **Draw tool** uses `useCoalescedEvents: true` — the browser's coalesced pointer moves, i.e. *sub-frame*
  input fidelity. This is documented as "higher-fidelity input, as the draw tool does".
  [DOCUMENTED]
- The **scribble** shape has its own documented module (`sdk-features/scribble.mdx`), meaning freehand
  input is a first-class stored representation, not a transient.
- Freehand points are stored on the shape. [SOURCE-CODE / DOCUMENTED via scribble module]
- The **eraser** is a tool, not a modifier. [DOCUMENTED — `sdk-features/eraser.mdx`]

[INFERRED] Freehand data must be persistent (undoable, syncable, exportable), which means the document
model needs a representation for an ordered list of pressure/time-varying points, plus a fitted/simplified
curve. This is a different shape of data from a Bézier path.

## Document Model

### The Store [DOCUMENTED — `sdk-features/store.mdx`]

> "The store is tldraw's reactive database. It holds all shapes, pages, bindings, assets, and other
> records that make up your document. The store is reactive: when data changes, the UI updates
> automatically. It validates all records against a schema and **tracks every change for undo/redo,
> persistence, and synchronization**."

Record shape:

```ts
{
  id: 'shape:abc123',      // branded string, type-prefixed
  typeName: 'shape',
  type: 'geo',
  x: 100, y: 200,
  props: { geo: 'rectangle', w: 300, h: 150, color: 'blue' },
}
```

The type prefix in the id ("This prevents accidentally mixing up IDs from different record types") is a
deliberate type-safety choice that survives JSON serialisation. [INFERRED]

### Record scopes — the explicit document/session/presence split

[DOCUMENTED]

| Scope | Persisted | Synced to other users | Example |
|---|---|---|---|
| `document` | Yes | Yes | Shapes, pages, bindings |
| `session` | Optional | No | Current page, camera position |
| `presence` | No | Yes | Cursor positions, user selection |

[INFERRED] This is the cleanest published statement of the document-state/editor-state distinction
available in any of the four products, and it is enforced by the type system (scope is part of the
record definition) rather than by convention.

### Indexed queries [DOCUMENTED]

```ts
editor.store.query.index('shape', 'parentId')   // Map<parentId, Set<shapeId>>
editor.store.query.records('shape', () => ({ type: { eq: 'geo' } }))
```
Query expressions support `eq`, `neq`, `gt`. Both `records()` and `index()` return **computed**
(signals-tracked) values.

### Computed caches [DOCUMENTED]

```ts
const boundsCache = editor.store.createComputedCache('shape-bounds', (shape) => calculateBounds(shape))
const bounds = boundsCache.get(shapeId)
```
Lazily computed on access, invalidated when the underlying record changes. Documented as how the editor
maintains shape bounds, geometry, and other derived data.

### Reactive vs non-reactive reads [DOCUMENTED]

`store.get(id)` creates a dependency; `store.unsafeGetWithoutCapture(id)` does not, for hot paths.

## Hierarchy

- Single `parentId` per shape. [DOCUMENTED via the index example and `sdk-features/parenting.mdx`]
- **Groups** are shapes with children, and there is a **focused group** concept that changes click
  targeting. [DOCUMENTED]
- **Frames** are shapes that clip and provide a local coordinate container. [DOCUMENTED]
- `parenting.mdx` documents reparenting semantics including what happens to geometry on reparent.
  [DOCUMENTED at module level; exact coordinate-preservation rules were not read in this pass]
- **Z-order is per-parent**, via the fractional index. [DOCUMENTED]

## Components / Reuse

tldraw has **no component/instance system**. Reuse is achieved by:

1. **Copy/paste and duplicate** (with index regeneration).
2. **Bindings** — persistent relationships, but directional from-one-shape-to-another, not containment.
3. **Custom shapes** — the real reuse mechanism: define a `ShapeUtil` subclass once and every instance
   of that shape type shares behaviour, geometry, snapping, culling, and rendering.
4. **Starter kits** — prebuilt compositions shipped as code.
5. **Binding-based "node shapes"** in the workflow starter kit.

[INFERRED] The absence of an instance/override system is the single largest document-model difference
between tldraw and Figma/Canva. It also means tldraw has no mechanism for "change the master, update
200 instances". For Spool, which wants both extensibility *and* design-system semantics, tldraw's answer
is insufficient — but its **custom shape type** mechanism is exactly the right extension point for
node/vector-like objects.

## Layout

tldraw has **no flow layout engine**. Positioning is absolute (x, y, w, h, rotation) with frames,
grouping, and snapping providing structure. [INFERRED from the absence of any layout module in the docs
index and from the framing of `sdk-features/shape-transforms.mdx`]

Structure is provided by:
- Frames (spatial, clipping).
- Binding-based constraints in third-party patterns (the workflow starter kit describes "layout
  constraints that keep shapes aligned" via bindings). [DOCUMENTED — `sdk-features/bindings.mdx`
  opening paragraph]

[INFERRED] tldraw demonstrates that a general-purpose editor can be built without a layout engine, by
making grouping + snapping + bindings expressive enough. It does **not** demonstrate that a design tool
for UI layouts should skip a layout engine.

## Styles

- Colours are a **small named palette** (`color: 'blue'`) rather than arbitrary values, per the example
  record. [DOCUMENTED]
- `sdk-features/themes.mdx` documents UI theming; `sdk-features/styles.mdx` documents canvas style
  utilities. [DOCUMENTED at module level]
- `user-preferences.mdx` documents cross-instance settings. [DOCUMENTED]

[INFERRED] tldraw optimises for **document portability and size** over design-system flexibility: a
named palette serialises compactly and renders identically everywhere. This is a deliberate trade that
Spool may not want if design systems are in scope.

## Assets

[DOCUMENTED — `sdk-features/assets.mdx` exists as a dedicated module; assets are store records]

- Assets are records with a `src`, dimensions, `mimeType`, and `name`.
- `TLAssetStore` is the resolution interface: `resolve(asset, context)` returns a URL.
- **`TLAssetContext` includes `steppedScreenScale`**: "the ratio of the shape's on-screen size (in CSS
  pixels) to the image's native size, rounded up to the nearest power of two." Multiplying by `dpr`
  gives device pixels. [DOCUMENTED — `sdk-features/performance.mdx`]
- Resolution updates are **debounced** so images don't thrash between sizes while zooming.
  [DOCUMENTED]

[INFERRED] `steppedScreenScale` rounded to powers of two is a classic mipmap-style trick that turns an
unbounded set of possible zoom levels into a small, cacheable set of asset variants. This is a directly
transferable technique.

## History

[DOCUMENTED — `sdk-features/history.mdx`]

### Model

- Two stacks: undo and redo.
- Each stack entry is either a **diff** (record changes) or a **mark** (a stopping point).
- Changes accumulate until a mark is created; the pending changes are then flushed to the undo stack as
  **a single entry**.
- Undo reverses all changes back to the previous mark, moves them to the redo stack, and applies the
  reversed diff **atomically**.

### Marks

```ts
const markId = editor.markHistoryStoppingPoint('rotate shapes')
editor.rotateShapesBy(ids, Math.PI / 4)
// undo returns to this mark
```

"Creating a mark flushes pending changes onto the undo stack. It doesn't clear the redo stack; the next
recorded change does that."

### Three history modes

| Mode | Undo stack | Redo stack |
|---|---|---|
| `record` | Add | Clear |
| `record-preserveRedoStack` | Add | Keep |
| `ignore` | Skip | Keep |

Documented usage:
- **`record-preserveRedoStack` is used for selection.** "This way you can undo, select some shapes, copy
  them, and then redo back to where you were."
- **`ignore` is used for writing your own pointer position** for collaborators. "Where your cursor was
  doesn't need to be undoable."
- Nested `run()` calls keep the outer mode unless they set their own; no mode applies during undo/redo.

### Bail and Squash — the two advanced primitives

**Bail** reverses changes *without* adding them to the redo stack — changes are discarded entirely.
```ts
const markId = editor.markHistoryStoppingPoint('begin drag')
// user drags
// user presses escape
editor.bailToMark(markId)
```
Used while cloning shapes: switching between translating and cloning mid-drag bails the accumulated
changes then applies the new mode's.

**Squash** combines all changes since a mark into a single undo step; intermediate marks are removed.
```ts
const markId = editor.markHistoryStoppingPoint('bump shapes')
editor.nudgeShapes(shapes, {x:10,y:0})
editor.nudgeShapes(shapes, {x:0,y:10})
editor.squashToMark(markId)   // one undo step
```
Used during image cropping: changes are individually undoable *while cropping*, and squashed to one entry
on exiting crop mode.

### Capture rules

- Only changes with source `'user'` are captured. Remote (synced) changes are **never** recorded.
- "Only store records take part in undo/redo; state held outside the store, like your own atoms, does
  not." [DOCUMENTED]

[INFERRED] This is the cleanest published answer to "what does the user perceive as one undoable
action": **one mark, placed at the start of each user-intent interaction.** Bail = "this interaction is
void"; squash = "these interactions are one action". Both are needed and they are different operations.
Spool's current prototype has neither.

## Keyboard

[DOCUMENTED — `sdk-features/actions.mdx`]

### Actions, not raw key handlers

```ts
{
  id: 'undo',
  label: 'action.undo',
  icon: 'undo',
  kbd: 'cmd+z,ctrl+z',
  readonlyOk: false,
  checkbox: false,
  isRequiredA11yAction: false,
  onSelect(source) { /* source: 'kbd' | 'menu' | 'toolbar' | ... */ }
}
```

- ~100 default actions covering editing, grouping, arrangement, export, zoom, preferences.
- "Menus and toolbars look up actions by ID and render them with their labels, icons, and keyboard
  shortcuts." **One registry drives menus, toolbars, and keys.**
- `onSelect` receives a `source` parameter — used for analytics.
- Context-sensitive labels: `label` may map menu context types to different keys.

### Shortcut string format

- Modifiers `+`-separated: `cmd` (alias `meta`), `ctrl`, `shift`, `alt` (alias `option`).
- Special keys: `del`, `backspace`, `enter`, `escape`, `space`, `left/right/up/down`.
- Commas bind several combinations: `'cmd+g,ctrl+g'`. "Every combination is active on every platform;
  the conventional `cmd+…,ctrl+…` pair covers Mac and everything else, and only the shortcut hint shown in
  menus is platform-specific."

### Layout-correct matching — the most important keyboard detail in this research

[DOCUMENTED]

> "Shortcuts match the character the browser reports for the key press, not the physical keycap, with
> two exceptions: shifted number-row keys match by position, so write them as `shift+<digit>` on every
> layout, and when the reported character is non-ASCII (Cyrillic, Greek, macOS Option dead keys)
> matching falls back to the physical key's US QWERTY character, so `cmd+z` still works on a Russian
> layout."
>
> "With shift held, the shifted and unshifted US spellings are the same binding, so `shift+:` and
> `shift+;` both fire when the browser reports `:` with shift held. **Bind what the keyboard emits, not
> what the keycap says**: on French AZERTY the `:` key emits `/` with shift held, so bind `shift+/`; on
> Japanese JIS the `^` key emits `~`, so bind `shift+~`. The characters `!`, `?` and `$` are reserved as
> legacy modifier markers and cannot be used as keys."

To bind `+` itself, double the trailing plus: `cmd++`. On layouts where `+` is shift+another key, use
`cmd+shift++`.

[INFERRED] Three separate matching strategies are needed: (1) emitted character, (2) physical US position
fallback for non-ASCII, (3) positional matching for the number row. A design tool that ignores this will
break for a large fraction of the world. Spool, as an open-source tool, will encounter this.

### Activation conditions

> "Shortcuts only fire while the editor is focused and the key event does not target a text input. They
> are also disabled when a menu is open, a shape is being edited, the editor has a crashing error, or the
> user has disabled keyboard shortcuts in preferences. In readonly mode, only actions with `readonlyOk`
> are bound. Actions marked with `isRequiredA11yAction: true` bypass the disabled check."

### Customisation

- `overrides.actions(editor, actions, helpers)` can modify `kbd`, delete actions, replace `onSelect`, or
  add new ones. Custom actions integrate with the keyboard system automatically.
- **Exception:** "`copy`, `cut`, and `paste` shortcuts are handled by native clipboard events rather than
  the `kbd` system, so changing their `kbd` has no effect." [DOCUMENTED] — a real limitation worth
  knowing about.

Default action ids by category [DOCUMENTED]:

| Category | Action ids |
|---|---|
| Editing | `undo`, `redo`, `duplicate`, `delete`, `copy`, `cut`, `paste` |
| Grouping | `group`, `ungroup` |
| Arrangement | `bring-to-front`, `bring-forward`, `send-backward`, `send-to-back`, `align-left`, `align-center-horizontal`, `align-right`, `distribute-horizontal`, `distribute-vertical` |
| Export | `export-as-svg`, `export-as-png`, `copy-as-svg`, `copy-as-png` |
| Zoom | `zoom-in`, `zoom-out`, `zoom-to-100`, `zoom-to-fit`, `zoom-to-selection`, `select-zoom-tool` |
| Preferences | `toggle-dark-mode`, `toggle-snap-mode`, `toggle-grid`, `toggle-focus-mode` |

Zoom-to-fit and zoom-to-selection as **first-class actions** [DOCUMENTED]. Spool's prototype has neither.

## Rendering / Performance

[DOCUMENTED — `sdk-features/performance.mdx`, `sdk-features/culling.mdx`, `sdk-features/shape-indexing.mdx`]

### The eight techniques, verbatim from the docs

1. **Viewport culling** — spatial index; off-screen shapes get `display:none`. "A canvas with 10,000
   shapes might only render 50." Culled shapes stay in the store and can still be selected, hit-tested,
   and exported. Selected shapes and the shape being edited are **never culled**.
2. **Reactive signals** — fine-grained dependency tracking; changing one shape's colour does not re-render
   shapes that do not care about colour.
3. **Batched store updates** — `createShapes`/`updateShapes` with multiple shapes produce **one**
   notification; `editor.run()` batches across calls.
4. **Debounced zoom** — `getEfficientZoomLevel()` returns a *stable* value during camera movement when the
   document has more than `debouncedZoomThreshold` (default 500) shapes. "Once the camera stops, the
   value updates to the true zoom level." Custom shapes must use it instead of `getZoomLevel()` for
   rendering-affecting properties.
5. **Geometry caching** — access via `editor.getShapeGeometry()`; the editor "handles caching,
   transforms, and bounds calculation".
6. **Level of detail** — see below.
7. **Image resolution scaling** — `steppedScreenScale`, power-of-two, debounced.
8. **Spatial/shape indexing** — see fractional indexing and computed indexes.

### Level of detail, documented concretely

- Sticky notes drop their box shadow for a plain bottom border.
- Dashed and dotted freehand strokes render as solid lines.
- Hatch pattern fill switches to a solid fallback colour.
- Text outlines turn off below `textShadowLod` (default 0.35), always off on Safari.
- All LOD transitions use `getEfficientZoomLevel()` so they don't flicker during camera movement.

### Culling API split

- `getNotVisibleShapes()` — bounds don't intersect viewport AND util allows culling.
- `getCulledShapes()` — the above, **minus selected shapes and the shape being edited**.
- Both are reactive and return the *same Set instance* while contents are unchanged, so reading is cheap.

[INFERRED] That identity-stability detail is the whole trick: reactive containers that keep referential
identity when nothing changed are what make fine-grained reactivity usable from a per-frame render loop.

### Performance measurement

- `editor.performance` (`PerformanceManager`) emits `interaction-end` (with `fps`, `p95FrameTime`, LoAF
  attribution), `camera-end`, `shapes-created`, `shapes-updated`, `shapes-deleted`, `frame`.
- "with no overhead when no listeners are attached".
- `PerformanceApiAdapter` wires the same events into native `performance.mark()`/`measure()`.
- Diagnostic starting points documented: `getCurrentPageShapeIds().size`, `getCulledShapes().size`,
  production builds only.

### Editor options that affect performance [DOCUMENTED]

| Option | Default |
|---|---|
| `debouncedZoom` | `true` |
| `debouncedZoomThreshold` | `500` |
| `maxShapesPerPage` | `4000` |
| `textShadowLod` | `0.35` |
| `snapThreshold` | `8` (screen px) |

## Collaboration

- `sdk-features/collaboration.mdx`, `sdk-features/sync.mdx` (23KB — the largest doc in the set),
  `sdk-features/cross-tab-sync.mdx`, `sdk-features/cursors.mdx`, `sdk-features/presence`-style records.
- Documented sync record scopes: `document` records sync and persist; `presence` records sync but do not
  persist; `session` records neither.
- "Changes that arrive from other users are marked `'remote'` and are never recorded [in history]."
  [DOCUMENTED — `sdk-features/history.mdx`]
- Fractional index jittering exists specifically to avoid collisions on concurrent insert.
  [DOCUMENTED]
- `@tldraw/sync` is a separate product; the specific conflict-resolution protocol was not read in this
  pass. **Unknown at the protocol level.**

## Prototyping

tldraw has no prototype mode. However, the **binding system** is the substrate from which prototype-like
behaviour is built. [DOCUMENTED — `sdk-features/bindings.mdx`]

### Bindings

- A binding is a **persistent record** with `fromId` and `toId`, plus `props` and `meta`.
- Directionality is meaningful: `from` owns the relationship. "If you move a rectangle that an arrow
  points to, the arrow binding's `onAfterChangeToShape` hook fires. If you move the arrow itself,
  `onAfterChangeFromShape` fires instead."
- Arrow bindings store the **normalized anchor point** on the target shape plus whether the attachment is
  "precise" or should snap to the shape's edge.
- The bindings index is a computed value that "updates incrementally as bindings change. Lookups are fast
  and never scan all records."

### BindingUtil lifecycle hooks [DOCUMENTED]

| Hooks | When |
|---|---|
| `onBeforeCreate`, `onAfterCreate`, `onBeforeChange`, `onAfterChange` | the binding record itself is created/modified; `onBefore*` can return a replacement record |
| `onAfterChangeFromShape`, `onAfterChangeToShape` | **a bound shape changes** — the primary synchronisation hooks |
| `onBeforeDelete`, `onAfterDelete` | the binding record is removed |
| `onBeforeDeleteFromShape`, `onBeforeDeleteToShape` | a bound shape is about to be deleted |
| `onBeforeIsolateFromShape`, `onBeforeIsolateToShape` | **the bound shapes are about to be separated** (one deleted/copied without the other) — "use these to *bake in* the binding's current state before it disappears" |
| `onOperationComplete` | all binding operations in a transaction have finished — compute aggregate updates |

[INFERRED] **The isolation vs. deletion distinction is a genuinely important architectural idea** that
appears nowhere else in the four products. Deletion is one cause of separation; copy/duplicate is
another. A relationship system that only models "delete the target" will produce dangling artefacts in
every other case.

### Automatic bookkeeping [DOCUMENTED]

- Deleting a shape removes its bindings and fires isolation + deletion callbacks.
- Copying shapes duplicates **only bindings between the copied shapes**.
- Moving shapes to different pages removes cross-page bindings automatically.
- Both bound shapes copied together ⇒ binding copied.
- `createBinding` checks both shapes' `canBind()` and skips the binding if either refuses.

## Export

- `Editor#getSvgString(shapes)`, `Editor#toImage(shapes, { format: 'png' })`. [DOCUMENTED]
- Actions: `export-as-svg`, `export-as-png`, `copy-as-svg`, `copy-as-png`, and `exportAs` supports
  **JSON**. [DOCUMENTED]
- Export operates on shape ID lists, not on "current selection" or "current frame". [DOCUMENTED]
- Culled shapes are still exportable. [DOCUMENTED]
- The **Screenshot tool** is a shipped example: drag a box, export it as an image. [DOCUMENTED]

[INFERRED] Export takes a *set of shapes* rather than a *view*. This means export is a pure function of
(document, shape set, options) and does not depend on camera or viewport — a much more testable design
than Figma's page/frame/selection/slice-scoped export.

## AI

tldraw's AI material is the most architecturally explicit of the four products. [DOCUMENTED —
`docs/ai.mdx`, `docs/driver.mdx`]

### Three patterns

1. **Canvas as output** — place generated images (`asset` + `image` shape), HTML previews (custom shape
   with `HTMLContainer` + sandboxed `iframe` with `srcDoc`), or embeds (`embed` shape util).
2. **Visual workflows** — AI as nodes in a binding-connected graph, with a `NodeDefinition` type
   declaring a validator, ports, a default value, and a body height; an execution engine resolves
   dependencies and runs nodes in order.
3. **Agents** — direct read/manipulate access.

### How agents see the canvas [DOCUMENTED]

> "The agent builds context from multiple sources:
> - A screenshot of the current viewport
> - Simplified representations of shapes within view
> - Information about shape clusters outside the viewport
> - The user's current selection and recent actions
> - Conversation history from the session"

[INFERRED] "Shape clusters outside the viewport" is a spatial-summary strategy — the agent gets a
structural map of far-away content without paying for a full render.

### How agents read content [DOCUMENTED]

> "Sending both to the model works best: **the image shows spatial relationships and styling, and the
> structured data gives exact text and positions.**"

Structured extraction: `{ id, type, text: getShapeText(shape), bounds: getShapePageBounds(shape) }`.

### How agents manipulate the canvas [DOCUMENTED]

- Agents perform operations through **typed action schemas**.
- Each action has a defined structure; the system **validates, sanitizes, and applies** actions.
- Streaming: `applyAction(action: Streaming<CreateAction>, ...)` with `action.complete`.
- The model emits a **simplified shape format** (`shape._type`) which a util converts into a real shape
  record: `helpers.removeOffsetFromShapePartial(shape)` (translating from the model's coordinate space
  back to the page), `convertPartialFocusedShapeToTldrawShape(...)`, `getDefaultShape(type, complete)`.
- "The sanitization layer handles common LLM mistakes. It corrects shape IDs that don't exist, ensures
  new IDs are unique, and normalizes coordinates."

[INFERRED] Three separable layers are named explicitly here:
1. **Model-facing schema** (simplified, offset-local, tolerant of mistakes).
2. **Sanitisation/normalisation** (ID repair, uniqueness, coordinate rebasing).
3. **Editor-facing operations** (real records via `editor.createShape`).

A design that lets an LLM emit raw records directly will be fragile. This three-layer split is a
directly transferable pattern.

### The Driver — an editor that is its own input source

[DOCUMENTED — `docs/driver.mdx`, `@tldraw/driver`]

- "Pointer, keyboard, wheel, and pinch events all flow through `editor.dispatch`, so they go through the
  editor's **normal tool state machines**."
- Fluent, chainable: `driver.click(100,200).pointerDown(300,400).pointerMove(500,600).pointerUp()`.
- Per-event modifier override as the 4th argument: `driver.click(100, 100, shapeId, { shiftKey: true })`.
- Modifiers pressed with `keyDown` carry into later pointer events because the driver reads modifier
  state from `editor.inputs`.
- Each input method emits a tick; `forceTick(count)` for multi-frame work.
- Own clipboard, independent of the system clipboard.
- Registers a `sideEffects` handler that records **every shape created while attached, whatever created
  it** — `getLastCreatedShape()`, `getLastCreatedShapes(5)`.
- Selection helpers: `translateSelection(50,0)`, `rotateSelection(Math.PI/4)`,
  `resizeSelection({ scaleX: 2 }, 'bottom_right')`.

[INFERRED] **This is the single most important AI-adjacent architectural decision in tldraw**: the driver
simulates *user input*, not operations. An agent that dispatches synthetic pointer events runs the same
state machine, the same snapping, the same history marks, and the same bails as a human. The alternative
(operation-level agent API) skips interaction entirely and needs a parallel implementation of every
behavioural rule.

The `getLastCreatedShapes` side-effect recorder is also the answer to "how does the agent learn what
happened?" — the *system* reports the result rather than the agent assuming its intent succeeded.

### Mermaid diagrams

`docs/mermaid.mdx` (6KB) — turning model-generated diagram text (Mermaid) into tldraw shapes.
[DOCUMENTED] This is a **structured-generation** pipeline: text grammar → shapes. Notably more
constrained and more useful than freeform generation.

### Starter kits

| Kit | Purpose |
|---|---|
| Agent | Full agent system with visual context and canvas manipulation |
| Chat | Canvas for sketching/annotation as context for chat |
| Branching chat | Visual conversation trees |
| Workflow | Node-based visual programming incorporating AI operations |

### LLM docs

tldraw publishes `llms.txt`, `llms-full.txt`, `llms-docs.txt`, `llms-releases.txt`,
`llms-examples.txt`, and a copy-as-markdown button on every article. [DOCUMENTED —
`docs/llm-docs.mdx`] [INFERRED] For an AI-native product, publishing machine-readable documentation of your
own document model and operation API is a first-class requirement, not a nice-to-have.

## Architectural Observations

### The published decomposition

```
Editor
├── Store              (reactive, schema-validated record database)
│   ├── Query          (indexes + computed record lists)
│   ├── ComputedCache  (memoised derived values)
│   └── Snapshots      (serialise / migrate / load)
├── SideEffects        (record lifecycle hooks: validation, cleanup, derived state)
├── HistoryManager     (diffs + marks, bail, squash, three capture modes)
├── Inputs             (pointer, keyboard, wheel, pinch, drag/drop)
├── Camera             (zoom, bounds, screen bounds)
├── Root StateNode     → Tools (branch) → States (leaf)
│   └── dispatch(event) with target re-dispatch
├── Snaps
│   ├── BoundsSnaps    (translate / resize / gaps)
│   └── HandleSnaps    (point / align)
├── PerformanceManager (interaction-end, camera-end, per-shape counts)
├── InstanceState      (selection, current page, grid, tool lock, cursor, focus, screen info)
├── UserPreferences    (cross-instance settings)
└── Exports            (SVG, PNG, JSON)
```

### Evidence-backed concepts, with their Spool-relevant properties

| Concept | Problem solved | Persistent? | In history? | Affects render? | Affects input? | AI-relevant? | Evidence |
|---|---|---|---|---|---|---|---|
| **Record** | Uniform storage + validation + migration for every document thing | Yes | Yes | Yes | Yes | Yes — the agent's target | SOURCE-CODE / DOCUMENTED |
| **Record scope** (document/session/presence) | Decides persistence + sync + history participation | Yes (the scope itself is) | Only `document` | Partly | No | Yes | DOCUMENTED |
| **Store** | One mutation point → history, persistence, sync, reactivity | Yes | Yes | Yes | No | Yes | SOURCE-CODE |
| **Side effect** | Keep derived state consistent without polluting call sites | Writes are | Yes (writes are) | Yes | No | Yes | DOCUMENTED |
| **StateNode** | Model multi-phase interactions explicitly | No | No | No | **Yes — this is the input model** | Yes — tools are first-class objects | SOURCE-CODE |
| **Event target** | Reclassify a raw event by hit-testing, inside the chart | No | No | No | Yes | Yes | SOURCE-CODE |
| **Tool lock flag** | Persist-vs-return policy, decoupled from the machine | No (instance state) | No | No | Yes | Yes | DOCUMENTED |
| **Binding** | Persistent directional relationship with lifecycle | Yes | Yes | Yes | Yes | Yes | SOURCE-CODE |
| **Isolation hook** | Handle relationship loss from copy, not just delete | N/A | N/A | Yes | Yes | Yes | DOCUMENTED |
| **Fractional index** | O(moved) reorder + collision-free concurrent insert | Yes | Yes | Yes | Yes (z-order hit-testing) | Yes | SOURCE-CODE |
| **History mark** | Define "one undoable action" | No (stack entries) | It *is* history | No | Yes (tools place marks) | Yes | DOCUMENTED |
| **Bail** | Discard an interaction without polluting redo | No | Yes | No | Yes | Yes | DOCUMENTED |
| **Squash** | Merge many steps into one after the fact | No | Yes | No | Yes | Yes | DOCUMENTED |
| **History mode** (`record` / `preserve` / `ignore`) | Per-operation control of history participation | No | Yes | No | No | Yes | DOCUMENTED |
| **BoundsSnaps** | Edge/centre/gap alignment with zoom-normalised tolerance | No (transient) | No | Yes (indicators) | Yes | Yes | SOURCE-CODE |
| **HandleSnaps** | Point-to-outline/point connection | No | No | Yes (indicators) | Yes | Yes | SOURCE-CODE |
| **ShapeUtil** | Per-type geometry, snapping, culling, text, tabs, binding | No (code) | No | **Yes** | **Yes** | Yes | SOURCE-CODE |
| **Computed cache** | Memoise expensive derived geometry | No (derived) | No | Yes | Yes (hit tests) | Yes | DOCUMENTED |
| **Spatial index** | Viewport culling + fast hit tests | No (derived) | No | Yes | Yes | Yes | DOCUMENTED |
| **Debounced zoom** | Keep LOD stable during camera motion | No | No | Yes | No | No | DOCUMENTED |
| **steppedScreenScale** | Power-of-two asset LOD, debounced | No | No | Yes | No | No | DOCUMENTED |
| **Action + kbd string** | One registry for keys, menus, toolbars | No | No | No | Yes | Yes | DOCUMENTED |
| **InstanceState** | Per-session editor state, reactive | No | Partly — `document`-scope only for selection history via preserve-redo | No | Yes | Yes | DOCUMENTED |
| **Driver** | Simulate input to drive real state machines | No | Yes (marks are real) | Yes (culling) | **Yes** | **Yes** | DOCUMENTED |
| **Agent action schema** | Model-facing typed operations with sanitisation | No | Yes | Yes | No | **Yes** | DOCUMENTED |

### Explicitly not established / limitations

- React is the host UI framework; this is a **web** architecture. Do not copy it. The abstractions
  (state chart, store, side effects, marks) are portable; the renderer, signals implementation, and DOM
  culling strategy are not.
- No components/instances.
- No layout engine.
- No text model beyond rich-text marks on a shape.
- `copy`/`cut`/`paste` shortcuts cannot be remapped.
- Tool-lock policy is not enforced centrally.

## Important Interaction Conventions

- The select tool's state names are a *vocabulary*: `idle`, `pointing_canvas`, `pointing_shape`,
  `brushing`, `translating`, `resizing`, `rotating`. Naming states this way makes interaction logs
  readable and makes performance telemetry meaningful.
- "Brushing" rather than "marquee" — the operation is described by what it does.
- Creation tools return to `select` unless tool-locked.
- A click with a creation tool creates **nothing** (round-trips to idle).
- Snapping is **off** by default; Ctrl/Cmd snaps.
- Selection is filtered so a container and its contents cannot both be selected.
- Tab walks siblings; arrows walk by direction.

## Notable Differences From Other Products

See `matrices/interaction-matrix.md` for the full grid. Highlights specific to tldraw:

| Behaviour | tldraw | Why it differs |
|---|---|---|
| Tool persistence | returns to `select` unless locked | Figma shape tools stay active; Figma's behaviour is the inconsistent one |
| Click with a shape tool | no-op | Figma click places a default-size shape |
| Snap default | off; Ctrl/Cmd to snap | Figma/Affinity default on |
| Snap disable | Ctrl/Cmd when snap mode on | Figma uses Ctrl/Cmd always |
| Snap tolerance | screen px ÷ zoom (zoom-invariant feel) | Not documented in other products |
| Alt-drag mid-gesture | bails and re-applies in new mode | Others typically require pressing Alt first |
| Selection storage | instance state (session scope) | Not documented elsewhere |
| Select parent/child | focus group + explicit APIs | Figma uses an implicit scope stack |
| Undo granularity | marks placed by tools; bail and squash available | Others group internally, undocumented |
| Remote changes | never recorded in history | Figma collaborative undo semantics undocumented |
| Keyboard | single `kbd` string per action, layout-correct matching | Figma ships 16 discrete layouts |
| Export input | a set of shape IDs | Figma takes selection/frame/page/slice |
| Bring-forward | only past *overlapping* shapes | Figma/Canva use plain array order |

## Changelog / Evolution

| Change | Architectural signal |
|---|---|
| `@tldraw/editor` split out with **no built-in tools** | The state chart and store became independently consumable. This is a deliberate decoupling of *engine* from *product*. |
| `StateNode` as the single interaction abstraction | Tools, selection states, and multi-phase interactions unified under one mechanism. |
| **Bindings** introduced with from/to + lifecycle hooks | Relationships moved from ad-hoc shape code to a first-class record type. |
| **Isolation hooks** added separately from deletion hooks | A relationship system had to be generalised from "delete" to "separate". |
| **History modes** (`ignore`, `record-preserveRedoStack`) | Selection had to be undoable *without* clearing redo — a subtle requirement discovered by use. |
| **Bail** added for mid-drag mode switching | Modifier-with-memory was implemented correctly by undoing the partial gesture, not by snapshotting two futures. |
| **Squash** added for crop mode | Some interactions want *fine-grained* undo during the gesture and *coarse* undo after it. |
| **Fractional indexing** with jitter | Collaboration and O(moved) reorder forced a non-integer z-order. |
| **Computed caches** and **query indexes** | Derived geometry and hierarchy traversal were hot enough to need explicit memoisation. |
| **getEfficientZoomLevel / debouncedZoom** | Per-frame zoom changes were visibly janking large documents. |
| **steppedScreenScale** | Image resolution requests were unbounded; powers of two made them cacheable. |
| `canSnap`, `getBoundsSnapGeometry`, `getHandleSnapGeometry`, `canCull`, `canBind`, `canTabTo`, `getText` | The core grew *hooks* rather than growing conditionals — the same extensible design as `overrides`. |
| **`@tldraw/driver`** package | The editor became driveable by tests, scripts, and agents. |
| **AI docs + starter kits + llms.txt** | AI became a first-class integration target with shipped reference architectures. |
| `snapType: 'point' \| 'align'` deprecating `canSnap` on handles | Handle snapping needed two qualitatively different behaviours; one boolean could not express them. |
| Frame rebuild ("You can now frame your shapes by drawing around them with the frame tool") — THIRD-PARTY | Frames moved from a property to a creation gesture. |
| Tidy-up / smart selection removed from tldraw in favour of explicit arrange actions | Moving away from "magic" layout inference toward explicit operations. |

## Sources

**Tier 1 — Official documentation (tldraw.dev + in-repo `apps/docs/content`)**

Retrieved directly from the repository at `main` (raw.githubusercontent.com), which is the authoritative
documentation source:

- `docs/editor.mdx` — Editor capabilities table
- `docs/tools.mdx` — Tools (state chart overview)
- `docs/ai.mdx` — AI integrations (agents, workflows, canvas-as-output)
- `docs/driver.mdx` — Driving the editor
- `docs/llm-docs.mdx` — LLM documentation
- `docs/mermaid.mdx` — Mermaid diagram ingestion
- `sdk-features/tools.mdx` — Tools (full StateNode reference) ⭐ primary source
- `sdk-features/store.mdx` — Store (records, scopes, queries, caches, snapshots, transactions) ⭐
- `sdk-features/selection.mdx` — Selection (rules, bounds, traversal, focus) ⭐
- `sdk-features/snapping.mdx` — Snapping (BoundsSnaps, HandleSnaps, gaps, indicators) ⭐
- `sdk-features/history.mdx` — History (marks, modes, bail, squash) ⭐
- `sdk-features/actions.mdx` — Actions (kbd strings, layout-correct matching, overrides) ⭐
- `sdk-features/instance-state.mdx` — Instance state
- `sdk-features/bindings.mdx` — Bindings and lifecycle hooks ⭐
- `sdk-features/shape-indexing.mdx` — Fractional indexing
- `sdk-features/culling.mdx` — Culling
- `sdk-features/performance.mdx` — Performance techniques, LOD, options, measurement ⭐
- `sdk-features/shapes.mdx` — Shapes and ShapeUtil hooks
- `sdk-features/handles.mdx` — Handles
- `sdk-features/geometry.mdx` — Geometry
- `sdk-features/shape-transforms.mdx` — Shape transforms
- `sdk-features/input-handling.mdx` — Input handling
- `sdk-features/signals.mdx` — Reactive signals
- `sdk-features/side-effects.mdx` — Side effects
- `sdk-features/parenting.mdx`, `groups.mdx`, `pages.mdx`, `assets.mdx`, `rich-text.mdx`,
  `text-measurement.mdx`, `scribble.mdx`, `eraser.mdx`, `pen-mode.mdx`, `image-export.mdx`,
  `frame-shape.mdx`, `geo-shape.mdx`, `default-shapes.mdx`, `clipboard.mdx`, `focus.mdx`,
  `user-preferences.mdx`, `options.mdx`, `styles.mdx`, `themes.mdx`, `animation.mdx`, `camera.mdx`,
  `coordinates.mdx`, `events.mdx`, `errors.mdx`, `readonly.mdx`, `locked-shapes.mdx`, `grid.mdx`,
  `visibility.mdx`, `validation.mdx`, `drag-and-drop.mdx`, `accessibility.mdx`, `internationalization.mdx`,
  `collaboration.mdx`, `sync.mdx`, `cursors.mdx`, `commenting.mdx`, `cross-tab-sync.mdx`,
  `user-following.mdx`, `shape-clipping.mdx`, `draw-shape.mdx`, `arrow-shape.mdx`, `note-shape.mdx`,
  `embed-shape.mdx`, `highlighting.mdx`, `indicators.mdx`, `click-detection.mdx`, `edge-scrolling.mdx`,
  `overlay-utils.mdx`, `license-key.mdx`, `external-content.mdx`, `cursor-chat.mdx`, `attribution.mdx`,
  `environment.mdx`, `instance-state.mdx`, `deep-links.mdx`, `ui-components.mdx`, `ui-primitives.mdx`,
  `update*`, `actions.mdx` — identified but not all read in this pass.
- https://tldraw.dev/llms.txt and sibling bundles (referenced, not fetched)

**Tier 2 — Official engineering**: tldraw maintainer essays (tldraw.substack.com) were surfaced but only
used for the frames-rebuild evolution note. **Limited in this pass.**

**Tier 3 — Source code** ⭐

The documentation *is* generated from an in-repo mdx corpus and describes the real implementation. Key
source locations referenced by the docs:

- `packages/tlschema/src/shapes/TLArrowShape.ts` (migrations per shape type)
- `packages/tldraw/src/lib/ui/context/actions.tsx` (the ~100 default actions)
- `@tldraw/utils` — vendored jittered fractional indexing
- `@tldraw/validate` — `T` validators including `indexKey`

**Tier 4 — Third-party**

- BigGo podcast, "Bringing Agents to the Canvas — Max Drake, tldraw" (Sep 2026) — used only for the
  maintainer's design-principle quote: "if your harness is getting more complex as models improve, you are
  overengineering it." Marked THIRD-PARTY.

**Tier 5 — Community**

- tldraw.substack.com "What's new in tldraw" — used for the frames-rebuild evolution note.

**Research gaps for tldraw**

- `sync.mdx` (23KB) — the largest and most important collaboration document — was **not read** in this
  pass. Conflict resolution and merge semantics remain **Unknown**.
- `parenting.mdx` — reparenting coordinate semantics not extracted.
- `text-shape.mdx` / `rich-text.mdx` / `text-measurement.mdx` — text model not extracted in detail.
- The signals implementation (fine-grained reactivity) was documented but not source-read; whether a
  non-web host can adopt it is **Unknown** from this research.
