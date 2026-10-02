# Architecture Extraction: History

## The core question

> What does the user perceive as ONE undoable action?

This is the question the research brief calls out, and it is the one where tldraw's documentation is
decisively better than every other product's.

## Four designs, four different answers

| Product | Model | Documentation quality |
|---|---|---|
| **Figma** | Local undo + named **version history** + **branching** | Product: high. Mechanism: **none** |
| **tldraw** | **Marks + diffs + three capture modes + bail + squash** | **Excellent** |
| **Affinity** | `⌘Z` / `⇧⌘Z`; scripting applies "often as a single undoable action" | Mechanism: **none** |
| **Canva** | Undo + **1,000 attributed versions** + **Trash** | Versions: high. Undo: **none** |

[DOCUMENTED where cited]

---

## tldraw's model, in full

[DOCUMENTED — `sdk-features/history.mdx`]

### Structure

> "The history manager maintains **two stacks**: one for undos and one for redos. **Each stack contains
> entries that are either diffs (record changes) or marks (stopping points).**"

> "When you modify the store, the history manager captures the change as a diff. **Changes accumulate until
> you create a mark**, then the pending changes are flushed to the undo stack as **a single entry**. This
> batching prevents every keystroke or mouse movement from becoming a separate undo step."

> "When you undo, the manager reverses all changes back to the previous mark, moves them to the redo stack,
> and applies the reversed diff **atomically**."

### The mark

```ts
const markId = editor.markHistoryStoppingPoint('rotate shapes')
editor.rotateShapesBy(editor.getSelectedShapeIds(), Math.PI / 4)
// Undoing will return to this mark
```

> "Creating a mark flushes pending changes onto the undo stack. **It doesn't clear the redo stack; the next
> recorded change does that.**"

### Three capture modes

| Mode | Undo stack | Redo stack | Documented use |
|---|---|---|---|
| `record` | Add | Clear | default |
| `record-preserveRedoStack` | Add | **Keep** | **selection changes** — "you can undo, select some shapes, copy them, and then redo back to where you were" |
| `ignore` | Skip | Keep | **writing your own pointer position** — "Where your cursor was doesn't need to be undoable" |

> "Nested `run` calls keep the outer mode unless they set their own, and **no mode is applied while an undo or
> redo is in progress**."

### Bail — discard

```ts
const markId = editor.markHistoryStoppingPoint('begin drag')
// User drags shapes around
// User presses escape to cancel
editor.bailToMark(markId)   // Roll back and discard all changes since mark
```

> "Bailing reverses changes **without adding them to the redo stack**. The changes are discarded entirely.
> Use this when canceling an interaction."

> "We use bailing while cloning shapes. A user can switch between translating and cloning by pressing or
> releasing the alt (option) key during a drag. When this changes, **we bail on the changes since the
> interaction started, then apply the new mode's changes.**"

### Squash — merge retroactively

```ts
const markId = editor.markHistoryStoppingPoint('bump shapes')
editor.nudgeShapes(shapes, { x: 10, y: 0 })
editor.nudgeShapes(shapes, { x: 0, y: 10 })
editor.nudgeShapes(shapes, { x: -5, y: -5 })
editor.squashToMark(markId)   // All three nudges become one undo step
```

> "Squashing **doesn't change the current state, only how history is organized**. Intermediate marks are
> removed."
>
> "**We use squashing during image cropping.** While the user adjusts the crop, each change is recorded and
> can be undone individually. When the user exits crop mode, we squash the intermediate changes into one
> history entry. A single undo restores the image to its state before cropping began."

### Capture rules

> "The history manager listens to store changes through a history interceptor. It **only captures changes
> with source `'user'`**; changes merged from other clients (source `'remote'`) are ignored, and internal
> writes that shouldn't be undoable use `history: 'ignore'`. **Only store records take part in undo/redo;
> state held outside the store, like your own atoms, does not.**"
>
> "The three history modes map onto three internal states: `Recording`, `RecordingPreserveRedoStack`, and
> `Paused`. The manager **pauses itself while applying an undo or redo** so those writes don't create new
> entries."

