# tldraw History — Source Investigation

> **What this file is.** An auditable, source-first record of how tldraw's history system actually works at
> `v5.5.1`, with every architectural claim tied to a file, symbol, and line range.
>
> **What this file is not.** Spool advice. Every Spool-relevance statement lives in
> [`tldraw-history-implications.md`](tldraw-history-implications.md). This file contains none.
>
> Read with: [`history.md`](history.md) (document-level treatment), [`../profiles/tldraw.md`](../profiles/tldraw.md),
> [`../matrices/history-matrix.md`](../matrices/history-matrix.md).
>
> **Label discipline.** Every finding carries one of:
> `SOURCE-CODE FACT` (established by reading the pinned source) · `DOCUMENTED FACT` (official tldraw docs)
> · `INFERENCE` (architectural reading, not established by source) · `SPOOL IMPLICATION` (reserved for the
> companion file — deliberately absent here).

---

## Investigation Metadata

| Field | Value |
|---|---|
| **Repository** | `https://github.com/tldraw/tldraw` |
| **Version / Commit** | tag `v5.5.1` → commit `0fc68fd86b819fe1c98eefe61fbe6dad217be6a3` |
| **Commit date** | Fri Oct 2 2026 (`git log -1 --format=%ad`) |
| **Date investigated** | 2026-10-02 |
| **Clone method** | `git clone --filter=blob:none --no-checkout --depth 1 --branch v5.5.1`, sparse checkout of `packages/{editor,store,tldraw,valtio,state-react,tlschema}` |
| **Relevant packages** | `@tldraw/store` (the reactive store + diff algebra) · `@tldraw/editor` (`Editor`, `HistoryManager`, tools, state chart) · `@tldraw/tldraw` (the default UI: actions, default SelectTool, default shapes) · `@tldraw/tlschema` (record type definitions, scopes, ephemeral keys) |
| **Package versions** | `packages/editor/package.json` → `"version": "5.5.1"` |
| **Primary files** | `packages/editor/src/lib/editor/managers/HistoryManager/HistoryManager.ts` (432 lines) · `packages/store/src/lib/RecordsDiff.ts` (347) · `packages/store/src/lib/Store.ts` (1467) · `packages/editor/src/lib/editor/Editor.ts` (11735) |

**Version-mixing note.** All findings in this file come from the single commit above. Where a claim in the
existing corpus (`history.md`, `profiles/tldraw.md`) was sourced from tldraw *documentation* rather than this
source, it is re-labelled here as `DOCUMENTED FACT` and — where source contradicts it — corrected with an
explicit **CORRECTION** block. There are **three corrections**; see `§ Important Edge Cases`.

**Not inspected.** `apps/*` (the tldraw.com marketing/demo application), `apps/docs` (documentation source),
`apps/examples`. Anything about the *product* tldraw.com is therefore out of scope here; see
`§ What Is Still Unknown`.

---

## Question Map

The brief's 15 questions, and where each is answered. Sections carry `### Qn — ...` sub-headings.

| Q | Question (abbreviated) | Answered in |
|---|---|---|
| Q1 | What are the layers between a mutation and the undo stack? | `§ History Architecture` (§Q1 sub-headings), `§ Store and Mutation Model` |
| Q2 | Where does boundary responsibility live? | `§ Marks`, `§ History Stopping Points`, `§ Actions / Operation Registry` |
| Q3 | What is a mark, exactly? | `§ Marks` (`### Q3 — Definition`) |
| Q4 | How are records captured into a diff? | `§ Record / Capture`, `§ Store and Mutation Model` |
| Q5 | What does `history: 'ignore'` actually do? | `§ Ignore` (`### Q5 — ...`) |
| Q6 | What are `atomic` / `batch` / `run` for? | `§ Transactions` (`### Q6 — ...`), `§ Editor.run` |
| Q7 | How is a continuous gesture recorded? | `§ Continuous Gestures` |
| Q8 | What does cancel do? | `§ Cancellation` |
| Q9 | What happens for nested operations? | `§ Nested Operations` |
| Q10 | What is the exact undo algorithm? | `§ Undo` |
| Q11 | Is redo symmetric? | `§ Redo` |
| Q12 | How are async / failing operations handled? | `§ Async Operations` |
| Q13 | Is selection in history? What about camera? | `§ Selection and Camera` (see **CORRECTION 1**) |
| Q14 | Is there an operation registry? | `§ Actions / Operation Registry` |
| Q15 | What is the AI-to-history relationship? | `§ AI and History` |
| — | Walkthroughs | `§ Source Traces` (5 traces) |
| — | Corrections + edge cases | `§ Important Edge Cases` |
| — | Gaps | `§ What Is Still Unknown` |

---

## Executive Summary

**The single most important structural fact:**

> tldraw's `HistoryManager` is **not** an operation log and **not** a snapshot stack. It is a
> **diff accumulator sitting behind a store interceptor**, and the *only* thing that creates a user-visible
> history boundary is an explicit call to `markHistoryStoppingPoint()` — a **mark** — made by whichever
> caller (a state node, an action, a UI component) happens to be executing.

This inverts the common assumption. History boundaries in tldraw are **not** derived from operations, and
**not** owned by the editor. They are **pushed** by callers at points of their own choosing. The manager is
a passive collector.

Six further findings, each with its own section:

1. **The capture point is the store, not the editor.** The manager subscribes via
   `store.addHistoryInterceptor` and filters on `source === 'user'`
   (`HistoryManager.ts:45-65`). Any mutation that reaches the store with user provenance is history —
   including ones no author intended to be undoable.
2. **`Editor.run()` does *not* create a history boundary.** `run` sets a *capture mode* (record /
   record-preserveRedoStack / ignore) and wraps in a reactive transaction
   (`HistoryManager.batch`, `HistoryManager.ts:97-125`). The real boundary is `markHistoryStoppingPoint`,
   which is a *separate* call. The canonical operation shape is two calls, in order:
   `markHistoryStoppingPoint(name)` then `run(() => {…})` (e.g. `ui/context/actions.tsx`).
3. **Nothing is sealed at the *end* of a gesture.** Diffs accumulate in a `pendingDiff` and are flushed to
   the undo stack by the *next* mark. `getNumUndos()` counts the pending diff so the UI can still offer undo
   (`HistoryManager.ts:78-80`). A 600-px drag is one entry because there is exactly one mark at its start and
   none until the next user action.
4. **Bail is not "undo" with a flag; it is undo with `pushToRedoStack: false`, plus a mark target.**
   `bail()` = `_undo({pushToRedoStack:false})`; `bailToMark(id)` additionally unwinds to a named mark.
   Entries popped during a bail are **discarded**, not moved. If the named mark is not found, the whole call
   is a no-op and the pending diff is restored (`HistoryManager.ts:188-192`).
5. **A fourth, undocumented-in-corpus capture mechanism exists: per-record-type `ephemeralKeys`.**
   `applyDiff(..., { ignoreEphemeralKeys: true })` is used for every undo/redo, and each record type
   declares which keys history must never reapply. For `instance_page_state` this makes **selection
   undoable** while `editingShapeId`, `focusedGroupId`, `hoveredShapeId`, `hintingShapeIds` and
   `erasingShapeIds` are not (`TLPageState.ts:211-230`). This **corrects** the corpus claim that selection is
   not in history.
6. **There is no AI feature in the tldraw OSS repository at this commit.** Exhaustive grep across
   `packages/*/src` for AI/make-real/agent surfaces returns nothing. The corpus's tldraw-AI material is
   `[OBSERVED]`/`[DOCUMENTED]` from the hosted product, not from this source. See `§ AI and History`.

---

## History Architecture

### Q1 — What layers sit between a mutation and the undo stack?

**SOURCE-CODE FACT.** The full chain, with no hidden hops:

```text
caller code
  └─ Editor.run(fn, opts)          captures history mode; batches reactivity
      └─ Store.put / Store.remove  atomic; before-hooks; diff = added/updated/removed
          └─ Store.updateHistory   tags 'user' | 'remote'; calls HistoryAccumulator.add
              └─ interceptor      HistoryManager, synchronous, per mutation
                  └─ pendingDiff  one in-flight RecordsDiff
  ── markHistoryStoppingPoint(id) ──▶ _mark(): flushPendingDiff() + push {type:'stop'}
  ── undo / redo / bailToMark ─────▶ fold N entries into one diff, one applyDiff
```

**INFERENCE.** There is no transaction manager, no command bus, no operation dispatcher. The only
intermediary between a `put` and the undo stack is a **single synchronous function registered on the store**.

### Layer map

```text
caller                     boundary primitive          storage                   capture
──────────────────────────────────────────────────────────────────────────────────────────────
state node onEnter      →   markHistoryStoppingPoint()  ─┐
action onSelect         →   markHistoryStoppingPoint()   │
UI component handler    →   markHistoryStoppingPoint()   ├→ HistoryManager         store.addHistoryInterceptor
programmatic caller     →   markHistoryStoppingPoint()  ─┘   (passive collector)      ↓
                                                                ↑                RecordsDiff
caller                   suppression primitive                   │                (added/updated/removed)
──────────────────────────────────────────────────────────────────────────────────────────────
any mutation      →   editor.run(fn, {history:'ignore'})   ─┐
any mutation      →   history:'record-preserveRedoStack'     ├→ HistoryManager.state
undo/redo         →   internal Paused                        ┘
unapply/applyDiff →   ephemeralKeys on record types
```

**SOURCE-CODE FACT.** The only registration of the manager into the store happens once, in its constructor
(`HistoryManager.ts:45-65`), which returns `this.store.addHistoryInterceptor(...)` and is stored as the
publicly-disposable `dispose`. `Editor` constructs it at `Editor.ts:387-392` and registers
`this.history.dispose` among `this.disposables` (`Editor.ts:925`).

### The two stacks and the pending diff

**SOURCE-CODE FACT.** `HistoryManager` holds:

| Field | Type | Line | Role |
|---|---|---|---|
| `state` | `HistoryRecorderState` | `:27` | capture mode: `Recording` \| `RecordingPreserveRedoStack` \| `Paused` |
| `pendingDiff` | `PendingDiff<R>` | `:28` | the not-yet-sealed accumulator |
| `stacks` | `atom('HistoryManager.stacks', {undos, redos})` | `:29-38` | two persistent stacks |

**SOURCE-CODE FACT.** The stacks are **persistent immutable linked lists**, not arrays
(`HistoryManager.ts:390-420`): `StackItem` holds `head` and `tail`, `EmptyStackItem` is a singleton whose
`tail` is itself and whose `length` is 0. `push` allocates a new node and never mutates. `stackToArray` is a
debug/telemetry helper only.

**INFERENCE.** The persistent-list representation exists so that pushing a mark or flushing a diff is O(1)
and produces a referentially new value that the enclosing `atom`'s `isEqual` (identity comparison on
`undos`/`redos`) can see. This is a reactivity requirement, not a history requirement — a Rust
implementation would use a `Vec` and get identical semantics.

### Entry types

**SOURCE-CODE FACT** (`packages/editor/src/lib/editor/types/history-types.ts:1-27`):

```ts
TLHistoryMark = { type: 'stop', id: string }
TLHistoryDiff = { type: 'diff', diff: RecordsDiff<R> }
TLHistoryEntry = TLHistoryMark | TLHistoryDiff
TLHistoryBatchOptions = { history?: 'record' | 'record-preserveRedoStack' | 'ignore' }
```

**INFERENCE.** A history entry is a union of *boundary marker* and *payload*. Both live on the same stack.
That single-stack-of-two-kinds is what makes `bailToMark` expressible as "pop until you see this marker".

### Ownership

**SOURCE-CODE FACT.** The manager is `protected readonly history: HistoryManager<TLRecord>` on `Editor`
(`Editor.ts:1546`). It is constructed with the store (`Editor.ts:387`), and its public API is re-exposed
verbatim through thin `Editor` methods:

| `Editor` method | Line | Delegates to |
|---|---|---|
| `undo()` | `:1558-1564` | `_flushEventsForTick(0)` → `complete()` → `history.undo()` |
| `redo()` | `:1589-1595` | `_flushEventsForTick(0)` → `complete()` → `history.redo()` |
| `markHistoryStoppingPoint(name?)` | `:1635-1638` | `history._mark(id)` where `id = "[name]_uniqueId()"` |
| `squashToMark(markId)` | `:1671-1674` | `history.squashToMark` |
| `bail()` | `:1686-1689` | `history.bail` |
| `bailToMark(id)` | `:1703-1706` | `history.bailToMark` |
| `run(fn, opts)` | `:1736-1748` | `history.batch(fn, opts)` |
| `canUndo` / `canRedo` | `:1570`, `:1601` | `@computed` over `getNumUndos()` / `getNumRedos()` |
| `clearHistory()` | `:1609` | `history.clear()` |

