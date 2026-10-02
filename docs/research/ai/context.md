# AI: Context

> What does an agent need to know about a document to act usefully on it?

This is the least-documented and most consequential question in the AI research. It determines what the
document model must expose, and therefore what the AI can do.

## The documented context sources

### tldraw — five sources

[Fully DOCUMENTED — `docs/ai.mdx`]

> "The agent builds context from multiple sources:
> - A screenshot of the current viewport
> - Simplified representations of shapes within view
> - Information about shape clusters outside the viewport
> - The user's current selection and recent actions
> - Conversation history from the session"

And for reading:

```ts
const shapes = editor.getCurrentPageShapes()
const textContent = shapes.map((shape) => ({
  id: shape.id,
  type: shape.type,
  text: editor.getShapeUtil(shape).getText(shape),
  bounds: editor.getShapePageBounds(shape),
}))
```

> "**Sending both to the model works best: the image shows spatial relationships and styling, and the
> structured data gives exact text and positions.**"
> [DOCUMENTED]

[INFERRED] **Two modalities, complementary, and neither is sufficient alone:**
- The **image** carries what structure cannot: visual relationships, styling, alignment, balance.
- The **structure** carries what the image cannot: exact strings, exact coordinates, ids, types.

### Canva — six sources

[Fully DOCUMENTED — canva.com/canva-ai]

| Source | Documented phrasing |
|---|---|
| **User intent** | "Describe what you want to create with **text or voice**" |
| **Connected tools** | "Canva AI 2.0 can **pull context from your connected tools** to generate on-brand designs — all through a conversation" |
| **Brand system** | "**Canva AI gets to know your brand's fonts, colours, and rules** so every design stays on brand" |
| **Current design** | "Chat with Canva AI 2.0 to refine your design, or make edits yourself" |
| **Web research** | "**Run web research** — Search for anything and bring the insights straight into your design" |
| **Style history** | "**Learns with you over time** — Canva learns how you create to personalize every output to your unique style" |

Plus **async/recurring execution** ("Schedule tasks … at a later date or on a recurring schedule").

### Affinity — what it deliberately does *not* share

[DOCUMENTED — affinity.studio/help/ai-connector-setup/] — privacy toggles:

| Shared | Not shared |
|---|---|
| The document (via MCP operations) | Files on your Desktop (opt-in) |
| Saved scripts (opt-in) | Networks (opt-in) |
| Canva AI Studio (opt-in, consumes allowance) | Telemetry / task hints (opt-in) |

[INFERRED] Affinity's model is **least-privilege by default**: the document is the only thing shared without
opt-in. That is a defensible default for a professional tool and a useful precedent.

## What context actually requires from the document model

Cross-referencing the documented context needs against the documented object models:

| Context need | Requires from the document model | Evidence |
|---|---|---|
| **Exact text** | A uniform `getText()` on **every** object type, returning `None` when absent | tldraw DOCUMENTED |
| **Exact positions** | Page-space bounds per object | tldraw DOCUMENTED |
| **Types** | A discriminator per object | All four |
| **Styling** | Resolvable fill/stroke/text style | Figma, Affinity |
| **Visual relationships** | A render/export read API | tldraw DOCUMENTED |
| **Brand grounding** | A variable/token system (Figma) or a palette (Canva) | DOCUMENTED |
| **Out-of-viewport structure** | A spatial index + a summariser | tldraw DOCUMENTED |
| **Recent actions** | An operation log, not just document state | tldraw DOCUMENTED |
| **Conversation history** | Per-session, not per-document | tldraw DOCUMENTED |
| **Bulk selection by property** | Structural search ("select all layers with the same fill/font") | Figma DOCUMENTED |
| **Variable values** | Mode-aware resolution | Figma DOCUMENTED |
| **Instance overrides** | A path-addressable override overlay | Figma DOCUMENTED (inferred) |

[INFERRED] **`getText()` as a per-object-kind hook is the single most important context requirement.** It means
the object model must support a *behavioural interface* (`get_text`, `get_geometry`, `get_style_summary`)
rather than a pure data structure — which reinforces the case for a discriminator-plus-behaviour-object
model (`document/objects.md`, option B/C) over a plain enum.

## Context budgets

[DOCUMENTED — indirect]

- tldraw limits in-view shapes by giving "simplified representations"; out-of-view shapes by giving only
  "information about shape clusters". [DOCUMENTED]
