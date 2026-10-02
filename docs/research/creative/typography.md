# Creative: Typography

A focused supplement to `interaction/text-editing.md`. That file covers interaction; this one covers the
**typography model**.

## The typography property surface

Across products, a text object carries these properties. Sources are noted per row.

| Property | Figma | Affinity | Canva | tldraw |
|---|---|---|---|---|
| Font family | Yes (String variable can bind) | Yes | Yes (Brand Kit fonts) | Unknown |
| Font style / weight | Yes (String for name; Number for numeric weight) | Yes | Yes | Unknown |
| Font size | Yes (**Number variable can bind**) | Yes | `⇧⌘.` / `⇧⌘,` | Unknown |
| Line height | Yes (**Number variable, px**) | Yes | `⌥⌘↑` / `⌥⌘↓` | Unknown |
| Letter spacing (tracking) | Yes (**Number variable, interpreted as px, not %**) | Yes | `⌥⌘.` / `⌥⌘,` | Unknown |
| Paragraph indent | Yes (**Number variable**) | Yes | Not documented | Unknown |
| Paragraph spacing | Yes (**Number variable**) | Yes | Not documented | Unknown |
| Horizontal alignment | Left / centre / right / justify | Yes | `⇧⌘L` / `⇧⌘C` / `⇧⌘R` | Unknown |
| Vertical alignment | **Only on fixed-size text layers** | Yes | `⌘⇧H` / `⌘⇧M` / `⌘⇧B` (anchor top/middle/bottom) | Unknown |
| Case transform | Not documented (`⇧⌘K` uppercases in Canva) | Yes | `⇧⌘K` | Unknown |
| Underline / strikethrough | Yes | Yes | `⌘U` / `⇧⌘S` | Rich-text marks |
| Superscript / subscript | Not documented | Yes | `⌘.` / `⌘,` | Rich-text marks |
| Lists | **Yes** ("Adjust list indentation `⌘[` / `⌘]`" is remapped per layout, implying list controls exist) | Yes | Numbered `⇧⌘7`, bulleted `⇧⌘8` | Rich-text marks |
| Max lines | **Yes** (mutually exclusive with max height) | Not documented | Not documented | Unknown |
| Wrap style | **Yes — Balance / Pretty** (no effect on auto-width) | Text wrap | Automatic | Unknown |
| Opacity / colour | Yes (Number variable can bind colour) | Yes | Yes | Yes |
| Baseline grid snapping | Not documented | **Yes — "Text can also snap to the baseline of other text (the first line only for text frames)"** | Not documented | Not documented |
| Font weight numeric only | Yes (String var for name, Number var for weight 400/700) | Yes | Not documented | Unknown |

[DOCUMENTED where cited]

## The five things that actually differ

### 1. Text sizing mode: auto-width vs. fixed

[Fully DOCUMENTED — Figma]

| Creation gesture | Mode |
|---|---|
| Single click | Auto width |
| Click and drag | Fixed size |
| Manual bounding-box resize | Switches to Fixed size |

Consequences: wrap style is ignored on auto-width; vertical alignment is ignored on auto-height; fixed-size
text in an auto-layout frame can overlap.

[INFERRED] This is the highest-value single typography decision, because it is made *by gesture* and it
determines every downstream behaviour.

### 2. Bounds vs. font size are distinct

[Fully DOCUMENTED — Figma]

> "You can also use the scale tool to change the size of a text layer. If you take this approach, you'll
> change the font size, as well as the bounds of the text layer. […] This can lead to **fractional font
> sizes** or **layers with subpixel positions and dimensions**. Aside from annoying pixel perfectionists, it
> can also lead to **unwanted export artifacts** and dimensions. If you just want to change the size of a
> text layer […] we recommend adjusting the **Font size** […] instead. This makes sure your font size is a
> **whole number**."

[INFERRED] Three concepts, kept separate:
1. **Bounds** — the layout box
2. **Font size** — the type metric (should be integral)
3. **Scale** — a bulk transform that changes both

### 3. Baseline snapping

[DOCUMENTED — Affinity only]

> "Text can also snap to the baseline of other text (**the first line only for text frames**) and artistic
> text objects can snap to the height of previously created artistic text."

Plus Affinity has a **baseline grid** in Layout Studio ("Snap to baseline grid (Layout Studio)").

[INFERRED] Baseline alignment is a professional typesetting feature that requires **real shaped text
metrics** (ascent/descent per line), not just a bounding box. It is also the most direct visual expression of
"this is a design tool, not a mockup tool".

### 4. Named text styles and their clipboard form

