# Document Model: Variables & Design Tokens

Only Figma has a system it calls "variables". The other three solve the same problem differently, and the
differences are architecturally significant.

## The four answers to "how do I make this reusable and changeable"

| Product | Mechanism | Referenced? | Mode-switchable? | Composable? | Enforced? |
|---|---|---|---|---|---|
| **Figma** | **Variables** + collections + modes + aliasing | **Yes** | **Yes** | **Yes (same-type aliasing)** | No |
| **Affinity** | Styles + presets + swatches | No (values copied) | No | Yes (styles compose) | No |
| **Canva** | Brand Kit + **Brand Controls** | No | No | No | **Yes (constraint)** |
| **tldraw** | Named palette | Yes (by name) | Yes (theme) | No | No |

## Figma's variable system

[Fully DOCUMENTED — "Overview of variables, collections, and modes"]

> "Variables are raw values—like color, numbers, and strings—that can change in value depending on the
> context of a design, such as light and dark modes, or mobile and desktop modes. Like styles and
> components, variables can also be **published to team libraries**. When you update the value of a
> variable, you can update designs across files accordingly."

### Types

| Type | Defined by | Can be applied to |
|---|---|---|
| **Color** | "color values with opacity, such as `#000000` at 80% opacity" | colour styles, fill colours, gradient stops, shadow effects, stroke colours, other colour variables |
| **Number** | "whole numbers or any decimal number up to the hundredth place, such as 12.75" | (long list below) |
| **String** | "a sequence of characters such as Inter, Hello world!, or 94102" | font family, font style/weight (name only), **layer visibility if "true"/"false"**, text content, **variant instances in prototyping**, other string variables |
| **Boolean** | "true and false values" | variant properties with true/false values; layer visibility |
| **Timing** | "Duration in milliseconds" | a preset animation's delay and duration in Figma Motion |
| **Easing** | "Easing curve or spring animation, either pre-built or custom" | animation presets and keyframes in Figma Motion |

### Number variables — the full property list

- Corner radius **and individual corner radius**
- Dimensions, **including minimum and maximum width/height**
- Font properties: **font size**, **font weight (numbers only, e.g. 400, 700)**, **line height**,
  **letter spacing (interpreted as Px, not %)**
- **Paragraph indent**, **paragraph spacing**
- Layout guides
- **Uniform grid size**
- **Row and column count (whole numbers only)**
- Width, height, margin, offset, gutter
- **Padding and gap**
- Shadow and blur effects: X, Y, blur, spread
- **Stroke weight: all, top, bottom, left, right**
- Text content
- Text styles
- Figma Motion: position distance, scale amount, rotation amount, size amount, opacity amount

[INFERRED] This list is the most useful published artefact in the research for designing a
design-token system, because it shows the intended ceiling: a token can drive **paint, typography,
layout, geometry, and animation**, not just colours.

Three embedded design decisions:
1. **Visibility is tokenisable** (via string `"true"`/`"false"` or boolean) — "show this in dark mode" is a
   token concern.
2. **Layout metrics are tokenisable** — spacing scales reach into the layout engine.
3. **Typed constraints exist**: weight is "numbers only", row/column count is "whole numbers only",
   letter spacing is "px, not %".

### Colour opacity rules [DOCUMENTED]

> "You can use number variables on a colour variable's opacity property. **If the number variable has a
> negative value, the opacity will default to 0%. If the number variable has a value greater than 100, the
> opacity will default to 100%.** You can also alias a colour to the variable while maintaining a separate
> opacity."

[INFERRED] The opacity channel is **clamped**, not rejected. That is a deliberate UI choice (accept bad
input, clamp it) and it means a resolver must clamp, not assume validity.

### String normalisation [DOCUMENTED]

> "Be sure to use exact spelling when creating string variables for font family and font style or weight.
> However, Figma will recognize the value if it includes hyphens (-), underscores (_), different casings
> (DM Sans, dm sans), and with or without spaces."

[INFERRED] Font-name matching is **fuzzy-normalised** (lowercase, separators stripped). This is a pragmatic
compromise between strict matching and full font-database resolution.

### Collections, groups, modes

[DOCUMENTED]

| Concept | Definition |
|---|---|
| **Collection** | "A set of variables and modes." Organises related variables (e.g. localisation strings vs. spatial values) |
| **Group** | Further organisation within a collection (e.g. colours for text vs. colours for strokes) |
| **Mode** | "A list of values for a variable in a collection, **storing one value per variable**. Modes also represent the different contexts of our designs." |
| Limit | **5,000 variables per collection**; modes per collection is plan-dependent |

Resolution:

> "When the variable is applied to a layer's property, **the layer expresses the value based on the mode
> it's currently in**."

Documented mode use cases: colour themes (light/dark), languages (to see how copy flows), device sizes
(to see how elements look with different spacing and padding).