- tldraw caps page size: `maxShapesPerPage` default **4000**. [DOCUMENTED]
- Canva caps Brand Kits at 1,000 and assets at 2,000 per category. [DOCUMENTED]
- Figma caps variables at 5,000 per collection. [DOCUMENTED]

[INFERRED] **Every product implicitly assumes bounded documents and bounded design systems.** The tldraw
"shape clusters outside the viewport" strategy is a documented **spatial summarisation** approach: rather than
omitting off-screen content, summarise it. That is a real technique and it is worth adopting.

[INFERRED] Context budgeting needs at least three mechanisms:
1. **Selection/bounds scoping** — only include what the user pointed at.
2. **Progressive detail** — a coarse summary first, details on request.
3. **Spatial summarisation** — counts and clusters for far-away content.

None of the four documents a token budget, but the mechanisms they use are the ones a budget would drive.

## The context/operation asymmetry

[DOCUMENTED — Affinity]

> "Work with existing documents. Your AI assistant **performs best when modifying, fixing, or automating
> tasks on an existing document. Asking it to design from scratch generally produces poor results.**"

[INFERRED] This is the single most important empirical finding in the AI research. It has a direct
architectural reading:

> **Context is what makes generation work.** The better the agent's model of the existing document, the
> better its edits. Generation from scratch requires the agent to invent context it does not have.

[INFERRED] **Therefore: investing in context assembly is investing in output quality.** A product that
generates well is one that can read well. This is the opposite of the usual assumption that generation is the
hard part.

## What context must *not* include

| Exclusion | Evidence | Reason |
|---|---|---|
| Off-screen detail | tldraw summarises rather than includes | Cost |
| Anything the user did not scope | tldraw's `bounds` parameter; driver selection helpers | Agency |
| Files / network / telemetry without opt-in | Affinity's privacy toggles | Privacy |
| Other users' private work | Not documented | — |

[INFERRED] For a multi-user document, context assembly needs a **visibility filter**: an agent should only
see what the requesting user can see. None of the four products documents this, but all four are
collaborative except Affinity. **Documented gap.**

## Brand/variable context — the highest-leverage content

[DOCUMENTED]

- Canva: "Canva AI gets to know your **brand's fonts, colours, and rules** so every design stays on brand."
- Figma: variables are publishable to team libraries and are the token mechanism.
- Affinity: the MCP capability "**Use Canva AI Studio features**" — i.e. the Canva Brand Kit is reachable
  through Affinity's agent.

[INFERRED] **A design system's value to an AI agent is that it is a vocabulary.** Concretely, the useful
payload is:

```json
{
  "tokens": { "color/action/primary": "#3B82F6", "space/gutter": 16, "type/body": {...} },
  "fonts": ["Inter", "Source Sans 3"],
  "constraints": ["only use fonts from the brand kit", "min contrast 4.5:1"],
  "components": { "Button": { "props": [...], "variants": [...] } }
}
```

[INFERRED] **This payload is only constructible if the design system is a real, addressable part of the
document model** — variables with names and values, a component library with declared properties, and
constraints. A Brand Kit without variables can still supply a palette, which is why Canva works.

[INFERRED] **Consequence for Spool:** the design-system model is an **AI prerequisite**, not an adjacent
feature. See `document/variables.md`.

## Conversation vs. document state

[DOCUMENTED — tldraw agent] "Chat history carries across prompts."

[INFERRED] Conversation history is **session state, not document state**:
- it must not be written to the file,
- it must not be undoable,
- it must not sync between collaborators.

In tldraw's terms this is `session`-scoped state, parallel to `document` and `presence`. That three-way split
(`document` / `session` / `presence`) is the cleanest published model for exactly this question.

## Verifying context is correct

[DOCUMENTED — tldraw driver]

```ts
const shape = driver.getLastCreatedShape()
const lastFive = driver.getLastCreatedShapes(5)
```

> "The driver registers an `editor.sideEffects` handler that records **every shape created while it's
> attached, whatever created it**."

[INFERRED] **The system can tell the agent what actually happened, independently of what the agent intended.**
That closes the loop: context → intent → operation → **observed result** → correction.

[DOCUMENTED — tldraw] The other half of the loop is `getSvgString` / `toImage`.

[INFERRED] Together these two mechanisms constitute the minimum viable AI-native editor:

```
read the document  →  decide  →  act  →  observe what happened  →  re-read and correct
      (structure + image)                          (getLastCreated*)         (export)
```

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `ai_inspector()` — a visual panel. No context assembly.
- `agent_suggestion(icon, label)` — static suggestion chips.
- **No context payload builder, no selection query API, no text extraction, no export, no variables.**

