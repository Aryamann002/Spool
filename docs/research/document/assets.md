# Document Model: Assets

## What counts as an asset

| Product | Assets | Are they referenced or embedded? |
|---|---|---|
| Figma | Images, video, GIFs, components, styles, variables, fonts, code blocks, embeds | **Referenced** (an image fill references an upload) |
| tldraw | **`AssetRecordType` records** — images, videos, "and other media" | **Referenced** (`image` shape has `props.assetId`) |
| Affinity | Placed content, brushes, gradients, swatches, overlays, fonts, **presets**, symbol libraries | Referenced; presets are portable bundles |
| Canva | **Uploads** (images, videos), Elements, Brand Kit assets, linked folders, stock, AI-generated media | Referenced from designs |

## tldraw: the clearest asset model

[DOCUMENTED — `sdk-features/assets.mdx`, `sdk-features/performance.mdx`]

```ts
const asset = AssetRecordType.create({
  id: AssetRecordType.createId(),
  type: 'image',
  props: { src: generatedImageUrl, w: 512, h: 512, mimeType: 'image/png',
           name: 'generated-image.png', isAnimated: false },
})
editor.createAssets([asset])
editor.createShape({ type: 'image', x: 100, y: 100, props: { assetId: asset.id, w: 512, h: 512 } })
```

Key facts:

- Assets are **records in the same store as shapes** — "Assets cover most drawing use cases" and they
  participate in snapshots, sync, migrations, and custom-record-type machinery.
- A shape references an asset by id; it does not embed pixels.
- An asset carries `src`, intrinsic `w`/`h`, `mimeType`, `name`, `isAnimated`.
- **`TLAssetStore` is a resolution interface**: `async resolve(asset, context) → url`.
- **`TLAssetContext` carries `steppedScreenScale` and `dpr`.**

### The stepped-scale resolution — the most valuable idea in this document

[DOCUMENTED — `sdk-features/performance.mdx`]

> "When you zoom out or resize an image shape, tldraw requests a lower-resolution version from your asset
> store. The `resolve` method on `TLAssetStore` receives a `TLAssetContext` with
> **`steppedScreenScale`: the ratio of the shape's on-screen size (in CSS pixels) to the image's native size,
> rounded up to the nearest power of two.** Multiply by `dpr` to get device pixels."

Example from the docs: "A 4000px-wide photo zoomed out to take up 200px on screen has a screen scale of
0.05, which steps up to 0.0625, so you'd serve a 250px-wide image (times `dpr`) instead of the full 4000px.
This reduces memory usage and decoding cost. **Resolution updates are debounced** so images don't thrash
between sizes during zooming."

```ts
const assetStore: TLAssetStore = {
  async resolve(asset, context) {
    if (asset.type !== 'image') return asset.props.src
    const width = Math.ceil(asset.props.w * context.steppedScreenScale * context.dpr)
    return `${asset.props.src}?w=${width}`
  },
}
```

[INFERRED] This turns an unbounded set of possible zoom levels into a **small, cacheable set of asset
variants** (powers of two), which makes HTTP caching and GPU texture caching effective. It is a mipmap
strategy for a document model, and it is directly portable to a native app: Spool would request a decoded
texture at a size drawn from `{…1/16, 1/8, 1/4, 1/2, 1, 2, 4…}`.

## Canva: reference counting is an explicit constraint

[DOCUMENTED — "Restore or delete designs and files from Trash"]

> "Some images won't delete — **Images or videos still used in designs won't be fully deleted until the
> design is removed.**"

[INFERRED] This is a **reference-counting invariant** in the asset graph. It implies:
- Deletion is a two-phase operation (unlink, then purge when refcount hits zero).
- The Trash (30-day retention) is where unreferenced-but-not-purged assets live.
- Cross-design references mean asset lifetime is bounded by the *slowest* referencing design.

[INFERRED] For an open-source local-first editor, the equivalent is: assets live in a document (embedded)
or in a shared asset library (referenced), and the editor must handle a missing reference gracefully.
**None of the four products document their missing-asset behaviour.** This is a documented research gap.

## Affinity: assets are resource bundles

[DOCUMENTED]

