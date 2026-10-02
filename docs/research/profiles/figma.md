# Interaction Profile Evidence — Figma

> **What this file is.** A catalogue of Figma's interaction conventions, written so that a future
> `Interaction Profile` for Spool could be *authored* from it.
>
> **What this file is not.** A design of that profile, and not an endorsement. A profile that
> reproduces Figma would be a choice, not a consequence of this evidence.
>
> Read with: `architecture/profiles.md` (the concept), `interaction/*.md` (behaviour detail),
> `matrices/interaction-matrix.md`, `matrices/keyboard-matrix.md`.

---

## 1. Summary

Figma is the **reference profile** for UI design work and the product whose conventions most people
already hold in their hands. Its profile is **not user-configurable** beyond choosing one of 16
keyboard layouts. Everything below is therefore a fixed repertoire that a `figma` profile would have
to either reproduce or deliberately depart from.

Three defining characteristics:

1. **A universal container.** Frame is the one concept that spans artboards, auto layout, components,
   prototypes, and export. Nearly every Figma convention follows from this.
2. **Scope by depth.** Selection is a set plus an implicit `nestingDepth`; `Enter` descends, `⇧Enter`
   ascends, `⌘`-click deep-selects. "What is selected" is always depth-relative.
3. **Three coexisting layout systems** (absolute, constraints, auto layout) rather than one, with an
   explicit escape hatch ("Ignore auto layout").

[INFERRED] Figma's coherence is not the result of a unified model; it is the result of a very
consistent *depth* and *frame* discipline on top of a model that admits several answers.

---

## 2. Tool vocabulary

[DOCUMENTED] Move (`V`), Frame (`F`), Rectangle (`R`), Ellipse (`O`), Line (`L`), Polygon, Star,
Text (`T`), Frame (`F`), Hand (`H`), Scale, Slice, Slice (`⌘⌥K`), Comment (`C` — actually `C` is
comment; Shape-selector `⇧R` combines rect/ellipse/line).

**Persist-after-use:** Shape tools, Frame and Comment **revert to Move after creation**. Hand and
Scale stay. Text stays active for continued typing. This is the single most-copied convention in the
industry.

**Not reachable by keyboard** [DOCUMENTED]: vector paths from the pen tool, connectors, and tables.
A profile that claims Figma parity must accept this asymmetry or exceed it.

[INFERRED] Figma's tool set is a *UI-design* tool set. It has no brush, no pen, no node editor, no
pixel tools. Any `figma` profile should be scoped to the same domain; pretending otherwise would make
the profile a lie.

---

## 3. Contexts and focus modes

[DOCUMENTED] Three, and they are the most-copied interaction idea in the corpus:

| Focus mode | Shortcut | Selects | Depth behavior |
|---|---|---|---|
| Select | `⌘F1` / `⌥1` | outermost | click picks top-level frame/group |
| Direct select | `⌘F2` | children of the current frame | click picks the child itself |
| Text edit | `⌘F3` | text inside a text object | caret placement |

Additional implicit contexts: inside a **component instance** (children become directly selectable and
locked), inside a **text object** (arrow keys move the caret, not the object), and during a **gesture**
(modifiers change meaning).

[INFERRED] These are not shortcuts; they are *selection scopes*. A profile that reproduces them needs
a depth/selection-scope model, not three bindings.

---

## 4. Modifier semantics by operation

[DOCUMENTED, from the shortcut reference and behaviour pages]:

| Operation | Modifier | Behaviour |
|---|---|---|
| Drag move | — | moves |
| Drag move | `⇧` | constrain to one axis |
| Drag move | `⌥` | duplicate-and-move (drag-copy) |
| Drag from unselected object | `⇧` | adds to selection instead of replacing |
| Resize | `⇧` | preserve aspect ratio |
| Resize | `⌥` | resize about the centre |
| Resize | `⇧⌥` | both |
| Rotate | `⇧` | 15° increments |
| Corners vs sides | `⌥` | while hovering a resize handle, temporarily shows corner handles |
| Scale tool | — | scales proportionally without changing layout mode |
| `⌘`-click | — | deep-select into a group |
| `⌥`-click | — | select the item behind the current one |

