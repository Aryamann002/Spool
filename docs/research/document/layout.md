# Document Model: Layout

Layout is where the products make the deepest architectural commitments, and where they disagree most.

## The three schools

| School | Products | Core claim |
|---|---|---|
| **Multi-modal with escape hatches** | Figma | Absolute + constraints + auto layout, coexisting, with explicit per-child opt-out |
| **Absolute + responsive hints** | Affinity, Canva | Position is explicit; container resize behaviour is a small declarative rule (constraints / relative positioning) |
| **No layout engine** | tldraw | Grouping + frames + snapping + bindings are expressive enough |

[INFERRED] **Figma is the only product with a flow layout engine.** Affinity, Canva, and tldraw all ship
without one, and all three are commercially successful. That is a strong signal that a flow layout engine
is *not* a prerequisite for a professional design tool — it is a prerequisite for a *specific class* of
design work (UI screens built to be responsive).

## Figma's constraints

[Fully DOCUMENTED — "Apply constraints to define how layers resize"]

Per-axis, on a child of a frame:

| Horizontal | Behaviour |
|---|---|
| `Left` | "maintains the layer's position, relative to the left side of the frame" |
| `Right` | "maintains the layer's position, relative to the right side of the frame" |
| `Left and right` | "maintains the layer's **size and position** relative to both sides of the frame. This may cause layers to grow or shrink along the x axis" |
| `Center` | "maintains the layer's position, relative to the horizontal center of the frame" |
| `Scale` | "will define the layer's **size and position as a percentage of the frame's dimensions**. It will then maintain those proportions as you resize it." (100px frame, 70px layer ⇒ 70%; resize frame to 200px ⇒ layer is 140px) |

Vertical is the same set with Top/Bottom/Center/Scale. **Default is Top + Left.**

Rules:
- Applies "to any layer within a frame. It's not possible to apply constraints to layers outside of a
  frame, **or layers in an auto layout frame**."
- Applies to nested frames.
- **Cannot be applied to groups.** "If you apply constraints to a group, Figma applies constraints to the
  **individual layers**."
- `Cmd/Ctrl` while resizing **temporarily ignores** applied constraints.
- "Depending on the layer's position, more than one constraint may achieve the same result." [i.e. the
  model has redundant representations]
- The blue dotted line on canvas shows applied constraints; a diagram in the sidebar lets you click lines
  to set them; `⇧` selects more than one constraint at a time.
- "The layer's size is **absolute**, and the constraints tell you how to distribute that size when the
  parent resizes" is the model Figma's guides article paraphrases. [DOCUMENTED — "Combine layout guides and
  constraints"]

[INFERRED] **Constraints are a declarative description of a size redistribution function**: given a parent
delta, how does the child's rect change? `Left and right` ⇒ position from left, width from delta. `Scale` ⇒
everything scaled by a ratio. `Center` ⇒ position from centre, size unchanged. Five functions per axis, ten
compositions.

## Figma's auto layout

[Fully DOCUMENTED — "Guide to auto layout"]

### Flows

| Flow | Description |
|---|---|
| **Vertical** | "places objects in your frame along the y-axis. Any objects you add, remove, or reorder will follow the y-axis" |
| **Horizontal** | Same along x |
| **Grid** | "places objects in columns and rows […] objects in a grid don't wrap to the next line. Instead, they are placed in a 'grid' and have the option to **span** multiple rows or columns" |

`⇧A` adds auto layout; "Figma will try to determine which auto layout flow—vertical, horizontal, or grid—
you want to use." `Ctrl⇧A` = **Suggest auto layout**. `⌥⇧A` = **Remove auto layout**.

### Wrap

Available for horizontal and vertical.
- Horizontal: "objects fill the frame from left to right and wrap into a new row when they run out of
  horizontal space."
- Vertical: "objects fill the frame from top to bottom and wrap into a new column when they run out of
  vertical space."
- **"Vertical wrap is not a masonry layout. Objects remain in auto-layout order and move to the next column
  only after the current column reaches the frame's available height."**

[INFERRED] That clarification is important: Figma's wrap is **greedy line-breaking with fixed container
height**, not masonry. Masonry requires height-aware balancing.

### Sizing modes (per axis)

