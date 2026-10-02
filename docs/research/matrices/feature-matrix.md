# Feature Matrix

> §32. Cross-product comparison of major capability. **No ranking** — the purpose is to expose
> differences and the reasons they may exist.
>
> Legend: **✔** documented or source-confirmed · **◐** partial / constrained · **—** absent ·
> **?** not documented
> Evidence labels are given in the right-hand column of each table where the claim is not obvious.

---

## 1. Canvas and navigation

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Infinite canvas | ✔ | ✔ | ✔ | — | [DOCUMENTED] Canva is a bounded page stack [DOCUMENTED] |
| Pages | ✔ | ◐ (pages exist) | ✔ | ✔ | |
| Frames / artboards | ✔ | ◐ | ✔ | ◐ | Frame is Figma's universal container [DOCUMENTED] |
| Zoom range clamp | ✔ | ✔ | ✔ | ✔ | Spool: `0.1`–`4.0` [SOURCE-CODE] |
| Zoom to fit | `⇧1` | `⇧1` | ✔ | `⌥⌘0` | |
| Zoom to selection | `⇧2` | `⇧2` | ✔ | — | |
| Zoom to 100% | `⇧0` | `⇧0` | ✔ | `⌘0` | |
| Zoom about cursor | ✔ | ✔ | ✔ | ✔ | |
| Minimap | — | — | — | — | **[OBSERVED] absent in all four** |
| Rulers | — | — | ✔ | — | |
| Guides as document objects | — | — | ✔ | — | [DOCUMENTED] |
| Grid (visual) | ✔ | ✔ | ✔ (pixel grid) | — | |
| Named view modes | — | — | — | ✔ | Canva Scrolling/Thumbnail/Grid/Presentation `⌥⌘1/2/3/P` [DOCUMENTED] |
| Camera in shared state | ◐ | ✔ | — | — | tldraw share links carry camera [SOURCE-CODE] |

## 2. Selection

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Single / multi-select | ✔ | ✔ | ✔ | ✔ | |
| Shift-additive | ✔ | ✔ | ✔ | ✔ | |
| Marquee (rectangular) | ✔ | ◐ | ✔ | ✔ | tldraw uses a **lasso** [SOURCE-CODE] |
| Marquee intersect vs contain | ◐ varies | ◐ | ✔ explicit: `⇧` contain, `⌃` intersect | ? | Affinity is the clearest [DOCUMENTED] |
| Select behind | `⌥`-click | `⌥`-click | `⌥`-click **cycles** | ? | Affinity's cycle is a different mechanism [DOCUMENTED] |
| Selection depth / nesting | ✔ `nestingDepth` | ✔ `focusedGroupId` | ✔ Node Tool | ✔ 3 focus modes | |
| `Enter` descends / `⇧Enter` ascends | ✔ | ◐ | ✔ `A` mode | — | |
| Keyboard traversal | ✔ | ✔ **geometric** | ◐ | ✔ | tldraw scores by distance + off-axis [SOURCE-CODE] |
| Directional multi-select | — | — | — | ✔ `⇧W/A/S/D` | **unique to Canva** [DOCUMENTED] |
| Locked objects | ✔ | ✔ | ✔ | ✔ | |
| Hidden objects | ✔ | ✔ | ✔ | ✔ | |
| Selection is transient state | ? | ✔ explicit | ? | ? | tldraw: selection in instance state [SOURCE-CODE] |
| Selection in undo history | ✘ | ✘ | ✘ | ✘ | **none of the four** |

## 3. Transformation

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Move | ✔ | ✔ | ✔ | ✔ | |
| Resize, 8 handles | ✔ | ✔ | ✔ | ✔ | |
| Rotate | ✔ | ✔ | ✔ | ✔ | |
| Skew / shear | — | — | ✔ | — | **Affinity only** [DOCUMENTED] |
| Scale tool (non-layout) | ✔ | — | ✔ | — | |
| `⇧` constrain axis | ✔ | ✔ | ✔ | — | |
| `⇧` preserve aspect | ✔ | ✔ | ✔ | via `F2` mode | |
| `⌥` duplicate on drag | ✔ | ◐ | — | — | |
| `⌥` resize about centre | ✔ | — | ✔ (3-way split) | — | Affinity splits corner modifiers **three** ways [DOCUMENTED] |
| `⇧` 15° rotate increments | ✔ | — | ✔ | — | |
| `⌘` constrain to 45° | — | ✔ | — | — | tldraw [SOURCE-CODE] |
| Numeric transform entry | ✔ | ◐ | ✔ | ✔ **relative modes** | Canva's panel is the most accessible [DOCUMENTED] |
| Pivot control | ✔ | — | ✔ (movable node origin) | — | |
| Group transform | ✔ | ✔ | ✔ | ✔ | |
| Nested transform | ✔ | ✔ | ✔ | ? | |
| Alignment | ✔ | ✔ | ✔ | ✔ | |
| Duplicate-and-move | ✔ | — | ✔ `⌘D` | ✔ `⌘D` | |

