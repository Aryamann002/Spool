# Creative: Raster & Image Editing

## The scope question

Spool must distinguish what is **fundamental to a design editor** from what belongs to a **full raster
editor**. This document draws that line based on evidence.

## What each product includes

| Capability | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Place an image | Yes | Yes (`image` shape) | Yes | Yes (Uploads, Elements) |
| Crop | Yes | Yes (`crop` tool states documented: `crop.pointing_crop_handle`) | Yes (Cropping tool) | Yes (element context menu) |
| Mask | Yes (`Mask` command) | Yes (`shape-clipping`) | Yes (Clipping mask `⇧C`) | Yes (frame masking for elements) |
| Adjustments (non-destructive) | Image adjustments on the fill | Not documented | **Adjustment layers** (Brightness/Contrast, Curves, HSL, Levels, Black & White, Invert) | Yes (Magic Edit, adjustments) |
| Filters | Yes (blur, noise) | Not documented | **Smart Filters** (Live filters) | Yes |
| Background removal | **Yes** (Magic Background/remove background) | No | **Background Removal tool** | **Yes** (AI) |
| Liquify / warp | No | No | **Liquify, Warp, Tone Mapping** | No |
| RAW processing | No | No | **Develop Studio** (full RAW) | No |
| Bokeh / panorama | No | No | Yes | No |
| Pixel selection tools | No | No | Marquee, Elliptical, Lasso, Polygon, Magnetic, **Flood Select** | No |
| Content-Aware Fill / Healing / Patch | No | No | Yes | No |
| Blend modes | Yes | Documented | **27 modes** incl. non-standard Pigment | Unknown |
| Slices / multi-format export | Yes | Yes (shape-list export) | **Slice Studio** | Per-design export |

[DOCUMENTED]

## The dividing line

[INFERRED] Three tiers:

### Tier 1 — fundamental to a design editor (every product has it)

| Capability | Why it's fundamental |
|---|---|
| Place an image | Design documents are made of images |
| Crop / fit | Cropping is how an image is composed into a layout |
| Mask / clip | Masking is how an image is composed into a shape |
| Scale / rotate | Trivial |
| Opacity | Universal |
| **Non-destructive adjustment** | The designer's core mental model: change it freely, it stays editable |
| **Resolution-aware loading** | Performance; see `document/assets.md` |
| Export to raster | Universal deliverable |

### Tier 2 — valuable, product-dependent

| Capability | Who has it | Notes |
|---|---|---|
| Filters/blur | Figma, Affinity, Canva | Blur is a *design* primitive (backdrop, depth), not just an image effect |
| Background removal | Figma, Affinity, Canva | Now expected; also an **AI operation** |
| Masks with vector shapes | All four | Common in UI design |
| Content-aware fill | Affinity | Useful for photo compositing; rarer in design work |

### Tier 3 — full raster editor, not a design tool

| Capability | Who has it |
|---|---|
| RAW processing pipelines | Affinity only |
| Liquify / content-aware move / healing | Affinity only |
| Histogram / curve / HSL adjustment layers | Affinity only |
| Pixel selection tools (lasso, magic wand, flood select) | Affinity only |
| Tone mapping, astrophotography, colour grading studios | Affinity only |

[INFERRED] **Every Tier 3 capability is Affinity-only.** Not one of the other three products ships a pixel
editor. This is strong evidence that Spool, aiming at "general-purpose design application" with Figma-like
familiarity, **does not need Tier 3** to succeed — and that adopting it would be a strategic error, not a
feature.

## Non-destructive editing is the load-bearing principle

[DOCUMENTED — Affinity]

- **Adjustment layers** — a layer that applies an adjustment to everything below it.
- **Smart Filters** — live, non-destructive filters; `⌘X` (`⇧⌘X`) toggles Live Liquify.
- `⌃⌘V` **Paste FX** — effects have their own clipboard path, **separate from Paste Style**
  (`⇧⌘V`). Two distinct "apply appearance" operations.

[INFERRED] **Effects and styles being separate clipboard operations is architecturally meaningful.**
- *Style* = identity ("this is a Button/Primary style").
- *Effect* = a stack of adjustments ("shadow, then outer glow, then 40% blur").

