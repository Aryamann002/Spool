# Interaction: Text Editing

Text is where the four products diverge most from each other, because text is simultaneously a *content*
model, a *layout* model, and an *input* model. Every product must answer the same three questions, and
they answer them differently.

## The three-state distinction, again

| State | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Text object selected | Selection + inspector shows typography | Selection + context toolbar | Selection | Selection |
| Text being edited | Caret + range, `Esc` to exit | `editingId` = the text shape | Text Frame focused | Text caret |
| Text range selected | Drag within the object | Rich-text mark range | Character range | Selection |

[INFERRED] All four need a **third state** beyond selected/edited: a *sub-range* selection. Spool's
prototype already models this correctly (`TextEditState { selected_range, selection_reversed,
marked_range, pointer_anchor }`) — this is one area where the prototype is ahead of its simplicity.

## Creation intent determines text layout mode

This is Figma's cleanest and most transferable text idea.

[Fully DOCUMENTED — "Adjust text dimensions and resizing"]

| Gesture | Resulting resizing property | Behaviour |
|---|---|---|
| **Single click** on canvas | **Auto width** | "the text layer to grow horizontally to accommodate any new text you add". Breaks lines only at explicit Enter |
| **Click and drag** | **Fixed size** | "the width and height of the text layer will stay the same, regardless of the text content" |
| **Manually resize the bounding box** | Switches to **Fixed size** | "When you manually change a layer's dimensions in the canvas, Figma will also update the resizing property to Fixed size" |

Consequences:
- **Wrap style (Balance / Pretty) has no effect on auto-width layers.** [DOCUMENTED] "Layers set to Auto
  width only break where you press Return or Enter, so wrap style has no effect on them."
- **Vertical text alignment only works on Fixed-size layers.** [DOCUMENTED — "Explore text properties"]
  "Layers with resizing set to Auto Width or Auto Height will ignore alignment."
- Figma recommends auto-width text inside auto layout; fixed-size text can **overlap** siblings.
  [DOCUMENTED]
- Text cannot have both a max height and a numeric max lines — setting one clears the other.
  [DOCUMENTED]

[INFERRED] **The creation gesture determines the sizing mode.** This is a general-purpose convention: the
user's drag tells the editor what kind of text box they want. Spool should adopt it — it costs nothing
and removes a whole class of "why did my text wrap like that" confusion.

## Scale tool vs. font size — an explicit warning from Figma

[DOCUMENTED]

> "You can also use the scale tool to change the size of a text layer. If you take this approach, you'll
> change the font size, as well as the bounds of the text layer. […] This can lead to **fractional font
> sizes** or **layers with subpixel positions and dimensions**. Aside from annoying pixel perfectionists,
> it can also lead to unwanted export artifacts and dimensions. If you just want to change the size of a
> text layer in relation to other elements in a design, we recommend adjusting the **Font size** in the
> Typography settings instead. This makes sure your font size is a **whole number**."

[INFERRED] Three separate concepts are being distinguished and it is worth keeping them separate:
1. **Bounds** (the layout box)
2. **Font size** (the type metric)
3. **Scale** (a bulk transform that changes both)

Conflating them produces fractional font sizes, which break pixel export and text metrics.

## Typography as a bindable property

Figma's **Number variables** can drive: font family (via String), font style/weight (String), font size,
font weight, line height, letter spacing ("interpreted as **px, not %**"), paragraph indent, paragraph
spacing. [DOCUMENTED — "Overview of variables, collections, and modes"]

**String variables** can set layer visibility when the value is `"true"`/`"false"` and can switch **variant
instances in prototyping**. [DOCUMENTED]

[INFERRED] This means typography in Figma is not a leaf property — it is a **resolvable expression tree**:
`font_size = Variable("type/scale/md")`, `letter_spacing = Variable("space/tight")`. Any Spool
design-system feature must therefore treat type properties as bindable, not as plain numbers.

## Layout interaction between text and layout engines

| Concern | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Text in flow layout | Auto width recommended; fixed causes overlap | No flow layout | Text Frames in Layout Studio; **baseline grid**; columns | No flow layout |
| Text measurement | Internal | **`sdk-features/text-measurement.mdx`** — a documented subsystem | Internal | Internal |
| Baseline snapping | Not documented | No | **Yes — "Text can also snap to the baseline of other text (the first line only for text frames)"** | Not documented |
| Text wrap | Wrap style (Balance/Pretty) | Rich text | Text wrap in frames | Automatic |
| Text extraction to plain string | Via Plugin API | **`ShapeUtil#getText(shape)`** — a core hook returning `undefined` when absent | Not documented | Not documented |

[INFERRED] tldraw's `getText()` being a **shape-util hook** rather than a text-subsystem API is
significant: it means every shape type can declare its own text extraction, which is exactly what an AI
agent needs to build structured context. See `ai/context.md`.

## Named text styles

