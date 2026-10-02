# Affinity

## Overview

Affinity (formerly Affinity Designer / Photo / Publisher, now "Affinity by Canva" with **Studios**) is a
professional creative suite: vector illustration, photo editing, and page layout in one application with a
shared document model. It is the reference product in this research for **serious creative tooling**:
node-level vector editing, snapping depth, and professional typography.

Critical framing for Spool: Affinity is the counter-example to "a design tool is a UI mockup tool". Its
document model has to survive curves, masks, compound paths, mesh gradients, RAW pipelines, and print
output. Any Spool architecture that only supports rectangles and text will not survive contact with real
creative work.

Current product state (Oct 2026): Serif rebranded to "Affinity", the suite is organised into **Studios**,
a **JavaScript scripting API** is in beta, and a **local MCP server** integrates with Claude Desktop.
[DOCUMENTED — affinity.studio/help]

## Product Philosophy

1. **One document, many disciplines.** Vector, pixel, and layout work share a document.
   [DOCUMENTED]
2. **Studios, not modes.** A Studio is "a workspace that groups tools and panels specific to a design
   discipline … or tailored to a custom way of working. They show in Affinity as Toolbar icons that can
   be swapped between." [DOCUMENTED — "About Studios"]
3. **Non-destructive by default.** Adjustment layers, live filters, clipping masks, Smart Filters,
   Live Liquify. [DOCUMENTED at feature level]
4. **Snapping is the core alignment primitive**, deeply configurable with presets per discipline.
   [DOCUMENTED]
5. **Precision is a first-class citizen.** Construction snapping for Bézier handles, force-pixel
   alignment, snap-to-gap, explicit units. [DOCUMENTED]
6. **Automation is native and now agent-addressable.** Scripting API + MCP server + scripting panels.
   [DOCUMENTED]

## Canvas Model

| Aspect | Behaviour | Evidence |
|---|---|---|
| Canvas shape | **Document-bounded** with pages/artboards; distinct from tldraw's infinite canvas | DOCUMENTED — snapping to page edge, spreads, margins, baseline grid |
| Page structure | Page layouts, margins, spreads, bleed | DOCUMENTED |
| Containers | **Container layers**, groups, frames | DOCUMENTED — "New Container Layer" `⌥⌘L` |
| Layers panel | Full tree, rename (`⇧⌘R`), lock (`⌃⌘L`), show/hide all (`⌃⌘H`), exclude-from-snapping | DOCUMENTED |
| Rulers/guides | Guides, margins, grids, baseline grid; all snappable targets | DOCUMENTED |
| Measurements | Document units are explicit and affect snapping ("Snapping always snaps to the currently set measurement unit") | DOCUMENTED |
| Pixel grid | Force Pixel Alignment; Move by whole pixels | DOCUMENTED |
| Slice Studio | Export artboards/layers/groups/objects/regions as export slices to multiple formats and sizes simultaneously | DOCUMENTED |

Key structural difference: Affinity's snapping system treats the **page itself** as a snap target
(spread edge, spread midpoints, margins, margin midpoints, baseline grid). [DOCUMENTED] Neither Figma nor
tldraw treats the document boundary as a first-class snap target in this way.

## Interaction Model

Affinity's interaction model is **tool-per-task with a context toolbar**, not a state chart. No state
machine is published. The reconstructed lifecycles below are [INFERRED] from documented tool behaviour.

### Node Tool lifecycle (reconstructed) [INFERRED, from DOCUMENTED options]

```
idle (Select Tool active)
  -> click / double-click a curve or shape
     -> node editing active
        -> marquee-select nodes
        -> drag nodes  (constrained by the documented snapping options)
        -> context toolbar operations:
             Sharp | Smooth | Smart (node type)
             Split Curve | Break Curve | Close Curve | Smooth Curve | Join Curves | Reverse Curve
             Transform Mode (bounding box over selected nodes)
             Enable Transform Origin (rotate about a movable origin)
  -> Esc
     -> back to object selection
```

### Modifier behaviour [DOCUMENTED — "Keyboard shortcuts for general editing"]

