# AI: Structured Generation

This document answers the narrowest and most consequential question in the AI research:

> What makes AI output *editable structured design* rather than *flat pixels*?

## The five output shapes, precisely

| Shape | Definition | Produced by | Editable in the design tool? |
|---|---|---|---|
| **A. Pixels** | A raster image of the design | Photo Generator, Magic Background, 3D generator, most image models | **No** |
| **B. Template instance** | A design template customised with user content | Canva Magic Design, Magic Studio | Partially — you edit the copy, not the master |
| **C. Code** | HTML/CSS or an app | Figma Make, Canva Code | **No** (rendered in an iframe/frame) |
| **D. Grammar-derived objects** | A model emits a constrained grammar; a deterministic parser produces objects | tldraw Mermaid | **Yes** |
| **E. Native document objects** | A model emits objects the editor consumes directly | Figma First Draft, Figma Design Agent, tldraw agents, Canva Magic Layers | **Yes** |

[INFERRED] The evidence shows the industry is moving B/C/A → **D/E**. Figma's arc (First Draft layers →
Make code → Design Agent layers) and Canva's Magic Layers ("Turn AI designs into editable layouts") both
point the same way.

## Why shape E is hard, and what makes it work

### The problem

A model emitting document objects directly will produce:

| Failure | Example |
|---|---|
| **Non-existent ids** | Referring to `shape:abc` when it does not exist |
| **Duplicate ids** | Generating the same id twice |
| **Unnormalised coordinates** | Emitting coordinates in its own local frame, or out of range |
| **Missing defaults** | Omitting required fields (fill, opacity, index) |
| **Invalid enums** | `type: "rectangle"` when only `geo` is valid |
| **Unknown types** | Inventing a shape type |
| **NaN / infinities** | Arithmetic on absent values |
| **Partial output** | Truncated JSON from a token limit |
| **Hierarchy violations** | A child whose parent doesn't exist |

[DOCUMENTED as observed problems — tldraw `docs/ai.mdx`]

### tldraw's solution — the three-layer pipeline

[Fully DOCUMENTED — `docs/ai.mdx`]

```tsx
class CreateActionUtil extends AgentActionUtil<CreateAction> {
  override applyAction(action: Streaming<CreateAction>, helpers: AgentHelpers) {
    const { shape } = action
    if (!shape || !shape._type) return                       // ← reject malformed

    // Translate from the model's coordinate space back to the page
    const shapePartial = helpers.removeOffsetFromShapePartial(shape)
    const result = convertPartialFocusedShapeToTldrawShape(this.editor, shapePartial, {
      defaultShape: getDefaultShape(shape._type, action.complete),
      complete: action.complete,                            // ← handle streaming
    })
    if (!result.shape) return

    this.editor.createShape(result.shape)                    // ← real editor operation
  }
}
```

And the sanitisation layer:

> "The sanitization layer handles common LLM mistakes. It **corrects shape IDs that don't exist**, **ensures
> new IDs are unique**, and **normalizes coordinates**."
> [DOCUMENTED]

[INFERRED] Four distinct mechanisms, each targeting a specific failure:

| Mechanism | Handles |
|---|---|
| `Streaming<T>` with `complete` flag | Partial/truncated output — defaults can differ until complete |
| `helpers.removeOffsetFromShapePartial` | **Coordinate-space mismatch** (the model reasons in its own frame) |
| `convertPartialFocusedShapeToTldrawShape` + `getDefaultShape` | Missing defaults, invalid enums, unknown types |
| Sanitisation (id repair, dedup, normalisation) | Referential integrity |

[INFERRED] **`removeOffsetFromShapePartial` is the most important and least obvious mechanism.** A model asked
to draw inside a region will emit coordinates relative to that region, because that is what the prompt
implies. Rebasing those coordinates is not a sanitisation detail — it is a semantic requirement of the
whole approach.

### Streaming

[INFERRED] `Streaming<T>` implies actions arrive incrementally and the editor applies them as they land:
"**streams results onto the canvas as they arrive**". [DOCUMENTED — tldraw agent architecture]

This has two consequences:
1. **The user sees partial results** (good for perceived latency).
2. **A stream can be interrupted**, so the history transaction must span the whole stream and be abortable.

[INFERRED] Combining streaming with one-undo-step history means: open a transaction at stream start, write
into it as results arrive, commit (or bail) at stream end.