| Product | Model | Clipboard form |
|---|---|---|
| Figma | **Text styles** — publishable to libraries; composable with variables | Via plugin / `⌘V` pastes the object |
| Affinity | **Styles panel** — paragraph / character / graphic styles | **`Copy Merged` `⇧⌘C`, `Paste Style` `⇧⌘V`, `Paste without Format` `⌥⇧⌘V`** |
| Canva | **Named text styles** — Body, Title H1, Subtitle H2, Heading H3, Subheading H4, Section header H5, bound to `⌥⌘0`–`⌥⌘5` | **`Copy text style` `⌥⌘C` / `Paste text style` `⌥⌘V`** |
| tldraw | Not documented | Not documented |

[INFERRED] "Copy text style / Paste text style" as **separate commands from copy/paste** appears in both
Affinity and Canva. It implies text style is a first-class, transferable document value with its own
clipboard representation. Figma's equivalent is less explicit at the interaction level.

## Input method and text editing mechanics

### What is documented

- Figma: text editing via double-click or the Text tool; ranges by drag or `⇧`-click; `Esc` to exit.
  [DOCUMENTED at the behaviour level]
- tldraw: `rich-text` module; marks on text; the AI pipeline calls `toRichText('❤️')`.
  [DOCUMENTED]
- All products: cut/copy/paste and find-and-replace (`⌘F` in Canva). [DOCUMENTED for Canva]

### What is NOT documented

- **IME composition behaviour** in any of the four products' documentation. **Unknown.**
- Caret navigation model (grapheme clusters vs. UTF-16 code units). **Unknown.**
- Bidirectional text (RTL) handling. **Unknown** for all four.
- Selection of grapheme clusters vs. code points. **Unknown.**

### Spool prototype's position on this [OBSERVED in source]

The prototype is **more careful here than the documentation of any of the four products**:

- `impl EntityInputHandler for CanvasView` — a real GPUI IME/preedit hook.
- `TextEditState { id, original_text, editing_text, selected_range, selection_reversed, marked_range,
  pointer_anchor }`.
- `marked_range: Option<Range<usize>>` — an **IME composition range**, distinct from the selection.
- UTF-8 ↔ UTF-16 conversion helpers: `utf8_boundary`, `utf16_to_utf8`, `utf8_to_utf16`,
  `utf16_range_to_utf8`, `utf8_range_to_utf16`, `previous_char_boundary`, `next_char_boundary`.
- `nearest_boundary_from_positions(text, &[(usize, f32)], x) -> usize` — **hit-testing computed glyph
  positions**, not character metrics.
- `selection_anchor(range, reversed)` — an explicit **anchor/focus** model for reversed ranges.
- `text_offset_at_local_point(text, local, zoom, window)` — caret positioning via a real text-shaping
  query at the object's local scale.

[INFERRED] `nearest_boundary_from_positions` taking measured positions is the correct approach and
implies GPUI provides per-glyph advances. `previous_char_boundary` / `next_char_boundary` appear to
implement grapheme-boundary awareness. These are the details that make text editing feel right, and they
are already present.

## Text inside containers — the difficult cases

| Case | Figma's documented behaviour |
|---|---|
| Text in an auto-layout frame | Auto width recommended; fixed-size text "won't resize to accommodate your text, which may cause **overlap** between layers" |
| Text inside a component instance | Can be overridden; aspect ratio not settable on instance children |
| Text in a frame with constraints | Constraints apply to the text object, not to its internal flow |
| Text max lines / max height | Mutually exclusive |
| Text in a prototype with scrolling | "As an auto layout parent's dimensions are content-driven, it will resize to fit the objects. To replicate scrolling overflow you will need to put the auto layout inside a regular frame." |
| Text as a variable binding | String variables can set text content |

[DOCUMENTED — "Guide to auto layout"]

[INFERRED] The last two rows are the ones that reveal the real constraint: **a flow layout that hugs its
content cannot clip its content.** Any layout system that supports both content-driven sizing and
scrolling/clipping needs two container types. Figma's answer (nested frames) is the general answer.

## Overflow

| Product | Documented |
|---|---|
| Figma | Max lines; max height (mutually exclusive); clipping via frame "Clip content"; vertical alignment only on fixed-size |
| Affinity | Text Frame vs Text Box distinction; text wrap; **baselines**; overflow behaviour not documented in pages read |
| tldraw | Text measurement module exists; overflow rules not documented in pages read |
| Canva | Text anchoring top/middle/bottom; automatic wrap; overflow rules not documented |

## Spool prototype: what exists

From `app/src/canvas.rs` and `app/src/shell.rs` [OBSERVED in source]:

- `DesignObject.text_content: Option<String>` — **a single flat string per object. No rich text, no runs,
  no marks.**
- `ObjectType::Text` is one of four types.
- `Tool::Text` with `Tool::creates_object() == Some(ObjectType::Text)`.
- Text creation: drag with the Text tool → `creation_geometry()` produces a `Geometry` with a minimum of
  `MIN_OBJECT_SIZE` (20×20). **No auto-width behaviour; no click-to-place.**
- Text editing entry points, in priority order in `begin_left_interaction`:
  1. already editing this object → reposition caret
  2. editing a *different* object and clicking this one → `commit_text_edit()` then continue
  3. `Tool::Text` + click on an existing `ObjectType::Text` → begin edit
  4. `Tool::Select` + `event.click_count >= 2` on text → begin edit
