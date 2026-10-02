# Interaction Profile Evidence — Affinity

> **What this file is.** A catalogue of Affinity's interaction conventions, written so that a future
> `Interaction Profile` for Spool could be authored from it.
>
> **What this file is not.** A design of that profile. Affinity already ships the closest thing to a
> profile system in the corpus — **Studios** — so this file is mostly a record of a shipped precedent
> rather than a reconstruction.
>
> Read with: `architecture/profiles.md` §2.3, `interaction/snapping.md`, `creative/vector.md`,
> `products/affinity.md`.

---

## 1. Summary

Affinity is the corpus's **precedent for interaction profiles**, and it arrives at them differently
from Figma, tldraw and Canva:

- **Studios** change *which tools and panels exist*. Documented studios: Vector, Pixel, Layout,
  Canva AI, Slice, Retouching, Color Grading, Typography, Compositing, Astrophotography, Scripting.
  Studios are documented as **fully customisable** — the user can build their own.
- **Snapping presets** change *which snapping rules are active* — 7 named presets including *UI
  design* and *Pixel work*, plus 4 candidate-scoping modes and a Candidate List.
- **Shortcut customisation** is complete.
- **Modifier semantics and gestures are not configurable at all.**

[INFERRED] Affinity decomposes the profile concept along two axes that are usually conflated:
*what exists* (Studios) and *which snapping rules apply* (presets). It leaves *how a gesture responds
to a modifier* in the core, compiled and unconfigurable. That decomposition is the single most
transferable finding in this file.

---

## 2. Tool vocabulary

[DOCUMENTED] Vector Persona / Vector Studio tools include: Select, Node, Pen, Brush (Pixel),
Rectangle, Ellipse, Polygon, Star, Line, Arc, Pie, Slice, Text, Frame, Gradient, Transparency,
Colours (fill/stroke), Outline Stroke, Crop, Pan, Zoom, Transform, Pixel selection, Dodge/Burn/
Sponge, Blur/Sharpen/Smudge, Clone, Healing Patch, and Persona-switching controls.

Tool activation is either one-shot (`Pen`, shapes) or a **persistent Persona** (Vector, Pixel, Layout,
…).

[INFERRED] There is no single "tool reverts after use" rule; it varies by tool and Studio. For a
profile this is a real difference from Figma's uniformity, and one a designer moving between products
will feel immediately.

---

## 3. Contexts and focus modes

[DOCUMENTED] Affinity has an explicit and unusually **latched** focus model:

| Context | Entry | Notes |
|---|---|---|
| Studio | user choice | determines available tools and panels |
| Persona | user choice within a Studio | Vector / Pixel / Export within Vector Designer |
| Node Tool | `A` | switches from object selection to path-node selection |
| Curve/handle mode | within Node Tool | corner vs smooth vs smart nodes |
| Text editing | in-place | |

`A` toggles Node Tool — **[INFERRED] this is a mode switch, not a tool selection**, which is why it is
a toggle rather than a key that selects a tool. It is closer to Figma's `⌘F1/F2/F3` ladder than to
Figma's tool shortcuts.

---

## 4. Modifier semantics by operation

This is the richest and most divergent table in the corpus. [DOCUMENTED]

| Operation | Modifier | Behaviour |
|---|---|---|
| Cycle overlapping objects under the cursor | `⌥`-click | **depth cycle**, not "select behind" |
| Marquee, fully contained objects only | `⇧`-drag | |
| Marquee, any partially intersected object | `⌃`-drag | |
| Selection box while dragging | `.` | cycles selection-box shapes |
| Corner-handle resize | modifier-dependent | **splits into three distinct behaviours** |
| `⌃`-drag with a selection box active | `⌃` | **mirror / shear**, not move |
| Move inside a group | `⌥⌘G` | |
| Move outside a group | `⌥⇧⌘G` | |
| Group / ungroup / duplicate | `⌘G` / `⌘J` / `⌘D` | |
| Hide selection while dragging | Studio option | an explicit *performance and clarity* affordance |
| Transform | Transform Mode toggle | persistent numeric transform mode |
| Cycle curve modes | Node Tool options | Sharp / Smooth / Smart |
| Pan | space-drag, middle-drag | |
| `/` quick actions | **"International and selected keyboards only"** | layout-dependent availability |

