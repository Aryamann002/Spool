# AI: Agents

## Vocabulary first

The research brief asks for a distinction between five things. The evidence supports it, with one caveat.

| Term | Definition | Documented example |
|---|---|---|
| **AI autocomplete** | Suggests the next token/element at the cursor. No plan, no tool use, no context beyond the cursor. | Not documented in any of the four products |
| **AI assistant** | Answers questions and suggests actions. Reads context; may or may not modify the document. | Figma's "actions menu" is not AI; Canva's help assistant |
| **AI generator** | Produces content from a prompt. One-shot. Output is the point. | Canva Photo Generator, Magic Design, Figma Make |
| **AI editor** | Modifies an existing document in response to instructions, in place, iteratively, with the user in control. | **Canva AI 2.0** ("Chat … to refine your design"), **Affinity MCP** ("add a gradient map effect to all images in my document"), Figma's agent editing images |
| **AI agent** | Reads context, forms a plan, executes a **sequence of operations**, observes results, and iterates. | **tldraw agent starter kit**, Figma Design Agent |

[INFERRED] **The generator/agent boundary is the observable one**: a generator produces output; an agent
performs operations and observes outcomes. tldraw's documentation makes this explicit by describing the
driver as the alternative to "calling editor methods" — that is, the agent's unit of work is an *action*,
not a *result*.

[INFERRED] **AI autocomplete is notably absent from all four products.** That is a real finding: the design
tooling market has skipped inline completion in favour of conversational and command-based interaction.

## The documented agent architectures

### tldraw — the only fully documented agent

[DOCUMENTED — `docs/ai.mdx`]

> "The agent gathers context from screenshots and structured shape data, applies the model's responses
> through a set of **typed actions**, and streams results onto the canvas as they arrive. **Chat history
> carries across prompts.**"

**API:**
```tsx
agent.prompt('Draw a flowchart showing user authentication')
agent.prompt({ message: 'Add labels to these shapes', bounds: { x: 0, y: 0, w: 500, h: 400 } })
```

**Context:**
- A screenshot of the current viewport
- Simplified representations of shapes within view
- Information about **shape clusters outside the viewport**
- The user's current selection and **recent actions**
- Conversation history from the session

**Actions:**
- Typed action schemas (`CreateAction`, …), each with a defined structure
- `AgentActionUtil<Action>` subclasses implement `applyAction(action, helpers)`
- The system **validates, sanitizes, and applies**
- `Streaming<Action>` with `complete` for incremental output

**Starter kits:**

| Kit | Purpose |
|---|---|
| **Agent** | Full agent system with visual context and canvas manipulation |
| **Chat** | Canvas for sketching and annotation as context for chat |
| **Branching chat** | Visual conversation trees with AI responses |
| **Workflow** | Node-based visual programming that can incorporate AI operations |

[DOCUMENTED]

[INFERRED] The four starter kits are four different *agent interaction models*, which means tldraw's authors
consider the interaction model — not the intelligence — to be the design variable. That is a strong signal
for Spool.

### Figma Design Agent

[DOCUMENTED — Figma Blog, May 2026]

> "You can start in Figma Design with our agent to **generate design layers** to clarify intent across
> **flows, states, copy, and structure**. Then, send [it onward] …"

[INFERRED] "flows, states, copy, and structure" is a **content taxonomy for design intent**, and it maps to
document concepts: flows → prototype connections; states → variants; copy → text content; structure →
hierarchy. The agent's stated job is *document structure*, not imagery.

### Affinity MCP — command-based, capability-gated

[DOCUMENTED — affinity.studio/help/ai-connector-setup/]

- **Claude Desktop is the only supported assistant.**
- Local MCP server; requires Affinity April '26 or later.
- "If you give your AI assistant permission, it can **save any completed workflow as a reusable script**."
- Privacy toggles per capability (files, networks, scripts, Canva AI Studio, local memory, telemetry).
- Connector-level **approval settings per MCP capability**.

[INFERRED] Affinity's agent is **not in the product** — it is an external assistant driving an MCP server.
That has three consequences: Spool-like applications get agent capability without shipping a model
integration, the agent has no privileged UI access, and every operation is auditable as a tool call.

### Canva AI 2.0 — conversational, context-bearing, schedulable

[DOCUMENTED — canva.com/canva-ai]

