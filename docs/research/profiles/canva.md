# Interaction Profile Evidence — Canva

> **What this file is.** A catalogue of Canva's interaction conventions, written so that a future
> `Interaction Profile` for Spool could be authored from it.
>
> **What this file is not.** A design of that profile. Canva's conventions are largely the product of
> *removing* choices rather than adding them, which makes it the most instructive and least portable
> profile in the corpus.
>
> Read with: `architecture/profiles.md` §2.4, `interaction/keyboard.md`, `ai/context.md`,
> `products/canva.md`.

---

## 1. Summary

Canva is the corpus's **approachability reference**: a powerful design system that stays reachable. Its
interaction model achieves this mostly by *withholding* persistent tools and *naming* its contexts
explicitly.

Four defining characteristics:

1. **No persistent tool.** [DOCUMENTED] Every creation action is a one-shot command —
   `T` text, `R` rectangle, `L` line, `C` circle, `S` shape, `⌘\` uploads, `⇧;` media. There is no tool
   state to be in, so there is no tool lifecycle to get wrong.
2. **An explicit, documented focus ladder.** [DOCUMENTED] `⌘F1` select, `⌘F2` direct select,
   `⌘F3` text edit — with a published a11y rationale.
3. **A bounded page stack, not an infinite canvas.** [DOCUMENTED] Four page views: Scrolling,
   Thumbnail, Grid, Presentation (`⌥⌘1/2/3/P`).
4. **Constraint by governance rather than by capability.** [DOCUMENTED] Brand Kit + Brand Controls
   **copy values** rather than reference them; there are no variables and no components.

[INFERRED] Canva's accessibility is its most transferable idea and its architecture is its least. A
"general-purpose design environment" that copies Canva's one-shot-command model would lose the very
thing — persistent tools with lifecycles — that the rest of the corpus treats as essential.

---

## 2. Tool vocabulary

[DOCUMENTED] One-shot creation commands, not tools:

| Command | Shortcut | Creates |
|---|---|---|
| Text | `T` | a text box |
| Rectangle | `R` | a rectangle |
| Circle | `C` | a circle |
| Line | `L` | a line |
| Shape | `S` | from a shape picker |
| Upload | `⌘\` | media from the computer |
| Media | `⇧;` | from the media panel |
| Same as last | (documented separately) | repeats the previous creation |

Additional documented commands: `⌘D` duplicate, `Esc` deselect/cancel, `⌥1` zoom to fit,
`⌥⌘0` zoom to fit, `⌘0` actual size, `⇧⌘0` zoom to fill, `⌥⌘G` go to page, `/` or `⌘E` quick actions.

[INFERRED] **There is no `V`-equivalent select tool**, because there is no tool mode at all: the Select
tool is the absence of a creation command. That is a genuinely different interaction model, and it is
why Canva can offer accessibility that tool-based editors cannot easily match.

---

## 3. Contexts and focus modes — the corpus's best-documented focus model

[DOCUMENTED]

| Focus mode | Shortcut | Selects |
|---|---|---|
| Select | `⌘F1` | the top-level element |
| Direct select | `⌘F2` | a nested element |
| Text edit | `⌘F3` | text inside a text element |

[INFERRED] Canva publishes these *as an accessibility contract*. The reason is structural: with
one-shot commands there is no tool state to describe, so focus mode is the **only** state a
screen-reader user needs to know about. [PROPOSED-QUESTION] If Spool keeps persistent tools (as the
rest of the corpus implies it should), it will need to publish *both* a focus mode and a tool state to
reach parity — which is a real accessibility cost of the tool model, and it is not discussed anywhere
in the corpus.

Additional documented contexts: **directional multi-select** (`⇧W/A/S/D` select all in a direction) and
`F8` (selects all objects in the same group/type context, [INFERRED]).

---

## 4. Modifier semantics by operation

[DOCUMENTED]

| Operation | Modifier | Behaviour |
|---|---|---|
| Proportional resize | `F2` + drag | F2 is a **mode**, not a modifier |
| Position/size entry | relative modes in the Position panel | **panel**, not keymap |
| Duplicate | `⌘D` | |
| Locked aspect | panel checkbox or `F2` | |
| Brand values | Brand Controls replaces the editable value | **constraint**, not a lock icon |
| Quick actions | `/` or `⌘E` | |

[INFERRED] Canva's `F2`-as-mode and its relative-position panel are **the most accessibility-friendly
conventions in the corpus**: every transformation is achievable without a modifier chord and without a
drag. [PROPOSED] This is the most directly transferable idea for Spool, precisely because it costs
nothing architecturally — it is a panel, not a gesture.

[INFERRED] Canva has essentially **no drag-with-modifier vocabulary**. That is not an oversight; it is
what makes the product approachable. Any `canva` profile would have to *remove* conventions that the
other three profiles treat as essential.

---

## 5. Creation semantics

[DOCUMENTED]

- Text, shapes, and media are created by one-shot command.
- **Templates** are the dominant creation path: a template is an editable document, not a flattened
  image.
- **Magic Layers** converts a flat image into an editable layout. [DOCUMENTED]
- Creation from scratch is supported but is not the recommended path.

[INFERRED] Template-first creation is a *document-model* choice (a document with named, replaceable
regions) expressed as an interaction convention. It is not reproducible by a profile; it requires the
document to support it.

---

## 6. Navigation

[DOCUMENTED]

| Action | Shortcut |
|---|---|
| Page view: Scrolling | `⌥⌘1` |
| Page view: Thumbnail | `⌥⌘2` |
| Page view: Grid | `⌥⌘3` |
| Page view: Presentation | `⌥⌘P` |
| Go to page | `⌥⌘G` |
| Zoom to fit | `⌥⌘0` |
| Actual size | `⌘0` |
| Zoom to fill | `⇧⌘0` |

[INFERRED] Four *named page views* is a navigation model the other three products do not have. It
treats the canvas as one of several presentations of a bounded document, rather than as the document.
For a Spool profile this is cheap to imitate (a camera + a layout mode) and it is the clearest example
of a navigation concept being a *presentation* concern rather than a document concern.

No minimap [OBSERVED].

---

## 7. Snapping configuration

[DOCUMENTED] Canva documents a position panel with **relative positioning modes** and alignment
controls. Detailed snapping rules, tolerance, and snap indicators are **not documented**.

[INFERRED] Canva's approach appears to be: precise entry through a panel, plus a best-effort visual
snap, rather than a rich snap engine with guides. That is a coherent design for bounded, page-based
designs and a poor fit for infinite-canvas work.

---

## 8. Selection model summary

| Concept | Canva's answer |
|---|---|
| Selected | element ids on a page |
| Additive | `⇧`-click |
| Directional multi-select | `⇧W/A/S/D` — a *geometry-directed* selection mode |
| Context select | `F8` [INFERRED] |
| Behind | not documented |
| Marquee | drag on canvas; mode not documented |
| Traversal | `Tab` between elements; behaviour not documented |
| Groups | supported; behaviour not documented |
| Locked | lock icon; Brand Controls is a different, softer mechanism |

[INFERRED] **Directional multi-select** is unique to Canva in the corpus and is a genuine
accessibility win: it gives a non-pointer path to "select everything to the right of here".
[PROPOSED] This is cheap for Spool to adopt — it is a selection operation, not a gesture — and it is
one of the few places where copying Canva is unambiguously a good idea.

---

## 9. History perception

[DOCUMENTED]

- Undo/redo (`⌘Y` / `⇧⌘Z`).
- **Version history: 1000 attributed versions**, each with an avatar and timestamp, **no time limit**,
  paid feature.
- **Trash: 30-day retention.**
- Asset deletion is blocked when another element references the asset.

[INFERRED] Canva's version history is unusually *deep* rather than unusually *branched*: no branching
is documented, but there is no time limit either. Compared with Figma (branching, time-limited) this is
a different trade: retrieve-anything vs branch-from-anywhere.

[INFERRED] The 1000-version, no-time-limit policy implies **the version store is not a diff of a
document but a set of stored documents or deltas that must remain cheap indefinitely**. That is a
storage decision with architectural consequences, and it is worth recording because a local-first Spool
would have to choose its own policy.

---

## 10. What a `canva` profile would have to reproduce

**Tier 1 — cheap, high value, mostly about removing things**
1. One-shot creation commands instead of persistent tools.
2. The `⌘F1/F2/F3` focus ladder, published as an accessibility contract.
3. Named page views (Scrolling / Thumbnail / Grid / Presentation).
4. `F2` proportional-resize mode.
5. A position panel with relative modes — every transform achievable without a chord or a drag.
6. Directional multi-select (`⇧W/A/S/D`).
7. `/` or `⌘E` quick actions.

**Tier 2 — document model, not interaction**
8. Template-first creation.
9. Brand Kit / Brand Controls (copy, not reference).
10. Magic Layers (image → structured layout).

**Tier 3 — deliberately absent**
11. No infinite canvas.
12. No pen tool, no node editing, no vector path model.
13. No components, no variables, no prototyping.

[INFERRED] The tension for Spool is direct: items 1–7 are excellent conventions, and item 1 in
particular **contradicts** the persistent-tool model that tldraw, Figma and Affinity all use and that
`architecture/interaction-runtime.md` recommends for Spool. Choosing `canva` as a profile would mean
choosing a different tool architecture, not merely different bindings.

---

## 11. What cannot be reproduced from the available evidence

| Item | Why not |
|---|---|
| Snap rules, tolerance, indicators | not documented |
| Marquee intersect-vs-contain | not documented |
| `Tab` traversal order | not documented |
| Group semantics | not documented |
| Whether `Esc` cancels or deselects when both apply | not specified |
| Magic Layers' output structure | "editable layout", unspecified |
| Brand Controls: what happens on conflict | partially documented |

---

## 12. Evidence gaps specific to this profile

- Canva's help documentation is written for outcomes, not mechanics; precise interaction rules are
  largely absent.
- Canva AI 2.0 names **six context sources** but does not describe what each contributes or how they
  are weighted. See `ai/context.md`.
- The relationship between Canva's template model and its document model is not documented.

## 13. Sources

- Canva (DOCUMENTED): Keyboard shortcuts, Canva AI / Magic Studio, Brand Kit, Brand controls,
  Version history, Templates, Magic Layers — canva.com/help ⭐⭐
- Related notes: `products/canva.md`, `interaction/keyboard.md`, `interaction/navigation.md`,
  `interaction/selection.md`, `document/components.md` (absence), `document/variables.md` (absence),
  `architecture/history.md`, `architecture/profiles.md`, `ai/context.md`