| Mode | Applicable to | Behaviour |
|---|---|---|
| **Hug contents** | Auto layout frames only | "keeps the smallest possible dimensions around its child objects while respecting any spacing values" |
| **Fill container** | Children of auto layout frames only; **not available for top-level frames** | "stretches to occupy all available space in their parent frame, while respecting any spacing values" |
| **Fixed width/height** | Any layer | "size stays fixed and unchanged, regardless of changes to surrounding spacing values and to child, parent, or sibling objects" |
| **Min width/height** | Both, combinable | "equal to or greater than the minimum" |
| **Max width/height** | Both, combinable | "equal to or lesser than the maximum" |

Critical derived rule:

> "**If any child objects within an auto layout frame are set to `Fill container`, the parent frame will no
> longer hug contents and become `Fixed` for the axis.**"

[INFERRED] A child's sizing mode constrains the parent's — an upward dependency. This is the kind of rule
that a naive layout engine will not discover on its own, and it explains why auto layout in Figma is
sometimes confusing.

Setting modes by gesture:
- **Double-click a vertical/horizontal edge** → Hug contents.
- **`⌥`/`Alt` + double-click a vertical/horizontal edge** → Fill container.
- Manually resizing a child to full available space ⇒ it is set to Fill container.
- Manually resizing a layer, or typing a value in W/H ⇒ that axis becomes Fixed.

### Spacing

- **Padding**: uniform, per-axis (vertical/horizontal), or per-side (top/right/bottom/left).
- **Gap**: a number, or **Auto** with `Between` / `Around` / `Evenly`.
- Per-axis resizing modes.
- Grid flow adds: column and row resizing, span.

Gestures: `⌥`/`Alt`-drag a box edge sets padding on the opposite side; `⌥⇧`/`Alt⇧`-drag sets all sides;
`⇧`-drag sets padding/spacing with big nudge.

### Ignore auto layout (formerly "absolute position")

> "An object with **Ignore auto layout** enabled is **excluded from an auto layout flow while keeping it in
> the auto layout frame**. The object and its surrounding siblings ignore each other, even as they resize
> and move. **Much like absolute position in CSS**, an object that ignores auto layout can be placed
> precisely where you want relative to its parent container. Objects with ignore auto layout enabled are
> treated as objects in a regular frame. This means you **can apply constraints** to determine how they
> respond when its parent auto layout frame resizes. Other auto layout settings, such as resizing and layout
> options, aren't available to these objects."

Trigger: drag an object into an auto-layout frame while pressing `⌃ Control` (Mac) / `S` (Windows).

[INFERRED] **This single feature is the clearest evidence that Figma's layout model is genuinely
multi-modal.** A child inside an auto-layout frame can:
1. participate in the flow (default),
2. be hidden from the flow but constrained (ignore auto layout),
3. not exist there at all.

Three modes in one container. Any Spool layout engine claiming Figma parity must support all three.

### Interplay with components

- Components are frames, so they can be auto layout. "There isn't currently a way to add auto layout in
  bulk."
- Instances can override padding and gap; **not** reorder, add, or delete layers.
- Nested auto-layout frames each have their own padding and gap — "allowing us for multi-dimensional
  layouts".

### Interplay with constraints

- "You **can't** apply constraints to child objects in an auto layout frame, unless the object ignores the
  auto layout flow."
- "You **can** apply constraints to the auto layout frame itself if it's nested within a regular frame."

### Interplay with text

- Fixed-size text in auto layout "may cause overlap between layers".
- Max lines and max height are mutually exclusive.

### Interplay with prototyping

- "As an auto layout parent's dimensions are content-driven, it will resize to fit the objects. **To
  replicate scrolling overflow you will need to put the auto layout inside a regular frame.**"
- Smart Animate "do not take into account the background of a frame".

[INFERRED] **A content-sized container cannot clip.** Two container types are required: content-driven
(hug) and fixed (can clip). This is a structural requirement, not a workaround.

## Affinity: constraints, no flow

