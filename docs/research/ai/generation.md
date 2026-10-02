# AI: Generation

## The critical question

> Determine whether AI output is **flat pixels**, **editable structured design**, **HTML/CSS**, or
> **native document objects**.

This is the single most important framing in the AI research, and the answer is: **all four exist, in
different products, and the trajectory is toward structured.**

## The evidence, product by product

### Figma — four surfaces, three output shapes

| Surface | Date | Output shape | Evidence |
|---|---|---|---|
| **First Draft** | 2023 | **Native Figma layers (editable)** | DOCUMENTED (product); THIRD-PARTY confirms editability |
| **Figma Make** | May 2025 | **Code / iframe preview** | DOCUMENTED — "a new prompt-to-app capability to help you quickly explore, iterate, and refine". Community sources state Make output is *not* editable as Figma layers |
| **Figma Sites** | — | Code-backed published site | DOCUMENTED |
| **Figma Buzz / Slides** | — | Mixed | DOCUMENTED |
| **Figma Design Agent** | May 2026 | **Native design layers** | DOCUMENTED — "generate **design layers** to clarify intent across flows, states, copy, and structure" |

[DOCUMENTED — Figma Blog, "Introducing Figma Make" (May 2025); "The Figma Design Agent is Here" (May 2026)]

[INFERRED] **Figma tried native layers (2023), moved to code (2025), and moved back to native layers
(2026).** That is a meaningful arc. The 2026 agent's stated purpose — "to clarify intent across flows,
states, copy, and structure" — is *document structure*, not pixels. And it "can then send" results onward,
implying Make is still a downstream surface.

[THIRD-PARTY] Community discussion consistently ranked "the AI output isn't editable" as the primary
complaint about Make, which is consistent with the pivot.

### Canva — raster first, structured second

| Capability | Output |
|---|---|
| Magic Design | **Template** customised from prompt + your media |
| Photo Generator | Raster image |
| Video Generator | Video |
| 3D Content Generator | 3D content |
| Shape Generator | Shape |
| AI-Powered Elements | Library elements |
| Magic Write | **Text** |
| Magic Background | Background |
| **Magic Layers** | **Editable layout** — flat image → selectable/movable/editable elements |
| Style Match | Restyled existing design |
| Magic Resize | Reflowed design in a new format/language/dimension |
| Canva Code | Code |

[DOCUMENTED — canva.com/canva-ai, canva.com/magic-layers, canva.com/help/use-magic-design]

### Affinity — no generation; operation-based AI

Affinity has **no generation feature of its own**. Its AI story is:

- A **local MCP server** exposing document operations to Claude Desktop. [DOCUMENTED]
- A capability to "**Use Canva AI Studio features**" via MCP, which consumes the Canva plan's AI allowance.
  [DOCUMENTED]
- **Documented guidance:** "**Work with existing documents.** Your AI assistant performs best when modifying,
  fixing, or automating tasks on an existing document. **Asking it to design from scratch generally produces
  poor results.**" [DOCUMENTED — bold emphasis in the source]

[INFERRED] Affinity's position is the most architecturally conservative and the most honest: **the AI edits
a professional document; it does not author one.** That is consistent with the document model's complexity.

### tldraw — three integration patterns

[DOCUMENTED — tldraw docs/ai.mdx]

1. **Canvas as output**: `embed` shape (sandboxed iframe), custom shapes rendering generated images or HTML
   via `HTMLContainer` + `iframe srcDoc sandbox="allow-scripts"`.
2. **Visual workflows**: AI as a node type in a binding-connected graph, with a `NodeDefinition` declaring a
   validator, ports, a default value, and a body height; an execution engine resolves dependencies.
3. **Agents**: typed action schemas, validated and sanitised, applied to the editor.

Plus **Mermaid diagrams**: model-generated diagram *text* (Mermaid grammar) → shapes. [DOCUMENTED]

[INFERRED] **Mermaid ingestion is the cleanest structured-generation pattern in the research.** It constrains
the model to a formal grammar with a deterministic mapping to shapes. It trades expressiveness for
reliability, and it produces native shapes, not pixels.

## The five output shapes, ranked by editability

| Rank | Shape | Who produces it | Editability | History | Export |
|---|---|---|---|---|---|
| 1 | **Native document objects** | Figma Design Agent, tldraw agents, Canva Magic Layers | Full | Yes | Yes |
| 2 | **A constrained grammar → objects** | tldraw Mermaid | Full | Yes | Yes |
| 3 | **Code (HTML/CSS)** | Figma Make, Canva Code | Not in the design tool | Not in the design tool | Rendered |
| 4 | **A template customised** | Canva Magic Design | Full, but template-bound | Yes (as a version) | Yes |
| 5 | **Raster pixels** | Photo Generator etc. | None | As an asset | Yes |