- **Presets**: bundles of styles, effects, brushes, gradients — a portable multi-resource format.
- **Symbol libraries**: third-party vector symbol collections from other applications.
- **Asset panel** with brushes, gradients, swatches, overlays.
- **Importers**: PDF, InDesign (IDML/INDD), Adobe Photoshop and Illustrator, Microsoft Publisher, **CAD**.
- **RAW support**: extensive per-brand format lists; **Develop Studio** for full RAW processing.
- **Save/export presets** from Slice Studio: "exporting artboards, layers, groups, objects or regions of
  your image as export slices to different file formats and image sizes simultaneously".

[INFERRED] Affinity treats assets as **professional resources** (brushes, gradients, CAD, RAW pipelines)
rather than as document-scoped media. The importers are the key difference: importing a PDF or an INDD file
produces a *document*, not an asset.

## Figma: assets referenced, with a slice/export split

[DOCUMENTED]

- Images, video, GIFs, components, styles, variables, fonts.
- **Slices** define named export regions: "exporting artboards, layers, groups, objects or regions" is
  Affinity's phrasing; Figma has Slices as a first-class layer type used for export configuration.
- Plugins expose assets via the Plugin API; the Figma REST API exposes images.
- **Component libraries** are cross-file asset references. [DOCUMENTED]

[INFERRED] Figma's slice model and Affinity's slice model are the same idea: a persistent named region
with an export configuration (format, scale, suffix), decoupled from what is being exported.

## Fonts as assets

| Product | Font handling |
|---|---|
| Figma | Fonts are a per-user/per-team resource; text references a font family + style. Missing font handling not documented here. |
| Affinity | Document fonts; fonts installable via presets |
| Canva | Brand fonts in Brand Kit; limited availability is a known Canva constraint [THIRD-PARTY] |
| tldraw | `text-measurement.mdx` implies a font-metrics service; font loading architecture not extracted |

[DOCUMENTED partially]

[INFERRED] Fonts are the hardest asset because they affect **measurement**, not just rendering. A missing
font changes layout, which changes bounds, which changes snapping and culling. This is the reason
`text-measurement` is a dedicated subsystem in tldraw.

## Asset metadata

| Product | Documented metadata |
|---|---|
| tldraw | `src`, `w`, `h`, `mimeType`, `name`, `isAnimated`, `type` |
| Figma | Unknown in detail |
| Affinity | Preset metadata unknown |
| Canva | Unknown |

[INFERRED] `name` + `mimeType` + intrinsic dimensions is the minimum viable metadata set. `isAnimated`
implies assets can be video/GIF and therefore need a notion of time and a playback state.

## Assets as AI context

| Product | Documented |
|---|---|
| tldraw | Agent context includes "simplified representations of shapes within view" and shape text; generated images are created as **asset records + image shapes** by the application, not by the model. |
| Canva | Magic Media, Photo Generator, Video Generator, Shape Generator, 3D Content Generator, AI-Powered Elements all produce **assets or elements** placed into the design. |
| Affinity (via MCP) | "Add a purple to orange gradient map effect to **all of the images in my document**" — an operation over an asset collection. |

[DOCUMENTED]

[INFERRED] **Assets are the most natural unit for AI generation.** All three AI-capable products generate an
*asset* and place it, rather than generating *document structure*. This has a direct architectural
implication: an asset store with a `create from generator` path is a prerequisite for image-generation AI,
and it is much simpler than having the model emit a fully-formed document.

## Asset lifetime and the document

| Question | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Embedded in the file? | Uploaded separately, referenced | **Records in the store** (in the document snapshot) | Placed in the document | Uploaded separately, referenced |
| Shared across documents? | Yes (libraries) | Only by copy | Via presets / symbol libraries | Yes (Brand Kit, folders) |
| Missing reference behaviour | Unknown | Unknown | Unknown | Unknown |
| Purge policy | Unknown | Unknown | Presets | **Refcount + 30-day Trash** |

[INFERRED] Only Canva documents a purge policy, and only as a user-facing consequence of reference
counting. **Missing-asset handling is a genuine gap in all four products' documentation**, and it is a real
requirement for an open-source editor where files move between machines.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **No asset concept whatsoever.** `DesignObject` has no `asset_id` field; `ObjectType` has no `Image`.
- No file import, no image decode, no texture cache.
- `shell.rs` has an **Insert** affordance in the toolbar (`TOOLS` const) — UI chrome only, no backing
  model. [OBSERVED]
