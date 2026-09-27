# Spool — Implementation Plan

## Status

Phases 1–3 corrected; Phase 4 — Puzzle exit implemented

## Important rule

Implement one phase at a time. Do not skip ahead or build future features while completing the current phase. After each phase, run the application, visually inspect it, fix obvious issues, report what changed, and wait for confirmation before starting the next major phase.

## Phase 0 — Foundation

**Goal:** Establish the project structure and visual foundation.

Tasks:
- Inspect the existing Next.js project and preserve configuration where reasonable.
- Establish global typography and color tokens.
- Establish the background texture system.
- Establish a basic viewport/camera container.
- Establish reduced-motion handling and responsive foundations.

Do not build the full website, puzzle loader, or world yet.

**Acceptance:** The project has a sound visual foundation without implementing later experience phases.

## Phase 1 — Void

**Goal:** Create the opening state.

Requirements: The viewport is controlled by the experience; it begins as a neutral/dark void with no visible conventional webpage; a subtle transition begins revealing the paper/fabric canvas; the texture feels physical.

**Acceptance:** The opening feels intentionally empty.

## Phase 2 — Paper canvas

**Goal:** Reveal the physical canvas.

Requirements: Warm paper/fabric surface; subtle grain and tonal variation; no obvious digital gradient; performant texture.

**Acceptance:** The background looks closer to physical paper than CSS UI.

## Phase 3 — Puzzle loading

**Goal:** Create the temporary assembly sequence.

Requirements: 12–24 deterministic puzzle silhouettes appear in a deliberate sequence at their final positions across the viewport. The underlying landing-page world is independently positioned DOM content. A temporary paper veil is cut away by the piece silhouettes to reveal that world progressively. Pieces use real world content, subtle opacity/scale entrances, and do not travel toward one another or form a separate central image.

**Important:** This sequence is temporary. Do not allow puzzle elements to remain after Phase 3.

**Acceptance:** The visitor recognizes that the real Spool landing page is being revealed in place, not assembled as a standalone jigsaw image.

## Phase 4 — Puzzle exit

**Goal:** Remove the puzzle composition cleanly.

Requirements: After the complete landing-page composition has been visible briefly, every temporary piece exits with a short, coordinated lift/fade/stagger. Only the reveal layer disappears; the independent landing-page world and paper canvas remain intact. The explicit end state is ready-for-thread, with no thread, camera pullback, navigation, or puzzle decoration.

**Acceptance:** The clean landing-page world remains visible with zero puzzle pieces, ready for Phase 5.

## Phase 5 — Thread logo

**Goal:** Reveal the Spool wordmark using black thread.

Requirements: Verify the candidate font at `public/fonts/wobble.ttf` against the Spool wordmark in `public/images/spool idea thingy.png`; use the exact supplied logo typeface/vector only. The thread enters the canvas and travels along the logo path; the wordmark is progressively revealed; the motion feels physical; the thread resolves into a crisp logo.

Technical direction: Prefer SVG path animation using techniques such as `stroke-dasharray` and `stroke-dashoffset`, or Motion/SVG animation where appropriate.

Do not fake this with a generic text fade or substitute font. If the candidate font does not match the supplied wordmark, report the blocker rather than approximating it.

**Acceptance:** The visitor can clearly perceive the thread drawing “Spool.”

## Phase 6 — Camera pullback

**Goal:** Reveal the larger creative world.

Requirements: The logo completes; the camera smoothly pulls backward; the larger world becomes visible; the transition feels spatial rather than like page navigation.

**Acceptance:** The visitor realizes the logo was sitting inside a much larger creative canvas.

## Phase 7 — World model

**Goal:** Create the draggable 2D world.

Conceptual camera:

```ts
type Camera = { x: number; y: number; scale: number }
```

Conceptual world item:

```ts
type WorldItem = {
  id: string
  x: number
  y: number
  rotation: number
  scale: number
  type: string
}
```

Requirements: The world is larger than the viewport; artifacts are independently positioned DOM elements; pointer dragging uses pointer capture; movement is smooth; world boundaries are sensible; dragging does not accidentally select text.

**Acceptance:** The visitor can grab the canvas and explore it.

## Phase 8 — First artifacts

**Goal:** Populate the world with real visual content.

Create a small curated set first, with these categories: generated website, poster, photograph, terminal/model output, typography/design experiment, handwritten note, and UI concept. Do not create dozens of artifacts. Quality over quantity.

**Acceptance:** The world visually communicates what Spool can create.

## Phase 9 — Scroll navigation

**Goal:** Add guided navigation while retaining free exploration.

Create predefined scene anchors, conceptually `hero`, `features`, `useCases`, `gallery`, and `about`. Scrolling smoothly moves the camera toward anchors; it does not lock visitors into a linear path; the current area may be communicated subtly.

**Acceptance:** Scrolling feels like traveling through the canvas, not scrolling a document.

## Phase 10 — Navigation and product information

**Goal:** Add minimal conventional UI around the world.

Potential controls: Spool, Gallery, Features, Use Cases, Docs, GitHub, and Download. These must not overpower the visual world.

## Phase 11 — Polish

**Goal:** Apply the design-engineering skill rigorously.

Review easing, duration, interruption behavior, pointer capture, drag boundaries, reduced motion, touch behavior, hover and press states, typography, spacing, texture, layering, shadows, image loading, and performance. Review major animations at slow speed. Fix anything mechanical or accidental.

## Phase 12 — Responsive

**Goal:** Make the experience usable across viewport sizes.

Desktop remains the primary art-directed experience. Mobile preserves the paper texture, visual identity, Spool wordmark, artifact exploration, and guided navigation. Do not simply scale down the desktop composition.

## Phase 13 — Final review

Verify that no puzzle pieces remain after loading; the thread logo works and uses supplied Spool branding; the world is draggable; scrolling navigates scenes; artifacts are independently rendered; no generic SaaS sections or unauthorized visual direction appeared; reduced motion works; there are no major performance problems or console errors; TypeScript passes; and the production build passes.

Only after this phase is the initial website considered complete.