| Input | Behaviour |
|---|---|
| `⇧`-click | Add to selection |
| `⇧`-drag (marquee) | Add/remove selection when marquee **fully contains** object |
| `⌃`-drag (Mar) / LMB-drag then RMB (Win) | Add selection when marquee **partially intersects** object |
| `⌥`-click | **Select overlapped object** (cycles through stacked objects) |
| `⇧`-drag (move) | Constrain movement to horizontal, vertical, or diagonal |
| `⇧`-drag **from inside the object's bounding box** | Same axis constraint (distinct from handle drags) |
| `⇧`-drag corner handle | Resize maintaining aspect ratio |
| `⌘`/`Ctrl`-drag corner handle | Resize **from the centre** |
| `⌃`-drag corner handle (Mac) / LMB then RMB (Win) | Rotate **around the opposite corner** |
| `⌃`-drag (with a selection box) | **Mirror shearing** |
| `⇧`-drag rotation handle | Rotate in **15° intervals** |
| `Alt` while positioning | **Temporarily override snapping** |
| `Esc` | Cancel a sizing, moving, or creating operation |
| Arrow keys | Nudge (configurable distance) |
| `⇧`-arrow keys | Nudge by a modified (larger, configurable) distance |

Two findings deserve emphasis:

1. **Affinity has two distinct marquee intersection modes** (full containment vs. partial intersection),
   separately bound. Figma only documents containment. This is a concrete, discoverable difference a user
   can notice.
2. **`⌥`-click cycles through overlapped objects.** Figma achieves the same via the Select Layer menu;
   tldraw has no equivalent; Canva has no equivalent. This is the cheapest possible "select behind"
   affordance and it costs nothing to implement.

### Selection box model [DOCUMENTED]

Affinity has an explicit, user-visible **selection box** abstraction:

| Action | Binding |
|---|---|
| Cycle Selection Box | `.` (full stop) |
| Set Selection Box | `⌘.` / `Ctrl+.` |
| Show/Hide selection box while dragging | Context-toolbar option "Hide Selection while Dragging" |

[INFERRED] A first-class, *addressable* selection box (with named variants for curve/node vs. object
selection) is a professional-tool pattern. Spool's prototype hard-codes a single implicit box.

## Tools

Affinity's tool set is far broader than the other three products. [DOCUMENTED — tool list in help index
and Vector Studio shortcuts page]

**Vector**: Node Tool (`A`), Pen, Pencil, Brush, Corner Tool, Curve Tool, Knife, Shape Builder, Vector
Flood Fill, Fill, Transparency, Gradient, Cropping, Transform, Zoom, Measure, Ruler, **Convert to
Curves** (`⌘⏎`), **Create Clipping Mask** (`⇧C`), Boolean operations (Add/Subtract/Intersect/Exclude).

**Pixel**: Brush, Pencil, Paint Brush Selector, Dodge, Burn, Smudge, Blur/Sharpen/Smudge, Healing,
Patch, Content-Aware Fill, Warp, Liquify, Repair, **Eye Dropper**, Selection tools (Marquee, Elliptical,
Lasso, Polygon, Magnetic), **Flood Select**, **Background Removal**, **Bokeh**, Panorama.

**Layout**: Text Frame, Text Box, Image Frame, **Baseline Grid**, Columns/Frames, Table, Text Wrap.

**Studios available** (12+) [DOCUMENTED — "About Studios"]: Vector, Pixel, Layout, **Canva AI Studio**,
Slice, Retouching, Color Grading, **Typography**, Compositing, Astrophotography, **Scripting**, plus
Liquify, Develop, and Tone Mapping available via Pixel ▸ Filters.

[INFERRED] Affinity's Studio system is *exactly* the "interaction profile" concept Spool is contemplating,
already shipped in a commercial product — and it is explicitly **customisable**: "Any Studio can be
customized. You'll be able to: switch on/off tools in the Studio; move tools between Studios; change the
Toolbar arrangement and switch on/off its options; change the context toolbar position; change panel
visibility, grouping and positioning." [DOCUMENTED] Strong evidence that a profile system is
architecturally reasonable and user-valued.

## Text

- **Text Frame** vs **Text Box** are distinct objects in Layout Studio. [DOCUMENTED]
- **Baseline grid** with text snapping to baselines; column frames; text wrap. [DOCUMENTED]
- **Typography Studio** groups "tools and panels that are focused on text formatting and styling."
  [DOCUMENTED]
- Text style application shortcuts: `⌥⌘0` (body) through `⌥⌘5` (section header H5) in Canva Docs.
  [DOCUMENTED — Canva; Affinity has its own text style system]
