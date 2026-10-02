# Creative: Export

## The export surface across products

| Target | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| PNG | Yes (with scale) | Yes (`export-as-png`, `copy-as-png`, `toImage`) | Yes | Yes |
| JPEG | Yes | Not documented | Yes | Yes |
| WebP | Yes | Not documented | Not documented | Not documented |
| SVG | Yes | Yes (`export-as-svg`, `copy-as-svg`, `getSvgString`) | Yes | Not documented |
| PDF | Yes (print, SVG-based) | Not documented | Yes | Yes |
| Video | Not documented | Not documented | Not documented | Yes (MP4) |
| GIF | Not documented | Not documented | Not documented | Yes |
| PPT | Not documented | Not documented | Not documented | Yes |
| **JSON / native** | **Yes (plugin API)** | **Yes (`exportAs` supports JSON)** | **Presets** | Not documented |
| **Code** | **Yes — Dev Mode: CSS, code snippets** | Not documented | Not documented | Canva Code |
| Scope | Selection / frame / page / **slice** | **A set of shape ids** | **Slice Studio: artboards, layers, groups, objects, regions** | Design or element |
| Multiple scales | Yes | Not documented | **Yes — "different file formats and image sizes simultaneously"** | Magic Resize (AI) |

[DOCUMENTED]

## The architectural question

> How does export relate to the document model?

The evidence is consistent:

### 1. Export is a pure function of (resolved document, a set of roots, options)

[DOCUMENTED — tldraw]

```ts
editor.getSvgString(shapes)       // shapes = a list of ids
editor.toImage(shapes, { format: 'png' })
```

Every tldraw export API takes **an explicit list of shapes**, not "the current selection" or "the current
frame".

[INFERRED] This is the cleanest export model found: no ambient state, trivially testable, and it makes
"export this subset" a first-class operation.

### 2. Export must resolve the entire document

To rasterise a selection, the exporter must resolve:
- **instances** (their overrides, variants, and main components),
- **variables** (including the **current mode** — a mode-dependent value cannot be a single output),
- **constraints** and **auto layout** (position and size),
- **assets** (at the required resolution — possibly *not* the stepped scale used on screen),
- **fonts** (or outline the text),
- **masks and clipping**.

[INFERRED] So **export shares the layout engine, the asset resolver, and the style resolver with the
editor**. Any "render to SVG/PDF" implementation that bypasses the editor's resolution pipeline will produce
different output than the canvas.

### 3. Export regions are persistent named objects

| Product | Region concept |
|---|---|
| Figma | **Slice** — a layer type carrying export settings (format, scale, suffix) |
| Affinity | **Slice Studio** — "exporting artboards, layers, groups, objects or regions of your image as export slices to different file formats and image sizes simultaneously" |
| tldraw | No slice concept; export takes an id list |
| Canva | Per-design or per-element export |

[DOCUMENTED — Figma, Affinity]

[INFERRED] **Slices decouple "what to export" from "what it is".** A design object is not an export
configuration; a slice is. This lets you export a *region* that is not an object, and export one object
multiple ways.

### 4. Handoff is a distinct, heavier export

| Product | Handoff |
|---|---|
| Figma | **Dev Mode**: CSS properties, code snippets, exact transforms (`transform: rotate(-90deg)` for a 90° Figma rotation, `matrix()` for flips), asset URLs, the export CSS |
| Canva | Canva Code |
| Affinity | Not documented |
| tldraw | Not documented |

[DOCUMENTED for Figma]

[INFERRED] **Code handoff forces the document model to be *interpretable as CSS*.** Figma's documented
deviations are revealing:
- Rotation sign is **inverted** relative to the Figma UI because CSS uses the opposite convention.
- Flip uses `matrix()` rather than a separate property, and **once a flip is applied, rotation continues to
  use `matrix()`**.

