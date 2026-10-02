# Document Model: Styles

## What "style" means in each product

| Product | Concept | Named? | Reusable? | Composable? | Tokenisable? | Publishable? |
|---|---|---|---|---|---|---|
| **Figma** | Fill / Text / Effect / Grid **styles** | Yes | Yes | **No** — "styles cannot be used in other styles or variables" | Via variables | Yes (libraries) |
| **Figma** | **Variables** | Yes | Yes | **Yes** — aliasing | **Yes** — this *is* the token layer | Yes |
| **tldraw** | Named **palette** (`color: 'blue'`) | Yes | Implicitly | No | No | No |
| **Affinity** | **Styles panel** (paragraph / character / graphic) | Yes | Yes | **Yes** — "Paste Style" implies composability | No (no token layer) | Presets |
| **Canva** | Named **text styles** + Brand Kit colours/themes | Yes | Yes | No | **No** (Brand Kit is a palette) | Yes (Brand Kit, Brand Templates) |

## Figma's two-layer system — the clearest published separation

[Fully DOCUMENTED — "Overview of variables, collections, and modes"]

> "**A style is great for creating a composite of values. Also, styles cannot be used in other styles or
> variables.**
> **Variables can be used to create multiple modes** — such as light and dark modes. Also, **variables can
> be applied to styles and other variables**, allowing the ability to implement design tokens."

[INFERRED] The distinction is:

| | Style | Variable |
|---|---|---|
| Represents | A **composite** of values (a whole text style: family + size + weight + line height + spacing) | A **single raw value** |
| Can contain variables | No | Yes |
| Can be aliased | No | Yes (same type only) |
| Supports modes | No | Yes (via collections) |
| Publishable | Yes | Yes |
| Limit | Unknown | 5,000 per collection |

[INFERRED] **Styles are one-level named bundles. Variables are a composable, aliasable, mode-switchable
value graph that sits *below* styles.** They solve different problems:
- Style = "this is what our body text looks like."
- Variable = "this is what `--text-body-size` is, and it is `14` in light mode and `16` in large mode."

## The full property surface variables can drive

[DOCUMENTED — Figma]

**Colour variables** — applied to: colour styles, fill colours, **gradient stops**, shadow effects, stroke
colours, and other colour variables. Opacity is separate and can be driven by a number variable, **clamped**
(negative ⇒ 0%, >100 ⇒ 100%).

**Number variables** — applied to:
- corner radius (including individual corners)
- dimensions, including min/max width/height
- font properties: **size, weight (numbers only, e.g. 400, 700), line height, letter spacing (interpreted
  as px, not %)**
- paragraph indent, paragraph spacing
- layout guides
- uniform grid size
- **row and column count (whole numbers only)**
- width, height, margin, offset, gutter
- padding and gap
- shadow and blur effects: x, y, blur, spread
- stroke weight: all, top, bottom, left, right
- text content (as a number)
- Motion animation: position distance, scale amount, rotation amount, size amount, opacity amount

**String variables** — applied to: font family, font style/weight (name only), **layer visibility if the
string is `"true"` or `"false"`**, text content, **variant instances in prototyping**.

> "Be sure to use exact spelling when creating string variables for font family and font style or weight.
> However, Figma will recognize the value if it includes hyphens (-), underscores (_), different casings
> (DM Sans, dm sans), and with or without spaces."

**Boolean variables** — variant properties with true/false values; layer visibility.

**Timing variables** — duration in milliseconds; applied to a preset animation's delay and duration.

**Easing variables** — easing curves or spring animations, pre-built or custom; applied to animation
presets and keyframes.

[INFERRED] This list is essentially a map of **every property in a design document that could plausibly be
tokenised**. It is the most useful single artefact in the research for a design-system feature, because
it shows the intended ceiling. Note three design decisions embedded in it:

1. **Visibility is bindable** to a string/boolean variable — i.e. "show this element in dark mode" is a
   design-system concern.
2. **Layout metrics are bindable** — spacing scale is not just for fills.
3. **Count-typed properties exist** (row/column count, whole numbers only) — a typed variable system, not
   just a stringly-typed one.

## Collections, groups, modes

[DOCUMENTED]

- A **collection** is "a set of variables and modes". Used to organise (e.g. one collection for
  localisation strings, another for spatial values).
- **Groups** within a collection further organise variables.
- A **mode** is "a list of values for a variable in a collection, storing one value per variable. Modes also
  represent the different contexts of our designs."
- "When the variable is applied to a layer's property, the layer expresses the value based on the mode it's
  currently in."
- Modes are used for: colour themes (light/dark), languages, device sizes (spacing/padding).
- Limit: **5,000 variables per collection**; number of modes per collection is plan-dependent.

