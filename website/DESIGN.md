# Spool — Design Direction

## One sentence

Spool is an interactive creative canvas where visitors explore the things a local AI design agent can create. The website should feel less like a webpage and more like walking into a physical designer's workspace.

## Core metaphor

Spool is about taking an idea and turning it into visual form. The website expresses that idea physically: a blank canvas appears; pieces of the canvas are discovered; the canvas becomes complete; a thread draws the Spool identity; then the visitor explores the resulting creative world. The website itself becomes an example of what Spool can do.

## Experience

The experience moves through these states:

### 01 — Void

The visitor initially sees a dark or neutral empty space. There should be almost nothing visible. The emptiness creates anticipation; do not immediately reveal the full website.

### 02 — Canvas forms

A warm paper/fabric texture gradually appears. It should feel physical rather than like a digital gradient: paper fibers, subtle grain, slight tonal variation, an imperfect surface, and almost imperceptible movement. Avoid obvious animated noise.

### 03 — Puzzle loading

Puzzle pieces appear in a deliberate sequence at their final positions across the viewport. Each physical silhouette briefly cuts through a paper veil to reveal the actual, independently positioned landing-page world underneath: photographs, typography, website and interface compositions, notes, diagrams, palettes, and other artifacts. They do not travel toward one another or assemble into a separate image; the composition assembles over time as the real world is revealed.

**Critical:** The puzzle pieces are not part of the final website. When the loading composition is complete, every puzzle piece disappears. No puzzle-piece cards, borders, or decorations remain. The only remnant is the conceptual idea that something was assembled.

### 04 — Thread reveal

After the puzzle composition disappears, a black thread enters and physically travels across the canvas to draw the Spool wordmark. Use the exact supplied Spool logo typography or vector; do not approximate it. The thread should have organic curves, subtle physicality, and smooth acceleration/deceleration rather than robotic linear movement. It eventually resolves into the complete, crisp logo.

### 05 — World reveal

After the logo completes, the camera pulls back. The visitor discovers that the logo exists inside a much larger canvas containing examples of what Spool can generate. The immediate impression should be: “These are things this tool can make.”

## Visual world

The world should feel like a designer's desk, editorial moodboard, physical studio wall, scanned design archive, creative notebook, or experimental technology lab. It should not feel like a SaaS dashboard, Figma clone, AI startup landing page, Pinterest clone, or generic portfolio.

## Material and palette

The primary surface is warm off-white or aged paper. The palette direction is warm ivory, cream, paper white, soft gray, charcoal, black, and muted dusty blue. Do not lock exact hex values prematurely; derive the final palette from supplied references. The surface should have subtle texture and tonal variation, not an obvious digital gradient.

## Typography

Typography is a primary visual element. Use a clean contemporary sans-serif for interface and body text, and a small monospace face for technical annotations where appropriate. Use only the supplied Spool logo typeface/vector for the wordmark. Large typography should have confidence and space; contrast large editorial statements with small technical annotations.

## Imagery and artifacts

Imagery should feel collected rather than generated as one coherent stock-photo set. Appropriate material includes photographs, scanned paper, screenshots, website mockups, posters, illustrations, diagrams, terminal windows, UI fragments, handwritten notes, and typography experiments.

Artifacts may overlap, have slightly different rotations, and use subtle shadows. They should feel physically placed onto the canvas. The visual world should not be mathematically sterile: use subtle rotation, overlap, texture, grain, uneven spacing, handwritten annotations, paper edges, and hierarchy. Imperfection should be art-directed, not random for its own sake.

## Spatial composition

The world is larger than the viewport, so visitors see only a portion at a time. Important areas need deliberate composition. The primary composition reference is the generated Spool concept board; it establishes the paper canvas, scattered artifacts, large Spool wordmark, editorial composition, and interactive-world concept. Secondary references establish a vocabulary of puzzle composition, retro technology, experimental editorial typography, photography, texture, collage, and Desert Ant Labs' restraint and product presentation. Do not copy references directly; extract their visual principles.

Conceptual spatial arrangement:

```text
                    GALLERY

       generated website

                         poster

              SPOOL

    photograph                 terminal

                     generated UI

          FEATURES

                         USE CASES
```

These positions are conceptual; compose the actual world deliberately during implementation.

## Interaction and scroll

Visitors can drag anywhere on the canvas to move the camera through the world. The world may have subtle inertia/physicality. It should feel like moving a large sheet of paper, not a webpage.

Scrolling is guided navigation toward predefined areas such as HERO, FEATURES, USE CASES, GALLERY, and ABOUT; it should not simply reveal normal vertical sections. Visitors retain free dragging and should feel the camera traveling through a larger space.

## UI

Navigation remains minimal and quiet so it does not compete with the canvas. Possible persistent controls include the Spool logo, GitHub, Download, and Docs.

## Motion personality

Motion should feel physical, elegant, slightly playful, calm, intentional, and responsive. Avoid excessive bounce, generic springs everywhere, fast UI gimmicks, constant movement, gratuitous parallax, and glowing effects. The opening sequence may be cinematic because it is a rare first-time experience.

## Anti-patterns

Never introduce AI purple gradients, AI blue glow, glassmorphism, 3D blobs, floating gradient orbs, generic SaaS card grids, “hero + 3 feature cards + testimonial,” stock-photo people smiling at laptops, excessive rounded cards, or huge animated gradient text. If something looks like a generic AI startup website, it is probably wrong for Spool.

## Assets

A candidate wordmark font exists at `public/fonts/wobble.ttf`; verify it against the Spool wordmark in `public/images/spool idea thingy.png` before treating it as the exact logo typeface. Do not approximate the wordmark or substitute another font. The existing visual references are in `public/images/` (including the Spool concept board, puzzle collage, retro-tech/editorial references, and Desert Ant Labs reference). Preserve these supplied files; do not move or rename them without a reason. Paper/grain textures belong under `public/textures/`.
