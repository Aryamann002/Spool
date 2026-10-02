# AI Matrix

> §28–§30. Cross-product comparison of AI capability, with the emphasis on **output form** — pixels
> versus editable structure — because that is the axis Spool's positioning turns on.
>
> Legend: **✔** · **◐** partial · **—** absent · **?** undocumented

---

## 1. Generation

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Generate a complete design | ✔ First Draft (2023) → Make (2025) | ◐ | ✔ Canva AI Studio | ✔ | **—** |
| Generate a layout / page | ✔ | ◐ | ✔ | ✔ | **—** |
| Generate components | ✔ | — | — | ◐ | **—** |
| Generate illustrations | ✔ | ◐ | ✔ | ✔ | **—** |
| Generate images | ✔ | — | ✔ | ✔ | **—** |
| Generate **text** | ✔ | ◐ | ✔ | ✔ | **—** |
| Generate style sets / palettes | ✔ | — | ◐ | ✔ **Brand Kit** | **—** |
| Generate from a prompt + reference image | ✔ | — | ✔ | ✔ | **—** |
| Generate from an existing document | ✔ | ◐ | ✔ **documented as the good path** | ✔ | **—** |
| Deterministic structured input (e.g. Mermaid) | — | ✔ **Mermaid → shapes** | — | — | **—** |

[INFERRED] Affinity's documented guidance — *"Work with existing documents… designing from scratch
generally produces poor results"* — is the only product-level statement in the corpus about **why**
structured generation succeeds. It is consistent with tldraw's architecture (edit focused shapes) and
with Canva's template-first model. Three products, three routes to the same conclusion: **generation
into an existing structure beats generation from nothing.**

---

## 2. Output form — the decisive axis

| Product | Output | Evidence |
|---|---|---|
| **Figma** | **native document objects** — Design Agent (May 2026) operates on the document | [DOCUMENTED] |
| **tldraw** | **native shapes** — model emits a simplified `_type` format; sanitised; `convertPartialFocusedShapeToTldrawShape` | [SOURCE-CODE] ⭐⭐⭐ |
| **Affinity** | **new document objects**; scripting API is an object command surface | [DOCUMENTED] |
| **Canva** | **editable design** — Magic Layers converts a flat image into an editable layout | [DOCUMENTED] |
| **Spool** | — | `ai_inspector()` in `shell.rs` is **UI chrome with no backing behaviour** [SOURCE-CODE] |

**Nobody in the corpus outputs only pixels, and nobody outputs HTML/CSS as the primary form.**

[INFERRED] HTML/CSS appears only as a *developer-handoff* export in Figma, never as the generative
target. This is direct evidence against the brief's concern that AI might bypass the document model:
in four mature products, the document model is the output.

---

## 3. Editing

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Modify the current selection | ✔ | ✔ | ✔ | ✔ | **—** |
| Modify "everything on the page" | ✔ | ◐ | ✔ | ✔ | **—** |
| Rewrite text | ✔ | ◐ | ✔ | ✔ | **—** |
| Change colours / styles | ✔ | ◐ | ✔ | ✔ | **—** |
| Change layout / arrangement | ✔ | ◐ | ◐ | ✔ | **—** |
| Rearrange many objects at once | ✔ | ◐ | ✔ | ✔ | **—** |
| Remove an object | ✔ | ◐ | ✔ | ✔ | **—** |
| Remove a background (image) | ✔ | — | ✔ | ✔ | **—** |
| Alter an image | ✔ | — | ✔ | ✔ Magic Edit | **—** |
| Resize a design to another ratio | ✔ | — | ✔ | ✔ **Magic Resize** | **—** |
| Add an interaction / prototype | ✔ | — | — | ◐ | **—** |
| Conversational follow-up | ✔ | ✔ chat | ✔ | ✔ | **—** |
| Iterative refinement of a generated result | ✔ | ◐ | ✔ | ✔ | **—** |

---

## 4. Context

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Uses the current selection | ✔ | ✔ | ✔ | ✔ | — |
| Uses the whole document | ◐ | ◐ (viewport-bounded) | ✔ | ✔ | — |
| **Viewport-bounded context** | ? | ✔ **explicitly** | ? | ✔ | — |
| Screenshot of the canvas | ✔ | ✔ | ✔ | ✔ | **— (renderer is not offscreen-capable)** |
| Recent user actions | ✔ | ✔ **explicitly** | ? | ? | — |
| Chat history | ✔ | ✔ | ✔ | ✔ | — |
| Brand / design-system context | ✔ | — | ✔ | ✔ **Brand Kit** | **—** |
| **Named, documented context sources** | ? | ✔ **6** | ✔ (guidance) | ✔ **6** | **—** |
| Clustering of distant shapes | — | ✔ | — | — | — |
| User can see what context was sent | ? | ? | ◐ privacy toggles | ✔ | — |

[INFERRED] **Only tldraw and Canva enumerate their context sources.** That is unusual rigour, and it
suggests context assembly is a designed subsystem rather than an emergent one. Spool cannot assemble
context at all today: `rendering.md` open question 2 (can the renderer draw headlessly?) is a hard
blocker on the screenshot half.

---