[DOCUMENTED at product level — help index lists "Key features - page layout"; "Columns/Frames"; "Text
Frames"; "Baseline Grid"]

Affinity 2 introduced **container layers with constraints**. Affinity's Layout Studio has text frames with
**columns** and a **baseline grid** — a flow concept for text only.

[INFERRED] Affinity's layout answer:
- Objects: absolute position.
- Container resize: constraints (like Figma).
- Text: frames with columns and a baseline grid (a flow model for one content type).
- No general flow layout for mixed children.

## Canva: relative positioning

[DOCUMENTED — Position panel; `⌥⌘0` zoom to fit; Magic Resize]

- Numeric X/Y/W/H plus **relative positioning modes** (left/centre/right × top/middle/bottom).
- Guides and rulers (`⇧R`), lockable (`⌥⌘;`).
- `⇧⌘J` "Justify elements-level".
- **Magic Resize** — an AI operation that reflows a design across formats, languages, and dimensions.

[INFERRED] Because a Canva page never resizes, declarative resize rules are unnecessary. Relative
positioning is enough. Magic Resize is a *post-hoc* solution to the same problem Figma solves *declaratively
at design time*.

## tldraw: no layout engine

[DOCUMENTED — the absence of any layout module; `sdk-features/frame-shape.mdx` provides spatial
containers]

tldraw provides:
- **Frames** — spatial containers with local coordinates.
- **Bindings** — persistent directional relationships, described as powering "**layout constraints** that
  keep shapes aligned".
- **Snapping** with gap centring and gap duplication.

[INFERRED] The "layout constraints" phrase in tldraw's bindings docs is a *third-party pattern* (the
workflow starter kit), not a core feature. tldraw's own answer to alignment is snapping + gap duplication:
drag until it snaps. That is fast and good for creative work, and it does not scale to 200-item lists.

## Layout as a document-level vs. node-level concern

| Product | Layout lives on | Granularity |
|---|---|---|
| Figma | The container (frame) declares a flow mode; each child declares sizing mode and (optionally) ignores the flow | Per node |
| Affinity | The container declares constraints for children | Per node |
| Canva | Per node (relative position) | Per node |
| tldraw | Nowhere; snapping is transient | Transient |

[INFERRED] All approaches that work are **per-node declarative**, not global. There is no "the document has
a layout algorithm" model.

## Layout and rendering

| Concern | Figma | Others |
|---|---|---|
| Overflow / clipping | Frame Clip content; **hug frames cannot clip** | Clipping mask (Affinity) |
| Content overflow | Text max lines, max height, truncation | Not documented |
| Scrolling | Frame overflow behaviour; scrolling overflow requires a regular frame around the auto layout | n/a |

## Layout and AI

From the operation vocabulary implied by Figma's documented layout model:

| Operation | Evidence |
|---|---|
| Create a container and apply a flow | Figma `⇧A` |
| **Suggest** a layout | Figma `Ctrl⇧A` — "Figma will try to determine which auto layout flow you want to use" |
| Change a child's sizing mode | Figma hug / fill / fixed / min / max |
| Ignore the layout flow for a child | Figma "Ignore auto layout" |
| Nest layouts | Figma "Nesting refers to the act of placing a layer inside of another layer" |
| Fix a layout the model got wrong | Figma community pain points |

[INFERRED] **Figma's "Suggest auto layout" (and its note that pressing `⇧A` also guesses) is the single most
important precedent for AI layout.** A layout *inference* step from a selection of absolutely-positioned
objects is a well-defined, high-value operation. It is also the hardest part of any AI layout system, and
Figma's own shipped version is widely regarded as imperfect. [THIRD-PARTY — widespread Figma community and
tutorial content on auto layout confusion]

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

- **No layout engine of any kind.**
- `DesignObject { position, size }` — **absolute only**, in the object's own coordinate space. Since there
  is no parent, `position` is effectively world space.
- `set_size` clamps to `MIN_OBJECT_SIZE` (20). No min/max properties, no aspect lock.
- `resized_geometry` computes a new box from a handle and delta. **No constraint propagation, because there
  is no container.**
- `Frame` is an object *type* with fill/stroke, **not a container**.
- `shell.rs::layout_fields()` renders an inspector section. [OBSERVED — a `layout_fields` function exists and
  is rendered; whether it is wired to any behaviour was not verified in this pass]
- `shell.rs::alignment_button(icon, selected)` exists. [OBSERVED — alignment UI; whether it performs an
  operation was not verified]
- No padding, gap, sizing modes, flow mode, wrap, grid, or ignore-layout concepts.

[INFERRED] The prototype is at absolute-positioning-stage-zero. Everything in this document is ahead.

## Candidate architectural implication

**Evidence:**

1. **A content-sized (hug) container cannot clip; two container kinds are required** to support both
   responsive UI and scrolling content. [Figma DOCUMENTED]
2. **A child inside a flow container can participate, opt out and become constrained, or be absent** —
   three modes. [Figma DOCUMENTED]