- "Describe what you want to create with **text or voice**."
- "Canva AI 2.0 can **pull context from your connected tools** to generate on-brand designs — all through a
  conversation."
- "**Chat with Canva AI 2.0 to refine your design, or make edits yourself. You're always in control.**"
- "**Generate elements** — Need an image, a chart, or headline? Just ask Canva AI 2.0, and easily add it to
  your design."
- "**Make it on brand** — Canva AI gets to know your brand's fonts, colours, and rules."
- "**Run web research** — Search for anything and bring the insights straight into your design."
- "**Learns with you over time** — Canva learns how you create to personalize every output to your unique
  style."
- "**Schedule tasks for later** — Schedule Canva AI tasks to run at a later date or on a recurring
  schedule."

[INFERRED] **Six context sources and two execution modes:**
- Context: connected tools, brand system, current design, web research, style history, user intent.
- Execution: interactive (chat) and **asynchronous/recurring**.

[INFERRED] "Learns with you over time" implies a **persistent per-user style profile** derived from their
edits. That is a distinctive and slightly unsettling capability worth noting explicitly.

## The operation vocabulary an agent needs

The research brief lists a candidate vocabulary. Cross-checking against documented capabilities:

| Operation | Documented in | Notes |
|---|---|---|
| `CreateObject` | tldraw (CreateAction), Figma (generate design layers), Canva | |
| `DeleteObject` | Affinity (implied), all | |
| `MoveObject` | tldraw `translateSelection`, `nudgeShapes` | |
| `ResizeObject` | tldraw `resizeSelection` | |
| `RotateObject` | tldraw `rotateSelection` | |
| `SetStyle` | Affinity ("gradient map effect to all images"), Canva Style Match, Figma | |
| `SetText` | Canva Magic Write, Figma ("copy") | |
| `GroupObjects` / `UngroupObjects` | tldraw (`group`/`ungroup` actions) | |
| `ReparentObject` | Affinity (`Move Inside` `⌥⌘G` / `Move Outside` `⌥⇧⌘G`) | Documented as a bound command |
| `AlignObjects` | tldraw (`align-*` actions), Affinity | |
| `DistributeObjects` | tldraw (`distribute-*`), Figma (Distribute + Tidy up) | Figma's two scopes matter |
| `FlattenObjects` | Affinity (`Merge Down` / `Merge Selected` / `Merge Visible`) | **Three scopes** |
| `CreateComponent` / `CreateInstance` | Figma | Not in tldraw/Affinity/Canva |
| `SetLayout` (auto layout) | Figma (`⇧A` add, `Ctrl⇧A` **suggest**) | **`Suggest` is an inference operation** |
| `ImportAsset` / `GenerateImage` | tldraw (asset records), Canva (Photo Generator) | |
| `CreatePrototype` (flow/connection/trigger) | Figma | |
| `Export` | tldraw (`getSvgString`, `toImage`), all | Also the **verification** API |
| `QuerySelection` / `QueryStructure` | Figma (`Select all layers that have the same fill/font`), tldraw (`getText`, `getShapePageBounds`) | |
| `RenameObject` | Affinity (explicitly documented MCP example) | |
| `QueryRecentActions` | tldraw (agent context includes "recent actions") | |
| `CreateTool` (generate a reusable script) | **Affinity** ("Create a tool in Affinity that generates vector patterns and has a UI") | |

[DOCUMENTED]

[INFERRED] Three observations:

1. **Every operation in the brief's candidate list is documented in at least one product**, which is
   evidence that the vocabulary is roughly right.
2. **The list is missing**: `RenameObject`, `QueryStructure` (bulk select by property), `Flatten` (with
   scopes), `SuggestLayout`, `QueryRecentActions`, `CreateTool`, `Export` (as verification).
3. **Every operation should be scoped to a selection or an explicit id set**, never "the document".
   [INFERRED from tldraw's driver (`translateSelection`, `getSelectedShapes`) and the `bounds` parameter in
   `agent.prompt`]

## The proposed model: Agent / Context / Intent / Planning / Operations / Preview / Approval / History

The research brief proposes:

```
Agent
 ├── Context
 ├── Intent
 ├── Planning
 ├── Operations
 ├── Preview
 ├── Approval
 └── History
```

Cross-checked against the evidence:

| Component | Evidence | Notes |
|---|---|---|
| **Context** | Strong. tldraw documents five context sources; Canva documents six. | Must include the **brand/variable system** |
| **Intent** | Strong. Every product is prompt-driven. | |
| **Planning** | Weak. tldraw's workflow starter kit has an execution engine that "resolves dependencies and runs nodes in order"; the agent starter kit does not document a planner. | Only visible when AI is a node in a graph |
| **Operations** | Strong. tldraw's typed action schemas; Affinity's scripting API. | |
| **Preview** | **Weak in documentation but strongly implied.** tldraw's export-as-verification; Canva's custom preview shapes (`iframe srcDoc sandbox="allow-scripts"`). | See below |
| **Approval** | Strong for *capabilities* (Affinity's MCP toggles). **Weak for individual operations** — none of the four products documents a per-operation approval UI. | |
| **History** | Strong. Canva versions, tldraw marks/bail, Affinity single-undo scripts. | |

[INFERRED] **Preview is the most under-documented and most important component.** Two documented mechanisms
imply it:

1. tldraw's custom preview shape:
```tsx
class PreviewShapeUtil extends ShapeUtil<PreviewShape> {
  component(shape) {
    return <HTMLContainer>
      <iframe srcDoc={shape.props.html} sandbox="allow-scripts" style={{...}} />
    </HTMLContainer>
  }
}
```
> "This pattern is useful for **'make real' style applications where users sketch a UI and an AI model
> generates working code to preview alongside the original drawing.**"
> [DOCUMENTED — `docs/ai.mdx`]

2. tldraw's export-for-verification:
```ts
const svg = await editor.getSvgString(editor.getCurrentPageShapes())
const { blob } = await editor.toImage(editor.getCurrentPageShapes(), { format: 'png' })
```
> "Sending both to the model works best: the image shows spatial relationships and styling, and the structured
> data gives exact text and positions."
> [DOCUMENTED]

[INFERRED] **Preview has two distinct meanings in the evidence:**
- **Rendered preview of the agent's output** (as a shape in the document, side by side with the user's
  sketch).
- **Preview for the agent itself** (export the result, look at it, decide whether to keep going).

The second is the self-correction loop and is more valuable; the first is what the "make real" pattern needs.

[INFERRED] **Approval per individual operation is undocumented in all four products.** Affinity gates
*capabilities*; nothing gates a specific mutation. This is either because it is unnecessary (undo covers it)
or because nobody has built it. Given that undo is one step and AI mistakes are common, a lightweight
"here is what I did, accept or revert" surface seems valuable — and Canva's version history with avatars is
the closest documented analogue.

## Chat history and branching

[DOCUMENTED — tldraw starter kits]

- **Chat**: "Canvas for sketching and annotation as context for chat."
- **Branching chat**: "**Visual conversation trees** with AI responses."
- "Chat history carries across prompts." [agent]

[INFERRED] **Branching chat is a genuinely novel interaction model** and directly relevant to Spool's "AI
generates candidates" use case: the user can explore multiple directions and keep them. It also has an
obvious analogue in Figma's **branching/merging** for files, and in Affinity's "save scripts under a new
name, never overwrite".

[INFERRED] Three products independently use the "**branch, don't overwrite**" pattern for AI/experimental
work. That is a convention worth adopting.

## Asynchronous execution

[DOCUMENTED — Canva] "Schedule Canva AI tasks to run at a later date or on a recurring schedule."

[INFERRED] An async agent model changes the AI runtime from a function into a **job system** with:
- a queue,
- credential/token storage,
- a result destination (a new version),
- notifications,
- and cancellation.

[DOCUMENTED gap] None of the four products documents the internal model. **This is a genuine "future"
capability**; it should not influence v1 architecture beyond "AI results are versions".

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `ai_inspector()` — a visual inspector panel.
- `agent_suggestion(icon, label)` — suggestion chips in the UI.
- **No agent runtime, no context assembly, no operation API, no tools, no approval, no preview.**

## Candidate architectural implication

**Evidence:**

1. **Agents perform typed operations, not outputs.** [tldraw DOCUMENTED]
2. **Context is multi-source**: viewport screenshot + structured shape data + out-of-viewport structure +
   selection + recent actions + conversation history (tldraw); plus connected tools, brand system, web
   research, and style history (Canva). [DOCUMENTED]
3. **Operations are selection- or bounds-scoped**, never document-scoped. [tldraw DOCUMENTED]
4. **Verification requires export + a result-reporting mechanism**, and this is what enables self-correction.
   [tldraw DOCUMENTED]
