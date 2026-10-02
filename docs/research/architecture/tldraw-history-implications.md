# tldraw History — Architectural Implications for Spool

Companion to `docs/research/architecture/tldraw-history-source.md`.
Evidence base: tldraw `v5.5.1` / commit `0fc68fd86b819fe1c98eefe61fbe6dad217be6a3`.
Spool side: read-only inspection of `app/src/canvas.rs`.

**This document contains no decisions.** It states what tldraw's source says, what that reading implies for
Spool, and what remains a question for the humans to answer.

---

## 1. Purpose and Scope

**Interpretation.** The source investigation answered one question: *how does tldraw turn low-level store
mutations into coherent user-visible history operations, and where does the boundary responsibility live?*
This document translates that into terms Spool can use for its own Q1/Q2 decision — and stops there.

Explicitly **out of scope**: rendering, performance, profiles, layout, components, multi-product comparison,
Figma/Affinity/Canva history, and any change to Spool code.

---

## 2. How to Read This Document

Every implication is split into three parts:

- **Evidence** — what the tldraw source (or Spool's current source) actually shows, with file/symbol where
  meaningful. Source-code facts are inherited from `tldraw-history-source.md` and not re-derived here.
- **Interpretation** — what that evidence suggests for Spool. Not a decision.
- **Proposed question** — the specific thing the humans must decide, phrased so it can be answered yes/no or
  A/B.

No section in this document recommends an option. Where a trade-off looks obvious, it is still stated as a
trade-off.

---

## 3. Evidence Base

- tldraw OSS `packages/{editor,store,tldraw,valtio,state-react,tlschema}` at tag `v5.5.1`.
- `HistoryManager.ts` (432 lines), `RecordsDiff.ts` (347), `Store.ts` (1467), `Editor.ts` (11735),
  `HistoryManager.test.ts` (910) read in full.
- 120 non-test `markHistoryStoppingPoint` call sites enumerated; the interaction families
  (`Translating`, `Resizing`, `Crop`, `Drawing`, `Rotating`, `Erasing`) traced to their history calls.
- Spool: `app/src/canvas.rs` — `History` (`:425-428`), `record/undo/redo` (`:648-745`),
  `CommandOperation` (`:379-385`), `DocumentCommand` (`:388+`), `Interaction` (`:992-1030`).

**Interpretation.** The corpus's existing history material is mostly `[OBSERVED]` product-level description.
This investigation is the first `[SOURCE-CODE]`-level evidence for tldraw, and it **contradicts two existing
corpus claims** (§Important Edge Cases of the source file). Any architecture document written later should
cite the source file, not the older notes.

---

## 4. The One-Page Model

tldraw's history, in five sentences:

1. Every store mutation becomes a `RecordsDiff` tagged `'user'` or `'remote'`; a synchronous interceptor
   feeds `'user'` diffs into a **single pending buffer**.
2. A **mark** (`markHistoryStoppingPoint`) seals the pending buffer into one undo entry and pushes a `stop`
   delimiter. That is the only seal.
3. Nothing seals at gesture end. The pending buffer is counted by `getNumUndos()` and flushed by the *next*
   mark or the next `redo()`.
4. **Undo/bail/redo are folds**: reverse (or forward) N entries into one accumulator diff and apply it once,
   atomically, with `ignoreEphemeralKeys: true`.
5. **Cancel is destructive** — bail with `pushToRedoStack: false`, discarding the popped entries.

**Interpretation.** Coherence comes from two algebraic primitives (`squashRecordDiffsMutable` and
`reverseRecordsDiff`), not from any operation or boundary abstraction. Everything user-visible about
"one action = one undo step" is a *call-site convention* with 120 hand-placed call sites.

---

## 5. Implication — Boundary ownership is a call-site duty

**Evidence.** `HistoryManager` never calls `_mark` itself; `_mark` has exactly one caller,
`Editor.markHistoryStoppingPoint` (`Editor.ts:1635-1638`). 120 non-test call sites. The manager is a passive
collector behind `store.addHistoryInterceptor` (`HistoryManager.ts:45-65`).

**Interpretation.** tldraw places no layer between "who writes" and "where history breaks". This is
architecturally *honest* — there is no parallel history layer that can drift out of sync — but it means the
granularity guarantee is only as good as the discipline of every author, and there is no test or type that
can enforce it.

**Proposed question.** Does Spool accept call-site discipline as the boundary mechanism, or does it want
boundaries to be *impossible to forget*?

---

## 6. Implication — Undo is a value fold, not an inverse-closure stack

**Evidence.** `_undo` (`HistoryManager.ts:128-202`) seeds `diffToUndo` from `reverseRecordsDiff(pendingDiff)`,
then `squashRecordDiffsMutable`s each popped `diff`'s reversal into the same accumulator, then applies it once
with `applyDiff(..., { ignoreEphemeralKeys: true })`. `reverseRecordsDiff` (`RecordsDiff.ts:80-92`) copies all
three collections before swapping.

**Interpretation.** The accumulator is a plain value, and reversal is a pure transform, which is what makes
"collapse N entries into one atomic apply" trivially correct. The trade-off: history stores *diffs of diffs*,
so an entry is a set-semantics merge rather than a recorded intention. Two different action sequences that
produce the same net change produce the same undo.

**Proposed question.** Should Spool's history store *intentions* (an operation plus its inverse) or *net
diffs*? These give different undo granularity when actions overlap.

---

## 7. Implication — Cancel is destructive, not "never recorded"

**Evidence (CORRECTION).** Escape in tldraw calls `bailToMark` → `_undo({ pushToRedoStack: false, toMark })`,
which **applies a reversed diff** and **discards** the popped entries (`HistoryManager.ts:143-175`, `:198`).
The existing corpus note `docs/research/architecture/history.md` describes cancel as "not recording"; that is
wrong.

**Interpretation.** After a cancel, the pre-gesture history region is *gone from the undo spine*. Pressing
`⌘Z` after Escape undoes whatever preceded the gesture. This is a materially different user-visible behavior
from "suppress recording", and it is the direct consequence of reusing undo as the cancel mechanism.

**Proposed question.** When Spool cancels an interaction, should the pre-interaction history remain undoable
(undo moves it to redo), or should it be truncated from the spine the way tldraw does?

---

## 8. Implication — Lazy commit (nothing seals at gesture end)

**Evidence.** `flushPendingDiff` has two callers only: `redo()` (`:211`) and `_mark()` (`:304`). `getNumUndos()`
adds 1 when the pending diff is non-empty (`:78-80`). `buildFromV1Document.ts:34` shows the degenerate case:
`editor.run(...)` with **no mark** at all.

**Interpretation.** An entry's identity is assigned by the *following* boundary, not its own end. This makes
"forgot the mark" fail silently (merging with the previous action) rather than loudly. The only defence is
`getNumUndos()` counting the pending region so the UI affordance is right.

**Proposed question.** Does Spool want an explicit commit/seal at gesture end, or tldraw's lazy scheme where
the next boundary does the sealing?

---

## 9. Implication — Nested operations are just more marks

**Evidence.** Alt-clone during a drag (`Translating.ts:132-140`, mark at `:173`) splits one gesture into two
mark-delimited regions — no nested structure. `HistoryManager.test.ts:180` (5 increments, 2 marks →
`getNumUndos() === 3`). `batch` is reentrant and creates no boundary (`HistoryManager.ts:97-125`).

**Interpretation.** Nesting is expressed as *delimiter placement*, not as a hierarchy. There is no "child
transaction", no partial-rollback scope, no stack depth. Cascading writes (bindings) are aggregated only
incidentally, by running inside one `store.atomic` (`BindingUtil.ts:159-172`, `Editor.ts:634`).

**Proposed question.** Does Spool need scoped/nested transactions (e.g. an interaction that can abort
*part* of its writes), or is delimiter-only granularity sufficient for the interactions Spool supports?

---

## 10. Implication — No rollback anywhere; cancel is manual compensation

**Evidence.** `store.atomic` (`Store.ts:1233-1277`) restores flags in `finally` and has **no inverse-diff
capture and no rollback**. `HistoryManager.batch` (`:97-125`) likewise: `try { transact(fn) } catch (error) {
annotateError(error); throw }` with no state restoration.

**Interpretation.** tldraw has no transactional guarantee for a failed mutation. The only recovery mechanism
is an explicit compensating history operation placed by hand — the clearest instance being the `.catch()` on
`convert-to-bookmark` that calls `bailToMark` (`ui/context/actions.tsx:475`).

**Proposed question.** Is Spool willing to have the same property (mutations are infallible by construction,
failures are handled by explicit compensation), or does Spool want real rollback for multi-step operations?

---

## 11. Implication — Three history modes, not two

**Evidence.** `TLHistoryBatchOptions.history` is `'record' | 'record-preserve-redo-stack' | 'ignore'`
(`history-types.ts:27`). `recording` clears redo (conditionally, only if non-empty); `recording_preserving_redo_stack`
records without clearing; `paused` records nothing. `modeToState` maps `record-preserveRedoStack` →
`recording_preserving_redo_stack` (`HistoryManager.ts:340-344`).

**Interpretation.** The third mode exists because selection changes (`Editor.ts:2175`, `:2748`) must be
*recorded* (else document and history disagree) but must not *destroy redo* (else a selection change costs
the user their undo branch). The name is a string literal with no in-source rationale beyond the
`TLPage.ts:213-218` comment, which documents a *different* mechanism (ephemeral keys).

**Proposed question.** Does Spool's history need a preserve-redo mode, and if so is it a mode or a flag on
the existing record operation? And does it exist for the same reason (selection) or a different one?

---

## 12. Implication — Session state is recorded but not replayed

**Evidence (CORRECTION).** `TLPage.selectedShapeIds` has `ephemeralKeys.selectedShapeIds: false` — *not*
ephemeral (`TLPage.ts:211-230`). Only `editingShapeId`, `hintingShapeIds`, `erasingShapeIds`, `hoveredShapeId`,
`focusedGroupId` are `true`. `_undo`/`redo` apply with `{ ignoreEphemeralKeys: true }`. The existing corpus
claim that "selection is not in history" (`matrices/history-matrix.md` §4, `architecture/history.md`) is
wrong in the way that matters.

**Interpretation.** The real rule is a two-axis split: (a) *is this state recorded?* and (b) *is this state
restored on replay?* tldraw's `ephemeralKeys` answers only (b). Selection is recorded, consumed into the
pending region, and silently dropped on replay. Camera (`Editor.ts:3552-3559`, `history: 'ignore'`) is
outside both axes.

**Proposed question.** Does Spool want the two-axis model (record vs restore) or a single in/out-of-history
flag? And is Spool's selection state part of the document or part of the view?

---

## 13. Implication — View state and document state are separated by *history mode*, not by type

**Evidence.** Camera uses `history: 'ignore'`. Selection uses `record-preserve-redo-stack`. Page switching
(`setCurrentPage`, `Editor.ts:5005`) uses `record-preserve-redo-stack` and *also* places a `'change-page'`
mark (`Editor.ts:4157`). Document settings, asset stores, and user records are all `ignore`
(`Editor.ts:1985`, `:5199`, `:5227`, `:4449`).

**Interpretation.** The scope field in `tlschema` (`'document'` vs `'session'`) does **not** decide history
behaviour; the decision is made per call site by the mode string. Schema scope and history scope are two
independent axes that happen to correlate. That is a source of confusion a reviewer would hit.

**Proposed question.** In Spool, should "is this view state or document state?" be a type-level distinction
that *automatically* determines history behaviour, or a per-call decision like tldraw's?

---

## 14. Implication — Squash is available but barely used

**Evidence.** `squashToMark` (`HistoryManager.ts:268-301`) has a full algorithm (collect, reverse, squash,
reinsert at the target, leave redo untouched) and order-sensitive tests (`:435`). Its only non-test caller is
`Crop.ts:26`, guarded by `getMarkIdMatching`. A miss logs `console.error` and returns.

**Interpretation.** tldraw treats squashing as a first-class primitive it does not actually depend on. This is
useful comparative evidence: it shows the *shape* of a squash API (by mark id, not by index) without
implying that squash is essential.

**Proposed question.** Does Spool need squash at all, or is "record one command per interaction" sufficient
given a bounded interaction vocabulary?

---

## 15. Implication — Corruption is a first-class, propagated state

**Evidence.** `Store.markAsPossiblyCorrupted` / `isPossiblyCorrupted` (`Store.ts:1175-1183`), raised from
`Editor.annotateError` when `willCrashApp` (`Editor.ts:1751-1773`); `Editor.crash` sets `_crashingError` and
emits (`Editor.ts:1831-1836`); `TldrawEditor.tsx:766-770` renders `<Crash>` which rethrows; `_flushEventForTick`
early-returns when `getCrashingError()`; `TLLocalSyncClient` refuses to persist a corrupted store
(`TLLocalSyncClient.ts:390`, `:411`).

**Interpretation.** tldraw's most interesting *architectural* pattern is not history at all — it is treating
"the document may now be inconsistent" as a sticky flag that every downstream subsystem (persistence, event
loop, render) must respect. History participates by *pausing* during replay.

**Proposed question.** Does Spool have (or want) an equivalent "document may be inconsistent" signal that
persistence and the render path must respect?

---

## 16. Implication — Async has no first-class story

**Evidence.** Pattern 1 (mark then `bailToMark` in `.catch`), Pattern 2 (`editor.run` around the synchronous
portion), Pattern 3 (`history: 'ignore'` for plumbing). `mergeRemoteChanges` (`Store.ts:1037-1051`) asserts
`!_isInAtomicOp` and tags entries `'remote'` so they are never recorded locally.

**Interpretation.** tldraw assumes mutations are synchronous and small. There is no async transaction, no
compensation log, no undo token, no awaitable boundary. An async operation must remember to place a mark
before and bail after. This is the weakest part of the design and the most directly relevant to AI edits.

**Proposed question.** When Spool has an operation that can fail or await (network, AI, file I/O), what is
the *structural* mechanism — not the convention — that makes it atomic from history's point of view?

---

## 17. Implication — AI has no precedent in the OSS source

**Evidence.** Exhaustive search at `v5.5.1` finds no AI surface: no `applyAIEdit`, no AI tool state, no AI
history handling. All corpus AI material is product-level `[OBSERVED]`/`[DOCUMENTED]` on hosted tldraw.com.

**Interpretation.** This file makes no claim about tldraw's AI history. What can be said structurally:
because mutations are tagged `'user'` unless in `mergeRemoteChanges` (`Store.ts:584-593`), an AI edit issued
through `store.put` would be indistinguishable from a human edit; because there is no operation registry,
there is nowhere to attach provenance; because replay uses `ignoreEphemeralKeys`, an `is_ai_generated`
field declared ephemeral would survive in state but not replay.

**Proposed question.** Does Spool need provenance (who/why/agent) on history entries, and if so does
provenance belong on the entry or on the record?

---

## 18. Implication — The operation registry and the boundary are the same surface

**Evidence.** No `TLCommand` / `CommandOperation` / command trait exists in the OSS repo at this commit.
Actions are closures over a context `Editor` (`ui/context/actions.tsx`); marks are placed inside the same
closure, adjacent to the mutation. 120 sites.

**Interpretation.** tldraw answers Q2 negatively: there is no operation layer to own history. This is
evidence *about the question*, not an answer to it. The cost is unenforced granularity; the benefit is no
possibility of drift between "operation defined" and "history boundary declared".

**Proposed question.** For Spool, is the operation registry the thing that *declares* history boundaries (one
place), or is it a second layer alongside a separate boundary mechanism (two places, risk of drift)?

---

## 19. Spool's current position (read-only observation)

**Evidence.** `app/src/canvas.rs`:
- `History { undo: Vec<DocumentCommand>, redo: Vec<DocumentCommand> }` (`:425-428`).
- `record` (`:648-680`) drops no-op changes (`retain(|c| c.before != c.after)`) and unconditionally
  `redo.clear()`.
- `undo`/`redo` (`:687-745`) pop one command and apply `before`/`after` fields — a **single entry per undo,
  no accumulation, no marks, no squash, no bail**.
- Gestures record at commit: `history.record(geometry_command(...))` after `apply_move` (`:1910-1913`,
  `:1917-1920`) — i.e. Spool already does what tldraw's lazy scheme produces, but *eagerly and explicitly*.
- Cancel is `Interaction::restore(&self, document)` (`:1007-1019`), which sets geometry back to
  `gesture.objects` snapshots and touches **no history at all** — because nothing was recorded yet.

**Interpretation.** Spool is already closer to tldraw's *observable* behaviour than its *mechanism* suggests:
one `record` per committed interaction, no entries for cancelled gestures, undo/redo symmetric. The
difference is that Spool records *after* the fact (so a crashed gesture records nothing) while tldraw records
*during* (so a crashed gesture leaves an undoable region). Neither is "wrong"; they fail differently.

**Proposed question.** Should Spool keep eager commit (nothing recorded until the gesture succeeds) or move
toward capture-time recording (record as you mutate, and let cancel roll back)? These differ on crash and on
partial-failure behaviour.

---

## 20. Q1 — Separation of document and editor/view

**Evidence (tldraw).** Camera is `history: 'ignore'`. Selection is `record-preserve-redo-stack` and is *not*
ephemeral. Page identity (`pageId`) is `false` (restorable). The `session` scope in `tlschema` (`TLPage.ts:211`)
coexists with the history mode chosen per call site. Replay applies `{ ignoreEphemeralKeys: true }`.

**Evidence (Spool).** `Document` (`canvas.rs:431-436`) holds only `objects`, `next_id`, `next_names`,
`layer_structure_revision` — no selection, no camera. `Interaction` holds gesture snapshots. Camera and
selection are struct fields on the canvas, outside `Document`.

**Interpretation.** Both systems *behaviourally* separate document from view. tldraw achieves it with a
runtime mode string per call site plus an ephemeral-key replay filter; Spool achieves it with a type
boundary — `Document` simply has no fields for selection or camera, so history cannot accidentally capture
them. The type boundary is structurally stronger than tldraw's convention.

**Proposed question.** Confirm that Spool's `Document` stays free of selection/camera permanently (type-level
guarantee), or whether Q1 is genuinely open because some selection state (e.g. "editing shape X") may later
need to be document-scoped.