[INFERRED] Figma's convention is **modifier-as-constraint, never modifier-as-mode** — except
`⌘`-click (scope) and `⌥`-click (depth). That is a coherent rule: `⇧` and `⌥` constrain the current
gesture; `⌘` changes what is being pointed at.

Note the deliberate overlap: `⌥`-click selects behind, and `⌥`-drag duplicates. Same modifier, two
gestures, no conflict — because click and drag are different interaction states.

---

## 5. Creation semantics

[DOCUMENTED] The **creation gesture determines the text behaviour**:

| Gesture | Result |
|---|---|
| Click with Rectangle tool | fixed 100×100 square (click = fixed size) |
| Drag with Rectangle tool | drag-defined size |
| Click with Text tool | **auto-width** text box that grows with content |
| Drag with Text tool | **fixed-width** text box that wraps |
| Click with Frame tool | default frame size |
| Drag with Frame tool | drag-defined frame |

[INFERRED] This is the most economical text model in the corpus: a *gesture* chooses a text
*behaviour* and the user never touches a setting. A profile must reproduce the mapping, not just the
tools.

Frames may also contain children; a rectangle with children becomes a **Group**, whose bounds are
*derived* from children and which cannot take constraints. Two container types exist because
"derived bounds, no constraints" and "authored bounds, constraints, layout" are incompatible.

---

## 6. Navigation

[DOCUMENTED]