- `commit_text_edit()` produces **one** `TextChange { before, after }` for the whole editing session —
  i.e. an entire multi-keystroke edit session is one undo step. This matches the industry norm.
- `escape_discards_text_buffer_without_mutation_or_history` — a test exists verifying Escape discards the
  buffer. [OBSERVED test name]
- `ObjectStyle { fill, stroke }` — **no typography properties at all.** Font size, family, weight, line
  height, letter spacing, alignment: **all absent.**
- `shell.rs::text_content_section` renders the content read-only. [OBSERVED]

## Missing text capabilities, in dependency order

1. **Typography properties on the object** (font, size, weight, line height, letter spacing, alignment,
   vertical alignment). Without these, text editing has no meaning.
2. **Font loading and font metrics** — required for `nearest_boundary_from_positions` to work with real
   fonts rather than the default.
3. **Text measurement as a service** — line breaking, width/height computation, and the auto-width /
   fixed-size resizing mode. tldraw has a dedicated module for this.
4. **Multi-line and explicit newline handling** — creation geometry is currently minimum 20×20.
5. **Auto-width vs. fixed-size mode** as a stored property.
6. **Rich text** (runs, marks, per-range styling). Explicitly a large addition.
7. **Named text styles**.
8. **Text overflow / clipping / max lines**.

[INFERRED] Items 1–3 are prerequisites for anything else. They are also the items that an AI operation API
would need ("set the font size of this text object", "make this text auto-width") to be useful.

## Candidate architectural implication

**Evidence:**

1. The creation gesture determines the text sizing mode. [Figma DOCUMENTED]
2. Font size and bounds are distinct; conflating them produces fractional sizes and export artifacts.
   [Figma DOCUMENTED]
3. Typography properties are bindable in Figma, so a design-system layer must be able to resolve them.
   [Figma DOCUMENTED]
4. Text style has its own clipboard representation in two of four products. [Affinity + Canva DOCUMENTED]
5. Text extraction for AI is a per-shape-type hook, not a text-subsystem function. [tldraw DOCUMENTED]
6. A content-driven (hug) layout cannot also clip; two container types are required.
   [Figma DOCUMENTED]
7. `getText()` returning `undefined` for shapes without text implies a uniform interface across all object
   types. [tldraw DOCUMENTED]

**Why it matters:** Text is the single most common object in design documents, and it is the object most
likely to be created and edited by an AI agent. A text model that is "a string" cannot support typography,
styles, layout modes, or design-system binding.

**Potential Spool approaches:**

- **A. String + typography properties on the object.** Simple; covers 80% of UI text. No rich text.
- **B. A + a `TextStyle` reference (local or library).** Adds style reuse and clipboard transfer.
- **C. B + variable/token binding for typography properties.** Required for a real design-system story.
- **D. C + rich-text runs.** Large; needed for professional typography; can be deferred.
- **E. A + explicit `TextSizing::{AutoWidth, Fixed, AutoHeight}` per axis.** Required for auto-layout
  compatibility.

**Tradeoffs:** A is what the prototype could grow into cheaply. B/C are where design-system value lives.
D is a major undertaking (a full rich-text engine with caret mapping across run boundaries). E is small
individually but becomes load-bearing once layout exists.

[INFERRED] The natural layering is A + E first (they are prerequisites for layout), then B, then C, then
D. D is the one feature where deferral is clearly correct — no product's *interaction* requires it, and
tldraw shipped for years without a comparable system.

**Decision: TBD — requires architecture review.**

## Open questions

1. Is text a first-class object type, or a shape with a text property? (tldraw: the latter; Figma: the
   former; both work.)
2. Are typography properties stored on the object, in a style, or in a variable?
3. Does Spool need rich text at all for v1? [tldraw suggests not]
4. How does Spool represent RTL / bidi? **No product documents this.**
5. What is the text measurement API, and is it cached?
6. Does the text layout cache invalidate on font load, font size change, or content change?

## Sources

- Figma: "Adjust text dimensions and resizing" (/27378154668951); "Explore text properties"
  (/360039956634); "Guide to text in Figma Design" (/360039956434); "Guide to auto layout"
  (/360040451373); "Overview of variables, collections, and modes" (/14506821864087)
- tldraw: `sdk-features/rich-text.mdx`, `sdk-features/text-measurement.mdx`, `sdk-features/text-shape.mdx`,
  `sdk-features/shapes.mdx` (`getText`), `docs/ai.mdx`, `docs/tools.mdx`
- Affinity: "Snapping" (/design-aids-snapping/); "Keyboard shortcuts for general editing"
  (/workspace-shortcuts-editing/); "About Studios" (/workspace-about-studios/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`TextEditState`, `EntityInputHandler`, UTF-8/16 helpers,
  `nearest_boundary_from_positions`, `text_offset_at_local_point`, `commit_text_edit`, `DesignObject`),
  `app/src/shell.rs` (`text_content_section`)