[INFERRED] **Shape 2 deserves more attention than it gets.** If a model can emit a *constrained,
schema-validated intermediate representation* rather than free-form document objects, the failure modes are
far smaller. tldraw's agent pipeline is exactly this:

> "The model emits a **simplified shape format** (`shape._type`) which a util converts into a real shape
> record: `helpers.removeOffsetFromShapePartial(shape)` (translating from the model's coordinate space back
> to the page), `convertPartialFocusedShapeToTldrawShape(...)`, `getDefaultShape(type, complete)`."
> [DOCUMENTED — `docs/ai.mdx`]

And the sanitisation layer:

> "The sanitization layer handles common LLM mistakes. It **corrects shape IDs that don't exist**, **ensures
> new IDs are unique**, and **normalizes coordinates**."
> [DOCUMENTED]

## The three-layer agent → document pipeline

Derived from tldraw's documented architecture. [INFERRED from DOCUMENTED]

```
Model output          (simplified, tolerant: _type, offsets, maybe bad ids)
      ↓
Sanitisation          (fix ids, dedupe ids, normalise coordinates, clamp values)
      ↓
Conversion            (simplified shape partial → real store record)
      ↓
Editor operation      (editor.createShape / updateShape — inside a transaction)
      ↓
Store + History + Render + Sync
```

[INFERRED] Each layer exists for a specific reason:

| Layer | Failure it prevents |
|---|---|
| Simplified format | The model needing to know the full schema (defaults, required fields, migrations, id prefixes) |
| Sanitisation | Malformed ids, duplicate ids, out-of-range coordinates, NaN, unknown types |
| Conversion | Shape defaults, property coercion, `complete`/streaming partial shapes |
| Editor operation | Bypassing history, side effects, bindings, culling, and reactivity |

[INFERRED] **A design that lets an LLM emit raw records directly into the document will break.** Every one of
these layers is load-bearing.

## Structured generation: what is actually achievable

| Task | Feasibility | Evidence |
|---|---|---|
| Diagrams (flowchart, sequence, ER) | **High** | tldraw Mermaid module; Figma agent "create diagrams" |
| Wireframe / rough layout from a prompt | High | Figma First Draft; Canva Magic Design |
| Icon generation | High | Canva Shape Generator |
| Text content | High | Canva Magic Write |
| Marketing asset composition | High | Canva Magic Design + Style Match |
| Brand-consistent design | High | "Canva AI gets to know your brand's fonts, colours, and rules" |
| Illustration / hero image | Medium | Raster generators |
| **Refining an existing design** | **High** | Affinity: "performs best when modifying, fixing, or automating tasks on an existing document" |
| **Full app / interactive prototype** | Medium | Figma Make; Canva Code |
| Generating a *correct* auto-layout structure | **Low** | Widely reported Figma auto-layout difficulty [THIRD-PARTY] |
| Generating production-quality Bézier vector paths | **Low** | No product claims this; Magic Layers produces approximate layers |
| Preserving exact layout across a reflow | **Medium** | Canva Magic Resize exists, implying it works but imperfectly |

[INFERRED] The feasible/unfeasible boundary correlates almost perfectly with **structuredness**:
- Formally-constrained output (Mermaid, templates, copy, colours) → high reliability.
- Free-form continuous geometry (paths, precise layouts) → low reliability.

[INFERRED] **This is a strong argument for Spool to make structured generation the primary path and to be
explicit that it does not attempt pixel-accurate or path-accurate generation.**

## Generation and document history

[DOCUMENTED — Canva Version history]

> "You can restore up to 1,000 previous versions, with no time limit on how long versions are kept."
> "You can also see **avatars next to each saved version, indicating who edited it**."

[INFERRED] **A generated design is a version, not a separate entity.** This is the right model: an AI
generation should be one undoable (or one versioned) document mutation, indistinguishable in kind from a
user's edit. Any "AI history" that is separate from document history will diverge.

[INFERRED] **Recording provenance per object** (not per version) is complementary and is a real
requirement: Magic Layers "instantly transforms" a flat design into layers, and those layers are inferred.
A future model or a user needs to know which objects were inferred. tldraw's `meta: JsonObject` on every
record is the natural place for such a flag.

## Scheduled and asynchronous generation

[DOCUMENTED — Canva AI 2.0]

> "**Schedule tasks for later** — Need a design for later? Schedule Canva AI tasks to run at a later date or
> on a recurring schedule."

[INFERRED] This is a genuinely different model from interactive generation: the agent runs when the user is
not present, and its output lands as a version. It requires:
- a task queue,
- credential/token storage,
- a place to deposit the result,
- and a notification.

This is a "future" capability for Spool, but it is worth noting because it changes the AI runtime from a
synchronous function into a **job system**.

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `ai_inspector()` — a **visual panel** in the inspector. No backing model.
- `agent_suggestion(icon, label)` — **UI chrome** suggesting agent actions.
- **No AI runtime, no model integration, no operation API, no document context, no agent history.**

