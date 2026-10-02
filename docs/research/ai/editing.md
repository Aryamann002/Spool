# AI: Editing

Editing is where the evidence is strongest, because all four products support it and at least two document
it in detail.

## What each product's AI can edit

| Operation | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| **Rename layers** | Yes (bulk; `Edit ▸ Select All ▸ other layers that have the same`) | Via agent actions | **Yes — "Rename the layers in my Affinity document"** (documented MCP example) | Not documented |
| **Restyle objects** | Yes | Yes | **Yes — "Add a purple to orange gradient map effect to all of the images in my document"** (documented MCP example) | **Yes — Style Match** |
| **Change text** | Yes ("clarify intent across flows, states, **copy**") | Yes | Not documented | **Yes — Magic Write** |
| **Change layout** | Yes | Yes | Not documented | **Yes — Magic Resize** |
| **Remove/change backgrounds** | Yes (Magic Background) | Yes (custom) | **Yes — Background Removal tool** | **Yes** |
| **Local image edits** | Yes ("edit images") | Yes (custom shapes) | Yes | **Yes — Magic Edit: "Brush over part of a photo and describe what to replace it with"** |
| **Delete/remove objects** | Yes | Yes | Yes | Yes |
| **Add interactions / prototype links** | **Yes — "add interactions" is part of the agent's stated purpose** | **Yes — bindings are agent-writable** | Not documented | Not documented |
| **Batch operations across many objects** | Yes | Yes | **Yes — explicitly the MCP use case** | Not documented |
| **Create reusable tooling** | Yes (plugins) | **Yes — starter kits, custom shapes** | **Yes — "Create a tool in Affinity that generates vector patterns and has a UI", saved to the Scripts panel** | Yes (Apps) |
| **Design from scratch** | Yes (First Draft, Make, Design Agent) | Yes (agent) | **Explicitly discouraged: "Asking it to design from scratch generally produces poor results"** | Yes (Magic Design) |

[DOCUMENTED where cited]

## The three edit interaction models

[INFERRED from the documented evidence]

### 1. Selection-scoped (Figma, tldraw)

The agent operates on the current selection, or on a region the user specified. tldraw's documented API:

```tsx
agent.prompt({
  message: 'Add labels to these shapes',
  bounds: { x: 0, y: 0, w: 500, h: 400 },
})
```
[DOCUMENTED — tldraw `docs/ai.mdx`]

[INFERRED] Bounds-scoped requests are essential: they let the user say "here" without enumerating ids.

### 2. Conversation-scoped (Canva AI 2.0)

> "**Chat with Canva AI 2.0 to refine your design, or make edits yourself. You're always in control.**"
> [DOCUMENTED]

[INFERRED] The sentence is a *product statement about coexistence*: manual editing and conversational editing
are equally first-class, and both operate on the same document. This means the document must be editable by
the user at any point in an AI conversation, and the AI's changes must not lock the document.

### 3. Command-scoped (Affinity MCP)

> "Describe a task in your own words and your AI assistant will use MCP to carry it out — **no manual steps,
> scripting, or deep knowledge of Affinity's interface required.**"
>
> Example prompts (all DOCUMENTED):
> - "Add a purple to orange gradient map effect to all of the images in my document."
> - "Rename the layers in my Affinity document."
> - "Create a tool in Affinity that generates vector patterns and has a UI."
> [DOCUMENTED — affinity.studio/help/ai-connector-setup/]

