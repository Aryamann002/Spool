# Architecture — AI Runtime

> Research note. Layer 2 of three. Covers §28–§30 of the brief.
> Layer 1 evidence lives in `docs/research/ai/*.md`. Layer 3 is not decided here.

---

## 1. The central finding

Every product in the corpus that produces *editable* output does so through the same three-stage
pipeline, and the pipeline shape is identical whether the caller is a human, a script, or a model.

```
intent  →  semantic operation(s)  →  document  →  history  →  rendering
```

The important negative finding is that **none of the four products exposes this pipeline to a model as
an API.** Affinity has the closest thing — a JavaScript scripting API — but it is aimed at humans.
Figma has MCP Server, which wraps the editor for agents, but that is a *transport* wrapper, not a
documented operation vocabulary. Canva's AI produces documents but does not document its operation set.

[INFERRED] So Spool's AI-native ambition is not "add AI features". It is **make the operation
vocabulary explicit enough that a model can be a first-class caller of it** — and then make the
non-human caller observable, reviewable, and undoable.

---

## 2. Stage 1 — output form: pixels vs structured

| Product | Output form | Evidence |
|---|---|---|
| Figma | native document objects (Design Agent); also image generation | [DOCUMENTED] |
| tldraw | **native shapes**, via an explicit simplification format | [SOURCE-CODE] ⭐ |
| Affinity | new document objects; documented guidance to work on existing documents | [DOCUMENTED] |
| Canva | editable design (Magic Layers: flat image → editable layout) | [DOCUMENTED] |

### 2.1 tldraw's simplified shape format is the most instructive artifact in the corpus

[SOURCE-CODE] tldraw's AI path does not ask a model for tldraw's full shape schema. It asks for a
**simplified format keyed by `_type`**, then sanitises it and runs
`convertPartialFocusedShapeToTldrawShape`.

Why this matters architecturally:

1. **The model never touches persistent state.** It produces a DTO.
2. **Sanitisation is a real, named stage.** Untrusted output crosses a boundary before it is admitted.
3. **"Focused" and "partial" are explicit.** A model is told what part of the document it is editing and
   may produce incomplete shapes.
4. **[INFERRED] the conversion is total and one-way.** Anything the model cannot express is dropped
   rather than half-created, because a partial shape is a corrupt document.

This is the same discipline as a deserialiser, and it is the pattern to copy: **model output is a
proposal in a restricted vocabulary, and admission is a separate, total function.**

---

## 3. Stage 2 — streaming and partial application

[SOURCE-CODE] tldraw exposes `Streaming<T>` for model responses.

[INFERRED] Streaming raises a question the corpus does not answer: **does a partially-received shape
become visible in the document, or is it buffered until complete?** For Spool the answer has
architectural weight, because:

- if partial shapes are visible, the document must tolerate incomplete objects mid-stream;
- if buffered, the user sees nothing and cannot judge progress, so progress must be reported outside
  the document.

[PROPOSED-QUESTION] The second is more likely correct for a design editor, because a design document
with a half-created frame is not a useful intermediate state. But this is not evidenced.

### 3.1 Canva's "scheduled tasks" is a different kind of asynchrony

[DOCUMENTED] Canva AI accepts scheduled tasks. [INFERRED] A scheduled AI task is *not* an editing
session — it runs when the user is absent, which means it must be safe to run against a document the
user is not watching. That is the same requirement as a headless/batch operation, and it is the
strongest available argument for the operation layer being independent of the editor runtime (see
`architecture/editor-runtime.md` Implication A).

---

## 4. Stage 3 — context assembly

[DOCUMENTED] Canva AI 2.0 names **six context sources**. [SOURCE-CODE] tldraw's context assembly is
explicitly: screenshot + simplified shapes **in view** + clusters + current selection + recent actions
+ chat history.

[INFERRED] Across both, the context is not "the document". It is a *projection* deliberately bounded by
the viewport, plus the selection, plus recent activity. Two consequences:

1. **Context assembly is a rendering problem**, because a screenshot is required. It therefore depends
   on the renderer being callable offscreen — an open question in `architecture/rendering.md` §8.