- No file persistence at all: `Document` is constructed in `Default::default()` with four hard-coded frames.
- No serialisation.

[INFERRED] Assets are entirely ahead of the prototype. Given that the prototype cannot yet save a file,
this is not a criticism — but it does mean the asset model is unconstrained by existing code, which is the
best possible time to design it.

## Candidate architectural implication

**Evidence:**

1. Assets are **records referenced by objects**, not embedded in objects, in every product that documents
   it. [DOCUMENTED]
2. An asset resolution interface with a **device-scale-aware request** (`steppedScreenScale`, power-of-two,
   debounced) is what makes large images affordable. [tldraw DOCUMENTED]
3. Assets have a **purge policy driven by reference counting**; unreferenced assets persist in a retention
   area. [Canva DOCUMENTED]
4. Export regions (slices) are **persistent named objects decoupled from what they select**.
   [Figma + Affinity DOCUMENTED]
5. Fonts are assets that affect **measurement**, hence a dedicated text-measurement subsystem.
   [tldraw DOCUMENTED]
6. **Missing-asset behaviour is undocumented in all four products.** [Documented gap]
7. AI generation produces **assets placed into the document**, not raw document structure.
   [tldraw, Canva, Affinity DOCUMENTED]

**Why it matters:** Assets are the entry point for images, video, fonts, and generated media. They also
determine the AI asset path, the export path, and file portability. And the resolution strategy is a
performance decision baked into the interface.

**Potential Spool approaches:**

- **A. Embedded media** (bytes in the file). Simplest; large files; no sharing.
- **B. Referenced assets in a document-level table** (tldraw's model). Portable; needs a missing-asset
  policy.
- **C. B + an asset resolution/resolution-service interface** with stepped-scale requests.
- **D. B + external asset libraries** (a file on disk, referenced by path). Open-source-friendly; breaks
  when files move.
- **E. C + D**: resolve through a chain (library → local cache → remote).

[INFERRED] For a local-first open-source editor, **E is the pragmatic answer**: assets stored in a
sidecar folder next to the document, referenced by relative path, resolved through a cache that also
implements stepped-scale decoding. This avoids bloating the document file and keeps assets shareable.

[INFERRED] The **missing-asset policy must be decided explicitly** because no product documents one.
Options: keep the frame with a placeholder, drop the object, or mark it. For an AI-generated document, a
third state — "this object references a generated asset that no longer exists" — is worth having.

**Decision: TBD — requires architecture review.**

## Open questions

1. Are assets embedded, referenced, or both?
2. What is the missing-asset behaviour? (No product documents one.)
3. Is there a resolution service with stepped-scale requests?
4. Where do fonts live, and what happens when one is missing (measurement changes)?
5. Are slices first-class objects?
6. Is there a shared asset library, or is a document self-contained?
7. What is the asset purge policy?
8. Does an asset carry an `is_generated` / provenance flag? (Needed for AI.)

## Sources

- tldraw: `sdk-features/assets.mdx` ⭐, `sdk-features/performance.mdx` ⭐ (`steppedScreenScale`, debouncing,
  LOD), `docs/ai.mdx` (generating assets and placing them), `sdk-features/store.mdx`, `sdk-features/draw-shape.mdx`,
  `sdk-features/scribble.mdx`
- Figma: "Adjust alignment, rotation, position, and dimensions" (/360039956914) (slices, export);
  "Guide to components in Figma" (/360038662654) (libraries)
- Affinity: "Getting started" index (/getting-started/) (importers, presets, supported formats); "About
  Studios" (/workspace-about-studios/) (Slice Studio)
- Canva: "Restore or delete designs and files from Trash" (/deleted-designs/) ⭐; "Set up Brand Kits"
  (/brand-kit/); "Canva AI 2.0" (/canva-ai/) (generators)
- Spool prototype: `app/src/canvas.rs` (`DesignObject`, `ObjectType`), `app/src/shell.rs` (`TOOLS`)
