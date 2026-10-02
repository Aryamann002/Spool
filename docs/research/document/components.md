# Document Model: Components & Reuse

## The reuse landscape

| Product | Reuse mechanism | Structural? | Updates propagate? | Variants? | Override? | Detach? |
|---|---|---|---|---|---|---|
| **Figma** | Components / instances / component sets / component properties / **matching layers** / libraries | **Yes** | **Yes** — "Instances are linked to the main component and receive any updates made to the component" | Yes | Yes | Yes |
| **tldraw** | **Custom shape types**; copy/paste; bindings; starter kits | Partly (shape *types*, not instances) | **No** | No | No | n/a |
| **Affinity** | Styles; asset libraries; presets; copy/paste; **scripts** | No | No | No | No | n/a |
| **Canva** | **Templates**; Brand Kit; Elements library; text styles; Apps | No | **No** — you edit a copy | No | No | n/a |

## What a component instance actually is

[Fully DOCUMENTED — Figma "Guide to components in Figma", "Explore component properties", "Create and use
variants", "Detach an instance from the component"]

> "There are two aspects to a component:
> **A main component** defines the properties of the component.
> **An instance is a copy of the component you can reuse in your designs. Instances are linked to the main
> component and receive any updates made to the component.**"

> "You can create component properties for any main component **or variants of a component set**, and apply
> them to **nested layers** of the component or variant."

Supported property kinds: **boolean**, **variant** (a switch to another variant in a set), **instance
swap** (a switch to another component), **text**. [DOCUMENTED]

Operations: **override** a property, **reset** an instance to remove overrides, **detach** an instance to
convert it into ordinary layers. [DOCUMENTED]

Auto-layout compatibility matrix (DOCUMENTED — "Guide to auto layout"):

| Action | Main component | Instance |
|---|---|---|
| Adjust vertical/horizontal padding | ✓ | ✓ |
| Adjust gap between | ✓ | ✓ |
| **Reorder layers** | ✓ | **✕** |
| **Add new layers** | ✓ | **✕** |
| **Delete or remove layers** | ✓ | **Hides layer only** |

[INFERRED] The three asymmetry rows are the load-bearing constraints:
1. An instance can change *geometry-affecting properties* of the layout (padding, gap).
2. An instance cannot change the *structure*.
3. Delete inside an instance **hides** rather than removes — so an instance carries a **per-child
   visibility mask**.

[INFERRED] So Figma's instance ≈

```
Instance {
    main_component: ComponentId
    variant: Option<VariantId>
    property_overrides: Map<PropertyPath, Value>     // includes nested-layer targets
    hidden_children: Set<ChildPath>                  // per-child visibility
}
```

That is a **reference + overlay + variant switch**, not a copy and not a template expansion.

## Instance swap — a proof that "component" was insufficient

Figma recommends instance swap as the way to vary an icon inside a component:

> "Want to add icons to instances? We recommend adding a **placeholder icon, with 0% opacity, to the main
> component. You can then swap out the icon for another component in your library."
> [DOCUMENTED — "Guide to auto layout"]

[INFERRED] This is a strong architectural signal. If the original component model could not parameterise
a *type* (which icon?), Figma needed a second mechanism (instance swap) plus a placeholder convention
(a 0%-opacity stand-in) plus a third mechanism (component properties reaching into nested layers) to
express it. Three features and a workaround, accumulated over time, because the original abstraction was
"a reusable frame" rather than "a typed, parameterised template".

[INFERRED] For Spool, this is the clearest argument for designing the reuse abstraction around
**parameterised slots** (a placeholder the instance fills) from the start, rather than retrofitting
instance swap later.

## Matching layers — a weak, structural reuse mechanism

[DOCUMENTED — "Select layers and objects"]

> "**Matching objects are identical layers that exist across more than one frame or group.** You can select
> all matching objects at once […] Many app designs use a search bar across the top of each frame. You can
> quickly select them and make edits to them at the same time."

- Toolbar button or `⌥⌘A` / `Alt Ctrl A`.
- `⇧`-marquee over matching objects adds/removes only matching objects.
- "Objects with sections can only match with other objects in that section."
- `Edit ▸ Select All ▸ other layers that have the same: Properties / Fill / Stroke / Effect / Text
  Properties / Font / another Instance`.

[INFERRED] "Select all layers that have the same fill / font / another instance" is a **structural search**
over the document, not a reuse mechanism. It is nonetheless a genuinely useful *query* capability, and it is
exactly the kind of thing an AI agent would want ("select every layer that uses font X").

## Libraries

[DOCUMENTED] Components and styles can be published to **team libraries**, shared across files and folders.
Local components exist per file. Library components are usable by anyone with view access to the library
file.

[INFERRED] Libraries introduce **cross-file references**, which means:
- Document identity is no longer self-contained (a file references other files).
- Undo of a "publish" operation affects other documents.
- Offline editing of library references needs a resolution step.
- Deletion has to consider references from other documents.

This is a substantial complexity tier above single-file components, and it is the point at which
collaboration stops being optional.

## The opposite model: templates

Canva's model: you start from a template, you edit **your copy**, the master is untouched. [DOCUMENTED]

[INFERRED] Consequences:
- No update propagation, so no "detached instance" confusion.
- No override bookkeeping.
- Copy-on-use is the only operation.
- Divergence is free and expected.

Canva's Brand Kit sits alongside templates but is **reference-free**: choosing a brand colour copies the
value; changing the brand colour later does not change existing designs. The only enforcement is
**Brand Controls**, which restricts the *palette offered in the UI*, not the values stored.

[INFERRED] This is a genuinely different philosophy from Figma's variables (which are *referenced* and
*modes-switchable*). See `variables.md`.