## Shape D — grammar-constrained generation

tldraw's Mermaid module turns diagram *text* into shapes. [DOCUMENTED — `docs/mermaid.mdx`]

[INFERRED] Properties of grammar-constrained generation:

| Property | Value |
|---|---|
| Output validity | Guaranteed (the parser rejects or reports) |
| Layout | **Deterministic** (the grammar defines positions) |
| Expressiveness | Low — only what the grammar covers |
| Reliability | **Very high** |
| Cost | Low (short output) |

[INFERRED] The trade is clear and favourable for the right categories. A flowchart, a sequence diagram, an
ER diagram, a sitemap, a journey map, a card layout — all are grammars. And grammars are exactly what an AI is
good at producing.

[INFERRED] **A stronger version of shape D** is a *layout DSL*: a constrained description of a screen
(columns, stacks, spacings, roles) that a deterministic layout engine resolves. That is the natural bridge
between "AI generates structure" and "AI generates coordinates", and it would sidestep the coordinate
problem that plague free-form generation.

## Shape B — template-constrained

Canva Magic Design generates a **template customised from a prompt and the user's own media**. [DOCUMENTED]

[INFERRED] Templates guarantee layout quality because the layout is authored by a human. The AI's job is
reduced to: pick a template, fill in content, adapt to format. This is why Canva's output reliably looks
professional and Figma's First Draft output does not always.

[INFERRED] This suggests a Spool strategy: **template-grounded generation** for anything that should look
polished, free-form generation for anything that should be novel.

## Shape A→E — decomposition (Magic Layers)

[DOCUMENTED — canva.com/magic-layers]

> "Upload your flat design, and Magic Layers instantly transforms it into a layout you can select, move, and
> edit. […] Fine-tune the details — Move and tweak them however you like. Change colors. Edit text directly.
> […] **No more do-overs — Don't regenerate your design to change one element — let Magic Layers do it for
> you.**"

Best results with JPEG and PNG. Works with any AI image generator including ChatGPT, Gemini, and Canva AI.
Beta; counts against the monthly AI allowance.

[INFERRED] **Decomposition is image segmentation + text recognition → document objects.** The pipeline is
roughly:

1. Segment the image into regions (backgrounds, shapes, text blocks, photos).
2. Classify each region (text / image / solid fill / graphic).
3. Recognise text and infer its properties (colour, approximate size, alignment).
4. Emit objects with geometry approximating the regions.
5. Mark them as **inferred** so downstream edits know they are approximate.

[INFERRED] Step 5 is required and is not publicly documented by Canva. It matters because:
- the geometry is approximate and will not snap exactly to anything;
- the text styling is guessed;
- the user needs to know which parts are safe to edit.

[INFERRED] Decomposition is a **retrieval** path into the structured document, and it is the most
differentiating AI feature available to Spool, because it makes *any* external AI output usable.

## Verification: how does the agent know it worked?

[DOCUMENTED — tldraw `docs/ai.mdx`]

> "**Sending both to the model works best: the image shows spatial relationships and styling, and the
> structured data gives exact text and positions.**"

Context sources:
- A screenshot of the current viewport
- Simplified representations of shapes within view
- **Information about shape clusters outside the viewport**
- The user's current selection and recent actions
- Conversation history

And for verification of the agent's own action:
```ts
const shape = driver.getLastCreatedShape()
const lastFive = driver.getLastCreatedShapes(5)
```
> "The driver registers an `editor.sideEffects` handler that records **every shape created while it's
> attached, whatever created it**."
> [DOCUMENTED — `docs/driver.mdx`]

[INFERRED] **This is the answer to "how does an agent learn what happened".** The *system* reports the result
of an action, rather than the agent assuming its intent succeeded. Combined with the export read API
(`getSvgString`, `toImage`), it gives a full **generate → render → inspect → correct** loop.

[INFERRED] **This loop is the actual architecture of AI-native design**, and it requires three things Spool
does not have: a scriptable export, an input driver, and structured object inspection.

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `ai_inspector()`, `agent_suggestion()` — visual only.
- **No generation, no schema, no sanitisation, no driver, no export API.**

## Candidate architectural implication

**Evidence:**

1. **Three layers between the model and the document** — simplified format, sanitisation, conversion — are
   required, and each targets a specific, documented failure class. [tldraw DOCUMENTED]