**SOURCE-CODE FACT — `undo()` is gated on interaction completion.** Both `undo()` and `redo()` call
`this._flushEventsForTick(0)` and then `this.complete()` **before** touching the manager
(`Editor.ts:1559-1561`, `:1590-1592`). `complete()` dispatches a `{type:'misc', name:'complete'}` event
(`Editor.ts:10273-10276`), which propagates through the state chart and, in a live gesture, runs the current
node's `onComplete` (e.g. `Translating.onComplete → this.complete()`, `Translating.ts:156-158`).

**INFERENCE.** You cannot undo mid-gesture in tldraw; pressing undo *ends* the gesture first (committing it)
and then undoes it. This is the same answer Figma gives, reached by a different route. It is a deliberate
simplification that removes the hard case entirely.

---

## Core Types

| Type | File | Shape | Notes |
|---|---|---|---|
| `RecordsDiff<R>` | `store/RecordsDiff.ts:29-36` | `{ added: Record<Id, R>; updated: Record<Id, [from: R, to: R]>; removed: Record<Id, R> }` | the unit of history |
| `reverseRecordsDiff(d)` | `RecordsDiff.ts:80-92` | swaps `added`↔`removed`, swaps each `[from,to]` tuple | copies the collections rather than aliasing, because callers squash into the result |
| `isRecordsDiffEmpty(d)` | `RecordsDiff.ts:115-117` | `!hasAnyKey(added) && !hasAnyKey(updated) && !hasAnyKey(removed)` | `hasAnyKey` is `for (const _ in obj) return true`, chosen over `Object.keys().length` for hot-path cost |
| `squashRecordDiffs(diffs, opts)` | `RecordsDiff.ts:163-186` | folds N diffs into 1 | non-mutating unless `mutateFirstDiff` |
| `squashRecordDiffsMutable(target, diffs, fromIndex)` | `RecordsDiff.ts:228-347` | in-place fold; owns `target`'s `[from,to]` tuples | the workhorse; called by `PendingDiff.apply`, `_undo`, `redo`, `squashToMark` |
| `squashRecordDiffsMutableByType(...)` | `RecordsDiff.ts:242-251` | type-filtered variant returning net size change | used by spatial/index bookkeeping, not by history |
| `HistoryEntry<R>` | `store/Store.ts:104-109` | `{ changes: RecordsDiff<R>; source: ChangeSource }` | the interceptor's payload |
| `ChangeSource` | `store/Store.ts:63` | `'user' \| 'remote'` | the provenance filter |
| `TLHistoryEntry` | `editor/types/history-types.ts:16` | `TLHistoryMark \| TLHistoryDiff` | what lives on the stacks |
| `TLHistoryBatchOptions` | `editor/types/history-types.ts:19-27` | `{ history?: 'record' \| 'record-preserveRedoStack' \| 'ignore' }` | the capture-mode enum, as a caller-facing type |

### The squash algebra

**SOURCE-CODE FACT.** `squashRecordDiffsMutableImpl` (`RecordsDiff.ts:253-347`) implements three rules:

1. *Added after removed in the same fold* ⇒ becomes an `updated` pair `[original, value]`
   (`:279-286`), because a record cannot be both.
2. *Updated for a record already `added`* ⇒ the added value is overwritten with the latest and the `updated`
   entry deleted (`:300-309`).
3. *Removed for a record that was `added` in the same fold* ⇒ both entries are dropped entirely
   (`:333-335`).

**SOURCE-CODE FACT.** Rule 3 is what makes create-then-delete within one boundary vanish from history.
Spool's `record()` no-op filter (`architecture/history.md`) achieves the same outcome at a different layer.

**INFERENCE.** The `[from, to]` tuple design is the minimum sufficient information to undo: `from` is the
complete previous record value, `to` is the complete new value. Undo of an `updated` entry is therefore
"write `from` back" — no field-level inverse logic, no dependency on which fields the mutation touched. Cost
is O(record size) per entry rather than O(changed fields).

**SOURCE-CODE FACT.** The implementation mutates `target.updated[id][1]` in place and documents that the
target must exclusively own its tuples (`RecordsDiff.ts:266-268`, `:320-321`). `reverseRecordsDiff` copies
its collections for the same reason (`RecordsDiff.ts:81-82`).

**INFERENCE.** This aliasing discipline is a direct consequence of running on a hot path where the same
diff object may be reachable from more than one stack entry. A Rust implementation with owned values would
inherit the semantics and lose the hazard entirely.

---

## Store and Mutation Model

### The mutation path

**SOURCE-CODE FACT.** Every mutation funnels through `Store.put(records)` (`Store.ts:617-690`) or
`Store.remove(ids)` (`Store.ts:710-741`). Both:

1. run inside `this.atomic(...)`;
2. run `sideEffects.handleBeforeChange` / `handleBeforeCreate` / `handleBeforeDelete` first
   (`:640`, `:659`, `:719`);
3. skip records whose validated value is `=== initialValue` (`:648-649`), and skip entirely if `!didChange`
   (`:683-684`);
4. call `this.updateHistory({added, updated, removed})` once (`:686-689`, `:735`).

**SOURCE-CODE FACT.** `updateHistory` (`Store.ts:584-593`) computes
`source: this.isMergingRemoteChanges ? 'remote' : 'user'` and calls `historyAccumulator.add({changes, source})`.

**SOURCE-CODE FACT.** `HistoryAccumulator.add` (`Store.ts:1363-1368`) pushes to `_history` **and immediately
invokes every registered interceptor synchronously**. So the manager's interceptor runs once per `put`/
`remove`/`applyDiff`, in-line, not batched.

**CORRECTION to a plausible assumption.** The *store listeners* are frame-batched — `historyReactor` is
constructed with `scheduleEffect: (cb) => (this.cancelHistoryReactor = throttleToNextFrame(cb))`
(`Store.ts:495-505`), and `_flushHistory` (`:516-553`) delivers squashed entries to `this.listeners`. But the
**history interceptor is not frame-batched**. History capture is synchronous and per-mutation; only the
public listener API is throttled.

### `Store.atomic` — batching, not rollback

**SOURCE-CODE FACT.** `Store.atomic(fn, runCallbacks = true, isMergingRemoteChanges = false)`
(`Store.ts:1233-1277`):

- reentrant (`_isInAtomicOp` guard at `:1234-1248`);
- sets `pendingAfterEvents = new Map()` and defers all after-hooks to `flushAtomicCallbacks`;
- on the way out calls `flushAtomicCallbacks(isMergingRemoteChanges)` inside a `try`, with a `finally` that
  clears `pendingAfterEvents`, restores `sideEffects.isEnabled`, `_isInAtomicOp`, and
  `isMergingRemoteChanges`.

**SOURCE-CODE FACT — there is no rollback.** Nothing in `atomic` restores `this.records` if `fn` throws. The
`finally` block restores *flags*, not *data*. Confirmed by `HistoryManager.test.ts:831-843`
("should maintain batch state correctly during errors"), which asserts only that a subsequent batch works, not
that the store was restored. See `§ Error / interruption semantics` below for what mitigates this.

**SOURCE-CODE FACT.** `flushAtomicCallbacks` (`Store.ts:1199-1230`) loops over after-events in waves, up to
`updateDepth > 100`, beyond which it `throw new Error('Maximum store update depth exceeded, bailing out')`.
Writes performed by after-handlers generate further after-events and are processed in the next wave.

**SOURCE-CODE FACT.** `HistoryManager.batch` is a *different* thing with a confusingly similar name: it
wraps `transact()` from `@tldraw/state` (the reactive graph transaction) and sets capture state. It touches
`Store.atomic` only indirectly.

### Remote / collaboration source

**SOURCE-CODE FACT.** `Store.mergeRemoteChanges(fn)` (`Store.ts:1037-1051`) runs `atomic(fn, true, true)`,
which flips `isMergingRemoteChanges` so every `put`/`remove` inside reports `source: 'remote'`. It asserts
`!this._isInAtomicOp` ("Cannot merge remote changes while in atomic operation"). Afterwards it calls
`ensureStoreIsUsable()`.

**SOURCE-CODE FACT.** `addHistoryInterceptor` (`Store.ts:1280-1285`) re-reads
`this.isMergingRemoteChanges` **at delivery time** rather than trusting the entry's stored source:

```ts
addHistoryInterceptor(fn) {
  return this.historyAccumulator.addInterceptor((entry) =>
    fn(entry, this.isMergingRemoteChanges ? 'remote' : 'user'))
}
```

**SOURCE-CODE FACT.** The manager's interceptor begins `if (source !== 'user') return`
(`HistoryManager.ts:46`). Remote changes therefore never enter `pendingDiff` and never clear the redo stack.

**INFERENCE.** `ChangeSource` is a provenance tag on the *write*, not on the *operation*. That is a weaker
and more useful guarantee than "the sync layer filters": any writer that forgets to tag itself as remote is
silently recorded as undoable. The design places the burden of correct tagging on every call site.

### `applyDiff`

**SOURCE-CODE FACT.** `Store.applyDiff(diff, { runCallbacks = true, ignoreEphemeralKeys = false })`
(`Store.ts:1067-1112`) converts a diff into a `put` + `remove` inside one `atomic`. When
`ignoreEphemeralKeys` is true and the target type has a non-empty `type.ephemeralKeySet`, it computes, for
each `updated` entry, a merged record that starts from `existing`, copies over only the **non-ephemeral**
keys present in `to`, and deletes any key present in `existing` but absent from `to` (also skipping
ephemeral keys). If nothing differs, the entry is dropped.

**SOURCE-CODE FACT.** Undo and redo both call it with `ignoreEphemeralKeys: true`
(`HistoryManager.ts:193`, `:243`).

**SOURCE-CODE FACT — `ephemeralKeys` is declared per record type.**
`createRecordType` accepts `ephemeralKeys?: { [K in keyof R]: boolean }` and builds `ephemeralKeySet` from
the `true` entries (`store/RecordType.ts:56`, `:62`, `:103-111`).

**SOURCE-CODE FACT — the tldraw instance-page-state record declares:**
`pageId:false, selectedShapeIds:false, editingShapeId:true, croppingShapeId:false, meta:false,
hintingShapeIds:true, erasingShapeIds:true, hoveredShapeId:true, focusedGroupId:true`
(`tlschema/records/TLPageState.ts:211-230`). Its `scope` is `'session'` (`:211`); `TLShape`, `TLPage`,
`TLAsset`, `TLBinding` are `'document'` (`TLShape.ts:568`, `TLPage.ts:147`, `TLAsset.ts:210`,
`TLBinding.ts:421`).

**SOURCE-CODE FACT — the in-source rationale for `editingShapeId`:**
> "editingShapeId is set with `history: 'ignore'`, so entering the editing state is never undoable. Marking
> it ephemeral keeps undo/redo from reapplying a stale editingShapeId (e.g. after a shape it pointed at was
> deleted), which could leave the editor pointing at a missing shape."
> — `TLPageState.ts:213-218`

**INFERENCE.** `ephemeralKeys` is not a history-capture mechanism; it is a **replay-safety** mechanism. It
answers "which fields of this record type are meaningless to write from history?" — editing pointer, hover,
hint set, focus scope. It is orthogonal to, and layered on top of, `record`/`ignore`, which answer "should
this write be captured at all?" The existing corpus has no record of it.

---

## Marks

### Q3 — Definition: what is a mark, exactly?

**SOURCE-CODE FACT.** `Editor.markHistoryStoppingPoint(name?)` (`Editor.ts:1635-1638`):

```ts
const id = `[${name ?? 'stop'}]_${uniqueId()}`
this.history._mark(id)
return id
```

`name` is **not** used for lookup — only for debugging. Lookup is by exact `id` string, or by substring via
the internal `getMarkIdMatching` (`HistoryManager.ts:317-326`), used only for a documented legacy
compatibility path in `Translating` (`Translating.ts:77-83`).

**SOURCE-CODE FACT.** `HistoryManager._mark` (`HistoryManager.ts:303-309`):

```ts
_mark(id: string) {
  transact(() => {
    this.flushPendingDiff()
    this.stacks.update(({ undos, redos }) => ({ undos: undos.push({ type: 'stop', id }), redos }))
  })
}
```

So a mark does exactly two things: **seal the pending diff** (pushing it as a `{type:'diff'}` entry below the
new mark), and **push a `{type:'stop'}` entry** above it.