## tldraw's answer: extensible shape *types*

tldraw has no instances. Its reuse mechanism is that a `ShapeUtil` subclass defines a shape *type*, and
every object of that type shares behaviour:

```ts
class PlayingCardUtil extends ShapeUtil<PlayingCard> {
  getBoundsSnapGeometry(shape) { return { points: [...] } }
  getHandleSnapGeometry(shape) { return { outline, points } }
  canSnap() / canCull() / canBind() / canTabTo() / getText() / getGeometry() / component() / getIndicatorPath()
}
```
[DOCUMENTED — `sdk-features/shapes.mdx`]

[INFERRED] **This is reuse of behaviour, not reuse of content.** A library of well-built shape types
(playing cards, kanban columns, flow nodes) is a legitimate and very effective reuse story for a
whiteboard tool — and it is the strategy that makes "VS Code for design" work for *tools*. It is not
sufficient for a *design system*, where the requirement is "this button appears 200 times and must stay in
sync".

[INFERRED] **Spool needs both**: tldraw-style behaviour reuse (extensible object kinds) AND
Figma-style content reuse (instances). They are orthogonal.

## Affinity's answer: procedural reuse

Reuse via styles, asset libraries, presets, and **scripts**. A script is a reusable artifact that
transforms a selection. [DOCUMENTED — "Scripting in Affinity"]

[INFERRED] Affinity has effectively replaced a design-system feature with a programming feature. For
professional creative work where every job is different, that is a reasonable trade. For a product aimed
at "design systems", it is not sufficient on its own.

## Canva's element-library answer

Elements (photos, videos, graphics, stickers, charts) + **Shape Generator** (AI custom shapes) + Brand Kit
assets + Brand Templates + Design Templates. [DOCUMENTED]

[INFERRED] Canva reuses *content* rather than *definitions*. There is no "the master poster" concept.

## Comparison of abstraction levels

```
Level 0  copy/paste            (all four)
Level 1  named styles          (Affinity, Canva, Figma)
Level 2  asset/resource libs   (Affinity, Canva/Figma)
Level 3  templates             (Canva, Figma libraries)
Level 4  typed parameterised templates with slots   (Figma, via components + properties + swap + variants)
Level 5  behaviour-level extension of the object model  (tldraw)
Level 6  procedural reuse via code  (Affinity, tldraw driver/AI)
```

[INFERRED] Spool's stated ambition ("general-purpose design application… templates… design systems… AI")
implies it needs levels 0–4 at minimum, level 5 for its extensibility thesis, and level 6 for its AI
thesis. That is a large surface, and the products show that each level was added *later* to a product that
lacked it.

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