| Product | Styles | Clipboard |
|---|---|---|
| Canva | Body, Title (H1), Subtitle (H2), Heading (H3), Subheading (H4), Section header (H5) — bound to `⌥⌘0`–`⌥⌘5` | **`Copy text style` `⌥⌘C` / `Paste text style` `⌥⌘V`** |
| Affinity | Paragraph / character / graphic styles | **`Copy Merged` `⇧⌘C` / `Paste Style` `⇧⌘V` / `Paste without Format` `⌥⇧⌘V`** |
| Figma | Text styles; publishable to libraries | Via the object / `⌘V` |
| tldraw | Not documented | Not documented |

[DOCUMENTED]

[INFERRED] Two of four products treat *style* as a transferable value distinct from content. That implies
style has an identity in the document model, not just a set of inline properties.

### 5. Typography as a token surface

[DOCUMENTED — Figma Number variables]

Line height, letter spacing (px), font size, font weight (numeric), paragraph indent, paragraph spacing, and
font family/style (String variables) are **all bindable**. And String variables can set **layer visibility**
and **variant instances in prototyping**.

[INFERRED] Typography is not a leaf of the document tree; it is at the bottom of a resolvable value graph.
See `document/variables.md`.

## Vertical alignment is conditional

[DOCUMENTED — Figma "Explore text properties"]

> "It's only possible to vertically align text in text layers with a **Fixed Size**. Layers with resizing set
> to Auto Width or Auto Height will ignore alignment."

Canva, by contrast, offers vertical anchoring unconditionally (`⌘⇧H` / `⌘⇧M` / `⌘⇧B`) — presumably because
Canva text elements always have a defined frame. [DOCUMENTED]

[INFERRED] Vertical alignment is only meaningful when the text box has a definite height. On an auto-height
text box there is no space to align within. Figma makes this explicit; Canva avoids the case.

## Wrapping

| Product | Model |
|---|---|
| Figma | **Wrap style: Balance (distribute lines evenly) or Pretty (avoid a single stranded word)**. Ignored on auto-width. |
| Affinity | Text wrap in frames |
| Canva | Automatic |
| tldraw | Unknown (`text-measurement.mdx` exists) |

[DOCUMENTED for Figma]

[INFERRED] "Balance" and "Pretty" are *line-breaking quality heuristics*, not wrapping modes. They are a
recognised typographic refinement (borrowed from CSS `text-wrap: balance` / `pretty`) and are cheap to
implement relative to their perceived polish.

## Overflow

| Product | Documented |
|---|---|
| Figma | Max lines **and** max height, **mutually exclusive**: "Adding a max height will set max lines to Auto. Setting max lines to a number will remove the layer's max height setting." |
| Affinity | Not documented in pages read |
| Canva | Not documented |
| tldraw | Not documented |

[DOCUMENTED for Figma]

[INFERRED] The mutual exclusion is a symptom: max lines and max height are two ways to express "stop here",
and allowing both creates ambiguous states. A cleaner model would have one overflow rule with a mode.

## Type in layout

| Case | Documented behaviour |
|---|---|
| Text in auto layout | Auto width recommended; fixed size "may cause **overlap** between layers" |
| Text with min/max size | **"Text layers cannot have both a max height and a set number of max lines"** |
| Text in a component instance | Aspect ratio not settable on instance children |
| Text in a scrolling frame | Requires the auto layout to be inside a regular frame |

[Fully DOCUMENTED — Figma auto layout guide]

[INFERRED] A content-driven text box in a hug-sized container in a scrolling frame is a contradiction that
Figma resolves by requiring two container types. Any Spool layout design must confront the same.

## Variable fonts