## 4. Snapping

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Object/edge snapping | ✔ | ✔ | ✔ | ◐ | |
| Centre snapping | ✔ | ✔ | ✔ | ? | |
| Grid snapping | — | ✔ | ✔ (pixel) | ? | |
| Guide snapping | — | — | ✔ | — | |
| Path/handle snapping | — | ✔ | — | — | tldraw `getHandleSnapGeometry` [SOURCE-CODE] |
| Angle/construction snapping | — | — | ✔ **different algorithm** | — | [DOCUMENTED] |
| Stated tolerance unit | ? | ✔ **screen px ÷ zoom** (8) | ? | ? | tldraw is the only one that states a unit |
| Snap → pure nudge return | ? | ✔ | ? | ? | |
| Colour-coded indicators | ? | ✔ | ✔ | ? | |
| Distance labels on guides | — | — | ✔ | — | [DOCUMENTED] |
| Indicator merging / dedupe | — | ✔ | — | — | [SOURCE-CODE] |
| Named snapping presets | — | — | ✔ **7 presets** | — | **[DOCUMENTED] the shipped profile precedent** |
| Per-layer snapping opt-out | — | — | ✔ `Exclude From Snapping` | — | **[DOCUMENTED] a document field, not a preference** |
| Snap modifier to disable | ✔ | ✔ (`Esc`) | ✔ | ? | |
| Applies during rotation | ✔ | — | ✔ | ? | |
| Applies during node editing | — | — | ✔ (construction snapping) | — | |

## 5. Text

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Click-to-type | ✔ | ✔ | ✔ | ✔ | |
| Drag-for-fixed-width box | ✔ | ✔ | ✔ | ✔ | **gesture determines text behaviour** [DOCUMENTED] |
| Auto width / auto height | ✔ | ◐ | ✔ | ✔ | |
| Rich text (per-range) | ✔ | ◐ | ✔ | ✔ | |
| Font selection | ✔ | — | ✔ | ✔ | |
| Font loading / missing fonts | ? | ? | ? | ? | **[all four undocumented]** |
| Line height control | ✔ | hard-coded | ✔ | ✔ | tldraw hard-codes [SOURCE-CODE]; Spool hard-codes `18.0` |
| Letter spacing, alignment | ✔ | ◐ | ✔ | ✔ | |
| Named text styles | ✔ | — | ✔ | ✔ `⌥⌘0`–`⌥⌘5` | |
| Variable fonts | ? | — | ? | ? | **[all four undocumented]** |
| IME / CJK input | ✔ | ✔ | ✔ | ✔ | |
| Text transform (resize-as-type) | ✔ | ◐ | ✔ | ◐ | |
| Text in containers | ✔ | ✔ | ✔ | ✔ | |
| Text in components | ✔ | — | — | — | |

## 6. Document model

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Document | ✔ | ✔ | ✔ | ✔ | ✔ (not serialisable) |
| Page | ✔ | ✔ | ✔ | ✔ (bounded stack) | ◐ (4 id ranges) |
| Frame / artboard | ✔ (universal) | ◐ | ✔ | ◐ | ✔ |
| Group | ✔ (derived bounds) | ✔ | ✔ | ✔ | — |
| Shape | ✔ | ✔ (extensible) | ✔ | ✔ | ✔ (4 types) |
| Text | ✔ | ✔ | ✔ | ✔ | ✔ |
| Image | ✔ | ✔ | ✔ | ✔ | — |
| Component / instance | ✔ | — | — | — | — |
| Variants | ✔ | — | — | — | — |
| Variables / tokens | ✔ (6 kinds) | — | — | — | — |
| Styles (reusable) | ✔ | — | ✔ | ✔ (text) | — |
| Assets (images, fonts) | ✔ | ✔ | ✔ | ✔ | — |
| Bindings | ✔ | ✔ (records) | ✔ | — | — |
| Constraints | ✔ | — | ✔ | ◐ (relative) | — |
| Layout (auto) | ✔ | — | ✔ (Layout Studio) | ◐ | — |
| Path / vector nodes | ◐ (pen) | — | ✔ (Node Tool) | — | — |
| Prototype graph | ✔ | — | — | ◐ | — |
| **Serialisation** | ✔ `.fig` | ✔ JSON/SVG/PNG | ✔ | ✔ | **— none** |
| Branded typed ids | — | ✔ `shape:abc` | — | — | `ObjectId(pub u64)` |