[INFERRED] There is nothing to preserve or migrate. Spool's AI work starts from zero, which means the
operation API (see `architecture/ai-runtime.md`) is the first real decision.

## Candidate architectural implication

**Evidence:**

1. The industry has tried **native layers**, **code**, and **raster**, and the newest Figma and Canva
   products converge on **native structured objects**. [DOCUMENTED]
2. The single highest-value differentiator is **converting flat output into editable objects** (Canva
   Magic Layers; Figma Design Agent). [DOCUMENTED]
3. **A simplified model-facing format + sanitisation + conversion** is required between the model and the
   document. [tldraw DOCUMENTED]
4. **Constrained grammars** (Mermaid, templates, copy) produce reliable native objects; free-form continuous
   geometry does not. [DOCUMENTED + INFERRED]
5. Generation is **most reliable when modifying an existing document**, least reliable from scratch.
   [Affinity DOCUMENTED, bold in source]
6. The **brand/variable system is the grounding context** for on-brand generation. [Canva + Affinity
   DOCUMENTED]
7. Generated output is a **version/undo entry**, not a separate entity; per-object provenance is the
   complementary requirement. [Canva DOCUMENTED + INFERRED]
8. Async/scheduled generation implies a **job system**, not a synchronous function. [Canva DOCUMENTED]
9. Inferred/recovered objects need an explicit "this was inferred" marker, since Magic Layers output is
   approximate. [INFERRED from Canva DOCUMENTED that it is a conversion of an image]

**Why it matters:** Generation is the most visible Spool differentiator and the easiest place to build
something that produces impressive demos and unusable documents. The evidence is unambiguous about what
works.

**Potential Spool approaches:**

- **A. Raster generation only** (image assets placed into a document). Cheapest; no structural advantage;
  the "flat pixels" failure mode the brief warns about.
- **B. Constrained generation** — the model emits a validated schema (a layout DSL, a diagram grammar, a
  template selection) that the editor converts to objects. Highest reliability; limited expressiveness.
- **C. Native-object generation with the tldraw three-layer pipeline** (simplified format → sanitise →
  convert → operation). Most expressive; needs a strong schema and sanitiser.
- **D. C + a "decompose" operation** (raster → inferred layers), mirroring Magic Layers.
- **E. B + C** — constrained for reliable categories, native for the rest.

[INFERRED] **E is the recommended direction** and is well-supported by the evidence: use the strongest
constraint available per task category, and only fall back to free-form native generation where the output
is simple enough to sanitise reliably.

[INFERRED] **D is the most differentiating and the most underestimated.** Magic Layers proves that
flat → structured conversion is (a) valuable enough to ship as a named product, and (b) the natural answer
to "the model gave me an image". Spool could implement the same conversion with its own document model, which
would make any external AI output usable inside Spool.

**Decision: TBD — requires architecture review.** [PROPOSED] Spool's AI generation should target native
structured output via a validated intermediate representation, with decomposition as a first-class fallback
for raster input.

## Open questions

1. What schema does the model emit — a simplified shape format, or a layout DSL?
2. What is the sanitisation layer's responsibility, and what does it reject?
3. Do generated objects carry a provenance flag?
4. Is a generation one undo step, or many?
5. Can the user inspect and correct the model's intermediate representation before applying?
6. Is "decompose a raster into layers" a Spool feature?
7. How does the agent know the brand/variable system? (See `ai/context.md`.)
8. Does Spool need asynchronous/scheduled generation?

## Sources

- Figma: "Introducing Figma Make: A New Way to Test, Edit, and Prompt Designs" (May 2025)
  (https://www.figma.com/blog/introducing-figma-make/); "The Figma Design Agent is Here" (May 2026)
  (https://www.figma.com/blog/the-figma-agent-is-here/) ⭐; "Config 2025: Pushing Design Further"
  (https://www.figma.com/blog/config-2025-recap/); Figma AI (https://www.figma.com/ai/) ⭐;
  "Review and restore older versions of designs" — Canva counterpart for version semantics
- Canva: "Canva AI 2.0" (https://www.canva.com/canva-ai/) ⭐;
  "Make any design editable — Magic Layers" (https://www.canva.com/magic-layers/) ⭐⭐;
  "Use Magic Design to generate design templates" (/use-magic-design/); "Use Magic Edit"
  (/using-magic-edit/); "Magic Design" (https://www.canva.com/magic-design/)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐⭐ (the "work with existing documents"
  guidance is quoted verbatim in source)
- tldraw: `docs/ai.mdx` ⭐⭐ (three patterns, agent architecture, sanitisation layer, custom preview shapes,
  starter kits); `docs/mermaid.mdx` ⭐; `docs/driver.mdx`; `sdk-features/actions.mdx`
- Spool prototype: `app/src/shell.rs` (`ai_inspector`, `agent_suggestion`)