- Affinity's **Styles** system (Styles panel) is the classic Adobe-derived model: named paragraph /
  character / graphic styles, with "paste style" (`⇧⌘V`) and "paste without format" (`⌥⇧⌘V`)
  as explicit, separate commands. [DOCUMENTED]

[INFERRED] "Paste Style" and "Paste without Format" as distinct bindings is a professional-tool
convention that mainstream design tools mostly lack. It implies styles are a first-class document concept
with a clipboard representation.

## Drawing

- **Pencil** freehand; **Brush** (pixel); **Pen** (Bézier with Alt/Option for corner points);
  **Corner Tool** (rounds corners while preserving node count — a genuinely distinctive Affinity feature);
  **Curve Tool**; **Knife**; **Shape Builder** (flood-add and erase across overlapping shapes);
  **Vector Flood Fill** (fill areas created by overlapping/intersecting shapes and open curves);
  **Vector warp nodes**. [DOCUMENTED]

[INFERRED] Corner Tool and Shape Builder are the two vector features that most directly encode "a shape
is a set of points with semantic corners, not just a point list." Both operate on multiple shapes at
once. Serious vector editing requires group-level point topology operations, not just per-shape node
editing.

## Document Model

| Concept | Evidence |
|---|---|
| **Document** | Root; has units, page setup, colour space | DOCUMENTED |
| **Page / Page layout / Spread / Margin** | Print-aware document structure | DOCUMENTED |
| **Container layer** | First-class layer type (`⌥⌘L`) | DOCUMENTED |
| **Pixel layer** | Separate layer type (`⇧⌘N`) | DOCUMENTED |
| **Group** | Yes (`⌘G`) | DOCUMENTED |
| **Curve / Path** | Nodes with per-node types (Sharp / Smooth / Smart), handles, winding fill mode | DOCUMENTED |
| **Shape** | Geometric primitives, convertible to curves | DOCUMENTED |
| **Text Frame / Text Box** | Separate types | DOCUMENTED |
| **Image Frame** | Placement + crop container | DOCUMENTED |
| **Compound path** | Via Boolean ops | DOCUMENTED |
| **Clipping mask** | First-class (`⇧C`) | DOCUMENTED |
| **Selection** | Object selection, marquee regions, pixel selections | DOCUMENTED |
| **Adjustment layer** | Non-destructive raster effects | DOCUMENTED |
| **Brush stroke** | Persistent stroke data | DOCUMENTED |
| **Smart Filter** | Live filter | DOCUMENTED |
| **Effect / FX** | Separate from style (`⌃⌘V` paste FX) | DOCUMENTED |
| **Style** | Named paragraph/character/graphic styles | DOCUMENTED |
| **Guide / Grid / Baseline grid** | Persistent, snappable, document-scoped | DOCUMENTED |
| **Swatch / Brush / Gradient / Overlay** | Persistent resources | DOCUMENTED |
| **Asset library / Asset panel** | Persistent resources | DOCUMENTED |
| **Component / Instance / Variant** | **Not present.** Affinity has **no component system.** | [DOCUMENTED by absence in the help index; THIRD-PARTY corroboration] |

[INFERRED] Affinity's reusable-object story is: styles, swatches, gradients, brushes, and asset libraries —
all *appearance and content* resources — but **no structural instance system**. This is the clearest
possible demonstration that a professional creative tool can ship without component/instances, because
its users' reuse needs are visual rather than structural.

## Hierarchy

- Layers panel with drag reorder, rename, lock, hide, exclude-from-snapping. [DOCUMENTED]
- `⌥⌘G` **Move Inside** / `⌥⇧⌘G` **Move Outside** — explicit reparenting operations with distinct
  bindings. [DOCUMENTED] [INFERRED] This is an operation most design tools express only as a drag
  gesture; making them discrete commands makes them scriptable and AI-addressable.
- `⌘E` Merge Down, `⇧⌘E` Merge Selected, `⌥⇧⌘E` Merge Visible — three distinct flatten operations with
  distinct scopes. [DOCUMENTED] [INFERRED] "Flatten" is not one operation; scoping matters.
- `⌘↑` Select Parent Layer. [DOCUMENTED]

## Components / Reuse