2. **Context assembly is zoom- and viewport-dependent**, so the same prompt yields different contexts
   depending on what the user can see. This is a correctness hazard for reproducibility and should be
   recorded with any AI result.

[SOURCE-CODE] tldraw also supports Mermaid → shapes, i.e. a *deterministic* structured input format
being converted into document objects. [INFERRED] that is an "operation API" in miniature and is more
reliable than a model for anything a grammar can express.

---

## 5. Stage 4 — verification

[SOURCE-CODE] tldraw's documented AI workflow includes **exporting the result** for inspection.

[INFERRED] This is the only documented verification mechanism in the corpus, and it is indirect: render
the document, look at it. There is no documented invariant checking, no schema validation against the
document model, and no diffing against an intent.

[PROPOSED-QUESTION] Verification is where Spool has the most room, because the operation layer (if it
exists) makes invariants checkable: after `SetText`, assert the text is present; after `ReparentObject`,
assert no cycle. Nothing in the corpus suggests any product does this.

---

## 6. Stage 5 — approval, preview, undo

| Product | Approval | Undo | Evidence |
|---|---|---|---|
| Affinity | per-capability approval in the MCP server, plus privacy toggles | MCP actions land in the normal undo stack | [DOCUMENTED] |
| Figma | MCP Server | normal undo | [DOCUMENTED] |
| Canva | scheduled tasks imply no in-session approval | version history (1000 attributed versions) | [DOCUMENTED] |
| tldraw | none documented | normal history | [SOURCE-CODE] |

[INFERRED] **Nobody distinguishes AI history from human history.** Every product routes agent changes
through the same undo/redo the user already has. That is a strong convention, and it is nearly free —
provided the AI calls operations rather than mutating state.

The `Agent { Context, Intent, Planning, Operations, Preview, Approval, History }` hypothesis in §30 is
therefore *mostly* already satisfied by an operation layer plus history, with **two genuine gaps**:

1. **Preview** — no product documents a dry-run or staged application. An operation layer over a
   persistent document would make a preview (compute-then-commit) natural; over a mutating API it is not.
2. **Approval granularity** — Affinity is the only product with per-capability approval, and it needed
   a purpose-built MCP server to get it. [INFERRED] per-operation approval is a consequence of having
   named operations to approve.

---

## 7. The agent as "another participant": what that actually requires

[INFERRED] §29's list (`CreateObject`, `MoveObject`, `SetStyle`, …) is a good sketch of the *vocabulary*,
but three properties are what make an agent a participant rather than a script:

| Property | Meaning | Why it matters |
|---|---|---|
| **P1. Addressable identity** | The agent can name what it is acting on. | Without stable ids the agent can only say "the thing on the right". |
| **P2. Observable outcome** | The agent can see the result. | Requires a read/query surface *and* an offscreen render for screenshots. |
| **P3. Reversibility** | The agent's work is one undo step, or a bounded set. | Requires history capture boundaries, not per-operation undo spam. |

A useful test: **P1–P3 are satisfied by the operation layer, and by nothing else.** The vocabulary
matters less than these three properties. This is the strongest architectural statement the AI
evidence supports, and it is independent of which model is used.

[INFERRED] A fourth property is implied by the corpus but never stated:

| Property | Meaning | Evidence |
|---|---|---|
| **P4. Bounded blast radius** | The agent's operations are constrained by what is permitted — e.g. a read-only selection, a single page, a single Studio. | Affinity per-capability approval; Canva's page-bounded documents |

---

## 8. Candidate architectural implications

> §46 format.

### Implication A — The operation registry is the AI integration point; everything else is optional

**Evidence:** [SOURCE-CODE] tldraw's model-facing path converts simplified output into shapes through
one total function; [DOCUMENTED] Affinity's MCP server exposes per-capability operations with approval;
[DOCUMENTED] Canva's AI produces editable documents. [INFERRED] all three need the same substrate: a
vocabulary of named, validated, undoable document operations.

**Why it matters:** §37 distinguishes *state mutation* from *operation*. An agent cannot express intent
against a mutation surface. So the operation layer is not a nicety for AI; it is the prerequisite.

