# Document Model: Hierarchy

## The central question

> Is hierarchy merely visual organisation, or does it also affect layout, rendering, interaction, and
> semantics?

The answer across four products is: **all four, and they differ in which**.

| Product | Hierarchy affects | Evidence |
|---|---|---|
| **Figma** | Layout (auto layout), semantics (instances), interaction (selection depth, parenting rules), rendering (clipping, z-order), export (frame-scoped) | DOCUMENTED |
| **tldraw** | Rendering (clips, coordinates), interaction (focused group, sibling scoping), z-order (per-parent index) | DOCUMENTED |
| **Affinity** | Rendering (masks, clipping), organisation (layers panel), reparenting (Move Inside/Outside) | DOCUMENTED |
| **Canva** | Organisation (groups), ordering | Shallow; groups exist but nesting semantics are undocumented |

## Container types per product

| Product | Containers | Has own bounds? | Clips? | Can hold a layout? |
|---|---|---|---|---|
| **Figma** | Section, Frame, **Group**, Component, Instance | **Frame: yes. Group: no** ("A group will inherit its bounds from the layers contained within it") | Frame clips (Clip content toggle) | **Frame can be an auto-layout frame** |
| **tldraw** | **Frame**, **Group** | Both yes | Frame clips (`shape-clipping.mdx`) | No flow layout |
| **Affinity** | **Container layer**, Group | Yes | Yes (clipping mask) | Text frames only (columns) |
| **Canva** | Group | Yes | Unknown | No |

[DOCUMENTED]

[INFERRED] **Figma's group-vs-frame asymmetry is the single most important hierarchy finding.** A group is
*not a container* in the layout sense — it is a transient selection convenience. Its bounds are derived
from its children, it cannot take constraints, and it cannot carry a layout. Figma's help docs state both
facts explicitly.

