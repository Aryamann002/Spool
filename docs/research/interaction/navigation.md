# Interaction: Navigation

Navigation = camera (zoom/pan), viewport framing, zoom-to-fit, zoom-to-selection, page navigation, and
the auxiliary spatial aids (rulers, guides, grids, minimaps).

## Camera as first-class state

| Product | Camera model | Documented surface |
|---|---|---|
| Figma | Infinite canvas; zoom + pan | Keyboard: `+`/`-`, `Cmd/Ctrl+` + `+`/`-`; Space+drag; `Shift 1` / `Shift 2` (community) |
| tldraw | **`editor` camera** with zoom, bounds, screen bounds, and separate shape/page coordinate spaces | `sdk-features/camera.mdx`, `sdk-features/coordinates.mdx` |
| Affinity | Bounded document; page/spread scrolling; canvas zoom | `⌥⌘C` Resize Canvas, `⌥⌘I` Resize Document; `⇧⌘P` Document Setup |
| Canva | **Bounded pages**; scrolling / thumbnail / grid / presentation views | `⌘+`/`⌘-`, `⌘0` actual size, `⌥⌘0` zoom to fit, `⇧⌘0` zoom to fill, `⌥⌘1/2/3` view modes, `⌥⌘P` presentation |

## Coordinate spaces

tldraw is the only product that **names** its coordinate spaces and provides a documented conversion
module. [DOCUMENTED — `sdk-features/coordinates.mdx`]

| Space | Meaning |
|---|---|
| **Screen** | CSS/viewport pixels; what the user sees |
| **Page** | Document space; what shapes store their `x`/`y` in |
| **Shape local** | Space relative to a shape's own origin and rotation |
| **Camera** | Zoom + pan offset |

Conversion API: `editor.pageToScreen`, `screenToPage`, and equivalents. The `driver` uses them: "Pointer
coordinates are in screen space. For page-space positions, use `editor.pageToScreen` to convert before
dispatching." [DOCUMENTED]

[INFERRED] Naming the spaces explicitly is essential and cheap. Spool's prototype has exactly two
(`world_to_screen` / `screen_to_world`) plus an ad-hoc `screen_to_object_local` helper. The missing one
is **shape-local**, which will be needed immediately for rotation and for text caret positioning under
rotation. The prototype already has `screen_to_object_local` — so the concept is present but un-named.

## Zoom-to-fit and zoom-to-selection

| Product | Fit | Selection | Other |
|---|---|---|---|
| Figma | `⇧1` (community-documented) | `⇧2` (community-documented) | `⇧0` = 100% |
| tldraw | **`zoom-to-fit` action** (DOCUMENTED, in the default action list) | **`zoom-to-selection` action** (DOCUMENTED) | `zoom-to-100`, `zoom-in`, `zoom-out`, `select-zoom-tool` |
| Affinity | Not documented in pages read | Not documented | — |
| Canva | **`⌥⌘0` Zoom to fit** (DOCUMENTED) | Not documented | **`⌘0` Zoom to actual size**, **`⇧⌘0` Zoom to fill** |

[INFERRED] Zoom-to-fit and zoom-to-selection are **universal expectations** for a design editor, and
both Figma and tldraw bind them to keys. Spool's prototype has neither — `Camera` has
`world_to_screen`, `screen_to_world`, a `resize()` that re-centres on viewport size change, and a
`fit` calculation using hard-coded `WORLD_BOUNDS`. [OBSERVED in source: `const WORLD_BOUNDS:
Size<f32> = size(764.0, 688.0); const WORLD_CENTER: Point<f32> = point(382.0, 344.0);`]

That hard-coded world bounds is a prototype affordance, but it means "zoom to fit" currently means "zoom
to the demo scene", not "zoom to the content".

## Pan

| Product | Middle mouse | Space+drag | Trackpad | Wheel |
|---|---|---|---|---|
| Figma | Yes (OBSERVED) | Yes | Two-finger scroll | Vertical scroll; `⇧`+wheel horizontal |
| tldraw | **Hand tool** is a first-class tool; `onMiddleClick` is an event handler | `onPointerMove` in the hand tool's dragging state updates the camera | — | `onWheel` is an event handler |
| Affinity | Not documented | Not documented | Not documented | Not documented |
| Canva | Not documented | Not documented | Native scrolling view | Native |

tldraw's approach is architecturally notable: **pan is a tool, not a modifier.** [DOCUMENTED — the hand
tool "implements `onPointerMove` to update the camera position as the user drags"] The docs also list
`onMiddleClick` and `onWheel` as distinct handler events on every state node.

[INFERRED] A "hand tool" is more discoverable than a hidden space-drag, more composable with the state
chart, and gives a natural place for pan-related affordances (pan-to-cursor, edge scrolling —
tldraw documents `sdk-features/edge-scrolling.mdx` separately). Spool's prototype uses
`button == Middle || (button == Left && self.space_held)` [OBSERVED in `begin_pan`] — the modifier
approach.