[INFERRED] **Modes are a document-level context switch.** Every variable applied anywhere resolves against
the *current* mode. This makes them extremely cheap to apply and switch, and it means they cannot express
per-object overrides without a new collection or mode.

### Aliasing [DOCUMENTED]

> "A variable can reference another variable. That is, you can apply a variable to another variable. Also
> called, 'aliasing'. […] **Any variable can reference another variable of the same type.**"

[INFERRED] The token graph is therefore:

```
primitive token     color/blue/500 = #3B82F6
      ↓ alias (same type)
semantic token      color/action/primary = color/blue/500
      ↓ referenced by
component property  Button.background = color/action/primary
```

Styles cannot participate: "styles cannot be used in other styles or variables" and "variables can be
applied to styles". So the graph lives **entirely in the variable layer**, with styles as optional
consumers.

### Interaction with prototyping [DOCUMENTED]

- `Set variable` — sets or modifies a variable value as a result of a trigger.
- `Set variable mode` — changes the mode of a page while prototyping.
- `Conditional` — "Check if a condition is met before performing an action by using an if/else conditional
  statement."
- String variables can select **variant instances**.

[INFERRED] **Variables + actions + conditionals together constitute a small scripting environment over the
document.** This is a significantly more powerful model than "prototypes are a list of links", and it is the
mechanism by which Figma implements interactive components and stateful prototypes.

## Canva: governance by constraint

[Fully DOCUMENTED — "Set up Brand Kits"]

- Brand Kit holds **brand logos, brand colours, colour themes, brand fonts, and assets**, plus **Brand
  Templates** and **linked folders**.
- **"To restrict them to using only colors and fonts that are in Brand Kits, set up Brand Controls."**
  [DOCUMENTED]
- 1,000 Brand Kits per team; 2,000 assets per category per kit.
- Can be created **automatically from a website or a PDF**.
- Roles: admin, brand designer, member.

[INFERRED] **Nothing references the Brand Kit.** When a user picks a brand colour, the hex value is written
into the element. Brand Controls only restrict the *choices offered in the UI*.

The trade:
- **Gain:** impossible to pick an off-brand colour; users cannot accidentally break brand rules; zero
  conceptual complexity; no resolution pass.
- **Lose:** changing the brand colour later does not update existing designs; no token semantics; no
  modes; no automated dark mode.

## Affinity: styles and presets, no tokens

[DOCUMENTED]

- Styles panel (paragraph / character / graphic).
- `⇧⌘V` Paste Style, `⌥⇧⌘V` Paste without Format.
- Swatches and colour themes in the Document Palette.
- **Presets**: portable bundles of styles, effects, brushes, gradients.
- **No variables/tokens documented.** [DOCUMENTED absence]

[INFERRED] Affinity's "design system" is a **portable file** (preset) rather than a **live binding**.

## tldraw: a named palette

[DOCUMENTED] `props.color` holds a palette name (`'blue'`). `themes.mdx` documents UI themes.

[INFERRED] This is a *reference* system (the shape names a colour) but with a **fixed, small palette**. It
gives themeability but not precision. tldraw also documents custom colour support through custom shapes,
which is how a host app extends beyond the palette.

## Variables as an AI grounding mechanism

This is the most important cross-product finding in this document.

| Product | Documented statement |
|---|---|
| **Canva** | "**Canva AI gets to know your brand's fonts, colours, and rules so every design stays on brand.**" (canva.com/canva-ai) |
| **Canva** | "Canva AI 2.0 can **pull context from your connected tools** to generate on-brand designs" |
| **Figma** | The Figma Design Agent "can generate **design layers**"; the platform publishes Variables as the token mechanism |
| **Affinity** | MCP capability "**Use Canva AI Studio features** — Use Canva AI Studio features to complete tasks. Premium and Ultra Canva AI tools will use up your Canva plan's monthly AI allowance." |

[DOCUMENTED]

[INFERRED] **The brand/token system is the AI's grounding context.** A model cannot respect a design system
it cannot reference. This makes variables not a "design system nicety" but a **prerequisite for credible
AI generation** — and it is the strongest argument in this research for treating a variable system as
foundational rather than as a later feature.

Note that Canva achieves grounding with **no variables at all** — via a Brand Kit that supplies the palette
and a constraint that keeps generation on-brand. So the mechanism is "here is the allowed vocabulary", not
necessarily "here is a reference graph".

## Variables and export

[DOCUMENTED — Figma Dev Mode]

- Number variables bind to CSS custom-property-friendly properties: corner radius, dimensions, min/max,
  font size/weight/line height/letter spacing (px), paragraph indent/spacing, padding, gap, shadow
  x/y/blur/spread, stroke weight, opacity.
- Figma exports `transform: rotate(-90deg)` for a 90° Figma rotation and `matrix()` for flips — so
  **variable-driven values and derived transforms both have to survive into code output**.