| Action | Shortcut |
|---|---|
| Pan | Space-drag, middle-drag, two-finger scroll |
| Zoom | `⌘`/`Ctrl` + wheel, pinch |
| Zoom to fit | `⇧1` |
| Zoom to selection | `⇧2` |
| Zoom to 100% | `⇧0` |
| Zoom in / out | `+` / `-` |
| Jump to a layer | `⌘\` then layer name (`Quick actions`) |
| Next/previous frame | `W` when the Frame tool is active |

- Zoom range is clamped; there is a documented minimum and maximum.
- `⌘\` opens a command palette over layers — a *navigation* palette, not an action palette.
- **No minimap** [OBSERVED]; navigation is by scroll position and by the layers panel.

[INFERRED] Figma's navigation assumes the user knows where they are. It has no spatial overview
affordance. Nothing in the corpus fills this gap.

---

## 7. Snapping configuration

[DOCUMENTED] Object snapping is on by default with a fixed tolerance. Snapping applies to moving,
resizing (edges and centres), drawing, and rotating. Guides can be created manually and pulled from
rulers. Layout gap indicators appear inside auto layout.

[INFERRED] Figma does **not** document: snap tolerance in px, a snap on/off preference, a grid snap
setting (grid is a *visual* setting), or snapping during node editing (there is no node editing).
Compare `interaction/snapping.md`.

[INFERRED] A `figma` profile would need to pick a tolerance that is undocumented. tldraw's rule —
**tolerance = screen px ÷ zoom** — is the only one in the corpus that states a unit and would make
Figma-like snapping reproducible at every zoom level.

---

## 8. Selection model summary

| Concept | Figma's answer |
|---|---|
| Selected | `Vec<id>` + implicit `nestingDepth` |
| Editing | depth = `nestingDepth + 1`, entered by double-click or `Enter` |
| Behind | `⌥`-click cycles deeper |
| Marquee | drag on empty canvas; `⇧` = intersect mode in some contexts |
| Additive | `⇧`-click / `⇧`-drag |
| Subtractive | `⌘`-click / `⌘`-drag on selected |
| Locked | not interactable; selectable in the layers panel |
| Hidden | not rendered, not selectable on canvas |
| Traversal | `Tab`-style through siblings; `Enter`/`⇧Enter` for depth |
| Selection ∩ layout | auto-layout children select as siblings, not as nested coordinate spaces |

---

## 9. History perception

[DOCUMENTED] A drag is one undo step. A text editing session is one undo step. A colour change across
a multi-selection is one step. Undo and redo are separate commands with no user-visible marks.

Separately, Figma has **version history** with named versions, an author and timestamp, and — most
importantly — **branching**, so a user can branch from any historical point. Version history is *not*
undo; it is a second, long-horizon system.

[INFERRED] Reproducing Figma's *feel* requires undo to behave as it does; reproducing Figma's
*product* requires both systems. These are separable and should be scoped separately.

---

## 10. What a `figma` profile would have to reproduce

Listed as requirements, not as commitments.

**Tier 1 — the conventions nearly every designer already expects**
1. Shape tools revert to Move after creation.
2. Click-to-type vs drag-to-type-box produce auto-width vs fixed-width text.
3. Click vs drag produces fixed vs drag-defined size.
4. `⇧`-constrain, `⌥`-duplicate, `⇧⌥`-constrain during transforms.
5. `⌘`-click deep-select; `⌥`-click select-behind.
6. `Enter` descends into a selection; `⇧Enter` ascends.
7. Escape cancels a gesture before it commits, and otherwise exits to the parent scope.
8. Frame is the universal container; Group has derived bounds.
9. `⇧1` / `⇧2` / `⇧0` / `+` / `-` navigation set.
10. Undo is one perceived action per user gesture.

**Tier 2 — structural conventions with deeper consequences**
11. Auto layout coexisting with absolute positioning and constraints.
12. The `nestingDepth` selection-scope model.
13. Text, vector, and non-UI tools *not* being present.

**Tier 3 — probably out of scope for a profile**
14. Variables and collections (these are document features, not interaction).
15. Component/instance semantics (document features).
16. Prototype triggers (document features).

[INFERRED] The tiering matters: tiers 1–2 are interaction; tiers 3 are document model. A profile that
claimed tier 3 would actually be prescribing the document model, which `architecture/profiles.md`
Implication B forbids.

---

## 11. What cannot be reproduced from the available evidence

| Item | Why not |
|---|---|
| Exact snap tolerance | not documented; must be chosen |
| Click/drag size thresholds | not documented |
| Marquee intersect-vs-contain rule | partially documented, not exhaustively |
| `Tab` traversal order in nested instances | not documented |
| Whether `⇧`-marquee intersects or contains | differs by context and version |
| Modifier re-read behaviour mid-drag | Figma latches; [INFERRED] not documented |
| Behaviour when dragging an object into a clipped frame | not documented |

[PROPOSED] These are the items where a profile author will be *choosing*, and the choice should be
recorded as a decision rather than reverse-engineered and presented as fidelity.

---

## 12. Evidence gaps specific to this profile

- No engineering source describes Figma's input or interaction architecture; every architectural
  statement in this file is [INFERRED].
- Figma ships no user rebinding, so there is no documented *intent* behind the shortcut layout.
- The interaction between Auto Layout and drag-reparenting is documented only as behaviour, not as a
  rule (does the child keep its relative offset? the size? the layout slot?). This is the single most
  important undocumented behaviour for anyone implementing a `figma` profile.

## 13. Sources

- Figma (DOCUMENTED): Keyboard shortcuts, Selecting & inspecting, Transform, Constraints, Auto
  layout, Components & variants, Variables, Prototype, Export — help.figma.com
- Figma (ENGINEERING-DISCLOSED): "Figma Rendering: Powered by WebGPU" (Sep 2025), "Keeping Figma
  Fast" (2023), "Building a professional design tool on the web" (2015)
- Related notes: `products/figma.md`, `interaction/selection.md`, `interaction/transformation.md`,
  `interaction/snapping.md`, `interaction/text-editing.md`, `interaction/keyboard.md`,
  `document/layout.md`, `architecture/profiles.md`, `architecture/interaction-runtime.md`