Both are defensible. **Having both** (a Hand tool *and* space-drag) is what most editors do.

## Arrow-key navigation: two different meanings

| Situation | Figma | tldraw | Canva |
|---|---|---|---|
| **Nothing selected** | Arrow keys **pan the canvas**; `⇧`+arrow pans faster, distance varies with zoom | Not documented (camera changes) | Not documented |
| **Selection exists** | Arrow keys **nudge the selection**; `⇧`+arrow uses big nudge | `nudgeShapes` | Arrow moves element small, `⇧`+arrow large |
| **Cardinal selection** | Not documented | `selectAdjacentShape('left'\|…)` on `Cmd/Ctrl+Arrow` | `⇧W/A/S/D` = multi-select closest in direction |

[INFERRED] Figma's dual meaning on the same keys — pan when nothing is selected, move when something is —
is elegant and worth copying. It is also a good demonstration that key handling must be a **function of
editor state**, not a static table.

## View modes

Canva is the only product that treats "view mode" as a first-class, key-bound set. [DOCUMENTED]

| Binding | Mode |
|---|---|
| `⌥⌘1` | Scrolling view |
| `⌥⌘2` | Thumbnail view |
| `⌥⌘3` | Grid view |
| `⌥⌘P` | Presentation mode |

[INFERRED] This is a small but real "interaction profile" seam: view modes change what is visible and which
commands apply, without changing the document. tldraw has `toggle-focus-mode` (`⌘./⇧⌘.`) as an action.
[DOCUMENTED]

## Spatial aids

| Aid | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| **Rulers** | Not documented as a toggle | Not documented | Yes (help index) | **Yes — `⇧R`** |
| **Guides** | Yes ("Layout guides" `⇧G`) | Not documented as a document-level feature | **Yes** — guides, margins, spreads, baseline grid | **Yes** — `⇧R` toggles rulers+guides, `⌥⌘;` locks guides |
| **Grid** | Pixel grid `⇧'`; **snap-to-pixel-grid works even when invisible** | `isGridMode` instance flag | Grid, uniform grid, baseline grid (Layout Studio) | Part of guides |
| **Minimap** | **No** | **No** | **No** | **No** |
| **Margins / bleeds** | No | No | **Yes** | No |
| **Sections** | **Yes** — top-level organisational containers | Pages instead | No | No |

[INFERRED] **None of the four products have a minimap.** That is a notable negative finding: minimaps are
common in map editors and game engines but absent from all four design tools. Spool should not add one
without a strong argument.

[INFERRED] Figma and tldraw solved the same need with different mechanisms:
- Figma: **Sections** — persistent, named, top-level spatial groupings that can be collapsed and moved.
- tldraw: **Pages** — persistent, top-level partitions of the document.

Both are *persistent document* concepts, not camera concepts. "Where am I" is partly a camera question and
partly a document-navigation question, and the products answer the second with document structure.

## Page / document navigation

| Product | Mechanism |
|---|---|
| Figma | Pages; `Ctrl+1` opens Layers; sections; `⌥⌘G` in Canva (not Figma) |
| tldraw | **Pages as store records**; `currentPageId` in instance state; session-scoped |
| Affinity | Pages/spreads, document-bounded |
| Canva | **`⌥⌘G` Go to page**, `⌘↵` add empty page, `⌘⏎` add page break, `⌘Delete` delete empty page, pages reorderable |

[INFERRED] Canva's `⌥⌘G` "Go to page" is an explicit, discoverable page-navigation command. With many
pages, some form of page switching/picker is required; only Canva and tldraw document one.

## Zoom-dependent behaviour

| Behaviour | Product | Detail |
|---|---|---|
| Arrow-key pan distance scales with zoom | Figma | "Hold Shift while pressing the arrow keys to increase the pan distance […] The distance changes based on your current zoom level." |
| Snap tolerance ÷ zoom | tldraw | Screen px ÷ zoom |
| **Debounced zoom for rendering** | tldraw | `getEfficientZoomLevel()`; stable above `debouncedZoomThreshold` shapes |
| Pixel grid visibility signals zoom level | Figma | Turn on `⇧'` and zoom until visible |
| Dot size clamped in screen space | Spool prototype | `let dot_size = (1.5 * self.camera.zoom).clamp(1.0, 2.0);` |

[DOCUMENTED except Spool, which is OBSERVED in source]

[INFERRED] The zoom-normalisation theme appears in every product but in different subsystems: input
distance, snap tolerance, render LOD, and visual chrome. A single helper — "convert a screen-space
constant to world space" — would unify all four. Spool's prototype already does this ad-hoc via the
`px!($value, $zoom)` macro.

## Edge scrolling

tldraw has a dedicated `sdk-features/edge-scrolling.mdx` module. [DOCUMENTED at module level]

[INFERRED] Auto-scroll when dragging near a viewport edge is a *long-press-or-drag* interaction feature
that requires knowing the pointer is inside the canvas but outside the content region. It interacts with
the state chart (which state is active) and with camera mutation. Worth noting as a known
long-session-infinite-canvas feature; not documented in enough detail here to specify.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