**SOURCE-CODE FACT — creating a mark does *not* clear the redo stack.** `_mark` returns `redos` unchanged
(`:307`). Redo is cleared only by the `Recording` branch of the interceptor, and only when
`this.stacks.get().redos.length > 0` (`:49-56`). `HistoryManager.test.ts:148-178`
("clears the redo stack if you execute commands, but not if you mark stopping points") pins this exactly.

### Answering "what is a mark"

| Candidate | Verdict | Evidence |
|---|---|---|
| A cursor | **No** — it is a value pushed *onto* the same stack as payloads | `_mark` pushes `{type:'stop', id}` at `:307` |
| A store revision | **No** — no store version is read or stored | `_mark` touches only `pendingDiff` and `stacks` |
| A stack position | **No** — it is not positional; it is an *entry* with an identity | `:307` |
| A snapshot | **No** — no document value is captured | `:303-309` |
| A diff boundary | **Yes, precisely** | it terminates the accumulation region below it |

**INFERENCE.** A mark is a **sentinel entry on the undo stack that closes the currently-open accumulation
region**. Everything below it (diffs since the previous mark) is one undoable unit; everything above it
begins a new one.

### Marks and nesting

**SOURCE-CODE FACT.** `markHistoryStoppingPoint` is not re-entrancy guarded. Calling it inside another mark
produces two consecutive `stop` entries, both on the undo stack.

**SOURCE-CODE FACT.** `squashToMark` (`HistoryManager.ts:268-301`) walks *down* from `undos.head` collecting
`diff` entries into `popped` until it reaches the named `stop`, discards **every intermediate `stop`** it
passed, and pushes one combined diff back above the target mark. So nesting is resolved by squash, not by
mark semantics: the outer mark wins, the inner ones disappear.

**SOURCE-CODE FACT.** `_undo` with `toMark` also walks past intermediate `stop` entries without stopping
(`:175-181`): a `stop` that is not the target is simply `break`-less-skipped.

**INFERENCE.** Nested marks cost nothing at capture time and are resolved lazily. That is what makes the
`Crop` pattern (§ Squash) work: the parent marks once, children mark per gesture, and the parent squashes on
exit.

### `mark A → mutation X → mutation Y → mark B`

**SOURCE-CODE FACT.** Trace it through the code:

```text
mark A
  _mark('A')            flushPendingDiff()  → pendingDiff empty ⇒ no-op
                        undos.push(stop 'A')
  undos = [stop A]                          redos = []

mutation X               store.put → interceptor (Recording) → pendingDiff.apply(X)
  pendingDiff = X       undos = [stop A]                        redos = []

mutation Y               store.put → interceptor (Recording) → pendingDiff.apply(Y)
  pendingDiff = squash(X, Y)
  undos = [stop A]                          redos = []

mark B
  _mark('B')            flushPendingDiff() ⇒ pushes {diff: squash(X,Y)}
                        undos.push(stop 'B')
  undos = [stop A, diff squash(X,Y), stop B]   redos = []
```

Consequences:

- `getNumUndos()` = `undos.length (3) + (pendingDiff empty ? 0 : 1) (0)` = 3
  (`HistoryManager.ts:78-80`). Note the count is **entries**, not user actions — a mark and its diff both
  count. `HistoryManager.test.ts:180-194` ("allows squashing of commands") pins this: 5 increments across 2
  marks report `getNumUndos() === 3`.
- `undo()` with empty pending diff: the `isPendingDiffEmpty` branch (`:147-160`) pops the `stop B` off
  `undos` onto `redos`, then the loop (`:163-185`) pops `diff squash(X,Y)` (squashing its reversal into
  `diffToUndo`), then hits `stop A` and `break loop` (`:176`), leaving `stop A` on `undos`. `applyDiff`
  writes `X.from` and `Y.from` back.
- One `undo()` reverts X **and** Y together. They are never independently undoable, because no mark separated
  them.

---

## Record / Capture

**SOURCE-CODE FACT.** The interceptor body (`HistoryManager.ts:45-65`), verbatim in behaviour:

| `state` | Action on a user-source change |
|---|---|
| `Recording` | `pendingDiff.apply(changes)`; then, **only if** `redos.length > 0`, reset `redos` to an empty stack |
| `RecordingPreserveRedoStack` | `pendingDiff.apply(changes)` |
| `Paused` | nothing |
| anything else | `exhaustiveSwitchError(this.state)` |

**SOURCE-CODE FACT.** `modeToState` (`HistoryManager.ts:340-344`) maps
`record → Recording`, `record-preserveRedoStack → RecordingPreserveRedoStack`, `ignore → Paused`.

**SOURCE-CODE FACT.** `PendingDiff.apply` (`HistoryManager.ts:366-378`) squashes into the accumulator and
then **conditionally recomputes emptiness**: a full `isRecordsDiffEmpty` recompute only if the incoming diff
added or removed records; otherwise emptiness is updated only if the accumulator was previously non-empty.
The comment explains this is because `for…in` over a dictionary-mode object pays an O(N) key-collection
prologue and this runs on every input tick while resizing many shapes.

**SOURCE-CODE FACT.** `flushPendingDiff` (`HistoryManager.ts:68-76`) is a no-op on an empty diff. Otherwise it
clears the accumulator and pushes `{type:'diff', diff}` onto `undos`, **preserving `redos`**.

**INFERENCE.** The three states are not three *policies*; two of them (`Recording` vs
`RecordingPreserveRedoStack`) differ in exactly one side effect — whether the redo stack is invalidated. That
is the entire reason the `preserveRedo` variant exists, and it is the single most commonly-missed detail in
hand-rolled undo implementations.

### Caller inventory (marks)

**SOURCE-CODE FACT.** 120 non-test call sites of `markHistoryStoppingPoint` exist across
`packages/editor/src` and `packages/tldraw/src`. Representative, by category:

| Category | Examples (file → mark name) |
|---|---|
| Tool gesture entry | `SelectTool/childStates/Translating.ts:85` `'translating'`; `Resizing.ts:95` `'starting resizing'`; `Crop/Crop.ts:19` `'crop'`; `shapes/draw/toolStates/Drawing.ts:212` `'draw start'`; `BaseBoxShapeTool/children/Pointing.ts:22,95` `` `creating_box:${id}` `` |
| Gesture mid-flight mode switch | `Translating.ts:173` `'translate cloning'`; `Translating.ts:191` `'translate'` |
| Selection | `PointingShape.ts:56,97,106` `'selecting shape'` / `'shape on click'` / `'clearing shape ids'`; `Idle.ts:547` `'selecting shape'`; `Idle.ts:562` `'clearing selection'` |
| Keyboard nudge | `Idle.ts:789` `'nudge shapes'` (guarded by `if (!ephemeral)`) |
| Style / property panel | `StylePanel/StylePanelContext.tsx`; `Toolbar/DefaultImageToolbarContent.tsx` `'aspect ratio'`; `AltTextEditor.tsx` `'set alt text'` |
| Menu / command | `ui/context/actions.tsx` — `'resize shapes'`, `'group'`, `'ungroup'`, `'delete'`, `'paste'`, `'bring to front'`, `'bring forward'`, `'send backward'`, `'send to back'`, `'duplicate shapes'`, `'locking'`, `'unlock all'`, `'flattening to image'`, `'move_shapes_to_page'`, `'frame-selection'`, … |
| Structural UI | `PageMenu/*.tsx` `'creating page'`, `'change-page'`, `'rename page'`, `'deleting page'`; `Editor.ts:4157` `'change-page'` |
| Drop / paste (async) | `editor/hooks/useCanvasEvents.ts:197,208` `'drop'`; `ui/hooks/useClipboardEvents.ts:171,183,195,377,569,582,617,908` `'paste'`; `:749` `'cut'` |

**INFERENCE.** Marks are placed by **every kind of entry point** — gesture states, menu items, style-panel
controls, page renames, async paste. There is no privileged subset. This is the practical answer to Q13: the
semantic operation surface and the history boundary are the *same* surface, because actions are flat records
whose `onSelect` calls `markHistoryStoppingPoint` and then mutates.

---

## Ignore

### Q5 — What does `history: 'ignore'` actually do?

**SOURCE-CODE FACT.** `history: 'ignore'` maps to `HistoryRecorderState.Paused`
(`HistoryManager.ts:340-344`), whose interceptor branch is an empty `break` (`:60-61`). Mutations still reach
the store; they simply do not enter `pendingDiff`, and therefore do not clear the redo stack.

**SOURCE-CODE FACT — precedence rule.** `batch` (`HistoryManager.ts:97-125`):

```ts
const previousState = this.state
if (previousState !== HistoryRecorderState.Paused && opts?.history) {
  this.state = modeToState[opts.history]
}
```

A nested `run` **cannot un-pause** an already-paused recorder. `HistoryManager.test.ts:399-433`
("nested ignore") pins both directions: inside `history:'ignore'`, a nested `{history:'record'}` is still
ignored; inside `record-preserveRedoStack`, a nested `{history:'ignore'}` *is* honoured for its own writes.

**SOURCE-CODE FACT — nesting does not create a new batch.** `batch` (`:105-124`): if `_isInBatch` is already
true it calls `transact(fn)` and returns immediately, without toggling `_isInBatch`. The outer `finally`
restores `previousState`. `HistoryManager.test.ts:815-829` ("should handle nested batches correctly") asserts
the inner callback runs (callCount 2) but does not nest the flag.

**SOURCE-CODE FACT — `ignore` sites in `Editor.ts`, by intent:**

| Line | Call | Why (from source context) |
|---|---|---|
| `:920-928` | constructor cleanup of `editingShapeId`/`hoveredShapeId`/`erasingShapeIds` | not user edits |
| `:1980-1988` | `updateDocumentSettings` | settings are not artwork |
| `:2862-2865` | `setEditingShape` | entering/leaving editing is not an undoable act |
| `:3091-3103` | `setCroppingShape` | same |
| `:3552-3559` | `_setCamera` | camera is view state |
| `:4447-4453` | `_ensureUserRecord` | presence record upkeep |
| `:4785` | `_setCameraState` | view state |
| `:5199`, `:5227` | `store.put(assets)` / `updateAssets` | asset bookkeeping, not artwork |
| `InputsManager.ts:547,686` | pointer/hover bookkeeping | transient |
| `TldrawEditor.tsx:835,845` | unmount cleanup | teardown |
| `bookmarks.ts:265`, `EmbedShapeUtil.tsx:162` | background URL→embed upgrade | derived write |

**SOURCE-CODE FACT.** `TLComment` carries a source-level instruction rather than an enforcement:
> "Comment mutations are deliberately not undoable — create/edit them with `{ history: 'ignore' }` to avoid
> multiplayer surprises like a comment reappearing after someone else deleted it."
> — `tlschema/records/TLComment.ts:79-82`

**INFERENCE.** `ignore` is a *disciplined convention* enforced by nothing but review. A new shape type, an
agent, or a plugin can silently produce undoable writes. The `ChangeSource` filter is the only hard gate, and
it is keyed on provenance, not on intent.

---

## Bail

**SOURCE-CODE FACT.** `bail()` = `_undo({ pushToRedoStack: false })` (`HistoryManager.ts:254-258`).
`bailToMark(id)` = `_undo({ pushToRedoStack: false, toMark: id })` (`:260-266`), guarded by `if (id)`.

**SOURCE-CODE FACT — what `_undo` does, step by step** (`HistoryManager.ts:128-202`):

1. Save `previousState` and `previousIsReplaying`; set `state = Paused`, `_isReplaying = true` (`:129-132`).
2. `pendingDiff = this.pendingDiff.clear()`; `diffToUndo = reverseRecordsDiff(pendingDiff)` (`:138-140`).
3. If `pushToRedoStack && pendingDiff non-empty` → push `{type:'diff', diff: pendingDiff}` onto `redos`
   (`:142-144`). **In bail this is skipped.**
4. If `pendingDiff` was empty, pop leading `stop` entries off `undos`, pushing each onto `redos` only if
   `pushToRedoStack`; stop early if a popped mark's id equals `toMark` (`:147-160`).
5. Otherwise enter the main loop (`:163-185`): pop `undos.head`; push onto `redos` only if
   `pushToRedoStack`; `case 'diff'` → `squashRecordDiffsMutable(diffToUndo, [reverseRecordsDiff(undo.diff)])`;
   `case 'stop'` → `if (!toMark) break loop`, else continue until `undo.id === toMark`.
6. If `toMark` was given and not found → `this.pendingDiff.restore(pendingDiff)` and **return immediately**
   (`:188-192`). Crucially `this.stacks.set(...)` has *not* been called yet, so the mutated local `undos`/
   `redos` variables are discarded and **the stacks are completely unchanged**.