Collapsing them into one "appearance" would prevent applying effects without a style, which is a common
design need.

[INFERRED] **Adjustment layers imply a re-evaluation graph.** If you change an adjustment's value, every
pixel beneath it changes — and if an object is *on* the adjustment layer, it must be composited through it.
That is a render-graph concern, not a per-object property.

### AI-native raster: Canva and Figma both landed on "brush + prompt"

| Product | Capability | Evidence |
|---|---|---|
| Canva | **Magic Edit** — "Brush over part of a photo and describe what to replace it with using Magic Edit in Canva. Add, remove, or modify elements in seconds." | DOCUMENTED (canva.com/help/using-magic-edit/) |
| Figma | "create diagrams, **edit images**, search your files" | DOCUMENTED (figma.com/ai) |
| Affinity | MCP capability "Add a purple to orange gradient map effect to **all of the images in my document**" | DOCUMENTED (affinity.studio MCP) |
| Affinity | Pixel Studio adjustment tools + MCP | DOCUMENTED |

[INFERRED] **"Brush over a region + describe the change" is the converged interaction pattern** for
AI-assisted raster editing, across at least two independent products. The region is expressed as a mask, and
the instruction is applied within it.

Architecturally this implies:
- Raster edits need a **mask/region** representation, independent of the image.
- The operation is `ApplyImageEdit(asset, mask, instruction)`.
- It is non-destructive (the original is preserved), which means the edit is a **layer/stack**, not a
  mutation.

This maps cleanly onto Affinity's adjustment-layer model, and it is a strong candidate for Spool's
`GenerateImage` / edit-image operation set.

## Masking as a container property vs. a mask object

| Product | Model | Consequence |
|---|---|---|
| Figma | **Frame's "Clip content"** — a container property | Simple; a mask must be the frame's own bounds |
| tldraw | `shape-clipping.mdx` — a shape clips its children | Same as Figma |
| Affinity | **Clipping mask as an object** (`⇧C`) | Any shape can mask any other; non-rectangular masks |
| Canva | Frame masking on elements | Undocumented in detail |

[DOCUMENTED]

[INFERRED] Affinity's model is more expressive (a circle can mask a photo) and more complex (mask objects
need resolution, ordering, and a lifetime tied to the masked object). Figma's model covers the common case
(rectangular crop) with almost no machinery.

## Image performance

[DOCUMENTED — tldraw]

- **Resolution LOD** via `steppedScreenScale` (power-of-two, debounced). See `document/assets.md`.
- **Built-in shape simplifications at low zoom**: sticky notes drop shadows; dashed/dotted freehand renders
  solid; hatch pattern fill falls back to a solid colour; **text outlines turn off below `textShadowLod`
  (0.35) and are always off on Safari**.
- `canCull()` opt-out is required for shapes with: shadows/glows extending past bounds, shapes that measure
  their DOM, animations that should run off-screen.

[INFERRED] For images, the analogous rules are:
- **Decode at the stepped scale**, not at native resolution.
- **Release the full-resolution decode** when zoomed out (a memory decision, not just a bandwidth one).
- **LOD for image effects**: blur radius, noise, and gradient complexity can be reduced at low zoom.
- Shadows/glows extending past bounds are the canonical reason an image must not be culled.

## AI and raster: the structured-output question

| Product | Raster AI output | Structured output |
|---|---|---|
| Canva | Photo Generator, Video Generator, Magic Media, 3D Content Generator, Backgrounds | **Magic Layers converts a flat design into editable elements** |
| Figma | Image generation and editing | **First Draft and the Design Agent produce design layers** |
| Affinity | Canva AI Studio (via MCP capability) | Scripting/MCP operations on layers |

[DOCUMENTED]

[INFERRED] The convergence is clear: **every AI-capable product generates rasters, and the differentiating
feature is whether generated rasters can be converted into document objects.** Canva shipped that conversion
as a named product (Magic Layers); Figma's Design Agent generates layers natively.

[INFERRED] For Spool, the implication is that the raster pipeline should be **layered and non-destructive**
from the start, so that an image with masks and adjustment layers can be "decomposed" into document
objects later. A flattened, destructive raster pipeline cannot be decomposed.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **Nothing raster.** `ObjectType { Frame, Rectangle, Ellipse, Text }` — no image.
- `shell.rs` has an **Insert** toolbar affordance with no backing model. [OBSERVED]
- No file I/O, no decoding, no texture management.
- Rendering (`render_object`) draws rects, ellipses, and text only.