[INFERRED] Modes are a **document-level context switch**, not a per-object override. Every variable
applied anywhere in the document resolves against the current mode. That makes them extremely cheap to
apply and very powerful — and it also means they cannot express "this one card uses the accent colour even
in light mode" without creating a new collection/mode combination.

## Aliasing and tokens

[DOCUMENTED]

> "A variable can reference another variable. That is, you can apply a variable to another variable. Also
> called, 'aliasing' […] **Any variable can reference another variable of the same type.**"

Example usage pattern (from third-party sources, THIRD-PARTY but consistent with the docs): `color/bg`
→ aliases `color/white`; `color/text` → aliases `color/neutral/900`; `space/gutter` → aliases `space/4`.

[INFERRED] **Aliasing is what makes a token graph possible**: primitive tokens → semantic tokens →
component properties. Because aliasing is same-type only and styles cannot reference variables, the graph
must live entirely in the variable layer.

## Local vs library styles

[DOCUMENTED] Both styles and variables can be **published to team libraries**, shared across files and
folders. Local ones exist per file.

## Affinity's style model

[DOCUMENTED]

- A **Styles panel** with named paragraph / character / graphic styles.
- **`Copy Merged` `⇧⌘C`**, **`Paste Style` `⇧⌘V`**, **`Paste without Format` `⌥⇧⌘V`** — three distinct
  clipboard semantics.
- **Effects are separate from styles**: `⌃⌘V` pastes FX.
- **Presets** bundle styles, effects, brushes, gradients into a portable package.
- **Swatches** and **colour themes** in the Document Palette.
- No variables/tokens layer is documented.

[INFERRED] Affinity's style is Adobe-lineage: a named, hierarchical, transferable appearance value. Its
strength is the *clipboard* semantics (paste style across documents); its weakness is that it cannot express
modes or tokens.

## Canva's model

[DOCUMENTED]

- Named **text styles**: Body, Title (H1), Subtitle (H2), Heading (H3), Subheading (H4), Section header (H5);
  bound to `⌥⌘0`–`⌥⌘5` (Canva Docs).
- **`Copy text style` `⌥⌘C` / `Paste text style` `⌥⌘V`** — a dedicated clipboard form for style alone.
- Brand Kit: brand logos, **brand colours**, **colour themes**, **brand fonts**, custom brand assets.
- **Brand Controls**: "To restrict them to using only colors and fonts that are in Brand Kits, set up Brand
  Controls."
- 1,000 Brand Kits per team; 2,000 assets per category per kit.
- Can be **created automatically from a website or a PDF**.
- **No variables.** [DOCUMENTED absence]

[INFERRED] **Governance-by-constraint**: the Brand Kit restricts the *choices offered*, and the chosen value
is **copied into** the design. Nothing references the Brand Kit. This is materially simpler than Figma's
reference model and it makes brand drift possible but brand *error* impossible.

## tldraw's model

[DOCUMENTED — `sdk-features/styles.mdx`, `sdk-features/themes.mdx`, `sdk-features/user-preferences.mdx`]

- Canvas colours are a **named palette** in the record (`color: 'blue'`).
- `themes.mdx` documents **UI** theming (light/dark), distinct from document colours.
- `user-preferences.mdx` documents cross-instance settings.

[INFERRED] A named palette serialises compactly, renders identically everywhere, and is trivially
themeable — but it cannot express "this shape is `#3B82F6`", which is a hard requirement for
pixel-accurate design work. tldraw's palette is a deliberate trade of precision for portability.

## Where styles sit relative to layout

| Product | Can a layout property be a style/variable? |
|---|---|
| Figma | **Yes** — number variables drive padding, gap, min/max dimensions, uniform grid size, row/column count, guide positions |
| Affinity | Not documented |
| Canva | Not documented |
| tldraw | No (no layout engine) |

[DOCUMENTED for Figma]

[INFERRED] Figma's decision to make spacing tokenisable is what makes a Figma design system different from
a Canva brand kit. It also means **Spool's style/variable system has to reach into the layout engine**, not
just the renderer.

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

```rust
pub struct Color { pub red: u8, pub green: u8, pub blue: u8 }
pub struct Fill { pub color: Color }
pub struct Stroke { pub color: Color, pub width: f32 }
pub struct ObjectStyle { pub fill: Option<Fill>, pub stroke: Option<Stroke> }
pub enum StyleEdit { Fill(Option<Color>), Stroke(Option<Color>), StrokeWidth(f32) }
```

- **Styles are fully embedded.** No style identity, no name, no reference.
- **Only two properties**: fill colour and stroke (colour + width). No opacity, no gradient, no effects, no
  corner radius, no typography, no blend mode.
- `Color` is u8 RGB only — **no alpha channel**, despite `Color` appearing in a `Fill` where opacity would
  live.