---

## 21. Q2 — Operation registry and ownership

**Evidence (tldraw).** No command type exists. Boundaries are call-site duties (120 sites); operations and
their marks are in the same closure. `batch` reentrant; no registry, no trait, no dispatch.

**Evidence (Spool).** `CommandOperation` (`canvas.rs:379-385`) is a 5-variant enum (Geometry, Style, Text,
Insert, Delete); `DocumentCommand` wraps it with private fields and constructors (`:392-424`); `record`
validates and normalizes before pushing.

**Interpretation.** Spool has taken the opposite bet from tldraw: a typed, validated, closed operation
vocabulary. This buys exhaustiveness (`match` on 5 variants in `undo`/`redo`) and normalization (no-op
filtering at `:653-673`), and costs expressiveness for anything outside the 5 variants. tldraw's diff-based
history is schema-agnostic: it works for any record type because it diffs records, not commands.

**Proposed question.** Is Spool's `CommandOperation` intended to be *closed* (exhaustive, and new features
require a new variant) or *open* (a trait/registry so new domains add implementations without touching
`undo`/`redo`)? The two have very different costs for AI and plugin-style mutation.

---

## 22. Q3 — History transaction boundaries

**Evidence (tldraw).** Boundaries are marks, placed by call sites; nothing seals at gesture end; the pending
region is counted but not stored. `store.atomic` gives reactivity batching, not history boundaries.