---

## The five concepts that must be extracted

### 1. Mark — the unit of "one undoable action"

**Problem:** A 600px drag generates hundreds of intermediate writes. A text-edit session generates dozens.
Grouping must happen somewhere.

**Evidence:** tldraw DOCUMENTED.

[INFERRED] **A mark is placed at the start of a user-intent interaction by the tool/interaction runtime**, not
by the history system. The history system only knows marks exist.

**Open question:** what happens to marks when a document is saved? tldraw's history is session state (it lives
outside the store), so it is not persisted. **Verify.**

### 2. Capture mode — what participates

**Problem:** Not everything the editor writes should be undoable. Cursor position, selection, and remote
changes should not clear the redo stack.

**Evidence:** tldraw DOCUMENTED, with two named use cases.

[INFERRED] This is a **per-write policy**, and the `preserveRedoStack` variant is a subtle but important
distinction that most hand-rolled implementations miss.

### 3. Bail — cancel

**Problem:** An aborted interaction should leave no trace, and should not destroy the redo stack.

**Evidence:** tldraw DOCUMENTED, used for Escape and for mid-gesture modifier changes.

[INFERRED] Bail and undo differ in exactly one respect: **bail does not push to the redo stack**. That is
what makes "press Escape and then Cmd+Shift+Z" behave the way users expect.

### 4. Squash — merge

**Problem:** Some interactions want *fine-grained* undo during the gesture and *coarse* undo after it.

**Evidence:** tldraw DOCUMENTED, used for crop mode.

[INFERRED] Squash is the inverse of mark-per-action: it retroactively collapses. Without it, either crop is
one step (bad: you cannot undo a single crop adjustment) or it is many (bad: you need twenty undos to get
out of crop mode).

### 5. Pause — re-entrancy

**Problem:** Applying an undo writes to the store, which the history manager observes. Without a pause, undo
creates new entries.

**Evidence:** tldraw DOCUMENTED — "The manager pauses itself while applying an undo or redo."

[INFERRED] This is a correctness detail that is easy to miss and produces catastrophic behaviour (undo that
breaks history) if omitted.

---

## Version history — the other system

[DOCUMENTED]

| | Figma | Canva |
|---|---|---|
| Granularity | File | Design |
| Naming | Yes (create, name, remove) | No |
| Retention | Unknown | **Up to 1,000 versions, no time limit** |
| Attribution | Unknown | **Avatars next to each saved version, indicating who edited it** |
| Branching | **Yes — merge creates an extra checkpoint first** | No |
| Access | Edit access | Owner or edit access; free users have none |
| Comparison | Yes | Yes ("Select the saved versions to compare them") |
| Copy a version | Unknown | **Yes — "Make a copy"** |