3. **A child's sizing mode constrains the parent's** ("if any child is Fill, the parent cannot hug").
   [Figma DOCUMENTED]
4. **Constraints and flow layout are mutually exclusive on the same node.** [Figma DOCUMENTED]
5. **Groups cannot take constraints**; constraints apply to their children. [Figma DOCUMENTED]
6. Sizing modes are **set by gesture** (drag the child to full size ⇒ Fill; type a number ⇒ Fixed), not
   only by a menu. [Figma DOCUMENTED]
7. **Vertical wrap is not masonry** — greedy line-breaking with a fixed container height. [Figma DOCUMENTED]
8. **Layout is per-node declarative**, never global. [INFERRED from all four]
9. **Three of four products ship with no flow engine** and are successful. [DOCUMENTED absence]
10. **Layout inference from absolute positioning is a shipped, valued, and imperfect feature**
    (Figma's `⇧A` / `Ctrl⇧A`). [Figma DOCUMENTED + THIRD-PARTY on quality]

**Why it matters:** Layout is the largest and hardest subsystem after text. It constrains the document
model (containers need a flow mode; children need sizing modes), the rendering path (clipping, order), the
interaction path (resizing a container triggers relayout, which must not jank), and history (a resize that
changes many children is still one undo step).

**Potential Spool approaches:**

- **A. No layout engine; add constraints later.** Matches Affinity/Canva/tldraw. Cheapest; sufficient for
  illustration and for screens that never resize.
- **B. Constraints only** (Figma's model without auto layout). Small, high value; handles responsive
  resize; no flow.
- **C. Flow layout only** (flexbox-like: direction, gap, padding, hug/fill/fixed/min/max). Handles dynamic
  content; needs two container kinds; more complex.
- **D. B + C coexisting** (Figma's model). Maximum expressiveness; maximum complexity; requires explicit
  precedence and the "ignore the flow" escape hatch.
- **E. A + layout *inference* as an operation later** (`SuggestLayout`), decoupled from the engine.

[INFERRED] **B is the highest value-per-complexity step** and is a strict prerequisite for C (constraints
apply to the "ignore the flow" children in Figma's model anyway). D is the Figma-faithful answer and is
what Spool's ambition implies, but it is a multi-year commitment.

[INFERRED] If an AI-generated layout is a goal, the engine must exist *and* the inference must exist; an
AI can no more invent a layout algorithm than a human can. So layout is an **AI prerequisite**, not an
AI-adjacent feature — which means deferring layout also defers a large part of the AI story.

**Decision: TBD — requires architecture review.**

## Open questions

1. Does Spool need flow layout at all? If the answer is "later", is the container model extensible enough to
   add a flow mode without migration?
2. Two container kinds (content-sized vs fixed) or one with a flag?
3. Are constraints per child or per container?
4. Does Spool need "ignore the flow", and if so, is it a child flag or a different relationship?
5. Is there a layout-inference operation, and is it part of v1?
6. Does resizing a container relayout all descendants in one frame, or is it deferred/async? (tldraw's
   `getEfficientZoomLevel` shows that *visual* stability during continuous change is a first-class concern;
   the same applies to layout.)
7. How does layout participate in culling? (A relayout changes every child's bounds, so the spatial index
   must be updated incrementally.)

## Sources

- Figma: "Guide to auto layout" (/360040451373) ⭐⭐ (flows, wrap, grid, sizing modes, gap/padding, ignore
  auto layout, components, constraints, text, prototyping, shortcuts); "Apply constraints to define how
  layers resize" (/360039957734) ⭐; "Combine layout guides and constraints" (/360039957934);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914); "Adjust text dimensions and
  resizing" (/27378154668951); Figma forum thread on "Control over Auto Layout default settings" (Jan 2022)
  — THIRD-PARTY for pain-point discovery
- Affinity: "About Studios" (/workspace-about-studios/); "Keyboard shortcuts for general editing"
  (/workspace-shortcuts-editing/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- tldraw: `sdk-features/frame-shape.mdx`, `sdk-features/bindings.mdx` ("layout constraints" phrasing),
  `sdk-features/snapping.mdx` (gap snapping as manual distribution)
- Spool prototype: `app/src/canvas.rs` (`DesignObject`, `set_size`, `resized_geometry`, `ObjectType::Frame`,
  `MIN_OBJECT_SIZE`), `app/src/shell.rs` (`layout_fields`, `alignment_button`)