## 7. Components, layout, styles

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Components / instances | ✔ | — | — | — | **[DOCUMENTED] absent from 3 of 4** |
| Component properties / overrides | ✔ | — | — | — | |
| Libraries / publishing | ✔ | — | — | — | |
| Local vs external components | ✔ | — | — | — | |
| Absolute positioning | ✔ | ✔ | ✔ | ✔ | |
| Constraints | ✔ | — | ✔ | ◐ | |
| Auto layout / stacking | ✔ | — | ✔ | ◐ | |
| Layout escape hatch | ✔ `Ignore auto layout` | — | — | — | |
| Fill / hug / fixed sizing | ✔ | — | — | — | |
| Min / max dimensions | ✔ | — | ✔ | — | |
| Named colour styles | ✔ | — | ✔ | ✔ (Brand Kit) | |
| Gradients | ✔ | — | ✔ | ✔ | |
| Shadows, blur | ✔ | — | ✔ | ✔ | |
| Opacity | ✔ | — | ✔ | ✔ | |
| Corner radius | ✔ | — | ✔ | ✔ | |
| 27 blend modes | — | — | ✔ | ◐ | [DOCUMENTED] |
| Variables driving paint/typography/layout | ✔ | — | — | — | **[Figma only]** |
| Brand governance by constraint | — | — | — | ✔ **Brand Controls** | **copy, not reference** [DOCUMENTED] |

## 8. Creative

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Pen tool (Bezier) | ✔ | — | ✔ | — | |
| Pencil / freehand | — | ✔ simplified | ✔ (pressure) | — | |
| Node editing | ◐ | — | ✔ | — | |
| Boolean path ops | ◐ (via plugins) | — | ✔ `⌘J` etc. | — | |
| Compound paths | ◐ | — | ✔ | — | |
| Winding fill mode | — | — | ✔ | — | |
| Crop / mask | ✔ | — | ✔ | ✔ | |
| Raster filters | — | — | ✔ | ✔ | |
| Background removal | ✔ | — | ✔ | ✔ | |
| Brush with pressure | — | — | ✔ | — | |
| Mesh gradients | ? | — | ✔ | ? | |
| Typography controls | ✔ | ◐ | ✔ | ✔ | |

## 9. History and versioning

| Capability | Figma | tldraw | Affinity | Canva | Notes |
|---|---|---|---|---|---|
| Undo / redo | ✔ | ✔ | ✔ | ✔ `⌘Y`/`⇧⌘Z` | |
| Command representation | ? internal | ✔ **diffs + marks** | ? | ? | |
| Bail (escape a live gesture) | ✔ (discard) | ✔ **`Alt`-bail** | ✔ | ✔ | |
| Squash | ✔ (implicit) | ✔ **`crop`** | ? | ? | |
| Compound operations | ✔ | ✔ `editor.run` | ✔ "often a single action" | ✔ | |
| Text session = one step | ✔ | ✔ | ✔ | ✔ | |
| Selection in history | — | — | — | — | **[all four exclude]** |
| Camera in history | — | — | — | — | **[all four exclude]** |
| Pause history | — | ✔ 3 capture modes | ? | ? | |
| **Version history** | ✔ | — | ✔ | ✔ | **separate system from undo** |
| Version authorship | ✔ | — | ✔ | ✔ **avatars** | |
| **Branching** | ✔ | — | ? | ✘ | **[DOCUMENTED] Figma branches; Canva does not** |
| Version retention | time-limited | — | ? | **1000 versions, no time limit** | |
| Trash retention | 30 days | — | ? | **30 days** | |