2. **Coordinate rebasing is a semantic requirement**, not a sanitisation detail. [tldraw DOCUMENTED]
3. **Streaming requires a transaction spanning the whole stream**, so partial output is abortable.
   [tldraw DOCUMENTED + INFERRED]
4. **Constrained grammars produce the most reliable native objects**; free-form coordinates are the least
   reliable. [tldraw Mermaid DOCUMENTED + INFERRED]
5. **Templates guarantee layout quality** because the layout is human-authored. [Canva DOCUMENTED]
6. **Decomposition (flat → structured) is a shipped, differentiated product**, and its output must be
   marked as inferred. [Canva Magic Layers DOCUMENTED + INFERRED]
7. **Verification requires an export read API plus a result-reporting mechanism** (`getLastCreatedShapes`).
   [tldraw DOCUMENTED]
8. Figma tried native layers, moved to code, and returned to native layers. [DOCUMENTED]

**Why it matters:** This is the difference between an AI feature that produces impressive screenshots and
one that produces usable documents. The evidence is unambiguous about which mechanisms are required.

**Potential Spool approaches for the model→document path:**

- **A. Direct emission.** The model writes document objects; the editor validates on load. Simple; fails on
  every documented failure class.
- **B. A model-facing simplified schema + sanitisation + conversion** (tldraw's three layers).
- **C. B + streaming with a transaction.**
- **D. C + a grammar/DSL mode** for structured categories (diagrams, layouts).
- **E. D + template-grounded generation** for polished output.
- **F. E + decomposition** (raster → inferred objects, provenance-flagged).
- **G. Any + a verification loop**: export the result, send it back with structured data, let the model
  correct.

[INFERRED] **B is the minimum viable correct design and is cheap.** C, D, and G are additive and each has
clear evidence. E and F are the differentiators.

[INFERRED] **G deserves particular attention because it is the least obvious and the most enabling.** An
agent that can *see its own output* and *query what actually happened* can self-correct. That converts AI
from a one-shot generator into a loop, and it is the mechanism that makes more capable models produce better
results without a more complex harness — which is precisely the tldraw maintainer's stated principle:
"**if your harness is getting more complex as models improve, you are overengineering it.**"
[THIRD-PARTY — Max Drake, tldraw]

[INFERRED] **B + C + D + G is the recommended core.** E and F are strategic additions.

**Decision: TBD — requires architecture review.**

## Open questions

1. What is Spool's model-facing schema, and how is it versioned as the document model evolves?
2. What exactly does the sanitisation layer repair, and what does it reject?
3. How is coordinate rebasing expressed?
4. Is generation streamed, and is a stream abortable as one history entry?
5. Does Spool support a layout DSL for constrained generation?
6. Are templates a first-class concept (needed for template-grounded generation)?
7. Will Spool implement decomposition (raster → inferred objects)?
8. Is there an export read API and a `getLastCreatedObjects` mechanism for verification?

## Sources

- tldraw: `docs/ai.mdx` ⭐⭐⭐ (three-layer pipeline, sanitisation, streaming, verification, context sources,
  starter kits); `docs/driver.mdx` ⭐ (dispatch, `getLastCreatedShape(s)`, clipboard, tick); `docs/mermaid.mdx`
  ⭐ (grammar-derived shapes); `sdk-features/geometry.mdx`, `sdk-features/shapes.mdx`
- Canva: "Make any design editable — Magic Layers" (https://www.canva.com/magic-layers/) ⭐⭐;
  "Canva AI 2.0" (https://www.canva.com/canva-ai/) ⭐; "Use Magic Design to generate design templates"
  (/use-magic-design/)
- Figma: "The Figma Design Agent is Here" (https://www.figma.com/blog/the-figma-agent-is-here/) ⭐;
  "Introducing Figma Make" (https://www.figma.com/blog/introducing-figma-make/); "First Draft" (2023
  announcement, referenced in secondary sources — THIRD-PARTY)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐ (guidance: work with existing documents)
- BigGo podcast, "Bringing Agents to the Canvas — Max Drake, tldraw" (Sep 2026) — THIRD-PARTY, used only for
  the harness-simplicity principle.
- Spool prototype: `app/src/shell.rs` (`ai_inspector`, `agent_suggestion`)