[NOT DOCUMENTED in any of the four products' pages reviewed]

[INFERRED] The research brief lists "variable fonts" as a topic. None of the four documents describe them.
tldraw's `text-measurement` module would be where the dependency would live. **This is a documented research
gap** — likely a deliberate product boundary (variable fonts add significant measurement complexity).

## Missing font behaviour

[NOT DOCUMENTED in any product]

[INFERRED] This is a serious gap, because a missing font changes **measurement**, which changes **bounds**,
which changes **layout**, **snapping**, **culling**, and **export**. A design editor must decide: substitute
a metrically-compatible font, substitute an arbitrary font and relayout, or refuse to open the file.

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

- `DesignObject.text_content: Option<String>` — **a flat string. No typography at all.**
- `ObjectStyle { fill, stroke }` — **no font family, size, weight, line height, letter spacing, or
  alignment.**
- Text rendering: `render_text_input` builds a GPUI text element; `line_height = gpui_px(18.0 *
  self.camera.zoom)` — **a hard-coded 18 world units of line height**, scaled by zoom. [OBSERVED]
- `text_offset_at_local_point` uses `nearest_boundary_from_positions(text, &[(usize, f32)], x)` — so
  **per-character positions are computed**, implying GPUI is being asked for glyph metrics. [OBSERVED]
- Caret, range selection, reversal, IME `marked_range`, and UTF-8/16 conversion are all present. [OBSERVED]
- `shell.rs::text_content_section` displays content read-only. [OBSERVED]
- **No font loading, no font family/size/weight, no wrapping, no alignment, no overflow, no text styles.**

[INFERRED] The hard-coded `18.0` line height is a strong signal that no font metric is being read: real
line height must come from the font's ascent/descent (and line-gap), scaled by the font size. The
`nearest_boundary_from_positions` function suggests character advance *is* available, which means the missing
piece is the font loading and per-object typography, not the measurement capability itself.

## Candidate architectural implication

**Evidence:**

1. **The creation gesture determines the text sizing mode**, which then determines whether wrap style and
   vertical alignment apply. [Figma DOCUMENTED]
2. **Bounds, font size, and scale are three distinct concepts**, and conflating them yields fractional font
   sizes and export artifacts. [Figma DOCUMENTED]
3. **Vertical alignment requires a definite box height.** [Figma DOCUMENTED]
4. Typography properties are **bindable in a token system**, so the renderer must resolve them.
   [Figma DOCUMENTED]
5. **Text style is a transferable value with its own clipboard form** in two of four products.
   [DOCUMENTED]
6. **Baseline snapping requires shaped text metrics** (ascent/descent per line), which are more than a
   bounding box. [Affinity DOCUMENTED]
7. **Max lines and max height are mutually exclusive** — two ways to express one rule create ambiguity.
   [Figma DOCUMENTED]
8. **Missing-font behaviour is undocumented in all four products**, and it affects measurement → bounds →
   layout → export. [Documented gap]

**Why it matters:** Text is the most common object in a design document, the most common target of AI
edits, and the object whose rendering depends on the most external state (fonts). Getting the typography
model wrong means reworking the renderer, the inspector, the AI context extraction, and export.

**Potential Spool approaches:**

- **A. String + a few object-level typography fields** (family, size, weight, line height, letter spacing,
   alignment). Covers most design work.
- **B. A + a `TextStyle` reference** (local or document-level), with a clipboard form. Adds reuse.
- **C. B + per-axis sizing mode** (`AutoWidth` / `Fixed` / `AutoHeight`). Required for layout compatibility
   and for Figma-like gesture semantics.
- **D. C + baseline metrics and baseline snapping.** Professional typesetting.
- **E. Any + a design-token layer** where typography properties resolve through variables.
- **F. Any + rich-text runs.** Large; deferrable.

[INFERRED] **A → C is the prerequisite chain and should be treated as one unit**, because neither is useful
without the other: typography properties without a sizing mode produce fixed text that overlaps in layout,
and a sizing mode without typography is meaningless.

[INFERRED] **B is cheap and delivers the design-system story at the typography level.** E is the layer that
makes AI generation credible. D is the "more than UI mockups" feature. F is the one that can be deferred
indefinitely.

**Decision: TBD — requires architecture review.**

## Open questions

1. Where do typography properties live — on the object, in a style, or resolved from a variable?
2. Does Spool load fonts from the system, from the document, or both?
3. What is the missing-font policy?
4. Is line height derived from font metrics or stored as a value?
5. Are per-axis sizing modes (`AutoWidth`/`AutoHeight`/`Fixed`) stored per text object?
6. Is baseline alignment in scope?
7. Rich text: yes, ever, or never?
8. How does typography export to SVG/HTML? (SVG text requires font embedding or outlining.)

## Sources

- Figma: "Adjust text dimensions and resizing" (/27378154668951) ⭐; "Explore text properties"
  (/360039956634) ⭐ (vertical alignment on fixed size only); "Guide to text in Figma Design"
  (/360039956434); "Guide to auto layout" (/360040451373) ⭐ (max lines/max height, text in layout, scaling);
  "Overview of variables, collections, and modes" (/14506821864087) ⭐ (typography bindings)
- Affinity: "Snapping" (/design-aids-snapping/) ⭐ (text baseline snapping); "Keyboard shortcuts for general
  editing" (/workspace-shortcuts-editing/) ⭐ (copy merged, paste style, text style bindings)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/) ⭐⭐ (all typography bindings, named text
  styles, copy/paste text style, anchors)
- tldraw: `sdk-features/text-measurement.mdx`, `sdk-features/rich-text.mdx`, `sdk-features/text-shape.mdx`
- Spool prototype: `app/src/canvas.rs` (`DesignObject.text_content`, `render_text_input`,
  `nearest_boundary_from_positions`, `text_offset_at_local_point`, hard-coded `18.0` line height),
  `app/src/shell.rs` (`text_content_section`)