[INFERRED] Spool's prototype happens to have two of the ingredients for structure context by accident:
`Document::objects()` (structured access) and `Document::hit_test(point)` (spatial query). What it lacks is
everything else.

## Candidate architectural implication

**Evidence:**

1. **Two modalities are needed** — a render for visual relationships and structure for exact text/positions.
   [tldraw DOCUMENTED]
2. **`getText()` must exist on every object kind** as a behaviour hook. [tldraw DOCUMENTED]
3. **Spatial summarisation** (clusters) is the documented approach for out-of-viewport content.
   [tldraw DOCUMENTED]
4. **Context quality determines output quality** — editing an existing document beats generating from
   scratch, documented. [Affinity DOCUMENTED]
5. **The brand/variable system is the agent's vocabulary**, and it must be addressable. [Canva + Figma
   DOCUMENTED]
6. **Bulk structural queries** ("all layers with the same fill") are a documented capability and an obvious
   agent tool. [Figma DOCUMENTED]
7. **Conversation history is session state**, not document state. [tldraw DOCUMENTED + INFERRED from the
   document/session/presence split]
8. **Result reporting must be system-side** (`getLastCreatedShapes`), not agent-assumed. [tldraw DOCUMENTED]
9. **Least privilege by default** is the documented privacy stance. [Affinity DOCUMENTED]
10. **Collaborative visibility filtering for agent context is undocumented in all four products.**
    [Documented gap]

**Why it matters:** Context assembly determines what the AI can do. It also determines what the document
model must expose — which means context requirements are effectively **architecture requirements**.

**Potential Spool approaches:**

- **A. No context.** The user must describe everything. Unusable.
- **B. Selection context only** (ids, types, text, bounds). Cheap; covers "fix this text".
- **C. B + viewport screenshot + structure.** tldraw's model. Covers most.
- **D. C + spatial summarisation** for out-of-view content.
- **E. D + brand/variable context.** On-brand generation.
- **F. E + structural queries** exposed as agent tools.
- **G. F + visibility filtering + privacy controls.**

[INFERRED] **C is the minimum useful context assembly and requires three things**: an export read API, a
uniform text-extraction hook, and a structured object serialiser. None of which the prototype has, and all of
which are needed for export anyway.

[INFERRED] **E is the highest-leverage for output quality** and is the strongest argument for building a
variable system as an AI prerequisite rather than a later feature.

[INFERRED] **The context requirements should be treated as an architecture checklist**, not as an AI feature
list. Specifically: does Spool have (a) a uniform text hook, (b) an export read API, (c) a structured
object serialiser, (d) a spatial index, (e) addressable tokens, (f) bulk structural queries? Every one of
these is also required by something other than AI.

**Decision: TBD — requires architecture review.**

## Open questions

1. What is Spool's context payload schema, and is it versioned?
2. How is the context budgeted for a large document?
3. Is there a uniform text-extraction hook on every object kind?
4. Is there a structured object serialiser independent of file persistence?
5. What is the out-of-viewport summarisation strategy?
6. How is brand/variable context injected?
7. Is agent context filtered by user visibility?
8. What privacy controls exist, and what is the default?
9. Is conversation history persisted per session, per document, or not at all?

## Sources

- tldraw: `docs/ai.mdx` ⭐⭐⭐ (context sources, verification, sanitisation); `docs/driver.mdx` ⭐
  (`getLastCreatedShapes`, side-effect recorder); `sdk-features/store.mdx` ⭐ (record scopes: document /
  session / presence); `sdk-features/shapes.mdx` (`getText`, `canCull`, `getGeometry`); `sdk-features/image-export.mdx`
- Canva: "Canva AI 2.0" (https://www.canva.com/canva-ai/) ⭐⭐⭐ (six context sources, brand grounding,
  connected tools, web research, style history, scheduling); "Make any design editable" (/magic-layers/)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐⭐ (privacy toggles, "work with existing
  documents" guidance, script promotion)
- Figma: "Overview of variables, collections, and modes" (/14506821864087) ⭐ (addressable tokens);
  "Select layers and objects" (/360040449873) ⭐ (bulk structural selection by property)
- Spool prototype: `app/src/shell.rs` (`ai_inspector`, `agent_suggestion`), `app/src/canvas.rs`
  (`Document::objects`, `Document::hit_test`, `Document::text_content`)
