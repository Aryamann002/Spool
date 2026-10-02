# Spool — Agent-Native Editing

> **Research and interface design for discussion, not an implementation and not a decision.** Evidence
> labels as in `spool-html-document-research.md`.

---

## 1. Executive summary

1. **The three agent paths can converge, but only through a normaliser, and the hard cell is file edits.**
   Semantic operations and MCP calls are already operations. An external file edit is a *text delta*, and
   converting it into an operation requires knowing what the delta *means* — which requires the cascade
   engine (research file §12). §4.
2. **One MCP `tools/call` = one history transaction** falls out of Spool's settled history direction
   (K3/K4) and the protocol's own request framing. The brief's "15 changes: 15 entries or 1?" question has
   an evidence-based answer, not a preference. §9.
3. **External edits must be admitted to history, or history is two systems that cannot explain each
   other.** This is a genuine new obligation created by making source canonical, and tldraw never had to face
   it because its document is not externally editable. §9.2.
4. **History should record semantic operations, not source ranges.** K3 makes ranges reconstructible; storing
   them creates a staleness class of bug. §9.1.
5. **The minimum agent interface is ~21 tools, and only 5 of them are mutations.** Reads dominate because
   the corpus's convergence is that agents succeed by *inspecting existing structure* — Affinity documents
   "work with existing documents; designing from scratch generally produces poor results". §11.
6. **The single highest-value agent capability is not a tool — it is `render_region()`.** The corpus records
   that every product verifies AI work by exporting and inspecting, and that offscreen rendering is missing
   from Spool while being a *general* capability, not an AI one. §11.4.
7. **MCP is a transport, not a representation, and its own spec requires a human in the loop.** Adopting it
   implies nothing about the document format — which is useful, because it means the agent interface can be
   built before the format question is answered. §3.
8. **Provenance is genuinely open in the market.** The corpus records it as undocumented in all four
   products. Spool could be first here, cheaply, via a field on the operation. §10.

---

## 2. Scope

Covers: the three agent paths; their convergence; validation and admission; transaction boundaries and
history; undo from agent work; provenance; conflicts with human and external edits; file watchers; context
extraction; and a minimum agent interface.

Does not cover: model selection, prompt engineering, retrieval strategies, MCP transport internals, or
multi-agent orchestration. Does not revisit the document-format question, which lives in
`spool-html-document-architecture.md`; where this document says "source", it means "the persistent
representation whatever it is", and the HTML/CSS specifics are cross-referenced.