**Evidence (Spool).** Boundaries are the `record` call at gesture commit (`:1910-1920`, `:1556`, `:1273`,
`:1982`, `:2014`). There is no marker concept at all.

**Interpretation.** Spool's eager commit and tldraw's lazy commit produce the *same observable undo
granularity* for the interaction set Spool currently supports (move, resize, create, delete, paste, style,
text). They diverge on: (a) crash mid-gesture, (b) async partial failure, (c) any gesture that is *not*
committed through a single `record` call. tldraw's marks buy the ability to bail to a *named* point in the
past; Spool's commit-based model has no equivalent.

**Proposed question.** Does Spool need named history marks (to bail/squash to a point), or is a flat stack of
committed interactions sufficient?

---

## 23. Q4 — Operation semantics

**Evidence (tldraw).** Semantics are diff-based and schema-agnostic; three squash rules govern overlap
(added-after-removed → update; update-over-added → overwrite; remove-after-add → cancel,
`RecordsDiff.ts:279-286`, `:300-309`, `:333-335`). `reverseRecordsDiff` is a total value transform.

**Evidence (Spool).** Semantics are per-variant and explicit: `undo` replays `before`, `redo` replays
`after`, for each of 5 variants (`:687-745`). Insert/Delete are symmetric inverses by construction.