**Approaches:**
- **A1.** Define operations as pure data (`Operation` enum) applied by one function, with the agent as
  an ordinary caller.
- **A2.** A1 plus a second, deliberately restricted vocabulary for models (a *simplified* format, as
  tldraw does), so the model's surface stays small even as the internal one grows.
- **A3.** A1 plus per-operation metadata (capability class, blast radius, reversibility) that an
  approval UI and a policy layer can read generically.

**Tradeoffs:** A1 is the minimum that works. A2 keeps prompt size and error rate down but adds a
conversion layer to maintain — though that layer is exactly where sanitisation belongs, which is a
feature. A3 is what makes Affinity-style per-capability approval possible without inventing a policy
language per feature.

**Decision: TBD — requires architecture review.**

---

### Implication B — Require a total, validating admission step between any untrusted input and the document

**Evidence:** [SOURCE-CODE] tldraw's `convertPartialFocusedShapeToTldrawShape`, with explicit
sanitisation of model output. [INFERRED] every other external input (imported SVG, pasted HTML, a
Figma file, a `.psd`) is the same shape of problem.

**Why it matters:** Spool will have at least five untrusted-input paths: AI output, file import,
clipboard, scripts, and eventually plugins. If each invents its own validation, the document model
acquires five subtly different notions of what is legal.

**Approaches:**
- **B1.** A single `admit(ExternalShape) -> Result<DocumentPatch, Vec<Problem>>` entry point.
- **B2.** B1, plus a document-level invariant checker run after every admitted patch (no cycles,
  ids unique, references resolve).
- **B3.** Defer until there is a second external-input path to share it with.

**Tradeoffs:** B1 is a good shape and cheap while the model is small. B2 is where real safety lives and
gets more valuable as the document model grows. B3 is the honest answer if there is only one input
path today — but the brief makes AI, import, and extensibility all in scope, so B3's premise is
already false.

**Decision: TBD — requires architecture review.**

---

### Implication C — Make AI-generated work reversible by construction, not by convention

**Evidence:** [DOCUMENTED] every product routes agent changes through ordinary undo; [DOCUMENTED]
Canva compensates with 1000 attributed versions and a 30-day trash; [SOURCE-CODE] tldraw's
`editor.run(fn, { history: 'squash' })` already expresses "collapse this into one history entry".

**Why it matters:** An agent performing a 200-operation refactor should be one undo. Without an explicit
capture boundary the user must press undo 200 times, and they will not trust the feature.

**Approaches:**
- **C1.** Every agent invocation is wrapped in a single capture boundary, regardless of operation count.
- **C2.** C1 plus a labelled boundary ("AI: converted 12 elements") so undo history is readable.
- **C3.** C1 plus a separate agent-specific history stream that the UI can show as one item while the
  document's own history stays clean.

**Tradeoffs:** C1 and C2 are one line each once capture boundaries exist, and are clearly correct. C3 is
cleaner in principle but introduces a second history model, which is a large cost for a presentational
benefit — defer.

**Decision: TBD — requires architecture review.**

---

### Implication D — Treat "preview" as a consequence of a persistent document, not a feature

**Evidence:** [DOCUMENTED] no product documents a dry-run or staged AI application. [INFERRED] the
reason is structural: if operations mutate a live view, staging requires duplicating the document;
if operations return a patch against a persistent document, preview is just "apply the patch you already
computed".

**Why it matters:** This is an argument for the document/editor split (`editor-runtime.md`
Implication A) from an *AI* direction rather than a rendering one. Two independent lines of evidence
converging on the same requirement is worth recording.

**Approaches:**
- **D1.** Operations return an applied patch; preview = compute the patch and show its summary.
- **D2.** D1 plus an offscreen render of the patched document — which also serves screenshot-based
  context and export. Highest value, highest cost (depends on `rendering.md` open question 2).
- **D3.** No preview; rely on undo. Cheapest, and honest for a v1.

**Tradeoffs:** D1 is nearly free if operations are patch-based and is the enabler for D2. D2 unlocks
three features at once — preview, context screenshots, and headless verification — which makes it the
highest-leverage item in this note. D3 is a legitimate choice if the stack cannot render offscreen, and
should be recorded as such rather than treated as a failure.