5. **Preview appears in two forms** — a rendered preview shape and an agent-facing render-for-checking.
   [tldraw DOCUMENTED]
6. **Approval is per capability, not per operation**, in the only product that gates anything. [Affinity
   DOCUMENTED]
7. **History is the document's own history** — agents do not get a separate history. [All DOCUMENTED]
8. **Branch, don't overwrite** is the convention for experimental work. [tldraw, Affinity, Figma DOCUMENTED]
9. **Four starter kits = four agent interaction models**, implying the interaction model, not the model,
   is the design variable. [tldraw DOCUMENTED]
10. **Asynchronous/recurring execution** implies a job system. [Canva DOCUMENTED]
11. **Every operation in the brief's candidate vocabulary is documented somewhere**, but the list is missing
    rename, structural query, scoped flatten, layout inference, and tool creation. [Cross-product]

**Why it matters:** An agent runtime is a consumer of everything else — the document model, the operation
API, history, selection, the camera, and export. Building it before those exist means rewriting it.

**Potential Spool approaches for the runtime:**

- **A. No agent runtime.** A model is called by specific features (generate, edit selection, remove
  background). Simple; limited.
- **B. A tool/operation registry** (the same registry the UI and scripts use) exposed to a model. The
  Affinity model. Cleanest.
- **C. B + a driver**, so the agent can choose between operations and input simulation. The tldraw model.
- **D. C + context assembly** (screenshot + structured data + selection + recent actions + history).
- **E. D + preview** (agent-facing export-for-verification, and optionally a user-facing preview mode).
- **F. E + branching** (multiple candidate directions).
- **G. F + approval surface** (show what changed, accept or revert).
- **H. Any + scheduling.**

[INFERRED] **B is the architectural keystone**: exposing the *same* operation registry to the UI, to scripts,
and to the model means agent capability is not a separate subsystem. That is the Affinity lesson and it is
the cheapest path to coherence.

[INFERRED] **D and E are the highest-value AI features** and are prerequisites for anything that claims to be
"AI-native" rather than "AI-assisted". Without the agent seeing and verifying its own output, quality is
bounded by single-shot reliability.

[INFERRED] **F (branching) is a differentiator and is cheap**: an agent that produces a candidate document
state which the user can accept or discard is more usable than one that overwrites.

**Decision: TBD — requires architecture review.** [PROPOSED] Spool's agent should be a *client of the same
operation registry the UI uses*, with a driver for interaction-faithful execution and an export loop for
self-verification.

## Open questions

1. Is the agent a client of the operation registry, or does it have its own tool layer?
2. What is in the context payload, and how is it budgeted?
3. Does Spool have a preview mode, and is preview rendered into the document or shown separately?
4. How is a streaming agent run committed, aborted, and previewed?
5. Is there a per-operation approval, or only per-capability?
6. Is there branching (multiple candidates)?
7. Does the agent see the brand/variable system? (It must, per Canva.)
8. Is there async/scheduled execution?
9. Does Spool publish machine-readable documentation of its operation API (tldraw's `llms.txt` pattern)?

## Sources

- tldraw: `docs/ai.mdx` ⭐⭐⭐ (agent architecture, context, typed actions, sanitisation, preview shapes,
  custom shapes for generated HTML/images, starter kits, verification); `docs/driver.mdx` ⭐⭐ (input
  simulation, `getLastCreatedShapes`, clipboard); `docs/llm-docs.mdx` ⭐ (llms.txt, copy-as-markdown);
  `sdk-features/history.mdx`; `sdk-features/actions.mdx` (action registry)
- Figma: "The Figma Design Agent is Here" (https://www.figma.com/blog/the-figma-agent-is-here/) ⭐;
  Figma AI (https://www.figma.com/ai/); "Guide to branching" (/360063144053)
- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐⭐⭐ (MCP, capability toggles, example
  prompts, script promotion, guidance); "Scripting in Affinity" (/scripting-in-affinity/)
- Canva: "Canva AI 2.0" (https://www.canva.com/canva-ai/) ⭐⭐ (six context sources, chat + manual editing,
  scheduling); "Make any design editable — Magic Layers" (/magic-layers/); "Review and restore older
  versions of designs" (/version-history/)
- Spool prototype: `app/src/shell.rs` (`ai_inspector`, `agent_suggestion`)