## 5. Agent surface

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| MCP server | ✔ **official** | ✔ (SDK consumable) | ✔ **local MCP server** | ? | **—** |
| Per-capability approval | ? | — | ✔ **documented** | ? | — |
| Privacy toggles per capability | ? | — | ✔ | ? | — |
| Scripting API for humans | ✔ plugins | ✔ (SDK) | ✔ **JavaScript, beta** | ✔ apps | **—** |
| **Named operation vocabulary** | ? | ✔ (actions) | ✔ (scripting commands) | ? | **—** |
| Operations documented as an API | ? | ✔ | ✔ | ? | **—** |
| Agent changes undoable normally | ✔ | ✔ | ✔ | ✔ | **—** |
| Blast-radius limits | ? | — | ✔ (per-capability) | ◐ (page-bounded) | — |
| Scheduled / background execution | — | — | — | ✔ **scheduled tasks** | **—** |
| Headless invocation without a UI | ✔ | ✔ | ◐ | ✔ | **✘** |
| Event-level driver (synthetic input) | — | ✔ **`@tldraw/driver`** | ? | ? | **—** |

[INFERRED] Affinity's local MCP server with per-capability approval is the most architecturally
informative entry in this table, because to build per-capability approval it had to define *what the
capabilities are*. That definition is the operation vocabulary. Approval is downstream of naming.

---

## 6. Verification and preview

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Preview before applying | ? | ◐ | ? | ? | **—** |
| Staged / partial application | ? | ◐ **streaming** | ? | ? | — |
| Export-then-inspect verification | — | ✔ **documented** | — | — | **—** |
| Invariant checking after apply | — | — | — | — | **—** |
| Diff shown to the user | ? | ? | ? | ? | **—** |
| Undo the whole AI action | ✔ | ✔ | ✔ | ✔ | **—** |
| **Per-operation approval** | ? | — | ✔ | ? | **—** |

[INFERRED] **Nobody documents a preview, a staged application, or invariant checking.** Verification is
indirect (export and look) in tldraw and absent elsewhere. This is the largest open space in the
corpus for a general-purpose AI-native editor, and it is exactly where a persistent document with an
operation layer would pay off (`architecture/ai-runtime.md` Implication D).

---

## 7. Provenance

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Objects marked as AI-generated | ? | — | — | ? | **—** |
| Generation metadata retained | ? | — | — | ? | **—** |
| "Regenerate this" on a result | ✔ | ◐ | ✔ | ✔ | **—** |
| Prompt retained per object | ? | — | ? | ? | **—** |

[INFERRED] **Provenance is undocumented in all four.** An open-source tool whose output may be
model-generated has an obvious reason to care about this that the commercial products do not.

---

## 8. Known model-side limitations

| Product | Documented behaviour | Evidence |
|---|---|---|
| Affinity | *"Work with existing documents… designing from scratch generally produces poor results"* | [DOCUMENTED] |
| tldraw | Context is viewport-bounded; partial shapes are explicitly supported | [SOURCE-CODE] |
| Canva | Magic Layers is image → structure, not prompt → structure | [DOCUMENTED] |
| Figma | (no documented limitation found) | — |

---

## 9. What the matrix exposes

[INFERRED] Six findings:

1. **Output form is settled by the market: native document objects.** Not pixels, not HTML/CSS.
2. **Generation-into-structure beats generation-from-nothing**, according to the one product that has
   documented the question, and according to the architecture of the other three.
3. **Context assembly is a designed subsystem** in the two products that enumerate it — and it depends
   on an offscreen render, which Spool cannot currently do.
4. **No product has a preview, a staged apply, or invariant checking.** This is open space, not a
   competitive gap Spool is behind in.
5. **Approval granularity is downstream of naming operations.** Affinity built the vocabulary in order
   to build the approval UI.
6. **Provenance is unaddressed everywhere** — and is disproportionately relevant to an open-source,
   AI-native tool.

## 10. Spool readiness, stated factually

| Prerequisite | Present? | Evidence |
|---|---|---|
| A document model an operation could target | ◐ flat, no serialisation, no hierarchy | `Document` in `canvas.rs` |
| A named operation vocabulary | **✘** 12 text actions | `canvas.rs:10-26` |
| History capture boundaries a caller can choose | **✘** | `History` has `record()` only |
| Offscreen render for screenshots | **✘** | `rendering.md` |
| Any AI surface | **✘** `ai_inspector()` is chrome | `shell.rs` |
| Model-facing restricted vocabulary + sanitisation | **✘** | — |
| Undo of a whole agent run | **✘** (would work if boundaries existed) | — |

[INFERRED] Of the seven prerequisites, one is partially present and six are absent. Two of them —
offscreen render and caller-chosen capture boundaries — are **not AI features at all**; they are
general editor capabilities that happen to be AI prerequisites. That is the most useful thing in this
matrix for planning: the AI roadmap's critical path runs through rendering and history, not through
model integration.

## 11. Sources

- tldraw (SOURCE-CODE) ⭐⭐⭐: simplified `_type` shape format, sanitisation,
  `convertPartialFocusedShapeToTldrawShape`, `Streaming<T>`, six-part context assembly, Mermaid
  conversion, export-for-verification, `@tldraw/driver`
- Affinity (DOCUMENTED): Canva AI Studio, JavaScript scripting API, local MCP server with per-capability
  approval, documented "edit existing documents" guidance, affin.co/affinity-sdk
- Figma (DOCUMENTED): First Draft (2023), Make (2025), Design Agent (May 2026), MCP Server
- Canva (DOCUMENTED): Canva AI 2.0 and its six context sources, scheduled tasks, Magic Layers,
  Magic Edit, Magic Resize, Brand Kit
- Spool (SOURCE-CODE): `ai_inspector()` and `agent_suggestion()` in `app/src/shell.rs`
- Cross-references: `ai/*.md`, `architecture/ai-runtime.md`, `architecture/history.md`,
  `architecture/rendering.md`, `architecture/document-model.md`