[Fully DOCUMENTED — Figma "View a file's version history", "Guide to branching"; Canva "Review and restore
older versions of designs"]

[INFERRED] **Version history is snapshot-based, server-side, and attributed.** It answers "what did this
document look like on Tuesday?", not "undo my last three actions". Conflating the two is a common modelling
error.

[INFERRED] Canva's **Trash** is a third system: whole-object recovery on a 30-day clock. Distinct from both
undo and version history, and Canva's documentation is explicit about the distinction.

---

## History and the other subsystems

| Subsystem | Interaction with history | Evidence |
|---|---|---|
| **Selection** | Should be undoable, must preserve redo | tldraw DOCUMENTED |
| **Camera** | Should **not** be in undo (no product documents camera history) | INFERRED |
| **Streaming agent output** | One transaction spanning the stream; abortable | INFERRED from tldraw streaming |
| **Remote collaboration** | Remote changes are never recorded | tldraw DOCUMENTED |
| **Bindings** | Records, so they participate | tldraw DOCUMENTED |
| **Cascade updates** | `onOperationComplete` collapses N binding writes into an aggregate | tldraw DOCUMENTED |
| **Layout** | A resize that moves 100 children is one entry | INFERRED |
| **Side effects** | Derived writes are part of the entry or are `ignore`d | INFERRED |
| **Detach instance** | Should be one entry | INFERRED |
| **Crop mode** | Squash on exit | tldraw DOCUMENTED |

---

## Spool prototype: the current history

From `app/src/canvas.rs` [OBSERVED in source]:

```rust
pub struct History { undo: Vec<DocumentCommand>, redo: Vec<DocumentCommand> }

pub struct DocumentCommand { operation: CommandOperation }

enum CommandOperation {
    Geometry(Vec<GeometryChange>),   // { id, before: Geometry, after: Geometry }
    Style(Vec<StyleChange>),         // { id, before: ObjectStyle, after: ObjectStyle }
    Text(Vec<TextChange>),           // { id, before: String, after: String }
    Insert(Vec<ObjectPlacement>),    // { object, index }
    Delete(Vec<ObjectPlacement>),
}
```

`History::record` filters out no-op changes and clears the redo stack. `undo`/`redo` apply the inverse.

**What is right:**

1. **Value-snapshot commands**, not deltas — no inverse-operation logic to get wrong.
2. **`Insert`/`Delete` carry the index**, so z-order is restored exactly. That is a detail many
   implementations get wrong.
3. **No-op filtering** on record.
4. **One drag = one command** (`geometry_command` is called once on commit).
5. **One text session = one command** (`commit_text_edit` produces a single `TextChange`).
6. **Escape leaves no history entry** — verified by the test
   `escape_cancels_creation_without_history_or_selection_changes`.

**What is missing:**

| Missing | Consequence |
|---|---|
| **No undo/redo key binding** | `main.rs` binds no `cmd-z` / `cmd-shift-z`, despite `History` existing. [OBSERVED] |
| **Selection changes are not recorded** | The user cannot undo a wrong selection |
| **No bail primitive** | Escape works by *not recording* rather than by *bailing*. This works for a pure-from-start gesture but breaks the moment incremental mutation is involved. |
| **No squash** | No interaction can have fine-grained-during / coarse-after undo |
| **No capture modes** | Everything recorded clears redo; nothing can opt out |
| **No remote-source concept** | Not needed yet (no collaboration), but will be |
| **No pause on undo** | `undo()` calls `document.set_geometry` etc. directly, not through the history interceptor, so it is safe *by construction* — but this is fragile if a store abstraction is added later |
| **History is owned by `CanvasView`** | Cannot be shared, serialised, or tested independently |
| **No version history** | No long-horizon recovery |

[INFERRED] **Escape-cancels-by-not-recording is a valid design *only* while gestures are pure functions of
(start, pointer, modifiers).** It is a cheaper alternative to bail. The moment a gesture writes incrementally
(as `apply_move` does), "not recording" no longer undoes anything.

---

## Candidate architectural implications

### Implication A — keep value snapshots; add transaction scope

**Evidence:** Spool's snapshot commands are correct and simple. tldraw's diffs require inverse logic.

**Why it matters:** Snapshot commands cannot express "add a property to 10,000 objects" cheaply, and they
duplicate memory per undo entry. Diffs are cheaper but need correct inverses.

**Approaches:**
- **A. Keep snapshots; add `Transaction { commands }` so one entry can hold several commands.**
- **B. Diffs** (tldraw's model).
- **C. Snapshots with structural sharing** (e.g. an immutable arena / `Arc` per object) so unchanged objects
  are not duplicated.

[INFERRED] **A is the right near-term move and is cheap.** C is the long-term answer if history memory
becomes a problem.

**Decision: TBD — requires architecture review.**

---

### Implication B — add a bail primitive and make gestures pure

**Evidence:** tldraw DOCUMENTED (bail + modifier-change re-apply). Spool's Escape currently works by not
recording.

**Why it matters:** Without bail, "press Escape mid-drag" must either have already mutated the document
(explicit restore, as `Interaction::restore()` does) or never have mutated it. Spool chooses the former, which
means every gesture mutates live and every cancel needs a manual restore. That is more code and more places
to be wrong.

**Approaches:**
- **A. Bail: don't mutate until commit.** Keep a pending operation; apply on pointer-up. Cleanest; costs a
  preview mechanism (which `CreationPreview` already provides for create).
- **B. Mutate live + explicit restore on cancel.** What Spool does. Works; more state.
- **C. B + a bail() helper** on the history stack that reverts to the mark, for cases where restore is not
  feasible.

[INFERRED] A is the architecturally cleaner choice and also matches "interactions are pure functions of
(start, pointer, modifiers)". It has a real cost: rendering a *pending* transform (a drag preview that is not
yet in the document), which tldraw handles by... actually mutating. So there is a genuine tradeoff.

**Decision: TBD — requires architecture review.**

---

### Implication C — selection in history, with preserve-redo

**Evidence:** tldraw DOCUMENTED, with the rationale stated.

**Why it matters:** It is a small feature with a subtle correctness argument, and it makes
"select the wrong thing, Ctrl+Z" work.

**Decision: TBD — requires architecture review.** Low effort, well-specified.

---

### Implication D — version history is a separate system

**Evidence:** Figma and Canva both ship snapshot-based, server-side, attributed version history alongside
local undo.

**Why it matters:** Modelling them as one system produces either infinite memory or lost recovery.

[INFERRED] For a local-first open-source editor, **autosave-with-rotation** is the equivalent: keep the last
N snapshots on disk, attributed to whoever saved. This is cheap and gives Canva's capability.

**Decision: TBD — requires architecture review.**

---

### Implication E — squash for mode-based interactions

**Evidence:** tldraw DOCUMENTED (crop).

**Why it matters:** Any future mode-based interaction (crop, pen path editing, gradient editing) will want
fine-grained-during / coarse-after undo.

[INFERRED] Low priority until such an interaction exists.

**Decision: TBD.**

## Open questions

1. Snapshot commands or diffs?
2. Does a transaction hold multiple commands?
3. Is there a bail primitive, and do gestures mutate live or defer?
4. Is selection recorded (with preserve-redo)?
5. Is there a capture-mode policy, or does everything clear redo?
6. Is there a squash primitive?
7. Is history owned by the editor or by the document?
8. Is there a version history / autosave-rotation, separate from undo?
9. Are remote/collab changes excluded?

## Sources

- tldraw: `sdk-features/history.mdx` ⭐⭐⭐, `sdk-features/store.mdx` (history interceptor, record scopes),
  `sdk-features/actions.mdx` (undo/redo actions, `readonlyOk`), `sdk-features/bindings.mdx`
  (`onOperationComplete`)
- Figma: "View a file's version history" (/360038006754); "Guide to branching" (/360063144053);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914) (undo shortcut)
- Affinity: "Scripting in Affinity" (/scripting-in-affinity/) ⭐ ("often as a single undoable action");
  "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/) (`⌘Z`, `⇧⌘Z`)
- Canva: "Review and restore older versions of designs" (/version-history/) ⭐⭐ (1,000 versions, avatars,
  make a copy, access rules); "Restore or delete designs and files from Trash" (/deleted-designs/) ⭐
- Spool prototype: `app/src/canvas.rs` (`History`, `DocumentCommand`, `CommandOperation`, `GeometryChange`,
  `StyleChange`, `TextChange`, `ObjectPlacement`, `History::record/undo/redo`, `geometry_command`,
  `commit_text_edit`, `Interaction::restore`), `app/src/main.rs` (no undo binding)