[INFERRED] A variable that resolves differently per mode cannot be exported as a single CSS value; it must
be exported per mode. This is an export-layer requirement that only becomes apparent once modes exist.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **Nothing.** No variables, no collections, no modes, no tokens, no brand kit.
- `Color { red, green, blue: u8 }` — **no alpha**, no variable reference, no palette.
- `Fill { color: Color }`, `Stroke { color: Color, width: f32 }` — hard-coded values on the object.
- `default_style(object_type)` and `edited_style(style, edit)` produce literal values. [OBSERVED]

[INFERRED] The absence of an alpha channel in `Color` is worth flagging: `Fill` is the natural home for
opacity, so adding variables later means deciding where opacity lives (`Color` vs. `Fill` vs. `ObjectStyle`)
before any of it is persisted.

## Candidate architectural implication

**Evidence:**

1. There are **three genuinely different architectures** for design tokens: referenced+aliased+modal
   (Figma), constraint-governed palette (Canva), and portable-file styles (Affinity). They are not
   variations of one idea. [DOCUMENTED]
2. **Variables must be typed** and aliasing must be **same-type only**. [Figma DOCUMENTED]
3. **Modes are a document-level context switch**, not a per-object override. [Figma DOCUMENTED]
4. A token can drive **paint, typography, layout metrics, geometry, counts, and animation timing/easing**.
   [Figma DOCUMENTED]
5. **Visibility and variant selection are tokenisable.** [Figma DOCUMENTED]
6. Colour opacity via a number token is **clamped**, not rejected. [Figma DOCUMENTED]
7. **Styles cannot participate in the token graph**; only variables can. [Figma DOCUMENTED]
8. The **brand/token system is the AI's grounding context**, and Canva achieves that grounding *without*
   variables, via a palette plus constraint. [Canva + Affinity DOCUMENTED]
9. Per-mode values cannot be exported as a single CSS value. [INFERRED]

**Why it matters:** A variable system is required for (a) design systems, (b) themes/modes, (c) credible
AI generation, and (d) token-based code export. It is also the system that most often gets retrofitted
badly, because it changes how *every* property is stored (resolved value vs. binding).

**Potential Spool approaches:**

- **A. No variables. Literal values only.** What the prototype has. Cheapest; no design system; AI
   generation will be off-brand.
- **B. A named palette + a UI constraint** (Canva's model). Very cheap; delivers on-brand output and a
  valid AI grounding vocabulary; no live updates.
- **C. Typed variables with aliasing, no modes.** The token graph; live updates; no dark mode.
- **D. C + modes.** Full Figma parity; the most complex.
- **E. C + per-mode resolution applied at render time**, with the resolved value cached per (object, mode).

[INFERRED] **B is the highest value-per-cost step and should not be skipped**: it costs almost nothing,
delivers brand consistency, and — critically — it gives an AI system a legitimate vocabulary to generate
from without requiring the full C/D stack. C then adds live-update power. D is the expensive tail.

[INFERRED] **The critical storage decision, which must be made before any of these:** does a property store
a *resolved value* or a *binding*? Figma stores a binding and resolves at read. That means every read path
(render, hit test, snap, export, inspector display, agent context) must resolve through the token graph —
including hot paths. Storing resolved values and re-resolving on token change is an alternative with a
different performance profile.

**Decision: TBD — requires architecture review.**

## Open questions

1. Resolved value or binding stored on the property?
2. Are variables typed? Which types?
3. Can variables alias? Same-type only?
4. Are there modes, and is the mode document-level or per-object?
5. Does a variable bind to a whole property or to part of one (e.g. colour + separate opacity)?
6. Can variables drive visibility, typography, and layout metrics — or only colour?
7. Where does opacity live (Color, Fill, or style)?
8. Are variables part of the file, or a separate shared library? (Libraries imply cross-file references.)
9. How does an agent receive the variable set as context? (See `ai/context.md`.)
10. Does mode affect export, and how?

## Sources

- Figma: "Overview of variables, collections, and modes" (/14506821864087) ⭐⭐; "Modes for variables"
  (/15343816063383); "Create and manage variables and collections" (/15145852043927); "Guide to variables"
  (/15339657135383); "Prototype actions" (/360040035874) (Set variable, Set variable mode, Conditional);
  "Guide to auto layout" (/360040451373) (min/max, padding, gap); "Adjust text dimensions and resizing"
  (/27378154668951); "Guide to components in Figma" (/360038662654)
- Canva: "Set up Brand Kits" (/brand-kit/) ⭐; "Canva AI 2.0" (/canva-ai/) ⭐ (AI grounding)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐ (Canva AI Studio as an MCP capability);
  "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/)
- tldraw: `sdk-features/styles.mdx`, `sdk-features/themes.mdx`, `sdk-features/shapes.mdx` (record example),
  `sdk-features/user-preferences.mdx`
- Spool prototype: `app/src/canvas.rs` (`Color`, `Fill`, `Stroke`, `ObjectStyle`, `default_style`,
  `edited_style`)