**Interpretation.** tldraw's diff algebra handles overlapping/concurrent edits automatically (squash rules
give it set semantics); Spool's command model is exact-by-construction and does not need those rules
because each command is already a before/after pair. tldraw's algebra exists *because* mutations are
captured from arbitrary call sites; Spool's does not exist because commands are constructed deliberately.

**Proposed question.** If Spool ever needs to capture mutations from arbitrary call sites (AI, scripts,
imports) rather than only from deliberate `record` calls, does its command model extend to that, or would
that require a different mechanism?

---

## 24. Q5 and Q6 — Interaction-to-history and AI-to-history

**Evidence (interaction).** tldraw interactions place a mark on `onEnter` and never seal; cancel is
`bailToMark(markId)` from `cancel()`; the safety net is `onExit` calling `bailToMark` itself when the state
is left without `complete()`/`cancel()` having run — `Resizing.ts:581-599`, citing issue #10401. Selection
changes use `record-preserve-redo-stack`; camera and editing/cropping ids are `ignore`.

**Evidence (AI).** No AI surface in the OSS repo. Structural facts only: everything not in
`mergeRemoteChanges` is tagged `'user'` (`Store.ts:584-593`); no provenance field exists; `ephemeralKeys`
would be the escape hatch for an AI marker.

**Interpretation.** tldraw's interaction→history contract is *symmetrical and minimal*: mark on enter, bail
on cancel, backstop on exit. There is no "interaction commits" step — the boundary is purely positional. For
AI, the contract is absent, which means an AI mutation path in Spool has to define its own boundary story
rather than copy one.