## 10. Input, keyboard, extensibility

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| User-remappable shortcuts | **—** | ✔ host-defined | ✔ **full** | — | — |
| Keyboard layouts | ✔ **16** | ✔ | ✔ ("international and selected") | ✔ | — |
| Focus-mode ladder | ✔ `⌘F1/2/3` | — | ✔ `A` | ✔ `⌘F1/2/3` | — |
| Screen-reader-friendly focus | ◐ | ? | ◐ | ✔ **documented** | — |
| Middle-drag pan | ✔ | ✔ | ✔ | ? | ✔ |
| Space-drag pan | ✔ | ✔ | ✔ | ? | ✔ |
| Pinch zoom | ✔ | ✔ | ◐ | ? | `PinchEvent` exists |
| Coarse-pointer awareness | ? | ✔ | ◐ | ✔ | **—** |
| Stylus / pressure | ◐ | ✔ `pressure` | ✔ | ◐ | **— (framework gap)** |
| Coalesced move events | ? | ✔ per-tool | ? | ? | **—** |
| Action / command registry | ? internal | ✔ **~100 with `kbd`** | ✔ scripting API | ✔ `/` palette | 12 text actions |
| Scripting / plugin API | ✔ plugins | ✔ (SDK) | ✔ **JavaScript** | ✔ apps | — |
| MCP server | ✔ | — | ✔ local | — | — |

## 11. AI

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Generate whole design | ✔ | ◐ | ✔ | ✔ | — |
| Generate image / illustration | ✔ | — | ✔ | ✔ | — |
| **Generate editable structure** | ✔ Design Agent | ✔ **native shapes** | ✔ | ✔ **Magic Layers** | — |
| Modify selection | ✔ | ✔ | ✔ | ✔ | — |
| Rewrite text | ✔ | ◐ | ✔ | ✔ | — |
| Change styles | ✔ | ◐ | ✔ | ✔ | — |
| Remove background | ✔ | — | ✔ | ✔ | — |
| Conversational editing | ✔ | ✔ chat | ✔ | ✔ | `ai_inspector` is chrome only |
| Scheduled / background tasks | — | — | — | ✔ | — |
| Agent surface (MCP) | ✔ | ✔ (SDK) | ✔ local | ? | — |
| Per-operation approval | ? | — | ✔ | ? | — |
| Agent changes undoable normally | ✔ | ✔ | ✔ | ✔ | — |
| Documented context sources | ? | ✔ **6, named** | ✔ ("work with existing documents") | ✔ **6, named** | — |
| Verification step | — | ✔ export-for-inspection | — | — | — |

## 12. Export and collaboration

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| PNG / JPEG / WebP | ✔ | ✔ | ✔ | ✔ | — |
| SVG | ✔ | ✔ | ✔ | ✔ | — |
| PDF | ✔ | — | ✔ | ✔ | — |
| Copy as code / CSS | ✔ | — | — | — | — |
| Export scale multiples | ✔ | ✔ | ✔ | ✔ | — |
| Headless / batch export | ✔ | ✔ `getSvgString` | ? | ✔ | — |
| Multiplayer | ✔ | ◐ | — | ✔ | — |
| Presence cursors | ✔ | ✔ (`presence` scope) | — | ✔ | — |
| Comments | ✔ | — | ✔ | ✔ | — |
| Offline editing | ✔ | ✔ local-first | ✔ | ◐ | — |

---

## 13. What the matrix exposes

[INFERRED] Read across rather than down, five patterns are visible that no single-product note would
show:

1. **Components and variables appear in exactly one column.** Figma, alone, has components, variants,
   libraries, and typed variables. tldraw, Affinity and Canva have none — all three are built without
   the reuse primitive that the mainstream product treats as central.
2. **Vector node editing appears in exactly one column.** Affinity alone. A profile promising
   Affinity-class expressiveness without a path model cannot be built.
3. **Both a camera and selection are excluded from history in all four.** This is a strong convention,
   not an oversight.
4. **Undo and version history are separate systems everywhere they both exist.** Figma and Canva both
   have them and treat them as distinct concepts with distinct semantics.
5. **Both snapping tolerance and the snap *result* shape are undocumented outside tldraw.** tldraw
   states a unit and returns a pure nudge; Affinity needs a richer result (colour, distance) and
   documents neither. The two are not obviously combinable, which is a live design question.
6. **Spool's column is dense with structural absences, not handler absences.** Its gaps correlate with
   document-model gaps — see `profiles/spool.md` §13.

## 14. Sources

All rows are drawn from the evidence already gathered and cited in:

- `docs/research/products/{figma,tldraw,affinity,canva}.md`
- `docs/research/interaction/*.md`, `docs/research/document/*.md`, `docs/research/creative/*.md`,
  `docs/research/ai/*.md`
- `docs/research/architecture/*.md` (Spool column)
- `docs/research/profiles/*.md`
- Primary sources: Figma help ⭐⭐⭐; tldraw docs + source ⭐⭐⭐; Affinity help ⭐⭐;
  Canva help ⭐⭐; Spool `app/src/**` (SOURCE-CODE)