## Candidate architectural implication

**Evidence:**

1. **Every Tier 3 raster capability is Affinity-only.** Three of four products ship without them and are
   commercially successful. [DOCUMENTED absence]
2. **Non-destructive adjustment with a separate effect stack** is the professional model, and effects have
   their own clipboard path distinct from styles. [Affinity DOCUMENTED]
3. **Adjustment layers imply a compositing graph**, not per-object properties. [INFERRED]
4. **"Brush + prompt" is the converged AI raster-editing interaction.** [Canva + Figma DOCUMENTED]
5. **Masking is either a container property (simple, rectangular) or a mask object (expressive,
   complex).** [Figma/tldraw vs. Affinity DOCUMENTED]
6. **Resolution LOD is power-of-two and debounced**, and shadows/glows are the canonical culling exception.
   [tldraw DOCUMENTED]
7. **AI-generated rasters are expected to become document objects**, so the raster pipeline should be
   decomposable. [Canva Magic Layers + Figma Design Agent DOCUMENTED]

**Why it matters:** Raster is a large surface area. Getting the boundary wrong (implementing Tier 3) costs
months and buys nothing; getting the *non-destructive* principle wrong forecloses AI decomposition later.

**Potential Spool approaches:**

- **A. Images only as placed bitmaps** (no crop, no mask, no adjustment). Minimum viable; no AI story.
- **B. A + crop/fill + opacity + basic adjustments** (brightness/contrast/saturate) stored non-destructively.
- **C. B + masks and a separate effect stack**, closer to Affinity's model.
- **D. C + adjustment layers** (a compositing graph).
- **E. B + "brush + prompt" AI image editing** with mask regions.

[INFERRED] **B is the right tier for a design editor, and B's non-destructive storage is what makes E
possible later.** The critical decision is not *which* effects to implement but *whether they are stored as
a stack that a later AI operation could target*. Storing `brightness: 1.2` inline on the image object is
cheap but makes "apply a gradient map to all images in the document" impossible; storing
`effects: [Brightness(1.2)]` does not.

[INFERRED] **D (adjustment layers) should be deferred** unless there is a compositing requirement, because
it is a render-graph change rather than a model change.

**Decision: TBD — requires architecture review.**

## Open questions

1. Is an image an object, or an object with an image asset?
2. Is crop stored as a rect on the object, or as a mask?
3. Are effects a stack (list) or a single value?
4. Is there a compositing graph (adjustment layers)?
5. What is the LOD policy for decoded images?
6. Can a raster layer be "decomposed" into document objects later? (Requires non-destructive storage.)
7. What is the file format for embedded images, and how does it interact with the asset store?

## Sources

- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/) ⭐ (Live Liquify,
  adjustments, Paste FX, Paste Style, clipping mask, blend modes, merge); "About Studios"
  (/workspace-about-studios/) ⭐ (Pixel Studio, Liquify, Develop, Tone Mapping, Color Grading, Retouching,
  Compositing, Astrophotography); "Snapping" (/design-aids-snapping/) (snap to pixel selection bounds)
- tldraw: `sdk-features/performance.mdx` ⭐⭐ (resolution LOD, steppedScreenScale, textShadowLod,
  canCull exceptions), `sdk-features/culling.mdx`, `sdk-features/assets.mdx`, `sdk-features/shape-clipping.mdx`,
  `sdk-features/tools.mdx` (crop tool states)
- Figma: figma.com/ai (image editing); "Guide to components in Figma" (/360038662654); product docs for Mask
- Canva: "Use Magic Edit to add, replace, and modify photos" (/using-magic-edit/) ⭐; "Canva AI 2.0"
  (/canva-ai/); "Magic Layers" (/magic-layers/); "Restore or delete designs and files from Trash"
  (/deleted-designs/) (asset refcounting)
- Spool prototype: `app/src/canvas.rs` (`ObjectType`, `render_object`), `app/src/shell.rs` (`TOOLS`)
