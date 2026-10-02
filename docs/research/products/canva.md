# Canva

## Overview

Canva is a template-first, brand-governed, web-native design platform. It is the reference product in
this research for **accessibility of a powerful design system**, **template-driven workflows**, and
**turning generated content into structured editable documents**.

Its deepest structural commitment is that most users never start from a blank canvas — they start from a
template, a Brand Kit, or an AI generation, and the system constrains what they can do so that the output
stays on-brand and structurally sane. [INFERRED from the documented feature set]

Scale note: Magic Studio has been used 5 billion times. [THIRD-PARTY — OpenAI case study citing Canva]

## Product Philosophy

1. **Start from a template, not a blank page.** Templates are the primary entry point; Magic Design
   generates customised templates from a prompt + your media. [DOCUMENTED]
2. **Brand governance is the ceiling, not a suggestion.** Brand Kits hold logos, colours, fonts, and
   assets; **Brand Controls** can restrict users to only colours/fonts present in Brand Kits.
   [DOCUMENTED]
3. **Every AI feature must land as editable elements, not pixels.** "Turn AI designs into editable
   layouts" is the headline AI feature (Magic Layers). [DOCUMENTED]
4. **One design, many outputs.** Magic Resize swaps formats, languages, and dimensions.
   [DOCUMENTED]
5. **Progressive disclosure.** Keyboard shortcuts are dense but discoverable via a single help page and
   `/`-quick-actions. [DOCUMENTED]

## Canvas Model

| Aspect | Behaviour | Evidence |
|---|---|---|
| Canvas shape | **Bounded by pages.** A design is a sequence of fixed-size pages | DOCUMENTED |
| Page | First-class: add, duplicate, delete, reorder, page break; scrolling/thumbnail/grid view | DOCUMENTED |
| Canvas outside pages | Canva does not present an infinite canvas for free placement | DOCUMENTED |
| Rulers/guides | Yes — `⇧R` toggles rulers and guides; `⌥⌘;` locks guides | DOCUMENTED |
| View modes | Scrolling view, Thumbnail view, Grid view, Presentation mode (`⌥⌘P`) | DOCUMENTED |
| Snap | Not documented as a user-facing global toggle | [DOCUMENTED absence] |

[INFERRED] Canva's canvas is fundamentally a **page stack with fixed-size artboards**, closer to
print/pre-press and slide tools than to Figma's infinite canvas. This is the strongest single constraint
on its interaction model: there is no "place an object anywhere", only "place an object on a page".

## Interaction Model

### Selection

- Click selects an element. Tap-and-hold on touch shows a menu with **Select Multiple**. [DOCUMENTED]
- `⇧`-click adds; `F8` toggles **element multi-select mode** so subsequent clicks don't deselect.
  [DOCUMENTED]
- **Directional multi-select**: `⇧W` up, `⇧A` left, `⇧S` down, `⇧D` right — "Multi-select closest".
  [DOCUMENTED] [INFERRED] This is tldraw's `selectAdjacentShape` model exposed directly to the user.
- `Tab` / `⇧Tab` select next/previous elements. [DOCUMENTED]
- `⌘A` select all, `Esc` deselect. [DOCUMENTED]

### Tools