Not present. See Document Model above. Reuse mechanisms available instead:

| Mechanism | What it reuses |
|---|---|
| Styles | Appearance bundles |
| Paste Style | Appearance, across documents |
| Asset library | Placed content (brushes, gradients, swatches, overlays) |
| Copy/paste | Subtrees, as independent copies |
| Scripts | *Procedures* — reusable because they are code |

[INFERRED] Affinity's reuse story is procedural rather than declarative. A script that batch-renames or
re-styles is a reusable artifact. This is a legitimate alternative to a component system, and it is the
model that the Affinity MCP integration extends.

## Layout

**No flow layout engine in Vector Studio.** Layout in Affinity is:

- Absolute positioning.
- **Constraints** on container layers (Affinity 2 introduced a constraint system with responsive resizing).
- **Snapping + guides** as the alignment mechanism.
- **Text frames with columns and baseline grids** for page layout — a genuine flow-ish concept for
  *text only*, not for arbitrary children.
- **Slice Studio** for export regions.

[INFERRED] Affinity separates two layout questions that Figma unifies: *how do things flow relative to
each other* (Figma: auto layout; Affinity: not modelled) and *how do things respond when a container
resizes* (Figma: constraints; Affinity: constraints). Affinity solves the second and leaves the first to
tools, snapping, and scripts.

[INFERRED] This is a legitimate and defensible position for a professional illustration tool, and a
poor position for a UI design tool. Spool should treat "does Spool need a flow layout engine?" as an
open question, informed by the fact that a mature, highly-regarded professional tool answers "no".

## Styles

- **Styles panel** with named styles, applied per character range / paragraph / object.
- **Paste Style** (`⇧⌘V`), **Paste without Format** (`⌥⇧⌘V`), **Copy Merged** (`⇧⌘C`).
- **Swatches**: palette, colour themes.
- **Effect stack** separate from style, with its own paste (`⌃⌘V`).
- **Blend modes**: 27 named modes, each with a dedicated `⌥⇧<letter>` shortcut, including two
  non-standard ones — **Pigment** and **Normal** — that do not exist in CSS. [DOCUMENTED] [INFERRED]
  Non-standard blend modes prove Affinity's renderer has a blend-mode vocabulary larger than the web's,
  which has consequences for any export target.

[INFERRED] Affinity's model is: **styles are embedded named values with an inheritance-like application
scope; there is no token/variable layer and no mode switching.** Contrast with Figma's two-layer
style+variable system and tldraw's named palette.

## Assets