[INFERRED] These are not bugs; they are evidence that Figma's **authoring model and its CSS projection are
different representations**, and that the projection is lossy. Any Spool handoff feature must accept the
same tension: `position/size/rotation/flip` in the editor vs. `left/top/width/height/transform` in CSS.

## The variable/mode export problem

[DOCUMENTED]

- Figma Number variables bind to "corner radius", "dimensions", "font size/weight/line height/letter
  spacing", "padding and gap", "shadow and blur", "stroke weight", "opacity", "layout guides", "grid size".
- Dev Mode generates CSS from these.

[INFERRED] **A variable that resolves differently per mode cannot be exported as one CSS value.** Options:
1. Export the currently-active mode only (what Figma appears to do).
2. Export per-mode CSS custom properties (`--token: value` in a `:root` block per mode).
3. Emit the resolved value and lose the token.

[INFERRED] Option 2 is what a real design-system handoff should do, and it means **export is mode-aware** —
which is a requirement that only becomes visible once modes exist.

## Font handling in export

[INFERRED, DOCUMENTED gap]

None of the four products document how fonts are handled in SVG/PDF export. The standard options are:
- Embed the font subset (SVG `<style>@font-face` with a data URI, or PDF font embedding).
- Convert text to outlines.
- Reference the font by name and assume availability.

[INFERRED] Figma's SVG export is known to embed/reference fonts, but this was **not documented** in the
pages reviewed. **Documented gap.**

[INFERRED] For Spool: outline-on-export is the safest default for reproducibility (matching the printed
output exactly) at the cost of text selectability; embedding is better for the web.

## Export as an AI capability

| Product | Documented |
|---|---|
| tldraw | **`editor.getSvgString(shapes)` and `editor.toImage(shapes, { format: 'png' })` are explicitly recommended as the way to let a model *read* the canvas.** "Sending both to the model works best: the image shows spatial relationships and styling, and the structured data gives exact text and positions." |
| Figma | The Design Agent generates design layers; Dev Mode produces code |
| Affinity | Slice Studio export |
| Canva | Magic Resize; Canva Code |

[DOCUMENTED — tldraw]

[INFERRED] **Export is a read API for the agent.** The same functions that power "download as PNG" power
"what does this design look like?", which is a prerequisite for an agent to verify its own work. This is a
small but important architectural point: export should be a first-class, scriptable read operation, not a UI
dialog.

## Multi-target export

| Product | Mechanism |
|---|---|
| Figma | One slice → PNG at scale 1x, 2x, 3x + SVG + PDF in one configuration |
| Affinity | **"different file formats and image sizes simultaneously"** from Slice Studio |
| Canva | Per-design export with a format picker; Magic Resize for format changes |
| tldraw | One shape set → one output per call |

[DOCUMENTED]

[INFERRED] Figma and Affinity both treat export as a **declarative configuration** attached to a named region,
and both can produce many outputs from it. tldraw treats it as a **function call**. The declarative model
scales better (a designer configures once); the functional model is simpler to implement and script.

## Spool prototype: what exists

From `app/src/shell.rs` and `app/src/canvas.rs` [OBSERVED in source]:

- `export_popover()` — a **UI popover listing format options** (from `popover_row(label, shortcut)` calls).
  No backing implementation. [OBSERVED]
- No serialisation, no renderer abstraction, no asset pipeline, no slice concept, no Dev Mode equivalent.
- `render_object` produces GPUI elements, not a paint-agnostic render tree — so there is **no
  renderer-independence boundary** that an exporter could reuse.

[INFERRED] The prototype's rendering goes directly from document data to GPUI elements inside `Render`
implementations. That means:
- There is no intermediate representation to serialise to SVG/PDF.
- Any export implementation would need to duplicate layout, text shaping, and asset handling.
- This is exactly the coupling the research brief warns against: *"Spool should not accidentally couple
  `GPUI view hierarchy` with `persistent design document hierarchy`"*.