**Decision: TBD — requires architecture review.**

---

### Implication E — Decide whether scripts and agents share one surface

**Evidence:** [DOCUMENTED] Affinity has a JavaScript scripting API (beta) whose operations are
"often as a single undoable action", a local MCP server, and documented guidance that *working with
existing documents produces better results than generating from scratch*. [SOURCE-CODE] tldraw has no
scripting API but does have `@tldraw/driver`, which synthesises real pointer/keyboard/wheel/pinch
events through `editor.dispatch`.

**Why it matters:** These are three different surfaces for the same underlying capability — intent
expressed against a document:

1. **Operation API** — precise, typed, agent-friendly, but not expressive.
2. **Scripting API** — expressive, human-facing, but a second language to sandbox and document.
3. **Event driver** — maximally faithful (it exercises the real interaction path), but low-level and
   unusable by a model.

[INFERRED] tldraw's driver is not an authoring surface; it is a *test* surface, and that is the right
role for it. Spool would benefit from all three at very different costs.

**Approaches:**
- **E1.** Operations only, for agents. Defer scripting.
- **E2.** Operations + an event driver (for tests and for future script authoring). Defer a scripting
  language.
- **E3.** All three, treating scripting as a first-class extension point from the start.

**Tradeoffs:** E1 is the smallest coherent answer and matches Spool's AI-native positioning. E2 adds
a genuinely valuable testing capability — headless interaction tests are the only way to verify an
interaction state machine — at a cost roughly proportional to the driver's completeness. E3 is the
"VS Code" answer literally, and front-loads sandboxing, a stable API, and a security model before any
of the core is solid.

**Decision: TBD — requires architecture review.**

---

## 9. Open questions

1. Does the model see the document as *simplified shapes* (tldraw) or as a natural-language
   description plus a screenshot? What does each cost in accuracy?
2. Are partial shapes ever visible during streaming?
3. What does "approve" mean for a 200-operation batch — all, some, or a previewable diff?
4. Should AI-generated objects be marked as such in the document (provenance metadata)? No product
   documents this.
5. Should an agent be able to see the whole document, or is viewport-bounded context the right
   constraint?
6. Does the AI runtime need to work without a window at all (scheduled tasks, CLI, CI)?
7. What happens when the AI's operations conflict with what the user is doing at the same time?
   [INFERRED] this is the collaboration problem in miniature and is likely the hardest unsolved one.
8. Is a screenshot-based round trip acceptable, or does it create an unacceptable latency floor?
9. Should the restricted model-facing vocabulary be a *separate* type from the internal operation type,
   or a marked subset of it?
10. Which operations should be classified as higher blast radius (delete-all, reparent-everything,
    batch restyle) so that approval and policy can treat them differently?

## 10. Sources

- tldraw (SOURCE-CODE) ⭐⭐⭐: AI shape conversion (`_type` simplified format, sanitisation,
  `convertPartialFocusedShapeToTldrawShape`, `Streaming<T>`), context assembly (screenshot,
  in-view simplified shapes, clusters, selection, recent actions, chat history), Mermaid conversion,
  export-for-verification, `@tldraw/driver`
- Affinity (DOCUMENTED): JavaScript scripting API (beta), "often as a single undoable action",
  affin.co/affinity-sdk, local MCP server with per-capability approval and privacy toggles,
  guidance on editing existing documents, workflow→script promotion
- Figma (DOCUMENTED): MCP Server, Design Agent (May 2026), Make (2025), First Draft (2023)
- Canva (DOCUMENTED): Canva AI 2.0 and its six context sources, scheduled tasks, Magic Layers,
  Magic Edit, Magic Resize, version history (1000 attributed versions, avatars)
- Related notes: `ai/*.md`, `architecture/document-model.md` (operation registry),
  `architecture/history.md` (capture boundaries), `architecture/rendering.md` (offscreen render),
  `architecture/editor-runtime.md` (document/editor split), `architecture/input-system.md`
  (single action surface)