7. Otherwise `store.applyDiff(diffToUndo, {ignoreEphemeralKeys: true})`, then
   `store.ensureStoreIsUsable()`, then `this.stacks.set({undos, redos})` (`:193-195`).
8. `finally`: restore `_isReplaying` and `state` (`:196-199`).

### Answers to "what state existed / what happens"

| Question | Answer | Evidence |
|---|---|---|
| Does cancellation create a history entry? | **No.** Popped entries are discarded; the pending diff is applied reversed and never pushed anywhere. | `:142-144` skipped when `pushToRedoStack === false` |
| Does it revert the store? | **Yes**, via `applyDiff(reverseRecordsDiff(…))` in one atomic block. | `:140`, `:193` |
| Does it create an inverse? | Yes, transiently — `reverseRecordsDiff` is computed then applied; the inverse is not stored. | `:140` |
| Does it prevent the history stack from changing? | **It changes it, but only by discarding.** Entries between the start point and now are popped off `undos` and dropped. The mark itself is popped too. | `:165` `undos = undos.tail` unconditional |
| What happens to redo? | **Untouched.** No push, no clear. The `Recording` branch that clears redo lives in the interceptor, which is `Paused` throughout. | `:142`; state set at `:131` |
| Do nested marks survive? | **No.** All `stop` entries between the bail target and the head are popped and discarded. | `:171-181` |
| Are side effects reverted? | **Yes, as a consequence.** `applyDiff` goes through `put`/`remove`, which fire before/after hooks; the revert is an ordinary mutation from the store's point of view. The revert itself is *not* recorded (`Paused`). | `:193`; `Store.ts:617-741` |
| Are selection / camera / editor state involved? | **Only as ordinary store records, subject to `ephemeralKeys`.** `selectedShapeIds` is replayable; `editingShapeId`, `hoveredShapeId`, `focusedGroupId` are not. | `TLPageState.ts:211-230` |