[INFERRED] Two features here have no counterpart anywhere else in the corpus:

1. **`⌃`-drag means *two different things* depending on whether a selection box is active.** Move when
   no selection box; mirror-shear when one is. [INFERRED] This is modifier-as-mode where the mode is
   itself an interaction state — precisely the case `architecture/profiles.md` §5.3 identified as the
   hard case, and it is shipped in a commercial product.
2. **Corner-handle modifiers split into three behaviours** rather than the usual two (aspect, centre).

[INFERRED] A profile that reproduces Affinity's modifier semantics must parameterise the *interaction
state machine*, not the keymap. No amount of keymap configuration gets there.

---

## 5. Creation semantics

[DOCUMENTED]

- Pen tool: click places a corner node, click-drag places a smooth node with handles; the path closes
  on click near the start node or via the Close command.
- Brush: freehand with pressure, producing a variable-width path.
- Shapes: click for default, drag for explicit; nodes are editable afterwards via Node Tool.
- `⌘J` creates a compound path / joins selection — the "make it one thing" operation exists as a
  first-class command.

[INFERRED] Affinity's creation model assumes **every created shape is immediately editable as a path**.
Figma's assumes shapes are rectangles until converted. This is a document-model difference expressed as
a tool difference, and it is the clearest example in the corpus of why "tool repertoire" and "document
model" cannot be separated cleanly for a profile.

---

## 6. Navigation

[DOCUMENTED]

| Action | Method |
|---|---|
| Pan | space-drag, middle-drag, two-finger scroll |
| Zoom | wheel, pinch, `⌘`+wheel |
| Zoom to fit | Studio/view option |
| **Rulers and guides** | first-class, per-Studio |
| **Pixel grid** | Pixel Studio, with forced pixel alignment |
| Studio switching | toolbar or Persona control |

- **Guides are a first-class document object**, not a transient overlay. [DOCUMENTED]
- **"Force Pixel Alignment"** is a snapping option that snaps to whole pixels regardless of zoom.

[INFERRED] Affinity treats navigation aids (rulers, guides, grid) as *document content* and snapping as
a *rule bundle*. Figma treats guides as a session overlay. That difference propagates into the
document model and into undo semantics.

---

## 7. Snapping configuration — the shipped profile precedent

[DOCUMENTED] This is the strongest evidence in the corpus that "interaction profile" is a reasonable
product concept.

| Dimension | Options |
|---|---|
| Presets | 7 named bundles, including **UI design** and **Pixel work** |
| Candidate scoping | 4 modes + a Candidate List panel |
| Targets | object edges/centres, grid, guides, rulers, other layers |
| Colour-coded guides | each snap type has its own colour |
| **Distance labels** | guides display numeric distance |
| Per-layer opt-out | **"Exclude From Snapping"** on individual layers |
| **Force Pixel Alignment** | snap to whole pixels regardless of zoom |
| **Construction snapping** | a *different algorithm* — angular and segment-relative |
| Action scope | some snapping options apply only within certain operations |

[INFERRED] Four separate ideas are bundled under "snapping" here, and each has different architectural
consequences:

1. **A preset bundle** → a named, user-selectable rule set. (This is the profile concept, scoped.)
2. **Per-layer opt-out** → snapping needs a *document* property (`excludeFromSnapping`), not a
   preference. [INFERRED] this is a document-model field, and an important precedent.
3. **Distance labels** → snapping indicators must carry data, not just geometry. [INFERRED] this is a
   constraint on the snap *result* type.
4. **Construction snapping** → a *different algorithm* selected by mode, not a different rule within
   one algorithm.

[INFERRED] Spool's snap engine, if it ever exists, needs to return a result object rich enough to
carry indicator type, colour, and distance — not a bare nudge. That reverses tldraw's minimal
`nudge`-only design, and the two are not obviously combinable.

---

## 8. Selection model summary