**Proposed question.** For AI edits in Spool: one undo step per agent action, per user-visible change, or per
agent turn? And should AI entries be visually distinguishable in the undo stack (i.e. does Spool need
provenance on the entry)?

---

## 25. What this does and does not settle

**Interpretation — what the evidence supports.**
- tldraw's boundary is a call-site convention (well evidenced, 120 sites).
- Undo/bail/redo are atomic value folds over diffs (well evidenced, tested).
- Cancel is destructive bail, not suppressed recording (corrected, well evidenced).
- Selection is recorded but not replayed; camera is fully outside (corrected, well evidenced).
- Spool's observable undo granularity already matches tldraw's for its current interaction set (observed).

**Interpretation — what it does not settle.**
- Whether Spool should move to capture-time recording or keep eager commit (a product/reliability trade-off,
  not a source question).
- Whether Q1's document/view split is permanently closed by the `Document` type (a design intent question).
- Whether Q2's operation registry is closed or open (an extensibility question).
- Anything about AI history (no precedent exists).

**Interpretation — is the evidence sufficient to begin the Spool Architecture Decision Session?**
**Partially.** The source-level facts needed to *have* the Q1/Q2 conversation are now in place, and two
existing corpus claims are corrected. But the remaining questions are *design intent* questions that no
amount of further source reading can answer — they need the humans to state what Spool is trying to be
(closed extensible core vs. schema-agnostic diff core; eager vs. capture-time history). **Recommendation:
the humans read both files together and answer Q1 and Q2 first; the Architecture Decision Session begins from
those answers, not from further agent research.**

---

## Appendix — Label key

- **Evidence** — source-code fact from the pinned tldraw tag, or read-only observation of Spool.
- **Interpretation** — my reading; not stated in either source.
- **Proposed question** — a decision for the humans; this document does not answer it.
- **CORRECTION** — a claim in the existing corpus that the source evidence contradicts (see §Important Edge
  Cases in `tldraw-history-source.md` for the full corrections).