- `shell.rs` has `common_fill`, `common_stroke`, `common_stroke_width` returning
  `Option<Option<T>>` (mixed / unset / set) — the multi-selection inspector pattern.
- `DocumentCommand::style(changes)` records style changes as before/after `ObjectStyle` pairs in history.
  [OBSERVED]
- No style palette, no styles panel, no tokens, no variables, no brand kit.

[INFERRED] The `StyleEdit` enum and `StyleChange` machinery are a reasonable shape for a
**value-level style system**, and the before/after snapshot model means adding properties is cheap
(`ObjectStyle` is `Copy` + `PartialEq`). The gap is *indirection*: there is currently no way to say "this
colour is `color/primary/500`".

## Candidate architectural implication

**Evidence:**

1. **Styles and variables are distinct systems with different composition rules**, and conflating them is
   a known failure mode Figma explicitly documents around. [Figma DOCUMENTED]
2. Variables can bind **visibility, layout metrics, grid counts, animation timing/easing, and variant
   selection** — not just colour. [Figma DOCUMENTED]
3. Variables are **typed** (colour, number, string, boolean, timing, easing) with type-safe aliasing.
   [Figma DOCUMENTED]
4. Modes are a **document-level context switch**, not per-object. [Figma DOCUMENTED]
5. Style has its **own clipboard representation** in Affinity and Canva, separate from copy/paste.
   [DOCUMENTED]
6. Brand governance can be **enforced by constraint** (Canva Brand Controls) or **expressed by reference**
   (Figma variables). These are different architectures. [DOCUMENTED]
7. A named palette (tldraw) cannot express an arbitrary hex colour — precision vs. portability trade.
   [INFERRED]

**Why it matters:** A style/variable system touches every renderer, every inspector, every
serialiser, every operation, and the AI context model (a design system's variables are the natural
grounding context — see `ai/context.md`). Retrofitting indirection into an embedded-value model means
migrating every stored document.

**Potential Spool approaches:**

- **A. Embedded values only.** What the prototype has. Simple; no design system; no AI grounding.
- **B. A + named styles** (`Object.style: Option<StyleId>`) with styles stored in a document-level table.
  Adds reuse and a clipboard form; no modes or tokens.
- **C. B + variables** (typed, aliased, collection + mode), with styles optionally referencing variables.
  Matches Figma; large.
- **D. Canva-style constraint governance** (a palette that limits choices) without reference semantics —
  much cheaper than C and achieves on-brand output.
- **E. A layered approach:** A → D (cheap on-brand guardrails) → B (named styles) → C (tokens/modes) later.

[INFERRED] E is the attractive path because D and B are independent of C: a palette constraint costs
almost nothing and delivers most of the brand-consistency value; named styles deliver reuse; tokens and
modes are the expensive layer and can come last.

[INFERRED] **However**, if the AI grounding story ("generate using *our* design system") is a v1 goal,
then C (or at least B + a palette) is needed early — because a model cannot respect a design system it
cannot reference.

**Tradeoffs:** Each tier is independent and additive, which is unusual and good. The risk is shipping a
model that cannot later express references without a file migration.

**Decision: TBD — requires architecture review.**

## Open questions

1. Are styles inline values, named references, or both?
2. Do properties store a *resolved* value or a *binding*? (Figma stores a binding and resolves at read.)
   This determines whether rendering needs a resolution pass.
3. Is there a document-level context switch (modes)? If not, how is dark mode handled?
4. Does alpha live on `Color` or on `Fill`?
5. Which properties are styleable — only paint, or also typography and layout metrics?
6. Is style part of the file, or a separate library?
7. How does a style interact with per-object overrides (an object sets fill, breaking the link)?

## Sources

- Figma: "Overview of variables, collections, and modes" (/14506821864087) ⭐; "Modes for variables"
  (/15343816063383); "Create and manage variables and collections" (/15145852043927); "Guide to variables"
  (/15339657135383); "Guide to auto layout" (/360040451373); "Adjust text dimensions and resizing"
  (/27378154668951); "Select layers and objects" (/360040449873)
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/) ⭐; "About Studios"
  (/workspace-about-studios/)
- Canva: "Set up Brand Kits" (/brand-kit/) ⭐; "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- tldraw: `sdk-features/styles.mdx`, `sdk-features/themes.mdx`, `sdk-features/user-preferences.mdx`,
  `sdk-features/shapes.mdx` (record example with `color: 'blue'`)
- Spool prototype: `app/src/canvas.rs` (`Color`, `Fill`, `Stroke`, `ObjectStyle`, `StyleEdit`,
  `StyleChange`, `DocumentCommand::style`, `default_style`, `edited_style`), `app/src/shell.rs`
  (`common_fill`, `common_stroke`, `common_stroke_width`)