- Images (incl. RAW), placed content, brushes, gradients, swatches, overlays, fonts, symbol libraries
  (third-party `.symbollibrary` from other apps), **preset** entities (bundles of styles, effects,
  brushes, gradients — Affinity's asset-bundle format). [DOCUMENTED]
- **Importers** for PDF, InDesign (IDML/INDD), Adobe Photoshop and Illustrator, Microsoft Publisher, and
  **CAD**. [DOCUMENTED — help index]
- **RAW support**: extensive per-brand format lists; **Develop Studio** for full RAW processing.
  [DOCUMENTED]
- **Save/export presets**: export to multiple formats and sizes simultaneously from Slice Studio.
  [DOCUMENTED]

[INFERRED] CAD import, InDesign import, and 27 blend modes are the clearest evidence that Affinity's
document model is far richer than a design tool's needs — and that a Spool document model designed only
for UI design would need substantial revision to accept imported creative content.

## History

- `⌘Z` undo, `⇧⌘Z` redo. [DOCUMENTED]
- Grouping semantics are not documented in the pages reviewed. **Unknown.**
- **Server-side file history** exists at the application level but was not documented in this pass.
  **Unknown.**
- **Scripting explicitly applies changes "often as a single undoable action."** [DOCUMENTED — "Scripting
  in Affinity"]

[INFERRED] That phrase is the clearest statement in any of the four products of the
`one-script = one-undo-step` requirement, and it is exactly the abstraction an AI operation API needs.

## Keyboard / Input

**Affinity is the only one of the four products with documented, full user customisation.**

> "Many shortcuts are the same as those for equivalent features in other apps, and **you can customize
> them to suit your way of working**. In the tables below, a blank entry means no shortcut is assigned by
> default—you can add one in Affinity's settings. N/A means the action is unavailable on the
> corresponding platform."

[DOCUMENTED — "Keyboard shortcuts for general editing"]

Also documented:
- Nudge distances are configurable: "For Mac: On the Affinity menu, select Settings. […] select Tools."
  [DOCUMENTED]
- Shortcut reference is **split by Studio**: Workspace, General editing, Vector Studio, Pixel Studio,
  Layout Studio. [DOCUMENTED] [INFERRED] Shortcuts are scoped per Studio — a form of contextual keymap.

Colour-related single-key shortcuts are culturally important and layout-sensitive: `X` swaps
foreground/background, `⇧X` swaps stroke/fill, `D` sets black/white, `/` sets no fill (documented as
"International and selected keyboards only"), `⌥Backspace` fills with foreground, `⌘Backspace` fills with
background. [DOCUMENTED]

[INFERRED] The explicit qualification "International and selected keyboards only" on `/` shows
professional tools do grapple with layout-correct shortcuts — but they handle it by *disabling* a
binding on non-matching layouts, rather than implementing the three-strategy matching tldraw documents.

## Rendering / Performance

Not publicly documented. **Unknown.** No engineering blog, no architecture article, no published
performance framework was found in this pass. This is a research gap and should be recorded as one.

The only observable inference: 27+ blend modes, live filters, and non-destructive RAW processing imply a
GPU-accelerated multi-pass compositor with a filter graph. [INFERRED, low confidence]

## Collaboration

**None.** Affinity is a single-user, local, file-based application with no multiplayer editing, no
presence, and no real-time sync. [DOCUMENTED by absence in the help index; corroborated by the product's
positioning]

[INFERRED] This is a deliberate division of the market. Affinity competes on local-first creative power,
not on collaboration.

## Prototyping

**None.** No prototype flows, triggers, or actions. **Unknown/absent.**

[INFERRED] Affinity's answer to "show me how this works" is a **frame presentation tool** (`⌥⌘P` is
Canva's; Affinity has its own presentation/stack view), not an interaction graph. [Low confidence — the
relevant page was not read in this pass.]

## Export

- Slice Studio: "for exporting artboards, layers, groups, objects or regions of your image as export
  slices to **different file formats and image sizes simultaneously**." [DOCUMENTED]
- Save/export presets. [DOCUMENTED]
- Standard raster + print formats implied; **the exhaustive format list is Unknown** from this pass.

[INFERRED] Slice-based multi-target export is architecturally identical to Figma's export-slice model and
tldraw's shape-list export model: export is a function of (document, a set of roots, options).

## AI

Affinity's automation story is the most concrete of the four products, and it is **agent-addressable**.

### Scripting [DOCUMENTED — "Scripting in Affinity"]

> "Scripting lets you automate repeated work in Affinity using JavaScript. Scripts can read the current
> document, optionally show a dialog to collect options, and apply changes—**often as a single undoable
> action**."

Documented uses: batch-editing selected objects (align, transform, rename, restyle); generating guides or
layout helpers from selection bounds; adjusting items across page ranges; **editing curve geometry
(e.g. adding points)**; running precise repeated tasks.

Documented limits — and this list is architecturally revealing:

> "Scripting is currently in beta. Some parts of Affinity are not yet scriptable […]
> Scripts are not just macros. Macros replay recorded actions, whereas scripts use code that checks and
> changes document content. **Scripting is not a replacement for built-in tools or a full plugin
> system.**
> Scripting does not:
> - Add new tools.
> - Modify the core user interface.
> - Run continuously in the background.
> - Intercept user input.
> - Override built-in tool behavior.
> - Provide real-time, event-driven automation.
> Scripts run when you launch them, at which point they perform their task and then finish."

The scripting API reference lives at **affin.co/affinity-sdk** and documents modules/classes including
**Document**, **Dialog**, and **Collection**. The API uses British English spellings (`centre`,
`colour`). [DOCUMENTED]

[INFERRED] The explicit "what scripting is not" list is a precise articulation of the boundary between a
*command API* and a *plugin system*. For Spool, the equivalent boundary statement would be worth writing
deliberately.

### MCP server [DOCUMENTED — "AI Automation with Claude"]

- Affinity runs a **local MCP server** on the device; it connects to supported AI assistants.
- Requires Affinity **April '26 or later** + Claude Desktop. macOS Big Sur 11+.
- Enable path: Affinity/Edit menu ▸ Settings ▸ **Model Context Protocol** ▸ **Enable MCP server** ▸ quit and
  reopen.
- **Approval settings are configurable per MCP capability** in the Claude connector config.
- **Privacy toggles** (this is the most interesting part):

| Toggle | Effect |
|---|---|
| Access files on your Desktop | Open, edit, save files on Desktop |
| Access networks | Use internet and local network connections |
| Use saved scripts | Read and run scripts in the scripting panel |
| **Save scripts to your scripting panel** | Persist a completed workflow as a reusable script |
| **Use Canva AI Studio features** | Use Canva AI Studio; consumes the Canva plan's monthly AI allowance |
| **Save task hints to your device's local memory** | Store task hints locally for similar future tasks |
| **Share task hints with Affinity** | Send anonymised task hints to improve Affinity's knowledge base |

- Documented example prompts: "Add a purple to orange gradient map effect to all of the images in my
  document"; "Rename the layers in my Affinity document"; "Create a tool in Affinity that generates
  vector patterns and has a UI."
- Documented guidance: **"Work with existing documents. Your AI assistant performs best when modifying,
  fixing, or automating tasks on an existing document. Asking it to design from scratch generally produces
  poor results."** [DOCUMENTED]
- Workflow → script promotion: "If you gave the AI assistant permission, it can save any completed
  workflow as a reusable script."

[INFERRED] Four architectural ideas here are directly relevant to Spool:
1. **The AI assistant operates on the document via a command API**, not by synthesising UI input. This is
   the opposite of tldraw's driver approach, and the two are complementary.
2. **Workflow-to-script promotion**: a successful agent run can be captured as a durable, reviewable,
   re-runnable artifact. This is a strong model for agent history and for user trust.
3. **Capability-level approval and privacy toggles**, rather than a single all-or-nothing "let the AI edit".
4. **Explicit acknowledgment that generation-from-scratch is weak while modification-on-existing is
   strong** — an important product constraint for AI-native design tooling.

## Architectural Observations

### Evidence-backed concepts

| Concept | Problem | Persistent? | In history? | Affects render? | Affects input? | AI-relevant? | Evidence |
|---|---|---|---|---|---|---|---|
| **Studio** | Context-sensitive tool/panel set | No (workspace) | No | No | **Yes** | Yes | DOCUMENTED |
| **Customisable Studio** | Per-profile tool/panel composition | No | No | No | Yes | No | DOCUMENTED |
| **Shortcut customisation** | User-assignable bindings | Yes (prefs) | No | No | **Yes** | No | DOCUMENTED |
| **Context toolbar** | Tool- or selection-dependent options | No | No | No | **Yes** | Partly | DOCUMENTED |
| **Selection box (addressable)** | Which bounds/manipulators are active | No | No | **Yes** | **Yes** | Yes | DOCUMENTED |
| **Selection-box cycle (`.`)** | Switch box variant | No | No | Yes | Yes | No | DOCUMENTED |
| **Snap presets** | Per-discipline snapping profiles | Yes (prefs) | No | Yes (indicators) | **Yes** | Yes | DOCUMENTED |
| **Screen tolerance** | Zoom-normalised snap distance | Yes (prefs) | No | Yes | Yes | Yes | DOCUMENTED |
| **Smart guides + labels** | Show alignment + measured distance | No (transient) | No | **Yes** | Yes | No | DOCUMENTED |
| **Colour-coded guides** | Red=horizontal, green=vertical, yellow=key point, blue=third plane, orange=projection grid | No | No | Yes | Yes | No | DOCUMENTED |
| **Candidate scoping** (list / immediate layers / immediate+children / all) | Bound snap cost | Yes (prefs) | No | No | Yes | Yes | DOCUMENTED |
| **Exclude from snapping** (per-layer) | Opt out of being a target | **Yes** (persistent) | Yes | No | Yes | Yes | DOCUMENTED |
| **Construction snapping** | Bézier handle constraints | Yes (prefs) | No | Yes | Yes | Yes | DOCUMENTED |
| **Node type** (Sharp/Smooth/Smart) | Corner vs Bézier vs auto-fit | **Yes** | Yes | **Yes** | **Yes** | Yes | DOCUMENTED |
| **Curve orientation** | Winding fill direction | **Yes** | Yes | **Yes** | Yes | Yes | DOCUMENTED |
| **Compound path / Boolean** | Multiple contours in one object | **Yes** | Yes | **Yes** | Yes | Yes | DOCUMENTED |
| **Clipping mask** | Clip without grouping | **Yes** | Yes | Yes | Yes | Yes | DOCUMENTED |
| **Container layer + constraints** | Responsive resize | **Yes** | Yes | Yes | Yes | Yes | DOCUMENTED |
| **Merge Down / Selected / Visible** | Three scoped flatten operations | No (operation) | Yes | Yes | No | **Yes** | DOCUMENTED |
| **Move Inside / Move Outside** | Explicit reparenting | No (operation) | Yes | Yes | Yes | **Yes** | DOCUMENTED |
| **Adjustment layer / Smart Filter** | Non-destructive raster effects | **Yes** | Yes | **Yes** | Yes | Yes | DOCUMENTED |
| **Slice (export)** | Named export region | **Yes** | Yes | No | No | Yes | DOCUMENTED |
| **Guide / Margin / Baseline grid** | Persistent layout aids | **Yes** | Yes | Yes | **Yes** (snap targets) | Yes | DOCUMENTED |
| **Document units** | Measurement system | **Yes** | Yes | No | **Yes** (snapping) | Yes | DOCUMENTED |
| **Preset (asset bundle)** | Portable multi-resource package | **Yes** | Yes | No | No | Yes | DOCUMENTED |
| **Scripting API (Document/Dialog/Collection)** | Scriptable document access | No (code) | **Yes — one action** | Yes | No | **Yes** | DOCUMENTED |
| **MCP server + capability toggles** | Agent access with scoped approval | No (runtime) | Yes (via script) | Yes | No | **Yes** | DOCUMENTED |
| **Workflow → script promotion** | Make agent runs durable | **Yes** (script) | No | No | No | **Yes** | DOCUMENTED |

### What Affinity proves

1. **A professional design tool can ship with no component/instance system.** Reuse via styles, assets,
   copy/paste, and procedural scripts is sufficient for professional creative work. [INFERRED]
2. **A professional design tool can ship with no flow layout engine.** Snapping + guides + constraints +
   scripts cover professional creative work. [INFERRED]
3. **A "persona/workspace" system is a shipped, customisable commercial feature.** [DOCUMENTED]
4. **Full keyboard customisation is achievable and expected in the professional segment.** [DOCUMENTED]
5. **A scripting API with "one script = one undoable action" semantics is shippable.** [DOCUMENTED]
6. **Local MCP integration with capability-level approval is shippable.** [DOCUMENTED]

### What Affinity does not prove

- That a general-purpose design application should omit layout or components — Affinity's user base does
  not demand them.
- That generation-from-scratch is viable. Affinity's own docs say it is not.

## Important Interaction Conventions

- `Esc` cancels the current *operation* (sizing, moving, creating) — operation-level, not
  document-level.
- `⌥`-click cycles overlapped objects.
- `⇧`-marquee = full-containment select; `⌃`-marquee = partial-intersection select.
- `⌘`+drag corner = resize from centre; `⇧`+drag corner = aspect lock; `⌃`+drag corner = rotate about
  opposite corner. This is a **three-way division of corner-handle modifiers** that Figma does not have.
- `.` cycles the selection box.
- `Alt` temporarily suspends snapping.
- Context toolbar is the home of operation-specific options; nothing is buried in a modal.

## Notable Differences From Other Products

| Behaviour | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| `Cmd/Ctrl+D` | Duplicate | Duplicate | **Deselect** | Duplicate |
| Duplicate | `Cmd+D` | — | `⌘J` | `⌘D` |
| Deselect | `Esc` | `Esc` | `⌘D` | `Esc` |
| Select-behind | `Cmd`-click deep select / Select-layer menu | — | `⌥`-click cycle | — |
| Marquee intersect mode | — | — | `⌃`-drag (partial) | — |
| Snap default | On | **Off** | On | Unknown |
| Snap disable | `Ctrl/Cmd` | `Ctrl/Cmd` | `Alt` | Unknown |
| Snap tolerance | not documented | **screen px ÷ zoom** | **screen tolerance, per preset** | not documented |
| Snap presets | on/off toggles | one toggle + grid | **7 named presets** | n/a |
| Layout engine | Absolute + constraints + auto layout | none | constraints + snapping | position panel + Magic Resize |
| Components | yes (5-part system) | none | **none** | Brand Kit + templates |
| Collaboration | yes | yes (sync) | **none** | yes |
| Prototyping | yes | no | **no** | limited (animate/present) |
| Scripting | Plugin API + MCP | SDK + Driver + starter kits | **JS scripting API + local MCP** | Apps SDK |
| Keyboard | 16 layouts, no rebinding | `kbd` strings | **full customisation** | fixed |

## Changelog / Evolution

| Change | Architectural signal |
|---|---|
| Designer / Photo / Publisher → **Affinity with Studios** | Product boundaries dissolved into configurable workspaces; the persona concept shipped. |
| **Vector / Pixel / Layout Studios shipped as distinct Studio types** | One document, multiple disciplines — the document model had to absorb all three. |
| **Canva AI Studio** added as a Studio | AI is a workspace, not a feature; also exposes the Canva integration surface. |
| **Typography Studio**, **Compositing Studio**, **Scripting Studio** added | Tools grouped by workflow rather than by medium. |
| **Container layers + constraints** (Affinity 2) | Responsive behaviour introduced into a tool that previously had none. |
| **Slice Studio** | Export promoted to a first-class object with multi-target output. |
| **Vector Flood Fill** and **vector warp nodes that obey snapping** | Vector tools extended into previously raster-only territory *while keeping snapping consistent*. |
| **Pencil tool in Layout Studio** | A vector tool in a layout workspace — discipline boundaries softened. |
| **Scripting (JavaScript, beta)** | A command API over the document. |
| **Workflow → script promotion from the assistant** | Agent runs become durable artifacts. |
| **Local MCP server (April '26)** | Document exposed to external agents at the operation level, with capability-level consent. |
| **Serif → Canva (Affinity acquired)** | Third-party AI generation integrated via a Studio, not a rewrite. |
| Serif used British English spellings (`centre`, `colour`) in its scripting API | A reminder that API naming is a long-term compatibility commitment. |

## Sources

**Tier 1 — Official product documentation (affinity.studio/help)**

- Snapping — https://www.affinity.studio/help/design-aids-snapping/
- Smart guides — https://www.affinity.studio/help/design-aids-dynamic-guides/
- Node Tool — https://www.affinity.studio/help/tools-tools-node/
- About Studios — https://www.affinity.studio/help/workspace-about-studios/
- Keyboard shortcuts for general editing — https://www.affinity.studio/help/workspace-shortcuts-editing/
- Getting started (index, used to enumerate all help topics) — https://www.affinity.studio/help/getting-started/
- Scripting in Affinity — https://www.affinity.studio/help/scripting-in-affinity/
- AI Automation with Claude (MCP) — https://www.affinity.studio/help/ai-connector-setup/
- Trusting scripts, Running scripts, Scripting examples, Tutorial: Swap Objects — index pages identified
  at https://www.affinity.studio/help/automation/
- Canva integrations — https://www.affinity.studio/help/canva-integrations/
- Release notes — https://www.affinity.studio/help/release-notes/

Note: affinity.studio/help rejects non-browser user agents; content was retrieved via a real browser.

**Tier 2 — Official engineering material**: none found. **Research gap.**

**Tier 3 — Source code**: closed-source. **N/A**

**Tier 4 — Third-party**

- Serif forum (forum.affinity.serif.com) — used only to confirm tool names and long-standing behaviours.
- YouTube tutorials (Node Tool, snapping, knife tool, shape builder) — used for feature existence only.

**Tier 5 — Community**

- Reddit r/AffinityDesigner — used to surface pain points (smart-guide confusion vs Illustrator, snapping
  predictability), marked as non-authoritative.

**Research gaps for Affinity**

- Rendering / performance architecture: entirely undocumented.
- Undo grouping semantics: undocumented.
- Frame presentation/stack view: not confirmed.
- Export format list: not enumerated.
- Vector Studio shortcut list: identified but not read (only the general-editing table was extracted).
- The scripting API reference at affin.co/affinity-sdk was **not** read. Its operation set is the single
  most valuable unread source for this research, because it is a shipped command API over a professional
  document model.
- Affinity's "no components" position is documented by absence; an explicit statement would be stronger.