[INFERRED] Affinity's MCP is a **document command API**, not a UI automation layer. The distinction is
visible in the privacy toggles: capabilities are named as document operations ("Access files on your
Desktop", "Use saved scripts", "Save scripts to your scripting panel"), not as UI interactions.

## Documented guidance on what works

[DOCUMENTED — Affinity, quoted in source]

> "**Work with existing documents.** Your AI assistant performs best when modifying, fixing, or automating
> tasks on an existing document. Asking it to design from scratch generally produces poor results."

> "**Retry failed tasks.** The AI assistant may not complete every task on the first attempt — this feature
> is still in active development. If it reports that something cannot be done, ask it to try again, as it
> often succeeds on a second attempt."

> "**Enable MCP local memory.** Turn on this setting in Affinity's MCP settings to allow it to store task
> hints in local memory for similar tasks in the future."

> "**Save your favourite scripts** — Once you've used your AI assistant to complete a workflow, you can save
> it as a script so you don't need to re-prompt each time."

[DOCUMENTED]

[INFERRED] Four capabilities are named here that are each a distinct architectural feature:

| Capability | Requirement |
|---|---|
| Retry on failure | The agent must be able to re-run without duplicating partial work |
| Local task memory | A persisted hint store, with a **share/opt-out** privacy control |
| Workflow → script promotion | Successful runs become durable, re-runnable artifacts |
| Document-scoped operation | The agent reads and writes the real document |

The retry requirement is subtle and important: if an agent partially applies changes and then fails, a naive
retry duplicates them. That is exactly what tldraw's **bail** primitive solves.

## Editing and history

[INFERRED] **Every AI edit must be one undoable unit.**

Evidence:
- tldraw's agent applies actions through `editor.createShape`/`updateShape` **inside the store**, so they
  are captured by the history interceptor exactly like user edits. [DOCUMENTED]
- Affinity's scripting "apply changes—**often as a single undoable action**". [DOCUMENTED]
- Canva's generated designs are **versions**. [DOCUMENTED]

[INFERRED] The failure mode to avoid is: agent applies 200 store writes → 200 undo entries → the user cannot
undo the agent's work coherently. tldraw solves this with `editor.run(..., { history: 'record' })` which
flushes everything into one entry. Affinity solves it with an explicit "single undoable action" guarantee.

[INFERRED] **A related requirement: an agent must be able to abort.** tldraw's `bailToMark` reverts and
*discards* (does not push to redo). An agent that fails midway should bail, not commit a half-result.

## Workflow → script promotion

[DOCUMENTED — Affinity]

> "If you give your AI assistant permission, it can save any completed workflow as a reusable script. This
> means you can repeat the process at any time, without needing to prompt again."
>
> "If you gave the AI assistant permission to save completed workflows to the Scripts panel, they can be
> run again at any time by going to Window ▸ General ▸ Scripts. **Click a script to run it on the current
> document immediately.**"
>
> "**Note:** Your AI assistant can save updated versions of a script under a new name, but **will not
> overwrite existing scripts directly.**"

[INFERRED] This is a remarkable design, and it has four architectural implications:

1. **An agent run is recorded as an executable artifact**, not just a result.
2. The artifact is **runnable later on a different document** — so it must be *parameterised* by its
   selection/context, or it re-derives context at run time.
3. **Versioning is by new-name, never overwrite.** Safe by construction.
4. It converts a probabilistic, expensive interaction into a cheap, deterministic one.

[INFERRED] For Spool this is arguably the single most valuable AI pattern in the research, because it makes
AI *cumulative*: the user's tenth workflow is faster than their first, and it does not require the model at
all after the first time.

## Editing and approval

[DOCUMENTED — Affinity MCP privacy settings]

| Setting | Effect |
|---|---|
| Access files on your Desktop | Open, edit, and save files on Desktop when required |
| Access networks | Use internet and local network connections when required |
| Use saved scripts | Read and use scripts in the scripting panel |
| **Save scripts to your scripting panel** | Save scripts from completed actions |
| **Use Canva AI Studio features** | Use Canva AI Studio; consumes the Canva plan's monthly AI allowance |
| Save task hints to local memory | Store task hints locally for similar future tasks |
| Share task hints with Affinity | Send anonymised task hints to improve Affinity's knowledge base |

Plus, in the connector configuration: "choose **approval settings for MCP capabilities**".

[DOCUMENTED]

[INFERRED] Approval is **per capability, not per session**. That is the right granularity: renaming layers and
deleting a file are different risk levels.

[INFERRED] Note what is *not* gated: the core document operations (layer rename, effects on images) are not
separately toggled. So the design is: **capabilities = outward-facing/side-effecting actions** (files,
network, scripts, external AI, memory, telemetry), while **in-document operations are ungated**. That is a
simple and defensible line.

## Editing via input simulation vs. commands

Two approaches, both documented:

| Approach | Product | Mechanism |
|---|---|---|
| **Command API** | Affinity (MCP + scripting API with `Document`, `Dialog`, `Collection`) | The assistant calls document operations |
| **Input simulation (driver)** | tldraw (`@tldraw/driver`) | The assistant dispatches pointer/keyboard/wheel/pinch events through `editor.dispatch`, running the real state machines |

[DOCUMENTED]

[INFERRED] **These are complementary, and the tldraw docs describe the key difference:**

> "Pointer, keyboard, wheel, and pinch events all flow through `editor.dispatch`, so they go through the
> editor's **normal tool state machines**. A pointerDown, pointerMove, pointerUp sequence with the draw tool
> active creates **real draw shapes**."
> [DOCUMENTED — tldraw `docs/driver.mdx`]

And:

> "To let an agent simulate user input instead of calling editor methods, see [Driving the editor]."
> [DOCUMENTED — tldraw `docs/ai.mdx`, explicitly framed as an *alternative* to calling editor methods]

| | Command API | Input simulation |
|---|---|---|
| Speed | Fast | Slow (many events) |
| Reliability | Depends on the API's coverage | High — the real code paths run |
| History | One transaction if wrapped correctly | Natural (marks) |
| Snapping / constraints / modes | **Not applied** | **Applied automatically** |
| Works for undocumented features | No | Yes |
| Can create shapes the UI can't | Yes | No |
| Testability of the *editor itself* | N/A | **Excellent — this is how tldraw tests itself** |

[INFERRED] **Command API for bulk/structural work; driver for anything that must respect interaction rules**
(snapping, alignment, snapping to guides, mode-specific behaviour). An agent that says "arrange these in a
grid, snapped" should get snapping; an agent that says "create 50 rectangles" should not pay the cost of
synthesising 150 pointer events.

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `ai_inspector()` — visual panel only.
- `agent_suggestion(icon, label)` — visual only.
- **No operation API, no agent loop, no context assembly, no approval model, no script promotion.**

## Candidate architectural implication

**Evidence:**

1. **Editing an existing document is far more reliable than generating from scratch** — Affinity's own
   guidance says so explicitly. [DOCUMENTED]
2. **Every AI edit must be one undoable unit**, and a failed run must be able to abort without leaving a
   partial result. [tldraw + Affinity DOCUMENTED]
3. **Command API and input simulation are complementary**: commands for bulk/structural work, the driver
   for anything that must respect snapping/constraints/interaction rules. [tldraw DOCUMENTED]
4. **Workflow → script promotion makes AI cumulative** and converts probabilistic runs into deterministic
   ones; scripts are versioned by new-name, never overwritten. [Affinity DOCUMENTED]
5. **Retry is a documented requirement**, and it constrains partial-application semantics. [Affinity
   DOCUMENTED]
6. **Approval is per capability, and capabilities are side-effecting actions** (files, network, scripts,
   external services, memory, telemetry) — not in-document operations. [Affinity DOCUMENTED]
7. **Local task memory is a documented feature with an opt-out**. [Affinity DOCUMENTED]
8. **Conversational and manual editing must coexist on the same document.** [Canva DOCUMENTED]

**Why it matters:** Editing is the highest-reliability AI surface and therefore the right place to build
Spool's AI credibility. But it requires the operation API, the history semantics, and the driver — all of
which are also prerequisites for profiles and testing.

**Potential Spool approaches:**

- **A. No editing.** Nothing.
- **B. A read-only "describe the selection" agent** that explains but does not modify. Safe; low value.
- **C. Editing via a command API** with one-undo-step batching and abort. The Affinity model.
- **D. C + a driver** so interaction-sensitive operations run through real interaction code. The tldraw
  model.
- **E. D + workflow → script promotion.** Cumulative AI.
- **F. E + per-capability approval.**

[INFERRED] **C is the prerequisite and D is the differentiator.** C makes AI edits coherent with history.
D makes AI edits *feel* like the product: an agent that arranges shapes uses the same snapping the user
does.

[INFERRED] **E is the highest-leverage item per unit of effort.** It is a small feature (record the
operation sequence, replay it later) with an outsized payoff, and it is explicitly validated by a shipping
product.

[INFERRED] **The driver (D) is also a general engineering asset**, independent of AI: it enables
**deterministic interaction tests**, which a design editor badly needs and which no product in this research
documents as a user-visible feature (tldraw ships it as an SDK package for exactly this reason).

**Decision: TBD — requires architecture review.** [PROPOSED] AI editing should be built on the same
operation API that the UI uses, with the driver available for interaction-faithful execution.

## Open questions

1. Is there one operation API used by the UI, the agent, and scripts?
2. How are AI edits grouped into one history entry, and how is a partial run aborted?
3. What is the approval model — per capability, per operation, or per session?
4. Are agent runs recorded as reusable scripts, and how are they parameterised?
5. Which operations go through the driver rather than the command API?
6. Is there local agent memory, and is it opt-out?
7. Does the user see a preview before an agent applies changes?

## Sources

- Affinity: "AI Automation with Claude" (/ai-connector-setup/) ⭐⭐ (MCP capabilities, privacy toggles,
  example prompts, guidance on existing documents, retry, local memory, workflow→script promotion,
  script versioning); "Scripting in Affinity" (/scripting-in-affinity/) ⭐ ("often as a single undoable
  action", what scripting is not)
- tldraw: `docs/ai.mdx` ⭐⭐ (agent API with bounds, custom preview shapes, sanitisation); `docs/driver.mdx` ⭐
  (input simulation, `translateSelection`/`rotateSelection`/`resizeSelection`, `getLastCreatedShape`);
  `sdk-features/history.mdx` ⭐ (`run` with history modes, `bailToMark`)
- Canva: "Canva AI 2.0" (/canva-ai/) ⭐ ("Chat … or make edits yourself", Magic Edit, Magic Resize, Style
  Match); "Use Magic Edit" (/using-magic-edit/); "Review and restore older versions" (/version-history/)
- Figma: "The Figma Design Agent is Here" (https://www.figma.com/blog/the-figma-agent-is-here/) ⭐
  ("generate design layers … across flows, states, copy, and structure"); Figma AI (https://www.figma.com/ai/)
- Spool prototype: `app/src/shell.rs` (`ai_inspector`, `agent_suggestion`)