Consequence: any "group" concept in a design tool is really two concepts:
- **A derived selection scope** (Figma's group; also tldraw's group).
- **A real container** (Figma's frame; tldraw's frame; Affinity's container layer).

Conflating them is a common mistake. Spool has neither today (its `DesignObject` has no parent), so it
must decide which it wants first.

## Figma's parenting rules, precisely

[Fully DOCUMENTED — "Select layers and objects" and "Apply constraints to define how layers resize"]

1. "When you click on an object that is part of a group or frame, we'll select the parent by default."
2. Double-click or `Enter` selects one level down; `⇧Enter` selects parent.
3. **Constraints** apply "to any layer within a frame. It's not possible to apply constraints to layers
   outside of a frame, **or layers in an auto layout frame**." Also: "You can't apply constraints to
   **groups**. A group will inherit its bounds from the layers contained within it. They aren't considered
   a single layer with bounds." And: "If you apply constraints to a group, Figma applies constraints to
   the **individual layers**."
4. **Auto layout** is a frame property; a child can **ignore auto layout** ("formerly known as absolute
   position") and then constraints apply to it.
5. **Z-order inside an auto-layout frame is inverted**: "Layer order works the opposite way inside an auto
   layout frame. This is because auto layout wasn't designed to support layers that overlap." Changing order
   inside auto layout changes *position*, not stacking.
6. Instances inherit the main component's structure. Overrides can change padding/gap, hide layers, but
   **cannot reorder, add, or delete** layers.
7. Reparenting via drag; "⌥`" (Mac) / `S` (Win) drags a shape into an auto-layout frame so that it
   *ignores* auto layout.

[INFERRED] Rules 3 and 5 are the two that break naive models:
- **Rule 3** means constraints and auto-layout are mutually exclusive on the same node, and groups
  transparently forward constraints to children.
- **Rule 5** means **z-order is not a property of an object; it is a property of an object *in a container*
  with a mode**. An object in an auto-layout frame has a position derived from order, not a free z-order.

## tldraw's hierarchy

[DOCUMENTED]

- One `parentId` per shape.
- **Frame**: a shape that groups and clips; a spatial container with local coordinates.
  (`sdk-features/frame-shape.mdx`)
- **Group**: a shape with children, plus a **focused group** concept that changes click targeting.
  (`sdk-features/groups.mdx`)
- **Z-order**: `index` is **relative to siblings within the same parent**, using a fractional string key.
  Reordering groups shapes by parent, finds the insertion point, generates new indices, and **"updates only
  the shapes that actually need new indices."** If shapes are already at the target, **no updates occur**.
- Reparenting: `sdk-features/parenting.mdx` (exact coordinate-preservation rules not extracted in this
  pass — **Unknown**).
- Selection: "you can't select both a group and its children at the same time, and selecting shapes inside
  a group focuses that group." Filtering happens in an after-change side effect.

[INFERRED] tldraw's `focusedGroupId` is the explicit form of what Figma implies with `nestingDepth`. It
is editor state, not document state, and it **changes what a click means** — a hierarchy-dependent input
behaviour.

## Affinity's hierarchy

[DOCUMENTED — "Keyboard shortcuts for general editing"]

- `⌘G` Group, `⇧⌘G` Ungroup.
- **`⌥⌘G` Move Inside** / **`⌥⇧⌘G` Move Outside** — explicit reparenting operations.
- `⌘↑` Select Parent Layer; `⌥]` next layer, `⌥[` previous.
- `⇧C` Create Clipping Mask.
- `⌘E` Merge Down / `⇧⌘E` Merge Selected / `⌥⇧⌘E` Merge Visible — **three scoped flatten operations**.
- `⌥⌘L` New Container Layer; `⇧⌘N` New Pixel Layer.
- `⌃⌘L` Unlock All; `⌃⌘H` Show All.

[INFERRED] **Move Inside / Move Outside as discrete, named, bound commands** is the most hierarchy-aware
interaction in any of the four products. Drag-and-drop reparenting is the universal approach; making
reparenting a first-class operation makes it scriptable and AI-addressable — and it is exactly the kind of
operation an agent needs ("move these three elements into that frame").

The three scoped merge operations matter for a second reason: **"flatten" is not one operation.** Merging
down, merging the selection, and merging everything visible produce different documents. An operation API
that exposes a single `Flatten` would be wrong.

## Ordering and z-order

| Product | Representation | Notes |
|---|---|---|
| Figma | Layer-panel position; "the layer's depth corresponds to an object's **z-index**" | Inverted inside auto layout |
| tldraw | **Fractional string index per sibling**, jittered for concurrency | O(moved) reorder; "bring forward" only moves past *overlapping* shapes by default |
| Affinity | Layers-panel order | — |
| Canva | `⌘]` / `⌘[` / `⌥⌘]` / `⌥⌘[` | Matches Figma |

[INFERRED] tldraw's `sendBackward`/`bringForward` overlap rule is a thoughtful interaction decision:

> "By default, these methods only move shapes past other shapes whose page bounds overlap theirs. This
> makes keyboard shortcuts feel intuitive: pressing 'send backward' moves a shape behind the shape it's
> actually covering, not behind some distant shape."
> [DOCUMENTED]

With `{ considerAllShapes: true }` you get plain array semantics. So tldraw has **two z-order semantics**
and makes the more intuitive one the default.

## Reparenting and coordinate preservation

| Product | Documented |
|---|---|
| Figma | Drag to reparent. `⌥`/`S` to reparent while ignoring auto layout. Instance children cannot be reordered/added/deleted. |
| tldraw | `parenting.mdx` exists; **exact coordinate rules Unknown from this pass** |
| Affinity | Move Inside / Move Outside; **whether absolute or relative coordinates are preserved is Unknown** |
| Canva | Group/ungroup; **coordinate behaviour Unknown** |

[INFERRED] This is a significant unresolved gap across all four products. Whether reparenting preserves
the object's *absolute* position (the intuitive behaviour) or its *relative* offset (the correct
behaviour when a frame has a rotation) is a real semantic question. Figma's answer is not documented.

[INFERRED] The defensible rule is: **reparenting preserves the object's world transform**, recomputing the
local position from the new parent's inverse transform. Anything else surprises users.

## Clipping and masking

| Product | Mechanism |
|---|---|
| Figma | Frame's **Clip content** toggle; **Ignore auto layout** on a child is explicitly compared: "an object that ignores auto layout can be placed precisely where you want relative to its parent container" |
| tldraw | `sdk-features/shape-clipping.mdx` — a dedicated module |
| Affinity | **Clipping mask** `⇧C`; masks are first-class objects |
| Canva | Not documented |

[INFERRED] Clipping is a **container property**, not a mask object, in Figma and tldraw; it is a **mask
object** in Affinity. Affinity's approach is more flexible (a mask can be any object, not the container),
Figma's is simpler. Affinity's "clipping mask" is closer to SVG `<clipPath>`; Figma's is closer to
`overflow: hidden`.

## Flattening / merging

| Product | Operations |
|---|---|
| Figma | **Flatten** (one command); **Flatten 3 layers**? (undocumented here); Boolean operations produce a single result |
| tldraw | Not documented |
| Affinity | **Merge Down**, **Merge Selected**, **Merge Visible** — three scopes |
| Canva | Tidy up (alignment, not flattening) |

[INFERRED] Flattening is destructive and irreversible in its styling (text becomes outlines, gradients
become per-path). It also collapses hierarchy. It needs at least two scopes (selection vs. container) to be
useful, and it must be a single undo step.

## Hierarchy as a rendering concern

| Product | Evidence |
|---|---|
| Figma | "Frames clip by default"; Dev Mode shows CSS `overflow` |
| tldraw | Frame clips; culling uses **page bounds** which account for the full ancestor chain |
| Affinity | Clipping masks |
| Canva | Unknown |

[INFERRED] Culling and hit testing must both resolve **world transforms through the ancestor chain**. If
the prototype's `DesignObject` gains a parent, `hit_test`, `objects_in`, `CULL_PADDING` bounds, and the
layers panel all change. This is why hierarchy is foundational.

## Hierarchy as an AI concern

From the operation vocabulary implied by all four products plus the documented agent behaviour:

| Operation | Hierarchy-aware? |
|---|---|
| Reparent / Move Inside / Move Outside | **Yes** — Affinity binds them |
| Group / Ungroup | Yes |
| Flatten (3 scopes) | **Yes** — Affinity binds them |
| Reorder within parent | Yes |
| Create a container and put existing children in it | Yes |
| Delete (with cascade policy) | **Yes** — Figma's instance delete only *hides* |
| Selection scope | Yes — tldraw's `focusedGroupId` |
| Snapshot-based undo | Yes |

[INFERRED] tldraw's driver makes hierarchy directly scriptable: `driver.click(100, 100, shapeId)` targets a
specific shape regardless of depth, and the tool's state machine does the rest. An agent using the driver
gets Figma-like deep selection for free.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **No parent field. No hierarchy. No groups. No frames-with-children.** `DesignObject` has `ObjectType::
  Frame` but a frame is a *shape with a fill/stroke*, not a container.
- Z-order is `Vec` index; `hit_test` iterates `.rev()`.
- `ObjectId::LANDING/EDITOR/FEATURES/MOBILE` are four top-level frames at fixed positions — a *scene*, not
  a hierarchy.
- The layers panel (`app/src/layers.rs`) renders a **flat list** from `project(objects)`; `LayerRow` has
  `{ id, name, object_type, selected, element_id }` — **no depth or parent field**, so there is no tree.
- `LayersProjection` tracks `revision` (mirroring `Document::layer_structure_revision`) and rebuilds the
  whole row list on structural change; selection changes produce an O(changed) presentation diff.
- PHASE15.md documents the finding that GPUI's retention granularity is the *entity*, and dirtying
  propagates **upward** (an invalidated child drags its ancestors and siblings through a re-render). So
  per-row retention is not achievable in this GPUI version. [ENGINEERING-DISCLOSED, from the vendored
  source]

[INFERRED] Adding hierarchy has a direct, measured consequence: the layers panel goes from an O(N) flat
list to a tree with expand/collapse state, which is both more state and more invalidation. The PHASE15
analysis suggests the panel will need windowed/virtualised rows, and that GPUI's model makes this harder
than in React.

## Candidate architectural implication

**Evidence:**

1. **Group ≠ container.** Groups have derived bounds, no layout, no constraints. Frames/containers have
   real bounds and semantics. [Figma DOCUMENTED]
2. **Constraints, auto layout, and z-order are mutually exclusive on the same node**; groups forward
   constraints to children. [Figma DOCUMENTED]
3. **Reparenting as discrete, named, bound operations** is the professional approach. [Affinity DOCUMENTED]
4. **Flattening has at least three scopes.** [Affinity DOCUMENTED]
5. **Z-order is per-parent** and must be an index that supports O(moved) reorder. [tldraw DOCUMENTED]
6. **Selection scope is editor state that changes what a click means.** [tldraw DOCUMENTED]
7. **Clipping is a container property or a mask object** — a real architectural fork. [Figma/tldraw vs
   Affinity DOCUMENTED]
8. Bring-forward has two semantics (overlap-aware and plain). [tldraw DOCUMENTED]

**Why it matters:** Hierarchy is the substrate for layout, components, sections/pages, clipping, group
transform, culling, and the layers panel. Adding it is a large, cross-cutting change; getting the container
abstraction wrong (group vs. frame) is very expensive.

**Potential Spool approaches for the container abstraction:**

- **A. One container type (frame).** Simple; loses Figma's group convenience.
- **B. Two container types: `Group` (derived bounds) + `Frame` (real bounds, clips, layout).** Matches Figma.
  Requires the editor to know which is which everywhere.
- **C. One container type with a `container_mode` flag** (`BoundsMode::{Derived, Own}`). One type, explicit
  flag, avoids duplicated code paths at the cost of a branch.

[INFERRED] C is a reasonable Rust middle path; B is the most faithful. A is a trap if components or
auto-layout arrive later.

**Tradeoffs for z-order:** integer array index (simple, O(n) reorder, breaks under concurrency) vs.
fractional index (tldraw; O(moved), collision-safe, more complex). For a local-first editor, integer
array indices are fine *until* hierarchy exists (then reorder cost becomes O(siblings)).

**Decision: TBD — requires architecture review.**

## Open questions

1. One container type or two (group vs. frame)?
2. Does reparenting preserve world transform or local offset? (No product documents this.)
3. What happens to z-order when an object enters a container? (Inherits the top of the new sibling list?)
4. Is there a *sections* concept (Figma) distinct from frames — purely organisational, no geometry?
5. How does the layers panel handle very deep trees? (GPUI retention limits per PHASE15.)
6. Is clipping a container property or a mask object?
7. What is the cascade policy for delete-with-children?

## Sources

- Figma: "Select layers and objects" (/360040449873) ⭐; "Apply constraints to define how layers resize"
  (/360039957734) ⭐; "Guide to auto layout" (/360040451373) ⭐; "Guide to components in Figma"
  (/360038662654)
- tldraw: `sdk-features/frame-shape.mdx`, `sdk-features/groups.mdx`, `sdk-features/parenting.mdx`,
  `sdk-features/shape-indexing.mdx` ⭐, `sdk-features/shape-clipping.mdx`, `sdk-features/selection.mdx`,
  `sdk-features/bindings.mdx`, `docs/driver.mdx`
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/) ⭐; "About Studios"
  (/workspace-about-studios/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`DesignObject`, `Document::hit_test`, `objects_in`), `app/src/layers.rs`
  (`LayerRow`, `LayersProjection`, `project`), `app/PHASE15.md` (GPUI retention model)