| Concept | Affinity's answer |
|---|---|
| Selected | object ids; Node Tool adds node-level selection |
| Behind | `⌥`-click **cycles overlaps** (a depth cycle) |
| Marquee | `⇧` = full containment, `⌃` = partial intersection |
| Additive | `⇧`-click |
| Group | `⌘G`; containers can be flattened at three scopes |
| Move in/out of group | `⌥⌘G` / `⌥⇧⌘G` |
| Flatten | three documented scopes |
| Locked | Studio-level locks |
| Traversal | layer panel; no documented geometric `Tab` traversal |

[INFERRED] The `⌥`-click *cycle* is different in kind from Figma's `⌥`-click *select behind*: cycling
requires maintaining a per-point stack of occluders, which is a hit-test result cache, not a hit-test
result. That has real architectural cost at scale and is invisible until it is missing.

---

## 9. History perception

[DOCUMENTED]

- Undo/redo of document changes.
- The **JavaScript scripting API** produces actions that are "often as a single undoable action" —
  the grouping decision is documented as *approximate*, not guaranteed.
- Studio switching and panel changes are not document history.

[INFERRED] "Often as a single undoable action" is a candid admission that the scripting API does not
have a rigorous transaction boundary. Compare `architecture/history.md`: a rigorous answer (tldraw's
`editor.run`) exists and is cheap if the operation layer is right.

---

## 10. What an `affinity` profile would have to reproduce

**Tier 1 — shipped precedent, low risk**
1. Studio concept: named bundles of tools + panels, user-creatable.
2. Snapping presets: named rule bundles, including a *UI design* preset.
3. Per-layer "Exclude From Snapping".
4. Colour-coded guides with distance labels.
5. Full keyboard customisation.
6. Guides as first-class document objects.

**Tier 2 — significant architectural cost**
7. Node Tool as a **mode** with corner/smooth/smart nodes, and path editing at all.
8. Pen tool with click/drag node semantics.
9. `⌥`-click overlap cycling.
10. `⇧`-containment vs `⌃`-intersection marquee.
11. Guides and rulers integrated with the snapping engine.

**Tier 3 — probably not profile material**
12. Blend modes, colour grading, astrophotography, compositing. (Feature scope, not interaction.)

[INFERRED] Tier 2 items 8–11 all require a **path/geometry model Spool does not have**. A profile
offered before that model exists would be a menu of unimplemented tools — the exact failure mode
`architecture/rendering.md` records for Spool's declared-but-unimplemented `Pen` and `Comment` tools.

---

## 11. What cannot be reproduced from the available evidence

| Item | Why not |
|---|---|
| The three distinct corner-handle modifier behaviours | documented as existing, not enumerated |
| Exact geometry of "Selection Box From Curves" | not specified |
| Winding fill mode semantics | referenced, not specified |
| The full set of 7 snapping presets' contents | the presets are named, their contents are not all listed |
| Which snapping options are action-scoped | partially documented |
| Whether `⌃`-drag shear depends on Studio or on the selection box being *active* | ambiguous |

---

## 12. Evidence gaps specific to this profile

- Affinity's documentation is Studio- and Persona-organised, which makes cross-cutting questions
  ("what does `⌘G` do everywhere?") surprisingly hard to answer.
- The scripting API and MCP server are recent; their transaction semantics are described loosely.
- Documented guidance states that agents should *"work with existing documents… designing from
  scratch generally produces poor results"* [DOCUMENTED] — a product-level statement that has direct
  architectural implications for Spool's AI work (see `ai/agents.md`).

## 13. Sources

- Affinity (DOCUMENTED): Personas/Studios, snapping, rulers and guides, Node Tool, keyboard shortcuts,
  Pixel grid, JavaScript scripting API, MCP server — affin.co/help ⭐⭐, affin.co/affinity-sdk
- Affinity (DOCUMENTED): product announcements for Designer 2 / Photoshop 2, Apr 2026 onward
- Related notes: `products/affinity.md`, `creative/vector.md`, `interaction/snapping.md`,
  `interaction/transformation.md`, `interaction/selection.md`, `ai/agents.md`,
  `architecture/profiles.md`, `architecture/interaction-runtime.md`