[INFERRED] The single most export-relevant architectural property is whether Spool has a
**renderer-independent paint tree** between the document and GPUI. Figma's "scene graph data model"
(engineering talk) and tldraw's per-shape `component()`/`getGeometry()` split are both evidence that mature
tools keep this boundary. Spool's prototype currently does not.

## Candidate architectural implication

**Evidence:**

1. **Export takes an explicit set of roots, not ambient state.** [tldraw DOCUMENTED]
2. **Export must resolve the full document** — instances, variables (including modes), layout, assets, and
   fonts. [INFERRED from the documented feature set]
3. **Export regions are persistent named objects decoupled from the document objects.** [Figma, Affinity
   DOCUMENTED]
4. **Code handoff requires a CSS projection that is lossy and non-obvious** (rotation sign inversion,
   flip → `matrix()`). [Figma DOCUMENTED]
5. **Mode-dependent variables cannot be exported as a single value**; a real handoff needs per-mode custom
   properties. [INFERRED]
6. **Export is a read API for AI**, not just a download dialog. [tldraw DOCUMENTED]
7. **Renderer independence is required** for export to share layout/text/asset logic with the editor.
   [INFERRED from Figma's scene graph + tldraw's shape/util split, ENGINEERING-DISCLOSED for Figma]
8. Font handling in export is **undocumented in all four products**. [Documented gap]

**Why it matters:** Export is the moment when every shortcut in the document model becomes visible. It is
also how a design tool delivers value outside itself. And it is the read API an AI agent uses to verify its
own work.

**Potential Spool approaches:**

- **A. Direct export from the document model** (bypass the GPUI view). Requires layout + text + asset
  resolution to be callable headlessly.
- **B. A renderer-independent paint/render tree** between the document and GPUI; export serialises the same
  tree. This is the mature-editor pattern.
- **C. B + slices** as persistent named export configurations.
- **D. C + a mode-aware CSS/code export.**
- **E. C + an export read API** usable by an agent (and by a screenshot/preview feature).

[INFERRED] **B is the load-bearing decision.** If Spool builds a paint tree (or a retained scene), export
and headless rendering both become straightforward, and AI verification/preview becomes possible. If it does
not, every new output format requires reimplementing layout and text.

[INFERRED] **The paint tree is also the answer to Spool's rendering-performance problem** (see
`architecture/rendering.md`), because it enables caching, diffing, and partial re-render — none of which is
possible when `render()` constructs GPUI elements directly from document data.

**Decision: TBD — requires architecture review.** Export and rendering are the same architectural question.

## Open questions

1. Is there a renderer-independent paint tree between the document and GPUI?
2. Are slices first-class objects?
3. Is export a pure function of (document, roots, options)?
4. How are modes handled in code export?
5. What is the font policy in SVG/PDF export?
6. Is there an export read API for agents (`renderToImage(region)`)?
7. Does export share the layout engine and asset resolver with the editor, or duplicate them?

## Sources

- Figma: "Adjust alignment, rotation, position, and dimensions" (/360039956914) ⭐ (CSS transform inversion,
  `matrix()` for flips, slices, export); "Guide to auto layout" (/360040451373); "Overview of variables,
  collections, and modes" (/14506821864087) (variable property targets → CSS)
- tldraw: `sdk-features/image-export.mdx` ⭐, `docs/ai.mdx` ⭐ (export as agent read API), `sdk-features/actions.mdx`
  (export actions), `docs/driver.mdx`
- Affinity: "About Studios" (/workspace-about-studios/) ⭐ (Slice Studio, multi-format/size export);
  "Getting started" index (/getting-started/) (supported file formats)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/) (presentation mode); "Canva AI 2.0"
  (/canva-ai/) (Magic Resize, Canva Code)
- Spool prototype: `app/src/shell.rs` (`export_popover`, `popover_row`), `app/src/canvas.rs` (`render_object`)