**Settled Spool constraints inherited.** K1 (document/runtime type separation), K2 (typed semantic
operations as the single mutation path), K3 (eager interaction commit; cancel creates no entry), K4
(semantic/operation-oriented history), K5 (do not copy tldraw's capture-time/bail model).

---

## 3. MCP as an integration surface

### 3.1 What the specification actually says

`OFFICIAL DOCUMENTATION` — Model Context Protocol, revision `2026-07-28`, `/server/tools`:

| Fact | Consequence for Spool |
|---|---|
| "Tools in MCP are designed to be **model-controlled**" | The agent picks the tool; Spool cannot rely on a fixed sequence |
| "For trust & safety and security, there **SHOULD** always be a human in the loop with the ability to deny tool invocations" | Every tool needs a deny path, including read tools |
| Tools have `inputSchema` (JSON Schema, default 2020-12) | Spool's operations already have a type; mapping is mechanical |
| Tools **may** have an `outputSchema`; "Servers MUST provide structured results that conform to this schema" | Spool can publish a self-describing contract |
| "a tool that returns structured content SHOULD also return the serialized JSON in a TextContent block" | Structured + text duplication is the interoperable default |
| "clients MUST consider tool **annotations** to be untrusted unless they come from trusted servers" | Spool must not rely on agent-supplied annotation text |
| `InputRequiredResult` + `elicitation/create` | Multi-round-trip elicitation is a first-class protocol feature |
| Tools may return `resource_link` / embedded `resource` with `file://` URIs | **MCP's own vocabulary includes "here is a project file"** |
| Servers SHOULD return tools deterministically to improve "LLM prompt cache hit rates" | Tool ordering is a cost optimisation, not cosmetics |

### 3.2 Three consequences

`INFERENCE`.

1. **MCP does not constrain the representation.** It is JSON-RPC over stdio/HTTP with JSON Schemas. Spool can
   adopt it before the format is decided — which means the agent interface is **not blocked** on the
   architecture gate (File 2 gate G4 is about *quality*, not feasibility).
2. **MCP and file editing are the same mechanism at the protocol level.** `resource_link` with a `file://`
   URI *is* a file handle. Spool's three agent paths are therefore not three subsystems; they are two
   surfaces over one operation system, with file access being a convenience.
3. **The human-in-the-loop requirement is not optional overhead** — it is the spec's own recommendation, and
   it aligns with the corpus's per-capability-approval finding for Affinity (`README.md` §AI: "Per-capability
   approval required first *defining what the capabilities are*. Approval is downstream of naming."). Spool's
   tool list *is* the capability list.

---

## 4. The three paths, and the normaliser

```
   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
   │  MCP tool    │   │  File edit   │   │  Script      │
   │  call        │   │  (external)  │   │  (MCP/sandbox)│
   └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
          │  already an op  │  a TEXT DELTA     │  already an op
          │                  ▼                   │
          │            ┌───────────┐              │
          │            │ NORMALISE │  ← the hard cell
          │            └─────┬─────┘
          └──────────────────┼──────────────────┘
                             ▼
                      ┌─────────────┐
                      │  VALIDATE   │  §5
                      └──────┬──────┘
                             ▼
                   semantic operations (K2)
                             ▼
                      history (K3/K4)
                             ▼
                    runtime → source (splice)
```

### 4.1 Paths 1 and 3 are the easy ones

`INFERENCE`. An MCP tool call and a script both arrive as *named operations with typed arguments*.
`SOURCE-CODE FACT` — Spool's prototype already has the operation vocabulary: `CommandOperation` is a closed
5-variant enum (`Geometry`, `Style`, `Text`, `Insert`, `Delete`, `canvas.rs:379-385`), `record` validates and
normalises by dropping no-ops (`canvas.rs:648-680`), and `undo`/`redo` replay `before`/`after`
(`canvas.rs:687-745`). An MCP tool is a thin wrapper over exactly this. **This is the strongest existing
asset for agent-native editing and it is already built.**

### 4.2 Path 2 is the hard one, and it is hard for a specific reason

An external file edit is:

```
   text delta (a byte range changed)
```

An operation is:

```
   intent ("set gap on #hero from 24px to 32px")
```

**INFERENCE**. The delta is *not* invertible into intent without a semantics model. Consider:

| External edit | What it means |
|---|---|
| `gap: 24px` → `32px` in `.hero` | Clear — but does it affect 1 element or 10⁴? (selector match count) |
| `.hero` → `.landing` (selector rename) | Structural — potentially the whole stylesheet's match set |
| adding `.theme-dark .button` | Depends on whether that node currently matches — i.e. on the DOM |
| adding a `<div>` wrapper | Moves *every* descendant's computed style |
| reordering two rules | Can change the winner for an unbounded set of elements |
| changing `--brand` in `:root` | Repaints everything using the token |

`SOURCE-CODE FACT` — Blink's answer to the last two is the invalidation-set subsystem
(`third_party/blink/renderer/core/css/style-invalidation.md`): rules are compiled into `InvalidationSets`
collected into a `RuleFeatureSet`; each node carries `NeedsStyleInvalidation`,
`ChildNeedsStyleInvalidation`, `NeedsStyleRecalc`, `ChildNeedsStyleRecalc`; a `PendingInvalidationsMap`
accumulates deferred work flushed before style is read. The document also states the sets "are not perfect,
they err on the side of correctness."

`INFERENCE`. So the normaliser needs, at minimum: a selector matcher, a cascade evaluator, and an
invalidator. **That is the cascade engine again** — the same blocker as File 2's finding 1. The honest
statement: **file-based agent editing and the cascade engine are the same dependency.** You cannot have the
first without the second.

### 4.3 A fallback that does not need the engine

`PROPOSED SPOOL DESIGN` — a **coarser normalisation** for path 2, usable before the cascade engine exists:

1. Parse the changed chunk with tree-sitter; get the changed ranges (`ts_tree_get_changed_ranges`).
2. For each changed `element`/`attribute` node whose `id` matches a resident node → emit a
   `set_attribute`/`set_text` operation. **No cascade needed** — the agent wrote the value.
3. For each changed `declaration` in a rule whose selector matches **exactly one** resident node → emit
   `set_style`. Matching requires a selector engine but *not* a cascade.
4. Everything else → **one `reconcile` operation** that replaces the affected subtree wholesale and is
   flagged in history as coarse.

`INFERENCE` — steps 1–3 cover the overwhelming majority of realistic agent edits (change a property, change
text, change an image) and produce *precise* operations. Step 4 is the honest fallback, and its granularity
is exactly what makes it visible to the user rather than silently wrong. This is what makes the agent path
buildable **before** gate G2 resolves, and it is the recommended sequencing.

---

## 5. Validation and admission

### 5.1 What must be validated

`INFERENCE`. Five layers, each with a distinct failure mode:

| Layer | Check | Failure mode it catches |
|---|---|---|
| **L1 Schema** | Argument shape, id exists, property is known | Typos; hallucinated properties |
| **L2 Semantic** | Property is legal on this node type; value parses to a typed value | `display: 12px` |
| **L3 Structural** | Parent/child constraints; no cycles; layout invariants | A frame whose children exceed it, if clipping is violated |
| **L4 Reference** | Asset, component, token, prototype target exists | Dangling refs — the corpus's largest pure gap (`document-model.md` Q9) |
| **L5 Resource** | Node count, byte count, raster dimensions, wall-clock | DoS — research §23.3: usvg has **no** budget |

`PROPOSED SPOOL DESIGN` — **admission is a single total function**, following the corpus's tldraw finding
(`ai-runtime.md` Implication B: "Require a total, validating admission step between any untrusted input and
the document"). A total function cannot leave the document in a half-validated state, which is the property
that makes "the agent did something bad" a non-event rather than a support burden.

### 5.2 Preview versus commit

`INFERENCE` — the corpus records preview as **unsolved in all four products**
(`README.md` §AI: "Preview / staged application: no"; "Dry-run before commit: no"; "Invariant checking after
apply: no"), and records the reason it would be *free*:

> "preview is the one item where a persistent document with an operation layer would pay off automatically:
> **staging is free when operations return a patch, and expensive when they mutate a view**."
> — existing corpus, `architecture/ai-runtime.md` Implication D

**INFERENCE** — Spool's `DocumentCommand`/`CommandOperation` structure is an operation-with-before/after
shape, so it is closer to the "returns a patch" end of that spectrum than a mutable view. `PROPOSED SPOOL
DESIGN`: make every operation *pure with respect to a document argument* — `op → (new_document, patch)` —
and both undo and preview fall out for free. This is a **small change to the operation signature with three
payoffs** (undo, preview, agent dry-run), and it is the single highest-leverage structural choice available
in the agent domain.

### 5.3 Sanitisation is not the same as validation

`INFERENCE`. Research file §23 establishes that a native Rust renderer with no `<script>` execution makes
sanitisation largely unnecessary — "there is nothing to sanitise because the feature is absent from the
output model" (the usvg property). Validation is different: it is about the *document being coherent*, not
about the *input being safe*. Both are needed; they are not substitutes, and conflating them is a common
design error.

---

## 6. Transaction boundaries

`INFERENCE`. The rule, stated once:

> **One MCP `tools/call` = one history transaction.** A script's `run` block = one transaction. One external
> file-watch batch = one transaction. One human gesture = one transaction.

| Boundary | Evidence |
|---|---|
| Human gesture = 1 transaction | K3; corpus `history-matrix.md` §1 ("4 of 4" convergence) |
| MCP call = 1 transaction | The protocol frames one `tools/call` as one request; §3.1 |
| Script block = 1 transaction | Affinity's scripting API operations are "often as a single undoable action" (corpus `profiles/affinity.md`) — and the corpus reads that as a candid admission the API *lacks* a rigorous boundary, which Spool should fix rather than copy |
| External batch = 1 transaction | `PROPOSED SPOOL DESIGN`; §9.2 |

**What a transaction boundary must support** (`INFERENCE`): multiple operations, applied atomically; a
single inverse; a single history entry with a human-readable label; all-or-nothing validation before any
mutation; and the ability to be *named*, so the undo UI can say "undo: agent reflowed hero" rather than
"undo: style change".

`SOURCE-CODE FACT` — the last one has a direct precedent: tldraw's
`markHistoryStoppingPoint(markId)` takes a name used for debugging
(`architecture/tldraw-history-source.md` §History Stopping Points), and the tldraw investigation concluded
that Spool should *not* adopt marks/bail — but the *naming* of a transaction is independent of the marking
mechanism and costs nothing.

---

## 7. Context extraction — what an agent needs to see

### 7.1 The corpus's findings, applied

`INFERENCE` from the existing corpus (`ai/context.md`):

- tldraw's context is **five sources**: screenshot + simplified in-view shapes + clusters + selection +
  recent actions + chat history.
- Affinity **deliberately** shares less.
- The strongest cross-product conclusion is "**generation into existing structure beats generation from
  nothing**."
- The context/operation asymmetry: context is expensive to produce and must be bounded.

`INFERENCE`. The HTML/CSS hypothesis changes exactly one thing here, and it is a large one: **the source
file *is* the subtree**, so `get_subtree` can return bytes an agent can reason about natively, with no
serialisation step and no lossy "simplified shape format". This is the strongest agent-side argument for
the text representation, and it is independent of the cascade problem.

### 7.2 What must *not* be in context

`INFERENCE`, from `ai/context.md` §"What context must not include":

- the whole document (the brief's "give LLM 500MB HTML file" prohibition);
- resolved/computed style as the *only* view — the agent needs **authored** style to edit it;
- derived caches of any kind;
- anything the agent is not permitted to modify (it invites attempts and forces denials).

---

## 8. Reading versus writing

`INFERENCE`. Read tools are cheap, plentiful, and where an agent's failures are recoverable. Write tools
are scarce and where failures are expensive. The ratio should be lopsided — the brief's own tool list is
already lopsided (14 reads, 7 writes) and this document keeps that shape.

| Category | Cost of a mistake | Policy |
|---|---|---|
| Read | Low | Always allowed |
| Write (semantic) | Medium — reversible by `⌘Z` | Allowed; human may deny |
| Write (source) | Medium — reversible if normalised (§4.3) | Allowed with normalisation |
| Destructive (delete, replace asset) | High | Approval |
| Anything crossing the project root | High | Denied by default |

---

## 9. History, undo, and the agent

### 9.1 History records operations, not source ranges

`INFERENCE`. With K3/K4, an agent operation is already a before/after pair. If a source-range were also
stored, it would go stale the moment an external edit shifts byte offsets, producing an undo that corrupts
the file. `PROPOSED SPOOL DESIGN`: **reconstruct the source range at undo time** by re-resolving the node id
and property in the current CST. The cost is that undo after an external edit may be non-minimal; the
benefit is that undo is never *wrong*.

### 9.2 The brief's question: human edits canvas, agent edits files, human presses undo

Three distinct cases, which the brief's framing conflates:

| Case | What should happen | What it requires |
|---|---|---|
| Undo after only agent operations | Undo the agent's transaction exactly | Nothing new |
| Undo after only external edits | **Undefined today.** Are external edits in history at all? | A decision (§9.3) |
| Undo spanning both | Requires one interleaved ordering | The same decision, plus labels |

`INFERENCE`. The three cases exist because **there are two mutation paths and only one history**. `PROPOSED
SPOOL DESIGN` — external edits *are* admitted to history, one transaction per watcher batch, labelled with
the file path and a summary of the ranges changed. Without this, the undo stack contains some operations and
not others, and the user cannot be told which is which — the exact failure the corpus attributes to Affinity
("often as a single undoable action" as a candid admission).

**This is a genuinely new obligation** created by making source externally editable. No corpus product has
it, because none of them is source-authorable.

### 9.3 "15 changes: 15 entries or 1 transaction?"

`INFERENCE`. One transaction, on three grounds:

1. **K3** makes a transaction = a completed unit of work; a `tools/call` is that unit.
2. **The corpus converges** that "one continuous gesture = one undo step" in 4 of 4 products
   (`history-matrix.md` §1). An agent turn is the agent's gesture.
3. **MCP frames it** as one request (§3.1), and the corpus notes "Agent changes route through ordinary undo in
   every product. Nobody distinguishes AI history from human history. That convention is nearly free —
   provided the agent calls operations rather than mutating state" (`README.md` §AI).

`PROPOSED SPOOL DESIGN` — the qualifier on (3) is the whole design constraint: **if the agent mutates state
directly instead of calling operations, "nearly free" becomes "never free".** Everything in §4 exists to
keep that true for file edits.

### 9.4 A required capability the corpus lists as absent everywhere

`INFERENCE` — the corpus's AI gap table includes "Diff shown to the user: no" and "Preview / staged
application: no", and `ai-runtime.md` Implication D says preview becomes free with a persistent document plus
an operation layer. Combining that with §5.2: **Spool can show an agent's proposed transaction as a diff
before committing it**, because the operation returns a patch. That is not a feature to build later; it is a
consequence of the operation signature chosen now.

---

## 10. Provenance

### 10.1 The market position

`INFERENCE` from the existing corpus (`README.md` §AI): "Provenance / 'this was AI-generated' — undocumented
in all 4"; "Per-operation AI approval — Affinity only (per-*capability*)"; "Are partial AI results visible —
undocumented"; "Contention between an agent and a live user — undocumented".

`INFERENCE`. This is the corpus's characteristic finding — "**open space, not a gap Spool is behind in**" —
and it is unusually cheap to occupy, because the operation system is the natural carrier and Spool is
building one anyway.

### 10.2 Three questions provenance must answer

| Question | Where the answer lives | `PROPOSED SPOOL DESIGN` |
|---|---|---|
| "Which of these did the agent change?" | Operation record | `origin: Human \| Agent \| External \| Import` |
| "Why did it change them?" | Operation record | optional agent-supplied rationale string |
| "Which agent / which run?" | Operation record | optional session id |

### 10.3 The representation question

`INFERENCE`. Provenance on the *operation* is transient (it lives in history). Provenance on the *node* is
persistent (it survives save, and an agent can read it). They are different features:

- **Operation provenance** — cheap, always available, answers "what did the agent just do".
- **Node provenance** — persistent, answers "is this object AI-generated", and needs a representation choice:
  an attribute (e.g. `data-spool-gen`), or CSS custom property, or nothing.

`INFERENCE` — research file §18 and the tldraw investigation both show the *ephemeral-key* pattern as the
established mechanism for exactly this (a marker that survives in state but is not replayed). `PROPOSED
SPOOL DESIGN`: if node provenance is wanted, it should be declared **non-restored on replay**, so undoing an
AI edit does not resurrect its "AI-generated" flag on the pre-edit version.

`INFERENCE` — the counterweight, which should be recorded: persistent node provenance is a **diff-noise
generator** (every agent edit adds a byte to the source) and a **merge-conflict generator** (two agents
touching the same node). Operation provenance has neither problem. `Decision: TBD — requires architecture
review` on node-level provenance specifically.

---

## 11. A minimum agent interface

`PROPOSED SPOOL DESIGN` — 21 tools, from the brief's list plus what the evidence adds. **Read/write ratio
17:4** is deliberate.

### 11.1 Read (17)

| Tool | Signature sketch | Notes |
|---|---|---|
| `get_project` | `() → { manifest, pages, components, styles, assets, counts }` | Manifest only. Never the whole document |
| `get_page` | `(page_id) → { id, name, node_count, chunk }` | Metadata, not content |
| `get_node` | `(node_id) → { id, type, parent, bounds, style, attrs, children[] }` | The single most-used read |
| `get_subtree` | `(node_id, depth?) → source text` | **Returns source verbatim if the format is text** — the §7.1 advantage |
| `get_style` | `(node_id) → authored declarations with file + range` | **Authored, not computed.** Solves L6 |
| `get_computed_style` | `(node_id, viewport?) → resolved values + provenance[]` | Provenance list = the §12 cascade question, exposed |
| `get_matching_rules` | `(node_id) → [{ rule, range, selector, declarations[], winner: bool }]` | The inspector-shaped read; a direct MCP port of DevTools' model |
| `get_assets` | `() / (page_id)` | Content-addressed ids + dimensions |
| `get_dependencies` | `(path) → { imports, references, dependents }` | Enables safe external edits |
| `get_selection_context` | `() → { selection, camera, visible region }` | The corpus's tldraw-style context source |
| `get_recent_operations` | `(n) → [{ op, origin, label, inverse_preview }]` | Lets an agent see what just happened |
| `get_history` | `(n) → labels` | |
| `get_constraints` | `(node_id)` | Only if Spool has constraints (research §7 L1) |
| `get_components` | `(page_id?)` | Definitions + instances |
| `get_variables` | `() → token table` | `get_computed_style` on `:root` |
| `render_region` | `(rect, width, height) → image` | **See §11.4** |
| `search` | `(query, kind) → [node_id, snippet]` | Cross-page |

### 11.2 Write (4)

| Tool | Batch? | Rationale |
|---|---|---|
| `apply_operations` | **Yes — this is the main one** | Accepts an array; one `tools/call` = one transaction (§6) |
| `set_node_style` | — | Convenience wrapper; sugar over `apply_operations` |
| `create_nodes` | — | Sugar |
| `delete_nodes` | — | Destructive → approval |

`PROPOSED SPOOL DESIGN` — **prefer one batched `apply_operations` over many fine-grained mutation tools.**
Reasons: (a) it makes the transaction boundary the *tool call*, which is exactly §6; (b) it lets the agent
propose a coherent multi-step change that is validated atomically (L1–L5 before any mutation); (c) it
returns a patch, which is what enables preview (§9.4); (d) it keeps the tool count low, which the MCP spec
implicitly rewards via prompt-cache hit rates (§3.1). Fine-grained tools remain useful for *interactive
single* edits where the user wants immediate feedback.

### 11.3 Is this sufficient?

`INFERENCE` — what is **missing**, stated honestly:

| Gap | Why | Status |
|---|---|---|
| **Multi-node selection semantics** | An agent asking "make these three the same width" needs a group operation | Probably needs a `measure`/`align` primitive family |
| **Component creation** | `create_component` / `override_instance_property` are structurally different from node edits | Likely needs its own tools |
| **Prototype edges** | Connections are not node property edits | Likely needs its own tools |
| **Transactions across MCP calls** | An agent may need to stage then commit | `InputRequiredResult` may cover it; unverified |
| **Undo/redo as agent tools** | An agent may need to back out its own mistake | Possible, but "agent can undo" is a policy question, not a capability one |
| **Conflict resolution** | An agent editing while a human drags | Needs a policy (§12), not a tool |

`INFERENCE` — the honest summary: **the read side is well-determined by the evidence; the write side is
determined by Spool's own document model**, which is still an open question. So the *minimum interface can be
specified now for reads, and only provisionally for writes.*

### 11.4 `render_region` is the highest-value tool

`INFERENCE`, from three corpus findings converging:

1. Verification is "export and inspect" in tldraw's AI path (`ai-runtime.md` §Stage 4).
2. Offscreen rendering is missing from Spool and is *"not an AI feature at all"* — it is a general capability
   that export, thumbnails, print, and tests all need (`README.md` §9).
3. Agents cannot see the canvas, so every visual judgement they make is guesswork. The corpus's convergence —
   generation into existing structure beats generation from nothing — only works if the agent can *verify* it
   generated into structure correctly.

`PROPOSED SPOOL DESIGN` — `render_region(rect, width, height) → PNG` should be built **before** the write
tools, for two reasons: it is required by export and tests regardless of agents, and an agent with a
verification loop is qualitatively better than one without. `render_region` at 10⁶ objects requires the
File 3 pipeline — so the two files' conclusions are coupled, and the agent interface is partly gated on
rendering scale.

---

## 12. Conflicts

### 12.1 Three races, none solved by the representation

| # | Race | Mechanism | `PROPOSED SPOOL DESIGN` |
|---|---|---|---|
| R1 | Spool writes a file while an agent writes it | Last-writer-wins loses data | Optimistic concurrency on a **content hash** held in the manifest; refuse on mismatch |
| R2 | The watcher fires for Spool's own write | Redundant reparse + spurious history entry | **Self-write suppression by content hash**, never by path |
| R3 | An agent edits while the user is mid-gesture | Half-applied gesture, stale snapshot | **Defer external application while a gesture is active**; queue and apply at gesture end; if the gesture's target changed, surface a conflict |
| R4 | Git merge brought in a third version | Spool's in-memory state is stale | Detect on file read; re-parse as a normal external edit |

`INFERENCE`. R2 is the one most often got wrong: suppressing by path means an agent's legitimate edit to the
same path is silently dropped. Suppression must be by **content hash of what Spool wrote**.

### 12.2 Conflict granularity

`INFERENCE`. Because history is operation-oriented (§9.1), a conflict is naturally expressed as **which
operations can still be applied**, not as "which lines". `PROPOSED SPOOL DESIGN` — a three-way classification
at conflict time:

| Case | Condition | Action |
|---|---|---|
| **No conflict** | The node/property is untouched by the local change | Apply |
| **Trivially resolvable** | Same property, same target, different value | Prefer external; record an operation so it is undoable |
| **Real conflict** | The local change and the external change touch overlapping structure | **Stop and surface.** Do not auto-resolve |

`INFERENCE` — the corpus is explicit that contention between an agent and a live user is **undocumented in
all four products**. This is open space; the cost of getting it wrong is the user's work disappearing, which
is why "stop and surface" is the safe default for the real-conflict case.

### 12.3 Selection and editor state survival

`INFERENCE`. K1 makes selection and camera runtime state, and research file §19 establishes they survive an
external edit **iff identity survives it** (`data-spool-id`, research §22.3). So node provenance is not the
only thing riding on identity — **the entire editor experience after an agent edit** is. `PROPOSED SPOOL
DESIGN`: after an external edit, remap the selection by id; any id that vanished becomes an explicit
"agent deleted a selected object" notification rather than a silent drop.

---

## 13. What must not be built yet

`INFERENCE` — items whose absence is currently correct:

| Do not build | Why |
|---|---|
| An agent that writes files *directly*, bypassing normalisation | Makes history two systems (§9.2) and undo unsafe (§9.1) |
| Fine-grained mutation tools before the batch tool exists | No transaction boundary (§6) |
| A learned/derived ranking of relevant context | §7.1's converged rule is *structural* context, which is cheap and reliable; learned retrieval is unproven here and adds a moving part |
| Multi-agent orchestration | Out of scope; no corpus evidence |
| Node-level provenance persistence | Diff and merge noise (§10.3); operation-level provenance captures most of the value |
| Any agent surface that predates `render_region` | An agent that cannot verify its own work is guesswork (§11.4) |
| An MCP server that is coupled to the document format | MCP is a transport (§3.2); coupling it to an undecided format is the expensive mistake |

---

## 14. Open questions

1. **Can §4.3's coarse normalisation be the *only* normaliser?** i.e. is the full cascade engine needed for
   agent editing at all, or only for inspector correctness? This is gate G2 in File 2, restated.
2. **Should external edits be undoable by the user at all?** §9.2 argues yes; a user may reasonably want
   external changes treated as "the file changed", not "an operation happened". Genuinely open.
3. **How are conflicts presented?** A textual diff, a node-level diff, or a modal choice? No corpus precedent.
4. **What is the right granularity for `apply_operations`?** 1 op, or up to N? Bounded N is safer for atomic
   validation but limits agent expressiveness. Untested either way.
5. **Does an agent need a "measure" primitive family** (measure, align, distribute) rather than only
   setters? §11.3 suggests yes; the corpus has no evidence on whether agents handle constraint-style better
   than setter-style.
6. **Should read tools be paginated, and does that break prompt caching?** The MCP spec's deterministic-order
   requirement (§3.1) suggests ordering matters; pagination interacts with it. Unverified.

---

## 15. References

Evidence is cited in `spool-html-document-research.md` §28 and not repeated. This file adds:

1. Model Context Protocol specification, revision 2026-07-28, `/server/tools` — https://modelcontextprotocol.io/specification/2026-07-28/server/tools — primary source for §3.
2. Chromium Blink, *CSS Style Invalidation in Blink* — primary source for §4.2.
3. tree-sitter `lib/include/tree_sitter/api.h` — primary source for §4.3.
4. `app/src/canvas.rs` (`CommandOperation` `:379-385`, `History` `:425-428`, `record` `:648-680`, `undo`/`redo` `:687-745`) — read-only; existing Spool operation system.
5. `docs/research/architecture/spool-html-document-research.md` — §12 (cascade), §18 (agent paths), §19 (watchers), §22 (identity), §23 (security).
6. `docs/research/architecture/spool-html-document-architecture.md` — §2 (constraints K1–K5), §10 (candidates), §11 (gates).
7. `docs/research/architecture/tldraw-history-source.md` — history naming; marks/bail not adopted per K5.
8. Existing Spool corpus, read-only: `ai/{context,editing,agents,generation,structured-generation}.md`, `architecture/ai-runtime.md`, `architecture/document-model.md`, `architecture/history.md`, `matrices/ai-matrix.md`, `matrices/history-matrix.md`, `profiles/affinity.md`, `README.md`.