Canva's creation model is **element-type buttons plus single-key shortcuts**, not a persistent tool
state: `T` text, `R` rectangle, `L` line, `⌘\` elbowed line, `C` circle, `S` sticky note, `⇧;` emoji.
[DOCUMENTED]

[INFERRED] There is no evidence of a persistent "current tool" concept. Creation is a one-shot command.
This is a deliberate simplification that removes tool-state complexity in exchange for less expressive
vector work.

### Transformation

| Action | Binding | Evidence |
|---|---|---|
| Move small | Arrow keys | DOCUMENTED |
| Move large | `⇧` + Arrow | DOCUMENTED |
| Rotate small | `⌥,` / `⌥.` (left/right) | DOCUMENTED |
| Rotate large | `⌥⇧,` / `⌥⇧.` | DOCUMENTED |
| Resize small | `⌘` + Arrow (per edge) | DOCUMENTED |
| Resize large | `⇧⌘` + Arrow | DOCUMENTED |
| Toggle proportional ↔ single-axis resize | **`F2`** | DOCUMENTED |
| Duplicate | `⌘D` | DOCUMENTED |
| Group / Ungroup | `⌘G` / `⇧⌘G` | DOCUMENTED |
| Justify elements-level | `⇧⌘J` | DOCUMENTED |
| Arrange forward / back / front / back-most | `⌘]` / `⌘[` / `⌥⌘]` / `⌥⌘[` | DOCUMENTED |
| Tidy up | `⌥⇧T` | DOCUMENTED |
| Lock element | `⌥⇧L` | DOCUMENTED |
| Lock guides | `⌥⌘;` | DOCUMENTED |
| Show layers | `⌥1` | DOCUMENTED |

[INFERRED] `F2` as a persistent proportional-resize toggle is a genuinely distinct design: instead of
holding a modifier during the gesture, you set a mode. This is the "modifier with memory" pattern that
tldraw implements via Alt-drag bail/re-apply, expressed as a toggle. Both are valid; the toggle is
discoverable and one-handed.

## Text

Canva's text model is the most *named-style*-driven of the four products. [DOCUMENTED]

- **Text styles**: Body text, Title (H1), Subtitle (H2), Heading (H3), Subheading (H4), Section header (H5),
  bound to `⌥⌘0`–`⌥⌘5`. Documented as available in **Canva Docs**.
- Vertical anchoring: `⌘⇧H` top, `⌘⇧M` middle, `⌘⇧B` bottom. [DOCUMENTED]
- Paragraph: bold `⌘B`, italic `⌘I`, underline `⌘U`, strikethrough `⇧⌘S`, subscript `⌘,`, superscript
  `⌘.`, uppercase `⇧⌘K`.
- Alignment: `⇧⌘L` / `⇧⌘C` / `⇧⌘R`.
- Spacing: `⌥⌘↑` / `⌥⌘↓` line spacing; `⌥⌘,` / `⌥⌘.` letter spacing.
- Size: `⇧⌘,` / `⇧⌘.`.
- Lists: numbered `⇧⌘7`, bulleted `⇧⌘8`.
- **Copy text style** `⌥⌘C` / **Paste text style** `⌥⌘V`. [DOCUMENTED]
- Font menu `⇧⌘-F`; Find & replace `⌘F`. [DOCUMENTED]

[INFERRED] "Copy text style / paste text style" as separate bindings is the Affinity convention and is a
strong indicator of a text-style system being a first-class document concept with a clipboard
representation.

## Drawing

Minimal. Lines and elbowed lines only (`L`, `⌘\`). No pen, pencil, or node editing is documented.
[DOCUMENTED absence] Canva compensates with **Shape Generator** (AI custom shapes) and a large
**Elements** library. [DOCUMENTED]

## Document Model

| Concept | Evidence |
|---|---|
| **Design** | The top-level document | DOCUMENTED |
| **Page** | Fixed-size, ordered, duplicable, deletable | DOCUMENTED |
| **Element** | The universal object type; Canva speaks of "elements", not "layers" or "objects" | DOCUMENTED |
| **Element type** | Text, shape, image, video, sticker, chart, QR code, table, … | DOCUMENTED |
| **Group** | Yes, via `⌘G` | DOCUMENTED |
| **Brand Kit** | Logos, colours, fonts, assets, **colour themes**, **Brand Templates**, linked folders | DOCUMENTED |
| **Design Templates** | Reusable whole designs | DOCUMENTED |
| **Folder / Project** | Organisation unit; can be linked to a Brand Kit | DOCUMENTED |
| **Comments** | First-class; `⌥⌘N` to add; `⌥⌘J` jumps to the comment on the selected element | DOCUMENTED |
| **Version** | Up to 1,000 saved versions per design | DOCUMENTED |
| **Trash** | 30-day recovery | DOCUMENTED |
| Components / instances / variants | **No structural component system.** | DOCUMENTED absence |
| Variables / modes | **No variables.** Brand Kit is a flat resource set | DOCUMENTED absence |

### Brand Kit in detail [DOCUMENTED]

- Types: **Team/organization Brand Kits** (managed by admins and brand designers, shared across teams)
  and **Personal Brand Kits** (private to one user, when enabled by an admin).
- Contents: brand logos, brand colours, **colour themes**, brand fonts, custom brand assets, **Brand
  Templates**, and **linked folders**.
- Limits: up to **1,000 Brand Kits per team**; up to **2,000 brand assets per category per Brand Kit**;
  one personal Brand Kit; personal kits don't count toward the team limit.
- **Brand Controls** restrict team members to only using colours and fonts from Brand Kits.
- Brand Kits can be created **automatically from a website or a PDF**.
- Separate roles exist: *admin* and *brand designer*.

[INFERRED] Canva's design-system model is: **a flat, permissioned resource pool with enforcement
("Brand Controls") rather than a binding system.** Nothing in a Canva design *references* a Brand Kit
colour token; the colour is chosen from an allowed list. Compare Figma's aliasing variables, which are
*referenced* and *switchable by mode*. Canva's approach is enforceable at the UI level and much simpler to
understand; Figma's is more expressive and requires a resolver.

This is one of the sharpest conceptual disagreements in the whole research:
**governance-by-constraint vs. governance-by-reference.**

## Hierarchy

- Layers panel (`⌥1`) with a flat-ish list; grouping via `⌘G`. [DOCUMENTED]
- Order via `⌘]` / `⌘[` / `⌥⌘]` / `⌥⌘[`. [DOCUMENTED]
- `⌥⇧T` "Tidy up" — aligns and distributes the selection. [DOCUMENTED]
- `⇧⌘J` "Justify elements-level". [DOCUMENTED]

[INFERRED] Canva's hierarchy is shallow: groups exist but there is no documented nested-container
model with independent coordinate space, no auto layout, and no constraints. Depth exists mainly for
organisation.

## Components / Reuse

There is no component/instance/variant system. Reuse is delivered by:

1. **Design Templates** — whole designs, including **Brand Templates** inside a Brand Kit.
2. **Elements library** — built-in and uploaded.
3. **Brand Kit** — logos, colours, fonts, assets, colour themes.
4. **Copy text style / paste text style.**
5. **Apps** — third-party integrations.

[INFERRED] This is a **template-and-substitution** reuse model rather than a **definition-and-instance**
model. The user never edits a master; they copy a design and edit the copy. The consequences are:
no global updates, no override semantics, no variant switching — but also far less confusion, and no
"detached instance" failure mode.

For Spool this is a genuine architectural fork, not a bug in Canva.

## Layout

Canva has **no flow layout engine** and **no constraints system**. Positioning is:

- Free placement within a bounded page.
- A **Position** panel with numeric X/Y/W/H and **relative positioning** (left/centre/right × top/
  middle/bottom).
- Alignment tools.
- Guides and rulers (`⇧R`).
- **Magic Resize** — an *AI* operation that reflows a whole design to a new format/language/dimension.
  [DOCUMENTED as an AI tool, not a layout engine]

[INFERRED] The Position panel's "relative" alignment modes are a lightweight, template-appropriate
substitute for constraints: rather than declaring "this stays left when the container resizes", you
declare "this is left-aligned within its frame". A page in Canva never resizes, so a declarative resize
rule would be unnecessary.

## Styles

- Named text styles (Body, Title, Subtitle, Heading, Subheading, Section header). [DOCUMENTED]
- Brand colours and colour themes from the Brand Kit. [DOCUMENTED]
- Copy/paste text style. [DOCUMENTED]
- **No effects/blend-mode system is documented.** **Unknown** — Canva does support shadows and some
  image effects, but the model is undocumented in the reviewed pages.
- **No variables/tokens.** [DOCUMENTED absence]

## Assets

- **Uploads**: images, videos, and the Trash explicitly tracks "Images" and "Videos" tabs.
  [DOCUMENTED]
- **Elements** library: photos, videos, graphics, stickers, charts, QR codes, audio.
- **Brand Kit assets** + **linked folders**. [DOCUMENTED]
- **Stock/AI assets**: Magic Media, AI image generator, AI video generator, AI-Powered Elements, Shape
  Generator, 3D Content Generator. [DOCUMENTED]
- **Asset referencing rules are documented**: "Some images won't delete — Images or videos still used in
  designs won't be fully deleted until the design is removed." [DOCUMENTED] [INFERRED] This is a
  reference-counting constraint: assets have references from designs and cannot be deleted while
  referenced. This is a real asset-graph requirement.

## History

Canva has **two entirely separate recovery systems**, and the documentation is explicit about the
difference.

### 1. Local undo/redo

- Undo `⌘Z`; Redo `⌘Y` or `⇧⌘Z`. [DOCUMENTED]
- Grouping semantics: **Unknown.**

### 2. Server-side Version History [DOCUMENTED — "Review and restore older versions of designs"]

- "You can restore up to **1,000 previous versions**, with **no time limit** on how long versions are
  kept."
- Requires the design owner or edit access; view/comment access cannot see version history.
- File ▸ Version history → compare saved versions → **Restore** or **Make a copy**.
- **Avatars appear next to each saved version, indicating who edited it.**
- Available on Canva Pro, Teams, Business, Education, Nonprofits; **not** available when designing via the
  Canva Button on a partner website.
- Free users have no access.

### 3. Trash [DOCUMENTED]

- 30-day recovery; empty trash is permanent; support can recover within 14 days; deleted images and
  videos cannot be restored.
- Explicitly distinguished from Version History: "Version History lets you view and restore previous
  versions of your design. Trash lets you recover designs that were deleted entirely from your account."

[INFERRED] Canva's Version History is **snapshot-based, server-side, attributed to authors, and unbounded
in time (up to 1,000 entries)**. Figma's is also snapshot-based and server-side with named versions and
branching. Neither is described as operation-based. This is strong cross-product evidence that
**long-horizon recovery and short-horizon undo are architecturally distinct systems**, even though users
use the words interchangeably.

## Keyboard / Input

Fully documented shortcut reference. [DOCUMENTED — "Canva keyboard shortcuts"]

### Notable details

- **Quick actions**: `/` or `⌘E`. [DOCUMENTED]
- **Open More actions menu for a selected element**: `⇧10`. [DOCUMENTED]
- **Focus movement is explicit and documented**: `⌘F1` Editor toolbar, `⌘F2` Canvas, `⌘F3` Side panel.
  [DOCUMENTED] This is the most accessibility-conscious focus model of the four products.
- **Surface switching**: `⌘F6` next surface, `⇧⌘F6` previous. [DOCUMENTED]
- **Go to page**: `⌥⌘G`. **Add page break**: `⌘⏎`. **Empty page**: `⌘↵`. **Delete empty page**: `⌘Delete`.
  [DOCUMENTED]
- **Comments**: `⌥⌘N` add, `⇧;` emoji, `⌘⏎` submit. [DOCUMENTED]
- **Zoom**: `⌘+` / `⌘-` / `⌘0` actual size / `⌥⌘0` zoom to fit / `⇧⌘0` zoom to fill. [DOCUMENTED]
- **View**: `⇧R` rulers+guides, `⌥⌘1/2/3` scrolling/thumbnail/grid, `⌥⌘P` presentation. [DOCUMENTED]

[INFERRED] Canva's shortcut set is *heuristically typed*: single letters for element insertion, `⌘`
for view/zoom/file, `⌥⌘` for structural, `⇧⌘` for style. It follows a consistent grammar, which makes
it learnable without a manual. Note `⌘F1..F3` for focus — a pattern Spool would benefit from.

**No user shortcut customisation is documented.** [DOCUMENTED absence]

## Rendering / Performance

Not publicly documented. **Unknown.** Canva is a large web application with a published OpenAI case study
about vision-based understanding, which implies server-side image/document analysis is part of the
infrastructure. [THIRD-PARTY]

## Collaboration

- Multiuser editing with live presence. [DOCUMENTED]
- **Version attribution via avatars per saved version.** [DOCUMENTED]
- **Comments anchored to elements**: "Jump to comment on selected element" (`⌥⌘J`). [DOCUMENTED]
  [INFERRED] Element-anchored comments require a persistent comment→element reference, i.e. the same
  "binding" shape tldraw has for arrows.
- Roles: owner, edit, view, comment. [DOCUMENTED]
- Brand Kit roles: admin, brand designer, member. [DOCUMENTED]

## Prototyping

Canva has **present and animate** capabilities, not a prototype graph. [DOCUMENTED at product level]

- **Presentation mode** (`⌥⌘P`).
- **Scrolling view** for long pages.
- Animation features exist (Magic Studio mentions "apply animations"), but **no trigger/action/flow
  model is documented**. [Unknown]

[INFERRED] Canva treats "presentation" as a view mode over pages, not as an interaction graph. This is a
meaningful product decision: for a content/marketing audience, page-to-page transitions are the
requirement, not conditional logic.

## Export

- Multi-format export of the current design (PNG, JPG, PDF, MP4, GIF, PPT and more) — [DOCUMENTED at
  marketing level]
- **Magic Resize** — "Swap formats, languages, and dimensions in a snap." [DOCUMENTED] This is an
  *AI-driven re-layout of an existing design into a new format*, not a scale multiplier.
- Export is per-design or per-element (contextual "Download" on an element). [DOCUMENTED]

[INFERRED] Canva's export is the least architecturally revealing of the four. Magic Resize is the notable
part: it is a proof that reflowing a design between aspect ratios is an AI-solvable problem today,
which raises the question of whether a deterministic layout engine is required at all.

## AI

Canva's AI surface is the most productised of the four, and its **output-shape strategy** is the key
finding.

### Generation

| Capability | Output shape | Evidence |
|---|---|---|
| Magic Design | **Template** customised from prompt + your media | DOCUMENTED |
| AI image generator / Photo Generator | Raster image | DOCUMENTED |
| Video Generator / Magic Clips | Raster video clip | DOCUMENTED |
| 3D Content Generator | 3D content | DOCUMENTED |
| Shape Generator | **Vector-ish shape** | DOCUMENTED |
| AI-Powered Elements | Library elements | DOCUMENTED |
| Magic Write | **Text** | DOCUMENTED |
| Magic Background | Background | DOCUMENTED |
| AI-Powered Templates | Templates | DOCUMENTED |
| Interactive Activities | Educational content | DOCUMENTED |
| Magic Edit | **Masked region replacement** — "Brush over part of a photo and describe what to replace it with" | DOCUMENTED |
| **Magic Layers** | **Editable layout** — see below | DOCUMENTED |
| **Canva AI 2.0** | Conversational design across the Visual Suite | DOCUMENTED |
| Canva Code | Code | DOCUMENTED |
| Style Match | Restyle an existing design | DOCUMENTED |

### Magic Layers — flat pixels to structured document

[DOCUMENTED — canva.com/magic-layers]

> "Upload your flat design, and Magic Layers instantly transforms it into a layout you can select, move,
> and edit."
> "Fix any element — Now that you've freed the design elements, move and tweak them however you like.
> Change colors […] Edit text directly — Fix the typo, change the font, and say what you want to say."
> "**No more do-overs — Don't regenerate your design to change one element — let Magic Layers do it for
> you.**"
> "Works with any AI image generator, including ChatGPT, Gemini and even with our very own Canva AI."
> Best results with JPEG and PNG. Counts against the monthly AI allowance. Beta.

[INFERRED] Magic Layers is **image segmentation + text recognition → document objects**. It is a
*retrieval* path into the structured document model, not a generative path. This is architecturally
significant: it proves the structured document model is the canonical form, and that even flat generated
images are expected to be *pulled into* it rather than being an end state.

It also implies the document model must tolerate **imperfect** recovered geometry and text — with an
explicit notion of "these objects were inferred" — a requirement Spool would inherit the moment it
supports any AI generation.

### Canva AI 2.0 — conversational, context-bearing [DOCUMENTED — canva.com/canva-ai]

- "Describe what you want to create with **text or voice**."
- "Canva AI 2.0 can **pull context from your connected tools** to generate on-brand designs — all through
  a conversation."
- "**Chat with Canva AI 2.0 to refine your design, or make edits yourself. You're always in control.**"
- "**Generate elements** — Need an image, a chart, or headline? Just ask Canva AI 2.0, and easily add it to
  your design."
- "**Make it on brand** — Canva AI gets to know your brand's fonts, colours, and rules so every design
  stays on brand."
- "**Run web research** — Search for anything and bring the insights straight into your design."
- "**Learns with you over time** — Canva learns how you create to personalize every output to your unique
  style."
- "**Schedule tasks for later** — Schedule Canva AI tasks to run at a later date or on a recurring
  schedule."

[INFERRED] Five context sources are named: (1) connected external tools, (2) brand system, (3) the current
design, (4) web research, (5) the user's own style history. Plus a **scheduled/recurring execution**
model. Plus an explicit statement that direct manual editing and conversational editing are **equally
first-class** — which is a product statement about coexistence, not replacement.

### Brand Kit as AI context

[DOCUMENTED] "Canva AI gets to know your brand's fonts, colours, and rules so every design stays on
brand." The Brand Kit is therefore not only a UI palette — it is the **primary grounding context for
generation**. Affinity's MCP integration goes further: "Use Canva AI Studio features" is an explicitly
toggled MCP capability that consumes the Canva plan's AI allowance.

[INFERRED] Across Canva and Affinity, **the brand system has become the AI's context provider.** This is
the single most important AI-context pattern found: an AI-native design tool should treat its
design-system/brand/variable system as first-class agent context, not as a cosmetic layer.

## Architectural Observations

### Evidence-backed concepts

| Concept | Problem | Persistent? | In history? | Affects render? | Affects input? | AI-relevant? | Evidence |
|---|---|---|---|---|---|---|---|
| **Bounded page stack** | Content must live in fixed frames | **Yes** | Yes | Yes | **Yes** (no free canvas) | Yes | DOCUMENTED |
| **Element** (universal object) | One concept instead of many | **Yes** | Yes | Yes | Yes | **Yes** | DOCUMENTED |
| **Design Template** | Entry point + reuse | **Yes** | Yes (as a version) | Yes | No | **Yes** | DOCUMENTED |
| **Brand Kit** | Governance + reuse + AI context | **Yes** | Yes | No | Yes | **Yes** | DOCUMENTED |
| **Brand Controls** | Enforce governance at the UI | Yes (config) | Yes | No | **Yes** | Yes | DOCUMENTED |
| **Colour theme** | Named multi-colour palette | **Yes** | Yes | Yes | Yes | Yes | DOCUMENTED |
| **Named text style + copy/paste style** | Typography reuse with a clipboard form | **Yes** | Yes | Yes | Yes | Yes | DOCUMENTED |
| **Relative position modes** | Alignment without constraints | **Yes** | Yes | Yes | Yes | Yes | DOCUMENTED |
| **Proportional-resize toggle (F2)** | Modifier with memory, as a mode | No (transient) | No | Yes | **Yes** | Yes | DOCUMENTED |
| **Directional multi-select (⇧WASD)** | Navigate-by-neighbour selection | No | No | No | **Yes** | Yes | DOCUMENTED |
| **Focus model (⌘F1/F2/F3)** | Explicit focus movement for a11y | No | No | No | **Yes** | Yes | DOCUMENTED |
| **Element-anchored comment** | Persistent comment→element reference | **Yes** | Yes | No | Yes | Yes | DOCUMENTED |
| **Asset reference counting** | Assets can't be deleted while in use | **Yes** | Yes | Yes | No | Yes | DOCUMENTED |
| **Version History (1,000, attributed)** | Long-horizon recovery | **Yes** (server) | It *is* history | No | No | No | DOCUMENTED |
| **Trash (30 days)** | Whole-object recovery | Yes (server) | No | No | No | No | DOCUMENTED |
| **Magic Layers** | Flat image → structured document | **Yes** (output) | Yes (as a version) | Yes | No | **Yes** | DOCUMENTED |
| **Magic Resize** | AI reflow across formats/languages | **Yes** (output) | Yes | Yes | No | **Yes** | DOCUMENTED |
| **Scheduled/recurring AI tasks** | Asynchronous agent execution | No (runtime) | No | Yes | No | **Yes** | DOCUMENTED |
| **Brand-as-context** | Grounding for generation | **Yes** | Yes | No | No | **Yes** | DOCUMENTED |

### What Canva proves

1. **A design platform can be extremely powerful and still approachable** if the entry point is a
   template and the palette is governed. [INFERRED]
2. **Governance-by-constraint works** and is materially simpler than governance-by-reference. [INFERRED]
3. **AI output is expected to become structured document objects**, and the industry has a shipped
   product (Magic Layers) doing exactly that. [DOCUMENTED]
4. **Template-based reuse can substitute for components** in content/marketing contexts. [INFERRED]
5. **A brand system is the natural grounding context for AI generation.** [INFERRED from documented
   behaviour across Canva and Affinity]
6. **Long-horizon version history must be a separate, attributed, server-side system.** [INFERRED]
7. **Focus movement should be explicit and documented** for accessibility. [DOCUMENTED]

### What Canva does not prove

- That omitting a component system is right for a general-purpose tool. Canva's users do not need it.
- That AI reflow replaces a layout engine. Magic Resize is an AI feature layered *on top of* free
  positioning, not a replacement for one.
- That bounded pages are sufficient. Canva cannot express "a flow layout that adapts", so it uses AI to
  bridge.

## Important Interaction Conventions

- `Esc` deselects; `⌘A` selects all.
- `F2` toggles a mode rather than holding a modifier.
- `⇧W/A/S/D` navigates and extends the selection by direction.
- `⌘F1/F2/F3` move focus between editor chrome and canvas.
- `/` opens quick actions.
- Single-key element creation (`T R L C S`) is the primary creation path — not a tool palette.
- Grouping and ordering use the Figma-standard `⌘G` / `⌘[` `⌘]` bindings, which suggests cross-tool
  convention convergence even among competitors.

## Notable Differences From Other Products

| Behaviour | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Canvas | Infinite | Infinite | Bounded pages/spreads | **Bounded page stack** |
| Layout | Absolute + constraints + auto layout | none | constraints + snapping | free placement + relative modes |
| Reuse model | components/instances/variants | custom shapes + copy | styles + assets + scripts | **templates + Brand Kit** |
| Governance | variables + modes (reference) | — | — | **Brand Controls (constraint)** |
| Snap | on | off | on + 7 presets | not a documented global |
| Resize aspect | `Shift` | `Shift` | `Shift` | **`F2` mode** |
| Selection growth | `Shift`-click / marquee | brushing | `⇧`/`⌃`-marquee, `⌥`-click cycle | **`⇧WASD` directional** |
| Focus | actions menu | editor focus | context toolbar | **`⌘F1/F2/F3`** |
| History | local undo + named versions + branches | marks/bail/squash | `⌘Z` | **undo + 1,000 attributed versions + Trash** |
| AI output | design layers (agent) / code (Make) | shapes via agent + driver | **document commands via MCP** | **editable layouts (Magic Layers)** |
| Collaboration | yes | yes | **no** | yes |
| Prototyping | yes | no | **no** | present-mode only |

## Changelog / Evolution

| Change | Architectural signal |
|---|---|
| Templates → **AI-Powered Templates / Magic Design** | The template is now generated per-user from prompt + their own media. The template concept absorbed generation. |
| Brand Kit → **Brand Kit + colour themes + Brand Templates + linked folders + automatic setup from a website/PDF** | The brand system grew from a palette to a governance platform with roles, limits, and enforcement. |
| Static tools → **AI-Powered Photo Editor ("point and click editing")**, **Magic Edit** (brush + prompt), **Style Match**, **Resize** | AI became **contextual and in-place** rather than a separate generation surface. |
| Flat AI images → **Magic Layers** ("Turn AI designs into editable layouts") | The company explicitly recognised that *flat pixels are a failure mode* and shipped the conversion path. |
| One-off prompts → **Canva AI 2.0 conversational**, with connected-tool context, web research, scheduled/recurring tasks, and style personalisation | AI moved from a **feature** to a **mode** with memory, external context, and temporal execution. |
| Local undo only → **Version History with per-version author avatars** | Recovery gained attribution; collaboration became visible in history. |
| Multiple page view modes → **Scrolling / Thumbnail / Grid / Presentation** | Explicit, keyboard-bound view states; a small but real "view profile" concept. |

## Sources

**Tier 1 — Official product documentation and marketing (canva.com/help, canva.com)**

- Review and restore older versions of designs (Version history) — https://www.canva.com/help/version-history/
- Restore or delete designs and files from Trash — https://www.canva.com/help/deleted-designs/
- Set up Brand Kits — https://www.canva.com/help/brand-kit/
- Canva keyboard shortcuts — https://www.canva.com/help/canva-keyboard-shortcuts/
- Use Magic Edit to add, replace, and modify photos — https://www.canva.com/help/using-magic-edit/
- Use Magic Design to generate design templates — https://www.canva.com/help/use-magic-design/
- Add, duplicate, and delete elements — https://www.canva.com/help/add-elements/
- Add, duplicate, and delete pages — https://www.canva.com/help/manage-pages/
- Make any design editable — Magic Layers — https://www.canva.com/magic-layers/
- Canva AI 2.0 — https://www.canva.com/canva-ai/
- Magic Design — https://www.canva.com/magic-design/
- Use Magic Studio safely and legally — https://www.canva.com/help/using-magic-studio-safely-and-legally/

Note: canva.com/help rejects non-browser user agents; content was retrieved via a real browser.

**Tier 2 — Official engineering material**: not found for rendering/history architecture. **Gap.**

**Tier 3 — Source code**: closed-source. **N/A**

**Tier 4 — Third-party**

- OpenAI case study, "Canva's AI-powered Magic Studio Used 5 Billion Times" — https://openai.com/index/canva/
  Used for scale and for the observation that Canva uses vision to "parse and understand the content of
  designs, from whiteboards to presentations."
- Shopify blog "How to Use Canva AI: 10 Magic Studio Features (2025)", Techopedia/skillademia shortcut
  aggregators — used only to cross-check shortcut bindings, never as primary evidence.

**Tier 5 — Community**

- Reddit r/canva threads on version-history limits and Free-plan restrictions — used only for pain-point
  discovery (Free users cannot see version history at all).

**Research gaps for Canva**

- Rendering/performance architecture: entirely undocumented.
- Undo grouping semantics: undocumented.
- Effects/blend-mode model: undocumented.
- Whether Canva's element model has any structural equivalent to instances: undocumented.
- Whether Magic Layers marks recovered elements as inferred: not stated publicly.
- The Apps SDK / Connect APIs were not examined; they are Canva's closest analogue to a plugin system.