```rust
pub struct Camera {
    offset: Point<f32>,
    zoom: f32,
    viewport: Size<f32>,
    initialized: bool,
}
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
const WORLD_BOUNDS: Size<f32> = size(764.0, 688.0);
const WORLD_CENTER: Point<f32> = point(382.0, 344.0);
```

- `world_to_screen` / `screen_to_world`. Two spaces only.
- `Camera::resize(viewport)` — on first call, centres on `WORLD_CENTER`; on subsequent calls, **adjusts the
  offset so the centre stays fixed** (`offset.x -= (viewport.width - self.viewport.width) / (2.0 *
  self.zoom)`). This is a nice, deliberate "keep the centre stable on window resize" behaviour.
- Zoom clamp 0.1–4.0 — a **narrow range**. Mature editors go far wider (tldraw and Figma both reach very
  high zoom for pixel work).
- `zoom_at(factor)`-style logic at line ~124: `self.zoom = (self.zoom * factor).clamp(MIN_ZOOM,
  MAX_ZOOM);`
- Initial fit at line ~143: `self.zoom = (available.width / WORLD_BOUNDS.width)...`
- Pan: middle mouse or space+left.
- `on_scroll_wheel` handler present.
- Background grid drawn with zoom-adaptive spacing.
- **No zoom-to-selection. No zoom-to-content. No pages. No guides. No rulers. No minimap. No view modes.**
- Wheel handler behaviour not verified in this pass — **Unknown** whether it zooms or scrolls.

## Candidate architectural implication

**Evidence:**

1. Coordinate spaces must be named and converted explicitly; at minimum screen, page, and shape-local.
   [tldraw DOCUMENTED]
2. Zoom-to-fit and zoom-to-selection are universal, key-bound expectations. [DOCUMENTED in Figma,
   tldraw, Canva]
3. Key handling must be a **function of editor state** (arrow keys pan when nothing is selected, move when
   something is). [Figma DOCUMENTED]
4. Zoom normalisation is needed in at least four subsystems (input, snapping, rendering, chrome).
   [DOCUMENTED across products]
5. Rendering must use a **debounced zoom** distinct from the true zoom. [tldraw DOCUMENTED]
6. Document partitioning (pages / sections) is a **document** concept, not a camera concept.
   [DOCUMENTED]
7. No product has a minimap. [OBSERVED]
8. Pan-as-tool and pan-as-modifier coexist in practice. [tldraw + Figma]

**Why it matters:** The camera is the second-most-touched subsystem after selection, and getting zoom
normalisation wrong produces subtly wrong behaviour in snapping, LOD, stroke widths, and input distances.

**Potential Spool approaches:**

- **A. Keep two spaces; add shape-local when rotation arrives.** Minimal.
- **B. Formalise a `CoordinateSpace` enum with typed conversions.** Prevents mixing screen and world
  coordinates — a class of bug that is very hard to find later.
- **C. Add zoom-to-selection / zoom-to-content as named operations from the start.** Low cost, high value,
  and required by any future operation API ("show me these objects").
- **D. Camera state in editor state, explicitly outside history.** All four products do this.
- **E. Add a Hand tool alongside space-drag.** tldraw's approach; better discoverability.

**Tradeoffs:** B is cheap insurance against a common bug class and costs nothing but discipline. C is
essentially free and is a prerequisite for good AI operation ergonomics. D is not really a choice — no
product records camera in undo.

**Decision: TBD — requires architecture review.** Note: camera should **not** participate in history
(all products agree), but zoom-to-selection *should* be a named operation that an agent can request.

## Open questions

1. Does the wheel zoom or scroll? (Spool prototype: unverified.)
2. What is Spool's zoom range? 0.1–4.0 is narrow for pixel-precise work.
3. Does zoom anchor at the cursor? (Not verified in the prototype.)
4. Is there a Hand tool, or space-drag only?
5. Does the camera participate in history? (Evidence says no, in all products.)
6. Does Spool need pages or sections, and when?
7. Should there be a minimap? (No product has one.)

## Sources

- tldraw: `sdk-features/camera.mdx`, `sdk-features/coordinates.mdx`, `sdk-features/tools.mdx` (hand tool,
  `onWheel`, `onMiddleClick`), `sdk-features/performance.mdx` (debounced zoom), `sdk-features/pages.mdx`,
  `sdk-features/edge-scrolling.mdx`, `sdk-features/actions.mdx` (zoom actions), `sdk-features/grid.mdx`,
  `sdk-features/instance-state.mdx`, `sdk-features/visibility.mdx`
- Figma: "Use Figma products with a keyboard" (/360040328653); "Select keyboard layout" (/5665442977431)
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/); "Snapping"
  (/design-aids-snapping/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`Camera`, `MIN_ZOOM`, `MAX_ZOOM`, `WORLD_BOUNDS`,
  `WORLD_CENTER`, `begin_pan`, `on_scroll_wheel`, `screen_to_object_local`, `px!` macro, grid drawing)