**SOURCE-CODE FACT.** `HistoryManager.test.ts:242-256` ("does not allow new history entries to be pushed if a
command invokes them while bailing") demonstrates the visible effect: after two marks and two batches,
`bail()` → count 6 → 2, count back to 2; `bailToMark('0')` → count 0, value 0.

**SOURCE-CODE FACT.** `HistoryManager.test.ts:258-271` ("supports bailing to a particular mark") shows a
multi-step undo across several marks in one call.

**SOURCE-CODE FACT.** `HistoryManager.test.ts:760-796` pins the miss path: `bailToMark('non-existent-mark')`
leaves the store value unchanged, preserves `getNumUndos() > 0`, and a subsequent `bailToMark('real-mark')`
still works.

**INFERENCE.** The nuance the brief asks about — bail is more than "undo the transaction" — is that **bail is
also a stack truncation**, not a stack append. A bailed gesture leaves no trace *and* leaves the redo stack
intact, which is exactly what makes "press Escape, then ⇧⌘Z" behave the way users expect.

---

## Squash

**SOURCE-CODE FACT.** `squashToMark(id)` (`HistoryManager.ts:268-301`):

```ts
let top = this.stacks.get().undos
const popped: Array<RecordsDiff<R>> = []
while (top.head && !(top.head.type === 'stop' && top.head.id === id)) {
    if (top.head.type === 'diff') popped.push(top.head.diff)
    top = top.tail
}
if (!top.head || top.head?.id !== id) { console.error('Could not find mark to squash to: ', id); return this }
if (popped.length === 0) return this
const diff = createEmptyRecordsDiff<R>()
squashRecordDiffsMutable(diff, popped.reverse())
this.stacks.update(({ redos }) => ({ undos: top.push({ type: 'diff', diff }), redos }))
```

Answering the brief's questions directly from that:

| Question | Answer |
|---|---|
| Why it exists | To retroactively collapse a range of already-sealed entries into one, without touching the document. |
| What it receives | A mark id; it walks `undos` **only** — `redos` is destructured and re-emitted unchanged. |
| What it combines | Every `diff` entry strictly above the target mark. Intermediate `stop` entries are **dropped, not converted**. |
| How two entries are merged | `popped.reverse()` then `squashRecordDiffsMutable` — the algebra of § Core Types. |
| Does the result contain a new snapshot/diff? | A **new diff**, squashed. Not a snapshot. |
| Does ordering matter? | **Yes.** `popped` is collected head-first (newest → oldest) and must be reversed before squashing, or `from`/`to` tuples would be applied backwards and the merged diff would be wrong. |
| Does redo stay correct? | Yes, because `redos` is untouched and the merge is a pure fold of already-valid forward diffs. |
| Is it used in real interactions? | **Yes — exactly one call site in the whole repository.** |

**SOURCE-CODE FACT — the one caller** is `packages/tldraw/src/lib/tools/SelectTool/childStates/Crop/Crop.ts`:

```ts
override onEnter() { this.markId = this.editor.markHistoryStoppingPoint('crop') }          // :19
override onExit() {
    if (!this.didExit) {
        this.didExit = true
        if (this.editor.getMarkIdMatching(this.markId) === this.markId) {                 // :25
            this.editor.squashToMark(this.markId)                                        // :26
        }
    }
}
override onCancel() {
    if (this.getCurrent()?.id !== 'idle') return                                          // :34
    if (!this.didExit) { this.didExit = true; this.editor.bailToMark(this.markId) }      // :35-38
}
```

**What problem the caller solves.** During a crop session, each handle drag makes its **own** mark —
`Cropping.ts:47` `'cropping'`, `TranslatingCrop.ts:38` `'translating_crop'`,
`Crop/children/Idle.ts:244` `'translate crop'`. So *during* the session each adjustment is individually
undoable. On exit, the parent collapses the whole session into one entry above the `'crop'` mark, so *after*
the session a single undo leaves crop mode entirely. The `getMarkIdMatching` guard exists because an earlier
`bailToMark` (from `Cropping.cancel`, `Cropping.ts:243`) may already have consumed the mark.

**SOURCE-CODE FACT.** `Crop.onCancel` demonstrates the state-chart interplay: "Parents handle events before
children, so a cancel during a child's drag would unwind the whole session here before the child bails to
its own mark. Only idle's escape ends the session; mid-drag children revert just their own change and return
to idle." — `Crop.ts:31-33`.

**SOURCE-CODE FACT.** `HistoryManager.test.ts:435-475` ("squashToMark works") is the canonical
order-sensitive trace: three marks, four squashed diffs, a squashed `a`→`b`→`''` range, then
`squashToMark('b')`, verifying state is unchanged by the squash, undo reaches `'a'`, redo reaches the end,
two undos reach zero, and a later `squashToMark('a')` makes a single undo reach zero.

**SOURCE-CODE FACT.** `squashToMark` on an unknown mark logs `console.error('Could not find mark to squash to: ', id)`
and returns without mutating (`HistoryManager.ts:281-284`; test at `:729-741`). On a mark with nothing above
it, it returns early (`:285-287`; test at `:751-757`).

**INFERENCE.** Squash is the only mechanism in the system that is **safe to call after the fact** without
knowing what happened. Undo and bail require an accurate mental model of the stack; squash only requires the
mark id and is a pure reorganisation of already-captured data.

---

## History Stopping Points

"SStopping point" and "mark" are synonyms in this codebase — `markHistoryStoppingPoint` is the public name,
`TLHistoryMark`/`type: 'stop'` the internal name, and the only constructor is `_mark`.

**SOURCE-CODE FACT — everything a mark does, exhaustively:**

1. Wrap in `transact()` (`@tldraw/state`) so the flush and the push are one reactive update.
2. `flushPendingDiff()` — push the accumulated diff as `{type:'diff'}` **if non-empty**.
3. Push `{type:'stop', id}` onto `undos`.
4. **Not** clear `redos`.

**SOURCE-CODE FACT.** Two internal callers of `_mark` exist beyond the 120 public ones: none. The internal API
`getMarkIdMatching` exists solely for the legacy `creating:{shapeId}` compatibility in `Translating.ts:77-83`,
documented as such in `Editor.ts:1641-1644` ("this is only used to implement some backwards-compatibility
logic").

**INFERENCE.** A mark has no payload beyond identity. Anything that wants a "named user action" in history —
for a history panel, for AI attribution, for a plugin API — would have to extend `TLHistoryMark`. tldraw's
`name` argument is deliberately *not* part of the mark's identity and is not retained beyond the id string's
prefix.

---

## Transactions

### Q6 — What are `atomic`, `batch`, and `run` actually for?

Short answer: **reactive batching and capture-mode scoping. None of them is a history boundary.** The details
are in this section and in `§ Editor.run`.

The word "transaction" means three distinct things in this codebase. Disambiguating them is essential.

| Layer | Symbol | File | Guarantees |
|---|---|---|---|
| Reactive graph | `transact()` | `@tldraw/state` | Effects/reactors run once at the end. No data semantics. |
| History capture scope | `HistoryManager.batch(fn, opts)` | `HistoryManager.ts:97-125` | Sets capture mode for the dynamic extent; batches reactive effects; **no history boundary**. |
| Store atomicity | `Store.atomic(fn, runCallbacks, isMergingRemoteChanges)` | `Store.ts:1233-1277` | Reentrancy guard, side-effect deferral, source tagging, `runCallbacks` toggle. **No rollback.** |

**SOURCE-CODE FACT.** `Editor.run` (`Editor.ts:1736-1748`) layers a third thing on top:

```ts
run(fn: () => void, opts?: TLEditorRunOptions): this {
    const previousIgnoreShapeLock = this._shouldIgnoreShapeLock
    this._shouldIgnoreShapeLock = opts?.ignoreShapeLock ?? previousIgnoreShapeLock
    try { this.history.batch(fn, opts) }
    finally { this._shouldIgnoreShapeLock = previousIgnoreShapeLock }
    return this
}
```

`TLEditorRunOptions` (`Editor.ts:328-330`) = `TLHistoryBatchOptions & { ignoreShapeLock?: boolean }`.

**SOURCE-CODE FACT.** `this.run` is called *inside* `Editor.dispatch`'s flush path:
`_flushEventsForTick` (`Editor.ts:10918-10934`) wraps the whole event drain in `this.run(...)`, meaning every
pointer-move batch is already a capture scope whose mode is `Recording` (no `opts.history` ⇒ state unchanged).

**INFERENCE.** So during a drag there are **two** nested `batch` frames: the tick-level one from
`_flushEventsForTick` and the per-operation one from `Editor._updateShapes` (`Editor.ts:9038`). Because nested
batches do not nest the flag, the inner one inherits the outer's state — which is why `updateShapes` cannot
silently change the capture mode.

---

## Editor.run

**SOURCE-CODE FACT.** Public method at `Editor.ts:1736-1748`. It:

- scopes `ignoreShapeLock` for the dynamic extent, restored in `finally`;
- delegates to `history.batch`, which scopes capture state for the dynamic extent, restored in `finally`
  (`HistoryManager.ts:122-124`);
- rethrows whatever `fn` throws, after `annotateError` (`HistoryManager.ts:114-117`, wired at
  `Editor.ts:389-391` with `willCrashApp: true`).

**SOURCE-CODE FACT — it does not create a history boundary.** Nothing in `batch` calls `_mark`,
`flushPendingDiff`, or touches `undos`. `HistoryManager.test.ts:399-433` confirms: a `batch` with no mark
produces one undoable entry *combined with whatever preceded it in the same pending region*.

**SOURCE-CODE FACT — error behaviour.** `batch`'s `try/catch/finally` (`HistoryManager.ts:112-119`) calls
`annotateError(error)` and rethrows; `_isInBatch` is reset in `finally`; `state` is restored in the outer
`finally`. `HistoryManager.test.ts:831-843` verifies a subsequent batch works. **No rollback of the store.**

### Caller categories

**SOURCE-CODE FACT — `editor.run` call sites, sampled by category** (`grep` over `packages/{editor,tldraw}/src`,
non-test):

| Category | Representative site | Shape of the call |
|---|---|---|
| Capture-mode suppression (the dominant use) | `Editor.ts:927,1985,2013,2864,3558,4449,5227` | `run(() => store.put(…), { history: 'ignore' })` |
| Selection / focus | `Editor.ts:2175, 2748` | `run(…, { history: 'record-preserveRedoStack' })` |
| Every low-level shape write | `Editor._updateShapes`, `Editor.ts:9038` | `this.run(() => { …; this.store.put(updates) })` — no `history` opt |
| Menu/action bulk mutation | `ui/context/actions.tsx` — `'resize shapes'`, `updateSelectedShapes(markName, …)`, `'convert-to-embed'` | `markHistoryStoppingPoint(name)` **then** `run(() => { … })` |
| Clipboard paste | `ui/hooks/useClipboardEvents.ts:377` etc. | mark, then `putExternalContent` |
| Import | `utils/tldr/buildFromV1Document.ts:34` | `editor.run(() => { … })` — **no mark**, so the whole import lands in the pending region |
| Internal maintenance | `Editor.ts:920` constructor cleanup | `run(…, { history: 'ignore' })` |
| Tick drain | `Editor.ts:10919` `_flushEventsForTick` | `this.run(() => { … })` |

**SOURCE-CODE FACT.** The action helper `updateSelectedShapes` (`ui/context/actions.tsx`) is the canonical
shape and is worth quoting as a pattern, not as code:

```ts
editor.markHistoryStoppingPoint(markName)
editor.run(() => { update(selectedShapeIds); kickoutOccludedShapes(editor, selectedShapeIds) })
```

**SOURCE-CODE FACT.** `buildFromV1Document` (`utils/tldr/buildFromV1Document.ts:34`) calls
`editor.cancel().cancel().cancel().cancel()` then `editor.run(() => { … })` with **no** `markHistoryStoppingPoint`.
It also calls `setCurrentPage`, `deletePage`, `selectAll`, `deleteShapes` — all of which mutate — inside that
one run.

**INFERENCE.** The import path relies on `Editor.cancel()` to unwind any live interaction, and on the absence
of a mark meaning the whole import is one pending region. It is *not* explicitly `history: 'ignore'`, so the
import is undoable as one entry. That is plausibly intentional (undo an import) but it is undocumented in the
source.

**INFERENCE.** `Editor.run` has **no caller that uses it as a transaction boundary**, because it is not one.
Its actual value is (a) capture-mode scoping and (b) reactive batching. The naming invites the misreading that
the corpus's `ai-runtime.md` Implication C depends on; the correct statement is that **`markHistoryStoppingPoint`
is the public boundary primitive**, and it is more public than the manager itself.

---

## State Machines and History

**SOURCE-CODE FACT.** `Translating` (`packages/tldraw/src/lib/tools/SelectTool/childStates/Translating.ts`) is
the reference interaction. Its history-relevant lifecycle:

| Moment | Line | History action |
|---|---|---|
| `onEnter` | `:70-86` | `this.markId = ''`; if `isCreating` reuse `creatingMarkId` (or legacy `getMarkIdMatching`), **else** `this.markId = markHistoryStoppingPoint('translating')` |
| `onEnter` tail | `:107-109` | `this.snapshot = this.selectionSnapshot; this.handleStart(); this.updateShapes()` |
| `onPointerMove` | `:128-130` | `this.updateShapes()` |
| `onKeyDown` (Alt) | `:132-140` | `startCloning()` → `reset()` → new mark → `duplicateShapes` |
| `onKeyUp` (Alt released) | `:142-150` | `stopCloning()` → `reset()` → new mark |
| `onPointerUp` | `:152-154` | `this.complete()` |
| `onComplete` | `:156-158` | `this.complete()` |
| `onCancel` | `:160-162` | `this.cancel()` |
| `onExit` | `:112-120` | cleanup only — **no history call** |
| `complete()` | `:206-235` | `updateShapes()`, `dragAndDropManager.dropShapes`, `handleEnd()`, tool revert, `transition('idle')` |
| `cancel()` | `:237-260` | `util.onTranslateCancel` per shape, then `reset()` → `bailToMark(this.markId)`, then `transition('idle')` |
| `reset()` | `:197-204` | `this.editor.bailToMark(this.markId)`; `snapshot.lastAppliedOffset = null`; `changeTracker.clear()` |

**SOURCE-CODE FACT.** The same shape recurs in `Resizing.ts` (`markId` field `:43`, mark `:95`, `cancel`
bail `:153`, `onExit` bail when `isCreating && !didFinish` `:594`), `Rotating.ts:168`,
`DraggingHandle.tsx:267`, `EraserTool/childStates/Erasing.ts:170`,
`shapes/note/toolStates/Pointing.ts:137`, `shapes/arrow/toolStates/Pointing.tsx:43`,
`shapes/line/toolStates/Pointing.ts:124,158,168`, `shapes/text/toolStates/Pointing.ts:131`,
`shapes/draw/toolStates/Drawing.ts:84,774`.

### The answer the brief asks for

> Do interaction states own history boundaries, or operations, or the Editor/HistoryManager?

**SOURCE-CODE FACT.** All three statements are partly true and they are *different* claims:

| Claim | Verdict | Evidence |
|---|---|---|
| Interaction states place boundaries | **True, and it is the dominant pattern.** | 20+ `onEnter`/`onCancel` sites across the tool state charts; `Translating.ts:85`, `Resizing.ts:95`, `Crop.ts:19`, `Drawing.ts:212` |
| Operations place boundaries | **Also true.** Actions place marks in `onSelect`, 120 call sites. | `ui/context/actions.tsx`, `ui/components/**` |
| The `HistoryManager` infers boundaries | **False.** It never calls `_mark` itself. | `HistoryManager.ts` — `_mark` has one caller, `Editor.markHistoryStoppingPoint` |
| The `Editor` owns boundaries | **False in substance.** It re-exposes a 3-line wrapper. | `Editor.ts:1635-1638` |

**INFERENCE.** The honest formulation is: **the history boundary is a property of the call site, not of any
architectural layer.** tldraw solved Q2's "who owns the operation registry" by making the operation surface
*be* the boundary surface — the same flat `TLUiActionItem` records that the keyboard, menu, and toolbar
dispatch through are what call `markHistoryStoppingPoint`. There is no second, parallel "history layer" to keep
in sync because there is no operation type system at all.

**SOURCE-CODE FACT.** `Resizing.onExit` (`:581-599`) shows the safety net: if the state is left *without*
`complete()` or `cancel()` having run — e.g. a tool shortcut pressed mid-drag — and the interaction was
creating, it calls `bailToMark(this.markId)` itself. Comment: "the shape created at the drag threshold was
never committed and would otherwise be left behind, empty and invisible (#10401)".

**INFERENCE.** `onExit` acting as a backstop for a missed `cancel` is the state-chart equivalent of a scope
guard. It is a pattern Spool's flat `Interaction` enum has no place to express, because `Interaction` has no
exit hook — `Interaction::restore()` is called explicitly by the Escape path only.

---

## Continuous Gestures

### Q7 — How is a drag recorded? One entry, or one per pointer event?

**SOURCE-CODE FACT.** One pending region, opened by a mark, never sealed by the gesture.

`Translating` is the canonical case. Its mark is placed once, in `onEnter`:

```ts
// packages/tldraw/src/lib/tools/Translating/Translating.ts:70-86
override onEnter() {
  this.markId = StopPoints.translating
  ...
  this.editor.markHistoryStoppingPoint(this.markId)
  ...
  this.editor.run(() => {
    ...
    this.startCaching()
    ...
  })
}
```

Every subsequent mutation is ordinary `store.put` inside `moveShapesToPoint` (`:567-681`), reached from
`onPointerMove` (`:128-130`). None of them calls `markHistoryStoppingPoint`. `onPointerUp` (`:152-154`) only
does `this.editor.complete()`, and `onComplete` (`:156-158`) calls `this.editor.complete()`. Neither touches
history.

**SOURCE-CODE FACT.** The gesture therefore produces one region containing:
1. whatever the `onEnter` block wrote (snapshot/offset bookkeeping is ephemeral or document, depending),
2. every per-move `put` of shape x/y,
3. the final resting position.

### Q7a — Why does that coalesce into a single undo entry?

**SOURCE-CODE FACT.** `PendingDiff.apply` merges each incoming diff into the pending one:

```ts
// packages/editor/src/lib/editor/managers/HistoryManager/HistoryManager.ts:346-383
apply(diff: RecordsDiff<R>) {
  squashRecordDiffsMutable(this._diff, diff)
  // Recompute emptiness — but only if the incoming diff could have changed it.
  if (diff.added.size || diff.removed.size) {
    this._isEmpty.set(isRecordsDiffEmpty(this._diff))
  }
}
```

and `flushPendingDiff` converts the accumulated region into exactly one stack item, but only when something
else next asks for history:

```ts
// HistoryManager.ts:68-76
flushPendingDiff() {
  if (this.pendingDiff.isEmpty()) return
  this.stacks.update(({ undos, redos }) => ({
    undos: undos.push({ type: 'diff', diff: this.pendingDiff.get() }),
    redos,
  }))
  this.pendingDiff.clear()
}
```

### Q7b — This is the counter-intuitive part: nothing seals at gesture end.

**SOURCE-CODE FACT.** `flushPendingDiff()` has exactly two callers inside `HistoryManager`: `redo()` (`:211`)
and `_mark()` (`:304`). There is no "commit"/"seal"/"endBatch" entry point anywhere in the class, and
`Editor` exposes none.

**SOURCE-CODE FACT.** The user-visible consequence is that the gesture's entry does not exist as a stack item
until the *next* mark or the *next* `redo()`:

```ts
// HistoryManager.ts:78-80
getNumUndos() {
  return this.stacks.get().undos.length + (this.pendingDiff.isEmpty() ? 0 : 1)
}
```

The pending region is counted as an undo step so the UI (menu enablement, the `⌘Z` affordance) is correct even
before it has been materialized.

**INFERENCE.** tldraw's undo granularity is therefore "region delimited by marks", where a mark is a *leading
delimiter*, not a trailing seal. Consecutive operations without an intervening mark collapse into one undo
step. `getNumUndos` is the only place the distinction surfaces.

### Q7c — What happens if a drag is never followed by a mark and the user presses undo?

**SOURCE-CODE FACT.** `_undo` reverses the pending diff first, so the in-flight gesture is undone correctly:

```ts
// HistoryManager.ts:128-141
private _undo(opts: { pushToRedoStack: boolean; toMark?: string }) {
  this.state.set('paused')
  this._isReplaying = true
  const pendingDiff = this.pendingDiff.get()
  this.pendingDiff.clear()
  const diffToUndo = reverseRecordsDiff(pendingDiff)
  ...
  if (pendingDiff.isEmpty()) { /* pop leading stops */ }
  else { /* walk undos, squashing into diffToUndo */ }
```

**SOURCE-CODE FACT — executable evidence.** `HistoryManager.test.ts:233` ("does not add new history entries
while undoing") and `:242` (the bail variant) both assert that after an undo/bail the stack contains only the
original entries — no partially-applied region is stranded.

**INFERENCE.** This is "lazy commit by the next boundary". It is correct because `squashRecordDiffsMutable`
is a semilattice join: applying N diffs into one buffer then reversing is equivalent to reversing the composed
diff. The cost is that the entry's identity is assigned by the *following* operation, so a crash between the
drag and the next mark loses the naming but not the data.

---

## Cancellation

### Q8 — What does "cancel" actually do?

**SOURCE-CODE FACT.** There is no `cancel history` primitive. Cancel is expressed as **bail to a mark**:
undo the pending region and everything after the named mark, while discarding the popped entries instead of
moving them to the redo stack.

`Translating.cancel` (`Translating.ts:237-260`) calls `this.reset()`, which is:

```ts
// Translating.ts:197-204
private reset() {
  if (this.markId) {
    this.editor.bailToMark(this.markId)
  }
  this.lastAppliedOffset.set(null)
  this.changes.clear()
}
```

and `Editor.bailToMark` is a two-line wrapper (`Editor.ts:1703`):

```ts
bailToMark(id: string): void {
  if (id) this.history.bailToMark(id)
}
```

which lands in `_undo({ pushToRedoStack: false, toMark: id })` (`HistoryManager.ts:186-190`).

### Q8a — Why `pushToRedoStack: false`? What is the semantic difference?

**SOURCE-CODE FACT.** In `_undo`, entries popped from `undos` are conditionally pushed to `redos`:

```ts
// HistoryManager.ts:143-175
const { undos, redos } = this.stacks.get()
const popped = undos.head
if (pendingDiff.isEmpty()) {
  // strip leading stop entries
} else {
  let next = popped
  while (next.type !== 'empty') {
    const entry = next.value
    if (pushToRedoStack) redos.push(entry)      // <-- the only difference
    if (entry.type === 'diff') squashRecordDiffsMutable(diffToUndo, reverseRecordsDiff(entry.diff))
    else if (!toMark || entry.id !== toMark) break
    else { next = next.next; continue }
    ...
  }
}
```

**INFERENCE.** The parameter turns *undo* (a reversible move between two stacks) into *bail* (a destructive
truncate of the undo spine). Undo populates redo so the user can return; bail does not, because returning to
a discarded, never-committed interaction state is not a state the user was ever shown.

**SOURCE-CODE FACT.** `bail()` (`HistoryManager.ts:204-207`) is `_undo({ pushToRedoStack: false })` with no
mark — "undo everything, keep nothing". It is called from `Editor.ts:1686` and has ~6 call sites, all of them
"the document is being rebuilt from scratch; the undo spine is no longer meaningful" (e.g.
`useClipboardEvents.ts` paste flows, `PageItemInput.tsx` rename).

### Q8b — What if the mark is not found?

**SOURCE-CODE FACT.** Bail is a **total no-op** on a miss, including not touching the pending diff:

```ts
// HistoryManager.ts:151-157
} else if (toMark) {
  this.pendingDiff.restore(pendingDiff)
  return                          // stacks.set(...) never reached
}
this.store.applyDiff(diffToUndo, { ignoreEphemeralKeys: true })
```

**SOURCE-CODE FACT — executable evidence.** `HistoryManager.test.ts` (bailToMark-miss case in the `:718+`
edge-case block) asserts both that the store is unchanged and that the pending diff is intact, i.e. a failed
bail does not even cost the user their in-flight gesture.

**INFERENCE.** The miss case is not an error path — it is the *normal* outcome of a bail that races a mark
that was never pushed, or a double-cancel (Escape pressed twice). Swallowing it silently is deliberate:
cancel is on a hot path and must not throw.

**SOURCE-CODE FACT.** `Crop.ts` shows the defensive guard style at the call site:

```ts
// packages/tldraw/src/lib/tools/Crop/Crop.ts:26
this.editor.squashToMark(this.markId)
```
is guarded by `if (this.editor.getMarkIdMatching(this.markId) === this.markId)`, and `Crop.onCancel` bails
only when `this.editor.getCurrent()?.id === 'idle'`, with the in-source comment explaining that parents must
see the event before children.

### Q8c — Which states call bail, and how often?

**SOURCE-CODE FACT.** ~25 `bailToMark` sites and 6 `bail` sites. Representative:

| Site | Context |
|---|---|
| `Translating.ts:198` | `reset()`, reached from `cancel()` and from Alt-clone abort |
| `Resizing.ts:153` | `cancel()` |
| `Resizing.ts:594` | `onExit` backstop when `isCreating && !didFinish` (issue #10401) |
| `Rotating.ts:168` | `cancel()` |
| `Erasing.ts:170` | `cancel()` |
| `Drawing.ts:84`, `:774` | `cancel()` / completion paths |
| `Crop.ts:37`, `Cropping.ts:243`, `TranslatingCrop.ts:97` | crop family |
| `DraggingHandle.tsx:267` | shape-handle drag cancel |
| `ui/context/actions.tsx:475` | **`.catch()` on an async `convert-to-bookmark`** → `bailToMark` |
| `PageItemInput.tsx:42` | rename failure |

**SOURCE-CODE FACT.** `ui/context/actions.tsx:475` is the clearest statement of intent: when an async
operation fails, the app *bails the mark it placed on the way in*, which restores the pre-operation document
state. This is a general-purpose compensating transaction.

**INFERENCE.** tldraw's answer to "async failed" is: place a mark before starting, bail to it on failure. It
does **not** implement rollback in the transaction sense (`atomic` has no rollback — see §Transactions), so
the compensating action is history manipulation, not state restoration.

---

## Nested Operations

### Q9 — What happens when an operation starts inside another operation?

Three distinct mechanisms exist. They are not interchangeable.

#### (a) Nested batches

**SOURCE-CODE FACT.** `batch` is reentrant and *does not* create a boundary:

```ts
// HistoryManager.ts:97-125
batch(fn: () => void, opts?: TLHistoryBatchOptions) {
  const previousState = this.state.get()
  if (previousState !== 'paused' && opts?.history) {
    this.state.set(opts.history)
  }
  const isReentrant = this._isInBatch
  if (!isReentrant) {
    this._isInBatch = true
    store.atomic(fn, true)
  } else {
    store.atomic(fn)
  }
  ...
  finally {
    if (!isReentrant) {
      this._isInBatch = false
      store.ensureStoreIsUsable()
      this.state.set(previousState)
    }
  }
}
```

**INFERENCE.** The inner call inherits the outer batch's state and contributes no separate entry, because no
mark is involved. `opts.history` on the inner call is honored *only if the outer state was not `paused`* — so
an inner `history: 'ignore'` inside a recording outer batch is silently upgraded to recording. This is
verified by `HistoryManager.test.ts:399` (nested ignore, both directions).

#### (b) Interaction invoked from inside a gesture — the mark does the work, not the nesting

**SOURCE-CODE FACT.** The real nesting case is an interaction that mutates the document while another
interaction's pending region is open. Because the pending region is a *single buffer* and the inner
interaction's mark is pushed onto `undos` without flushing... in fact `_mark` *does* flush:

```ts
// HistoryManager.ts:303-309
private _mark(_id: string) {
  this.store.transact(() => {
    this.flushPendingDiff()
    this.stacks.update(({ undos, redos }) => ({
      undos: undos.push({ type: 'stop', id: _id }),
      redos,
    }))
  })
}
```

**INFERENCE.** `_mark` is therefore the *only* implicit seal in the system: placing a mark mid-gesture splits
the open region into "everything before this mark" + "everything after". This is how `Translating`'s
Alt-press clone (`startCloning`, `Translating.ts:132-140`) gets its own undo step without the gesture
ending — the `translate cloning` mark at `:173` splits it.

**SOURCE-CODE FACT — executable evidence.** `HistoryManager.test.ts:180` — five increments with two marks
produce `getNumUndos() === 3` (diff, stop, diff-region-counted) rather than five.

#### (c) Cascading binding writes — `onOperationComplete`

**SOURCE-CODE FACT.** `Editor.ts:634` invokes `bindingUtil.onOperationComplete(...)` when an operation would
produce an invalid binding type. The hook is declared at
`packages/editor/src/lib/editor/bindings/BindingUtil.ts:159-172`.

**INFERENCE.** Arrow bindings, text bindings, and relationship bindings produce cascading writes (move a
shape → move its bound arrow → move the arrow's label). Without aggregation these would be many tiny diffs.
`onOperationComplete` is the aggregation point, and because the whole thing runs inside one `store.atomic`,
they all squash into one pending region — so it is correct without any history-specific machinery.

**SOURCE-CODE FACT.** `buildFromV1Document.ts:34` is the degenerate case: it calls `editor.run(...)` with
**no mark** and calls `editor.cancel().cancel().cancel().cancel()` first. The entire import therefore lands as
one pending region, and any subsequent operation seals it into a single undo step.

**INFERENCE.** tldraw's rule for "one user-meaningful action = one undo step" is enforced socially (call
sites) and *incidentally* by the atomic/squash machinery, not by any mechanism that can detect an action
boundary. A new action that forgets its mark silently merges with the previous one.

---

## Undo

### Q10 — What is the exact undo algorithm?

**SOURCE-CODE FACT.** `_undo` (`HistoryManager.ts:128-202`), in order:

1. `state.set('paused')`, `_isReplaying = true` — **no new entries can be recorded while replaying**.
2. Detach and clear `pendingDiff`.
3. `diffToUndo = reverseRecordsDiff(pendingDiff)` — seed the accumulator with the in-flight region.
4. If the pending region was empty, pop and discard leading `stop` entries (the boundary itself is not an
   undo step).
5. Otherwise walk `undos` from the head:
   - every popped entry is pushed to `redos` **iff** `pushToRedoStack`,
   - `type: 'diff'` → `squashRecordDiffsMutable(diffToUndo, reverseRecordsDiff(entry.diff))`,
   - `type: 'stop'` → **break, unless** `toMark` was supplied, in which case continue until the id matches.
6. If `toMark` was supplied and never matched: restore the pending diff and return without touching stacks.
7. `store.applyDiff(diffToUndo, { ignoreEphemeralKeys: true })`.
8. `store.ensureStoreIsUsable()`.
9. `stacks.set({ undos: popped, redos })`.

**INFERENCE.** Undo is a **fold over the stack with an algebraic accumulator**, not a stack of inverse
closures. Because step 5 accumulates into one `RecordsDiff` and step 7 applies it once, a single undo can
collapse an arbitrary number of stack entries — including across `stop` boundaries when `toMark` is used — and
the store observes exactly one atomic `applyDiff`. That is why undo is a single frame of reactivity for the
renderer and why it cannot leave a half-applied intermediate state.

### Q10a — Why is `reverseRecordsDiff` a copy, not a mutation?

**SOURCE-CODE FACT.** `RecordsDiff.ts:80-92` copies every collection before swapping:

```ts
export function reverseRecordsDiff<R extends UnknownRecord>(diff: RecordsDiff<R>): RecordsDiff<R> {
  return {
    added: new Map(diff.removed),
    removed: new Map(diff.added),
    updated: Object.fromEntries(
      Object.entries(diff.updated).map(([id, [from, to]]) => [id, [to, from]])
    ),
  }
}
```

**INFERENCE.** Because reversal is a value transform, the same diff object can be reversed more than once
(`squashToMark` reverses popped entries; `_undo` reverses pending) without corruption. This is what lets the
accumulator be a plain `RecordsDiff` value rather than a builder object.

### Q10b — Where does `ignoreEphemeralKeys` matter here?

**SOURCE-CODE FACT.** Both `_undo` (`:198`) and `redo` (`:248`) apply with `{ ignoreEphemeralKeys: true }`.
`Store.applyDiff` (`Store.ts:1067-1112`) uses this to strip per-record-type ephemeral keys from the applied
values.

**SOURCE-CODE FACT.** `RecordType.ts:56` declares `ephemeralKeys?: {[K in keyof R]: boolean}`, and
`:103-111` builds `ephemeralKeySet` from the entries whose value is `true`.

**INFERENCE.** Undo restores **document** state and deliberately does **not** restore selection-ish or
interaction-ish state. This is the mechanism behind the corrected claim in §Important Edge Cases.

### Q10c — What is the user-facing entry point?

**SOURCE-CODE FACT.** `Editor.undo()` (`:1558-1564`) does two things before delegating:

```ts
undo(): void {
  this._flushEventsForTick(0)
  this.complete()
  this.history.undo()
}
```

**SOURCE-CODE FACT.** `_flushEventsForTick` (`Editor.ts:10937+`) early-returns if `getCrashingError()` is set.

**INFERENCE.** Draining the side-effect queue and dispatching `{type:'misc', name:'complete'}` before undo
guarantees that deferred shape effects (bindings, layout, snapping) have already been applied and therefore
already captured in the diff being undone. Without this, undo could replay a pre-effect snapshot and the
effects would fire twice. **Order matters: flush, then complete, then undo.**

**SOURCE-CODE FACT.** `Editor.ts:1546` declares `protected readonly history`, and `:925` registers
`this.history.dispose` on editor teardown.

---

## Redo

### Q11 — Is redo symmetric with undo?

**SOURCE-CODE FACT.** Structurally yes, semantically no. `redo()` (`HistoryManager.ts:210-252`):

1. `state.set('paused')`, `_isReplaying = true`.
2. **`this.flushPendingDiff()`** — a pending region is materialized onto `undos` *before* the redo, so the
   redo operates on a settled spine.
3. Pop from `redos`, skipping leading `stop` entries.
4. `squashRecordDiffsMutable(diffToRedo, entry.diff)` — **forward** accumulation, no `reverseRecordsDiff`.
5. `store.applyDiff(diffToRedo, { ignoreEphemeralKeys: true })`, `ensureStoreIsUsable()`, `stacks.set`.

**SOURCE-CODE FACT.** `_undo` step 5 pushes popped entries to `redos` in the same order they were popped;
`redo` pushes them back to `undos` in the order it popped them. The reversal is in the diff, not the spine.

### Q11a — Why does recording clear the redo stack only sometimes?

**SOURCE-CODE FACT.** The interceptor (`HistoryManager.ts:45-65`):

```ts
this.store.addHistoryInterceptor((entry, source) => {
  if (source !== 'user') return
  switch (this.state.get()) {
    case 'recording':
      this.pendingDiff.apply(entry.changes)
      if (this.stacks.get().redos.length > 0) {
        this.stacks.set(({ undos }) => ({ undos, redos: EMPTY_STACK_ITEM }))
      }
      break
    case 'recording_preserving_redo_stack':
      this.pendingDiff.apply(entry.changes)
      break
    case 'paused':
      break
    default:
      exhaustiveSwitchError(this.state.get(), 'unknown state')
  }
})
```

**SOURCE-CODE FACT.** `Store.updateHistory` (`Store.ts:584-593`) tags each entry
`isMergingRemoteChanges ? 'remote' : 'user'`, and the interceptor drops every `'remote'` entry.

**INFERENCE.** The distinction is about whether the app still *believes* its redo branch is meaningful.
Selection changes (`setSelectedShapes`, `setFocusedGroup`, `setCurrentPage`) use
`record-preserveRedoStack` precisely because clearing redo for a selection change would destroy the user's
ability to undo a real edit, while *not* recording the selection change would leave the document and its
history disagreeing. tldraw's answer to that tension is a third history mode, not a type-level distinction.

**SOURCE-CODE FACT — executable evidence.** `HistoryManager.test.ts:148` — marks do not clear the redo
stack, commands do. `:208` — an "inconsequential" mutation under
`record-preserveRedoStack` preserves redo.

---

## Async Operations

### Q12 — How does tldraw handle operations that span time or fail?

There is **no async transaction, no compensation log, and no undo token**. Three patterns, in increasing
order of explicitness:

**Pattern 1 — mark on the way in, bail to it on failure.** `ui/context/actions.tsx:475`: the
`convert-to-bookmark` action places a mark, awaits, and in `.catch()` calls `bailToMark(markId)`.

**Pattern 2 — `editor.run()` around the synchronous portion only.** Most actions are shaped:

```tsx
// ui/context/actions.tsx — representative shape
onSelect() {
  const markId = uniqueId()
  editor.markHistoryStoppingPoint(markId)
  editor.run(() => { /* synchronous mutation */ })
}
```

**Pattern 3 — suppress history for the plumbing.** Asset and asset-store writes are `history: 'ignore'`
(`Editor.ts:5199`, `:5227`); document settings are `ignore` (`:1985`); user records are `ignore` (`:4449`).

**SOURCE-CODE FACT.** `Store.mergeRemoteChanges` (`Store.ts:1037-1051`) asserts `!this._isInAtomicOp` and
calls `ensureStoreIsUsable()`. Remote changes arrive tagged `'remote'` and are **never** recorded locally —
this is tldraw's answer to "a background sync arrives mid-gesture".

**INFERENCE.** tldraw's async story is deliberately weak, and weak in a specific way: the system assumes
mutations are synchronous and small. When they are not, the author must remember to place a mark and to bail
on failure. There is no structural enforcement, no `AsyncOp` type, no rollback. This is a known cost of the
"history boundary is a call-site convention" design, and it is directly relevant to Spool's Q2.

**SOURCE-CODE FACT.** `Store.atomic` (`:1233-1277`) has **no rollback**: on throw it only restores the
flags in `finally`. There is no inverse-diff capture for the general case. Consequently the *only* way to
undo a failed operation is the explicit `bailToMark` in a `.catch`.

---

## Selection and Camera

### Q13 — Is selection in history?

**This corrects the existing corpus.** See §Important Edge Cases, Correction 1.

**SOURCE-CODE FACT.** `TLPageState` (`tlschema/src/pages/TLPage.ts:211`) has scope `'session'` and declares
its own `ephemeralKeys` (`TLPage.ts:211-230`), **not** `history: 'ignore'` at the schema level.

**SOURCE-CODE FACT.** The declared ephemeral flags are:
- `editingShapeId: true`
- `hintingShapeIds: true`
- `erasingShapeIds: true`
- `hoveredShapeId: true`
- `focusedGroupId: true`

and these are **`false`** (i.e. *restorable*):
- `pageId: false`
- `selectedShapeIds: false`
- `croppingShapeId: false`
- `meta: false`

**SOURCE-CODE FACT.** The in-source comment at `TLPage.ts:213-218` explains that `editingShapeId` is set with
`history: 'ignore'` so that entering edit state is never itself undoable, and that the ephemeral flag prevents
a stale editing id from being reapplied by an undo.

**SOURCE-CODE FACT.** `Editor.setSelectedShapes` (`:2175`) runs under `record-preserveRedoStack`;
`setFocusedGroup` (`:2748`) likewise. `setEditingShape` (`:2864`), `setCroppingShape` (`:3091-3103`), and
`_setCamera` (`:3552-3559`) run under `history: 'ignore'`.

**INFERENCE — and this is the load-bearing correction.** Selection *changes are recorded as pending history
diffs* (via `record-preserveRedoStack`), and they *are stored in history*. They are merely **not re-applied on
undo**, because `applyDiff(..., { ignoreEphemeralKeys: true })` strips them — and they do not clear redo,
because the mode is `record-preserveRedoStack`. The corpus claim "selection is not in history" is wrong in
the sense that matters: selection changes **consume pending-diff capacity and are part of the accumulated
region**, they just do not survive replay.

**INFERENCE.** Camera is the cleaner case: `_setCamera` uses `ignore`, so the camera is fully outside the
document/history model. `app`-level and asset-level writes are also fully outside.

**SOURCE-CODE FACT.** `Editor.ts:4157` uses mark id `'change-page'` for page switching, and
`setCurrentPage` (`:5005`) uses `record-preserveRedoStack`.

**INFERENCE.** Page switching is a history *boundary* and a *recordable* mutation, but camera movement within
a page is neither. That is a consistent rule: **the document reference and the selection set are document
state; the viewport is view state.**

---

## Actions / Operation Registry

### Q14 — Is there an operation registry, and where do marks live?

**SOURCE-CODE FACT.** There is no operation registry, no command interface, no operation trait. tldraw has
**no `TLCommand`, no `CommandOperation`, no `CommandOperationKey`** anywhere in the OSS repository at this
commit. Operations are *data records* that actions `store.put` directly.

**SOURCE-CODE FACT.** The closest analogue is the UI action item list: flat, serializable
`TLUiActionItem`-shaped records consumed by the keyboard handler, the menus, the context menu, and the
toolbar. Actions are `() => void` closures over an `Editor` from context
(`packages/tldraw/src/lib/ui/context/actions.tsx`), not dispatched command objects.

**SOURCE-CODE FACT — this is the key structural fact.** The mark call lives **inside the action's
`onSelect`**, i.e. in the same closure as the mutation, adjacent to it in source:

```tsx
// ui/context/actions.tsx — representative shape (group / ungroup / delete / paste / z-order)
onSelect() {
  const markId = StopPoints.group
  editor.markHistoryStoppingPoint(markId)
  editor.run(() => { ...mutations... })
}
```

**SOURCE-CODE FACT.** 120 non-test call sites of `markHistoryStoppingPoint`. Representative:

| Site | Mark id |
|---|---|
| `Translating.ts:85`, `:173`, `:191` | `'translating'`, `'translate cloning'`, `'translate'` |
| `Resizing.ts:95` | `'starting resizing'` |
| `Crop/Crop.ts:19` | `'crop'` |
| `Drawing.ts:212` | `'draw start'` |
| `BaseBoxShapeTool/children/Pointing.ts:22`, `:95` | `` `creating_box:${id}` `` |
| `SelectTool/childStates/Idle.ts:547,562,714,735,789` | `'nudge shapes'` (guarded by `if (!ephemeral)`) |
| `ui/context/actions.tsx` | `'resize shapes'`, `'group'`, `'ungroup'`, `'delete'`, `'paste'`, `'bring to front'` |
| `useCanvasEvents.ts:197`, `:208` | `'drop'` |
| `useClipboardEvents.ts:171,183,195,377,569,582,617,908`, `:749` | `'paste'`, `'cut'` |

**INFERENCE.** The operation registry and the history boundary are the *same surface*. tldraw avoids the
"who owns operation semantics" problem by not having an operation layer at all — the boundary is placed by
whoever writes the record. The cost is 120 hand-placed marks with no compiler or runtime check, and the
benefit is that adding an operation cannot be forgotten at the history layer, because there is no separate
history layer to forget.

**INFERENCE — relevance to Q2.** The tldraw answer is a *negative* answer to Q2: it is evidence that
Q2 (a typed operation registry with history ownership) is a real question, not a solved problem that the
incumbent has already answered. Spool's `DocumentCommand` / `CommandOperation` in `app/src/canvas.rs` is the
opposite architectural bet; the comparison is the point of this investigation.

**INFERENCE — relevance to AI.** Since AI code writes to the store directly, it inherits the *entire*
boundary problem: an agent cannot be trusted to call `markHistoryStoppingPoint` correctly, and there is
nothing in the type system that prevents a burst of AI edits from merging into one undo step. This is
extrapolated, not observed (§AI and History).

---

## AI and History

### Q15 — What is tldraw's AI-to-history relationship?

**SOURCE-CODE FACT.** An exhaustive search of the pinned OSS checkout at `v5.5.1` for AI-related history
surfaces returns nothing: there is no `applyAIEdit`, no AI tool state, no AI entry in the action registry, and
no AI-specific history handling in `Editor` or `HistoryManager`.

**DOCUMENTED FACT.** All tldraw AI material in this corpus is product-level and observed on the hosted
tldraw.com product, not in this OSS tag: `docs/research/profiles/tldraw.md`,
`docs/research/matrices/history-matrix.md`, `docs/research/architecture/ai-runtime.md`.

**INFERENCE.** Therefore this file makes **no claim** about how tldraw handles AI-generated mutations. Any
statement of that form would be invention. What *can* be said, structurally, is a conditional derived from
the source facts above:

1. Because history boundaries are call-site conventions, an AI mutation path has no natural place to put one.
2. Because `Store.updateHistory` (`Store.ts:584-593`) tags everything not in `mergeRemoteChanges` as
   `'user'`, an AI edit issued through `store.put` would be recorded as **user** history — indistinguishable
   from a human edit in the undo stack.
3. Because there is no operation registry, there is no place to attach provenance (who/why/which agent).
4. Because undo applies `{ ignoreEphemeralKeys: true }`, a per-record `isAIGenerated` field declared in
   `ephemeralKeys` would survive in state but not replay — a workable, source-supported escape hatch.

**Proposed question (not a decision).** If Spool allows AI mutation, does it need a third history mode, or a
provenance field, or a per-AI-operation mark? tldraw's source provides no precedent either way.

---

## Source Traces

### Example 1 — Move (drag a shape from A to B)

| Step | Code | History effect |
|---|---|---|
| 1 | `SelectTool` → `Pointing` → `Translating.onEnter` | `markHistoryStoppingPoint('translating')` (`Translating.ts:85`) — flushes the pending region, pushes a `stop` |
| 2 | `Translating.onEnter` → `editor.run` | records `history` mode for the block; no mark |
| 3 | `onPointerMove` × N → `moveShapesToPoint` → `store.put` | each `put` is `atomic`, runs before-hooks, calls `updateHistory` once → interceptor fires synchronously → `pendingDiff.apply(changes)`; redo cleared |
| 4 | (optional) Alt held → `startCloning()` | `markHistoryStoppingPoint('translate cloning')` (`:173`) — **flushes step 1–3 into one diff**, pushes a `stop` |
| 5 | `onPointerUp` → `onComplete` → `editor.complete()` | dispatch `{type:'misc', name:'complete'}` — **no history effect** |
| 6 | user presses `⌘Z` | `Editor.undo()` → `_flushEventsForTick(0)` → `complete()` → `HistoryManager.undo()` → `_undo({pushToRedoStack:true})` |
| 7 | | pending region reversed + seeded into `diffToUndo`, prior stack entries popped until the `stop`, all pushed to `redos`, single `applyDiff(..., {ignoreEphemeralKeys:true})` |

**Result.** One `⌘Z` returns the shape to its pre-drag position, including the drag's start offset and any
intermediate squash. One entry per mark-delimited region.

### Example 2 — Cancel (Escape mid-drag)

| Step | Code | History effect |
|---|---|---|
| 1–3 | as Example 1 | one open pending region |
| 4 | Escape → `Translating.cancel()` (`:237-260`) | `this.reset()` (`:197-204`) |
| 5 | | `editor.bailToMark(this.markId)` → `_undo({pushToRedoStack:false, toMark:'translating'})` |
| 6 | | pending reversed; walk `undos`; popped entries **discarded**, not pushed to `redos`; break at the matching `stop` |
| 7 | | `applyDiff(diffToUndo, {ignoreEphemeralKeys:true})` restores the pre-drag document |
| 8 | | `lastAppliedOffset.set(null)`; `this.changes.clear()` — in-memory state reset, not history |

**Result.** Document restored. **Redo stack untouched** — the user cannot undo the undo. If the mark is
missing, nothing happens at all (no throw, pending preserved).

### Example 3 — Squash (crop leaves, then they are merged)

| Step | Code | History effect |
|---|---|---|
| 1 | `Crop.onEnter` | `markHistoryStoppingPoint('crop')` (`Crop.ts:19`) |
| 2 | drag crop handle | pending region accumulates the moving shapes |
| 3 | `Crop.onExit` | guard: `if (this.editor.getMarkIdMatching(this.markId) === this.markId)` |
| 4 | | `editor.squashToMark(this.markId)` → `HistoryManager.squashToMark` (`:268-301`) |
| 5 | | walk `undos` collecting `diff` entries into `popped` until the named `stop`; intermediate `stop`s discarded |
| 6 | | `popped.reverse()` (**order matters**); `squashRecordDiffsMutable(diff, popped)` |
| 7 | | push one new `diff` onto the target position; `redos` untouched |

**Result.** N separate drag regions become one undo step. `squashToMark` has **exactly one real caller** in
the OSS repo (`Crop.ts:26`), guarded so a missing mark cannot corrupt the spine; `squashToMark` itself logs
`console.error('Could not find mark to squash to: ', id)` and returns on a miss.

### Example 4 — Nested Operation (Alt-clone during a drag)

| Step | Code | History effect |
|---|---|---|
| 1–3 | `Translating.onEnter` + moves | one open pending region |
| 4 | Alt keydown (`:132-140`) | `startCloning()` → `markHistoryStoppingPoint('translate cloning')` (`:173`) |
| 5 | | `_mark` flushes the pending region into a `diff` on `undos` and pushes a `stop` |
| 6 | further moves | a **new, second** pending region opens |
| 7 | keyup (`:142-150`) → `stopCloning()` | no history call |
| 8 | pointerup | `complete()` only |

**Result.** `⌘Z` once undoes the clone drag; `⌘Z` again undoes the original drag. The nesting is expressed
purely as *two marks delimiting two regions* — there is no nested-batch or parent/child history structure.
This matches `HistoryManager.test.ts:180` (five increments, two marks, `getNumUndos() === 3`).

### Example 5 — Undo then a new mutation

| Step | Code | History effect |
|---|---|---|
| 1 | 3 committed regions on `undos`, 2 on `redos` | — |
| 2 | `⌘Z` | `_undo({pushToRedoStack:true})`: 1 popped entry → `redos` (now 3) |
| 3 | user drags a shape | interceptor in `recording` state → `pendingDiff.apply` **and** `redos` cleared (`HistoryManager.ts:50-56`) |
| 4 | user presses `⌘Z` | `_undo` seeds from the new pending region, then pops the one remaining `undos` diff |
| 5 | user presses `⇧⌘Z` | `_undo({pushToRedoStack:false, toMark:undefined})` via `bail()` — or `redo()` is now empty |

**Result.** Standard linear-undo semantics. The clearing is conditional on `redos.length > 0` — a performance
guard, not a semantic one (`HistoryManager.test.ts:148`).

---

## Important Edge Cases

### CORRECTION 1 — Selection *is* recorded; it is not replayed

**CORRECTION.** `docs/research/matrices/history-matrix.md` §4 (“What is not in history”) and
`docs/research/architecture/history.md` both state that selection is absent from tldraw's history.

**SOURCE-CODE FACT.** `TLPage.selectedShapeIds` is declared in `TLPage.ts:211-230` with
`ephemeralKeys.selectedShapeIds: false` — i.e. it is **not** ephemeral. Selection changes are written under
`history: 'record-preserve-redo-stack'` (`Editor.ts:2175`, `:2748`), which reaches
`HistoryManager` state `recording_preserving_redo_stack` and therefore calls
`this.pendingDiff.apply(entry.changes)` (`HistoryManager.ts:55-56`).

**SOURCE-CODE FACT.** Only `editingShapeId`, `hintingShapeIds`, `erasingShapeIds`, `hoveredShapeId`, and
`focusedGroupId` carry `true`.

**CORRECTION (precise form).** Selection changes **are** accumulated into the pending diff and **are** part of
history entries. They are (a) not re-applied when history is replayed, because `_undo`/`redo` call
`applyDiff(..., { ignoreEphemeralKeys: true })`, and (b) not a reason to clear the redo stack, because the
mode is `record-preserve-redo-stack`. “Not in history” is wrong; “not restored by history” is right.

**INFERENCE.** The practical consequence: a region that contains both a shape move and a selection change
replays the move and silently discards the selection. Any test or UX assumption that “undo restores
selection” is false in tldraw.

### CORRECTION 2 — Cancel is not “just don't record”

**CORRECTION.** `docs/research/architecture/history.md` implies Escape works by never recording the
interaction, so no undo entry exists.

**SOURCE-CODE FACT.** Escape routes to `Translating.cancel()` → `reset()` → `editor.bailToMark(markId)`
(`Translating.ts:237-260`, `:197-204`), and `bailToMark` calls
`_undo({ pushToRedoStack: false, toMark: id })` (`HistoryManager.ts:186-190`), which **applies a reversed
diff to the store** (`HistoryManager.ts:198`).

**CORRECTION (precise form).** Cancel is a *destructive* history operation. It reverses the pending region,
applies it to undo the gesture's writes, and **discards** the popped stack entries instead of moving them to
the redo stack. The pre-gesture region is *erased from the undo spine*, so `⌘Z` after Escape undoes whatever
came *before* the gesture, not the gesture itself.

**INFERENCE.** This is the mechanism behind a rule that is easy to state and easy to get wrong: **cancel is
not undo.** If a design assumed Escape merely suppressed recording, it would be surprised that bail also
truncates history and cannot be reversed.

### CORRECTION 3 — `squashToMark` exists, is fully implemented, and has exactly one caller

**SOURCE-CODE FACT.** `HistoryManager.squashToMark` (`:268-301`) walks `undos`, collects `diff` entries into
`popped`, discards intermediate `stop`s, **reverses** the collected list, squashes them into one diff with
`squashRecordDiffsMutable`, and pushes the result at the target position. `redos` is untouched. On a miss it
logs `console.error('Could not find mark to squash to: ', id)` and returns.

**SOURCE-CODE FACT.** The only non-test caller in the pinned tree is `Crop.ts:26`, guarded by
`if (this.editor.getMarkIdMatching(this.markId) === this.markId)`.

**INFERENCE.** Squashing is a first-class history primitive with a real algorithm and real test coverage
(`HistoryManager.test.ts:435`, order-sensitive), but tldraw itself barely uses it. It is an *available*
capability, not a *relied-upon* one — which makes it useful comparative evidence rather than a pattern to
copy.

### Edge cases that matter for any reimplementation

| # | Case | Source behavior |
|---|---|---|
| 1 | Undo with an empty pending region and a leading `stop` | The `stop` is popped and skipped, so a boundary is never itself an undo step. |
| 2 | `bailToMark` miss | `pendingDiff.restore(pendingDiff)` then early `return`; **stacks and store both untouched**, no throw. |
| 3 | `squashToMark` miss | `console.error` + return; spine unchanged. |
| 4 | Undo while undoing (`_isReplaying`) | `state = 'paused'`, so the interceptor records nothing; the store’s own re-entrancy guards prevent stack growth. (`HistoryManager.test.ts:233`) |
| 5 | Operation on an empty stack | `EMPTY_STACK_ITEM` sentinel; the while loop exits on `type === 'empty'`; no throw. |
| 6 | Nested `batch` | Reentrant via `_isInBatch`; inner call is `store.atomic(fn)` only; outer restores state in `finally`. |
| 7 | Exception inside `batch` | `annotateError(error)` then rethrow. `store.atomic` has **no rollback**. The test at `:361`-block asserts the system is usable afterwards, not that state was restored. |
| 8 | Stack depth > 1000 | `Store` caps history accumulation at `historyLength: 1000` (`Store.ts:361-363`). The `HistoryManager` stacks themselves are unbounded persistent lists. |
| 9 | Replaying a diff whose records were edited remotely | Mitigated by `mergeRemoteChanges` being tagged `'remote'` and therefore never entering the local stack; but `applyDiff` is not conflict-aware. |
| 10 | Store considered corrupt | `markAsPossiblyCorrupted` (`Store.ts:1175-1183`), raised from `Editor.annotateError` when `willCrashApp`; `TldrawEditor` renders `<Crash>` which rethrows (`TldrawEditor.tsx:766-770`, `:799-800`); `TLLocalSyncClient` refuses to persist when `isPossiblyCorrupted()` (`TLLocalSyncClient.ts:390`, `:411`); `_flushEventForTick` early-returns. |
| 11 | `flushAtomicCallbacks` recursion | Waves up to depth 100, then throws (`Store.ts:1199-1230`). |
| 12 | History interceptor added late | `addHistoryInterceptor` (`Store.ts:1280-1285`) re-reads `isMergingRemoteChanges` **at delivery time**, so an entry delivered after a remote merge starts is tagged `'remote'` even if it was produced before. |

---

## What Is Still Unknown

Things this investigation **could not** establish from the pinned source, stated so they are not mistaken for
findings:

1. **Whether the call-site convention is actually adhered to at scale.** 120 mark sites were located and the
   interaction families were traced, but no mechanism enforces it and the whole-repo correctness of the
   convention was not audited line by line. Some call sites may be misplaced or missing.
2. **Intended semantics of a mark that is placed but never squashed or bailed.** The system leaves these as
   plain spine entries; nothing in the source says whether that is intended or merely tolerated.
3. **Why `record-preserve-redo-stack` exists beyond selection.** The name is a string literal; the only
   in-source justification is the `TLPage.ts:213-218` comment about ephemeral keys, which explains a
   *different* mechanism. The causal reason for the third mode is not documented.
4. **Interaction with persistence.** `TLLocalSyncClient` refuses to persist a store flagged as possibly
   corrupted, but the full path from `Store` to the local database — and whether history is persisted at all —
   was outside this investigation.
5. **Multiplayer merge semantics.** `mergeRemoteChanges` is a single tag switch; the actual merge/rebase of
   concurrent diffs is not in this tag's history path.
6. **Anything about AI.** No AI history surface exists at this commit (§AI and History).
7. **Product-level tldraw.com behavior.** `apps/*` was deliberately not inspected; every claim here is about
   the OSS packages at `v5.5.1`.

---

## Sources

**Repository.** `https://github.com/tldraw/tldraw`

**Version.** Tag `v5.5.1`, commit `0fc68fd86b819fe1c98eefe61fbe6dad217be6a3`. Packages inspected:
`packages/editor`, `packages/store`, `packages/tldraw`, `packages/valtio`, `packages/state-react`,
`packages/tlschema`. Not inspected: `apps/*`.

**Primary files** (line counts as read):

| File | Lines |
|---|---|
| `packages/editor/src/lib/editor/managers/HistoryManager/HistoryManager.ts` | 432 |
| `packages/editor/src/lib/editor/managers/HistoryManager/history-types.ts` | 27 |
| `packages/editor/src/lib/editor/managers/HistoryManager/HistoryManager.test.ts` | 910 |
| `packages/store/src/lib/RecordsDiff.ts` | 347 |
| `packages/store/src/lib/Store.ts` | 1467 |
| `packages/store/src/lib/RecordType.ts` | — |
| `packages/editor/src/lib/editor/Editor.ts` | 11735 |

**Secondary files** (representative): `Editor.ts` call sites enumerated inline;
`packages/tldraw/src/lib/tools/Translating/Translating.ts`;
`packages/tldraw/src/lib/tools/Resizing/Resizing.ts`;
`packages/tldraw/src/lib/tools/Crop/Crop.ts`; `packages/tldraw/src/lib/tools/Drawing.ts`;
`packages/tldraw/src/lib/tools/Rotating/Rotating.ts`; `packages/tldraw/src/lib/tools/Erasing.ts`;
`packages/tldraw/src/lib/ui/context/actions.tsx`;
`packages/tldraw/src/lib/ui/hooks/useCanvasEvents.ts`; `packages/tldraw/src/lib/ui/hooks/useClipboardEvents.ts`;
`packages/editor/src/lib/editor/bindings/BindingUtil.ts`;
`packages/tlschema/src/pages/TLPage.ts`; `packages/tlschema/src/shapes/TLShape.ts`;
`packages/tlschema/src/pages/TLComment.ts`; `packages/tldraw/src/lib/TldrawEditor.tsx`;
`packages/sync-core/src/lib/TLLocalSyncClient.ts`.

**Existing Spool corpus files consulted for cross-checking** (read-only, not modified):
`docs/research/architecture/history.md`, `docs/research/matrices/history-matrix.md`,
`docs/research/architecture/editor-runtime.md`, `docs/research/architecture/input-system.md`,
`docs/research/architecture/ai-runtime.md`, `docs/research/architecture/document-model.md`,
`docs/research/architecture/interaction-runtime.md`, `docs/research/profiles/tldraw.md`.

**Label key.** `SOURCE-CODE FACT` = read directly from the pinned source. `DOCUMENTED FACT` = stated in
source comments, tests, or the existing corpus. `INFERENCE` = my reading, not stated in source.
`CORRECTION` = a claim in the existing corpus that this investigation found to be wrong.