- **Nothing.** No component, instance, variant, component property, template, style, or library concept.
- `ObjectType { Frame, Rectangle, Ellipse, Text }` — no component types.
- `ObjectStyle { fill, stroke }` is **embedded on the object**, not referenced. There is no style identity.
- The shell has `share_popover()` and `export_popover()` — **UI chrome only**, no underlying mechanism.
  [OBSERVED]
- `Tool::Comment` is declared but `creates_object()` returns `None` — a placeholder.

## Candidate architectural implication

**Evidence:**

1. An instance is **reference + property overlay + per-child visibility mask + variant switch**, not a copy
   and not a template expansion. [Figma DOCUMENTED]
2. Component properties can target **nested layers by path**. [Figma DOCUMENTED]
3. Instance children cannot be reordered, added, or deleted; delete only hides. [Figma DOCUMENTED]
4. The *lack* of parameterised slots forced Figma to bolt on instance swap plus a 0%-opacity placeholder
   convention. [Figma DOCUMENTED + INFERRED]
5. Instance swap exists because "reusable frame" could not express "which of these". [INFERRED]
6. Libraries introduce **cross-file references**, a complexity tier above single-file components.
   [INFERRED from DOCUMENTED library behaviour]
7. Structure *search* ("select all matching layers") is a distinct, valuable capability from structure
   *reuse*. [Figma DOCUMENTED]
8. Behaviour reuse (custom shape types) and content reuse (instances) are **orthogonal**. [INFERRED from
   tldraw + Figma]
9. Two products ship with no instances at all and remain successful in their markets.
   [DOCUMENTED absence]

**Why it matters:** Instances are the single most expensive document-model feature in this research:
they require stable paths into subtrees, an override overlay, a visibility mask, cycle detection (a
component containing itself), variant graphs, and library resolution. They cannot be added incrementally
to a model that lacks the underlying structure.

**Potential Spool approaches:**

- **A. No instances. Templates + styles + custom object kinds.** Honest, matches Affinity/Canva/tldraw.
  Ceiling: no live design system.
- **B. Instances as a late addition**, once hierarchy and styles exist. Requires stable child paths and an
  override overlay designed in from day one.
- **C. Instances designed as a *general* template mechanism from the start**: a `Template { root,
  slots: Map<SlotId, ObjectRef>, params }` where components are templates with one slot (itself). Variants
  are parameterised templates; instance swap is a parameter whose type is another template.
- **D. Signature-based "matching layers" only** — structural search, no propagation.

[INFERRED] C is the interesting one: modelling components as a special case of a **general parameterised
template with typed slots** avoids the Figma retrofit problem. A button component with an `{icon}` slot, a
`{label}` slot, and a `{variant}` enum is one concept; a component with no slots is the degenerate case.

**Tradeoffs:** C is significantly more abstract than Figma's model and harder to explain. B is the
pragmatic path. A is viable and much cheaper.

**Decision: TBD — requires architecture review.** This is the most consequential open question in the
research, because it determines whether the document model must support stable subtree paths and an
override overlay.

## Open questions

1. Are components templates with typed slots, or frames with an override overlay?
2. Do instances appear at all in Spool's lifetime? If yes, when?
3. Is there a library/publish concept, or is Spool local-first with file exchange?
4. What is the override path syntax — index, name, or explicit id?
5. How are cycles handled (component containing an instance of itself)?
6. Is there a "matching layers" structural search, and is it exposed to agents?
7. How do instances interact with per-instance transforms and with the layers panel?
8. What does detaching mean, and is it a single undo step?

## Sources

- Figma: "Guide to components in Figma" (/360038662654) ⭐; "Explore component properties"
  (/5579474826519) ⭐; "Create and use variants" (/360056440594); "Detach an instance from the component"
  (/360038665754); "Guide to auto layout" (/360040451373) ⭐; "Select layers and objects" (/360040449873);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914)
- tldraw: `sdk-features/shapes.mdx` ⭐ (ShapeUtil hooks, custom shape types), `docs/examples/shapes/*`,
  `sdk-features/store.mdx` (custom record types)
- Affinity: "Scripting in Affinity" (/scripting-in-affinity/); "About Studios" (/workspace-about-studios/)
- Canva: "Set up Brand Kits" (/brand-kit/); "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`ObjectType`, `ObjectStyle`), `app/src/shell.rs` (`share_popover`,
  `export_popover`)
