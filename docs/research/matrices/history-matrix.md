# History Matrix

> §20 and §38. What each product treats as **one undoable action**, and how the boundary is decided.
> Companion to `architecture/history.md`, which carries the full treatment; this file is the comparison.
>
> Legend: **✔** · **◐** partial · **—** absent · **?** undocumented

---

## 1. The core question: what is one undoable action?

| User gesture | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| A 600 px drag | **1 step** | **1 step** | **1 step** | **1 step** | **1 step** ✔ |
| A multi-selection move | 1 step | 1 step | 1 step | 1 step | 1 step ✔ |
| A text editing session | 1 step | 1 step | 1 step | 1 step | **1 step** ✔ |
| A style change across 20 objects | 1 step | 1 step | 1 step | 1 step | 1 step ✔ |
| Creating 5 objects in 5 gestures | 5 steps | 5 steps | 5 steps | 5 steps | 5 steps ✔ |
| An align operation | 1 step | 1 step | 1 step | 1 step | **—** |
| A paste | 1 step | 1 step | 1 step | 1 step | 1 step ✔ |
| A group operation | 1 step | — | 1 step | 1 step | **—** |
| A script invocation | ? | — | **"often" 1 step** | ? | **—** |
| An AI operation | 1 step (normal undo) | 1 step | 1 step | 1 step | **—** |
| A whole AI refactor | ? | ✔ via `editor.run` squash | ? | ? | **—** |

[INFERRED] **All four products converge on the same answer: one continuous gesture is one step.** This
is the most standardised behaviour in the entire corpus, and Spool already matches it. The interesting
divergence is entirely in *how* that is achieved.

---

## 2. Representation

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Command objects | ? internal | ✔ `Command`s | ? | ? | ✔ `DocumentCommand` |
| Before/after **value snapshots** | ? | — | ? | ? | ✔ **yes** |
| **Diffs against a mark** | ? | ✔ **yes** | ? | ? | — |
| Inverse operation computed | ? | ✔ (diff reversal) | ? | ? | ✔ (stored after-state) |
| Z-order position stored with the op | ? | ✔ (fractional index) | ? | ? | ✔ (`Insert`/`Delete` carry index) |
| Operation granularity | ✔ typed | ✔ typed | ✔ | ? | ✔ `CommandOperation` enum |
| Structural sharing | ? | ? | ? | ? | **—** |
| Copy-on-write document | ? | ✔ **reactive store** | ? | ? | **—** |

[INFERRED] Spool's snapshot approach is the *easier* of the two designs and is entirely defensible. The
differences appear at scale: snapshots are O(document) per step, diffs are O(change). tldraw's choice
is forced by its reactive store; Spool's is not, so it is currently unforced and unmotivated.

---

## 3. Capture boundaries

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Marks (open a named point) | ? internal | ✔ **yes** | ? | ? | **—** |
| Bail (revert to a mark, keep gesture) | ✔ discard | ✔ **`Alt`-switch + `Esc`** | ✔ discard | ✔ discard | ✔ discard |
| Squash (collapse a range) | ✔ implicit | ✔ **`crop`** | ? | ? | **—** |
| Pause history entirely | ? | ✔ **`history: 'ignore'`** | ? | ? | **—** |
| Caller-chosen scope | ? internal | ✔ **`editor.run(fn, opts)`** | ◐ via scripting | ? | **—** |
| No-op diffs filtered | ? | ✔ | ? | ? | ✔ **`record()` filters** |
| Redo cleared on new change | ✔ | ✔ | ✔ | ✔ | ✔ |

[INFERRED] tldraw is the only product whose history boundary is a **public, caller-chosen parameter**.
That is what makes it usable by tools, plugins and agents; Figma/Canva's boundaries are internal
product decisions. `architecture/ai-runtime.md` Implication C depends on this being public.

---

## 4. What is *not* in history

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Selection | **✘** | **✘** | **✘** | **✘** | **✘** ✔ consistent |
| Camera | **✘** | **✘** | **✘** | **✘** | **✘** ✔ consistent |
| Hover | ✘ | ✘ | ✘ | ✘ | ✘ |
| Active tool | ✘ | ✘ | ✘ | n/a | ✘ |
| Panel layout | ✘ | ✘ | ✘ | ✘ | ✘ |
| Open/closed panels | ✘ | ✘ | ✘ | ✘ | ✘ |

[INFERRED] **This is the strongest universal convention in the corpus** — all four exclude selection and
camera. Spool matches it. The convention is not arbitrary: undo restores *the artwork*, not the *view*
of the artwork.

---

## 5. Undo *during* a live gesture

| Situation | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Press `⌘Z` mid-drag | gesture ends / ignored | **possible with bail** | ? | ? | **not modelled** |
| Press `⌘Z` mid-text-edit | ? | ✔ | ? | ? | **not modelled** |
| Switch modifier mid-drag | ignored | **`Alt` re-enters as preview** | **re-read** (`⌃`-shear) | n/a | n/a |

[INFERRED] Only tldraw treats undo as something that can happen *during* a gesture, and only because
bail makes the gesture reversible. Everything else makes undo unavailable mid-gesture. This is the
mechanical reason bail exists.

---

## 6. Long-horizon history — the second system

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Version history | ✔ | **—** | ✔ | ✔ | **—** |
| Per-version author | ✔ | — | ✔ | ✔ **avatars** | — |
| Per-version timestamp | ✔ | — | ✔ | ✔ | — |
| Manual naming of versions | ✔ | — | ✔ | ✔ | — |
| **Branching from a version** | ✔ **yes** | — | ? | **✘** | — |
| Retention limit | time-limited | — | ? | **1000 versions, no time limit** | — |
| Trash / soft-delete | ✔ 30 days | — | ✔ | ✔ **30 days** | — |
| Restoring a deleted object | ✔ | ? | ✔ | ✔ | **—** |
| Export at a past version | ✔ | — | ✔ | ✔ | — |

[INFERRED] **Undo and version history are separate systems in every product that has both.** Figma
branches; Canva stores a deep, unbounded, attributed log. Neither derives one from the other. Spool
needs to decide which, if either — and `architecture/history.md` Implication E notes that a local-first
Spool could approximate Canva's policy cheaply via autosave rotation.

---

## 7. Deletion semantics

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Delete is undoable | ✔ | ✔ | ✔ | ✔ | ✔ |
| Delete is reversible *outside* undo | ✔ version history | ? | ✔ | ✔ trash + version history | **—** |
| Deleting a container keeps children | ✔ | ✔ | ✔ | ✔ | n/a |
| **Delete vs binding isolation** | ◐ | ✔ **modelled explicitly** | ◐ | — | n/a |
| Asset deletion blocked when referenced | ✔ | ✔ | ✔ | ✔ **documented** | **—** |

[INFERRED] tldraw is the only product that documents *deletion semantics for references* — binding
isolation versus cascading deletion. Everywhere else this is unspecified, and it is exactly the
question a document model must answer before bindings exist.

---

## 8. What the matrix exposes

[INFERRED] Four findings:

1. **The user-visible contract is universal.** One gesture = one step, in all four products, and Spool
   already matches it. This is the part of history that is genuinely settled.
2. **The mechanism is where they diverge.** Snapshots (Spool) vs diffs+marks (tldraw) is the whole
   debate, and it is a debate about *cost at scale*, not about behaviour.
3. **Only tldraw exposes the boundary.** Everything else hard-codes it. Spool's AI ambition and its
   script/plugin ambition both require a public boundary — this is a hard prerequisite, not a nicety.
4. **Selection and camera are excluded everywhere, without exception.** Spool is consistent. This
   should be written down as a *rule*, because the tempting feature (restore selection on undo) is a
   divergence from all four products and would need its own justification.

[INFERRED] A fifth, quieter point: **no product in the corpus lets a user configure history
granularity.** If Spool's profiles were to expose it, that would be a genuine novelty and a genuine
hazard (`architecture/profiles.md` Implication B).

## 9. Spool's ten-row gap, condensed

[SOURCE-CODE] From `architecture/history.md`, the gaps in one view:

| Capability | Spool | Cost to close |
|---|---|---|
| Undo/redo reachable by keyboard | ✔ (via raw key path) | move to the action path |
| Marks | ✘ | medium |
| Bail | ✘ | medium–high (needs Alt-mode + a gesture re-entry) |
| Squash | ✘ | medium |
| Pause history | ✘ | low |
| Caller-chosen scope (`run(fn)`) | ✘ | low |
| Selection with redo preserved | ✘ | low |
| Selection restored on undo | ✘ **and contrary to all four products** | — deliberate decision needed |
| History owned by the view | ✘ | **structural** |
| History serialisable | ✘ | follows from the above |

## 10. Sources

- tldraw (SOURCE-CODE) ⭐⭐⭐: marks, diffs, bail, squash, three capture modes, `editor.run`, binding
  isolation-vs-deletion — see `architecture/history.md`
- Figma (DOCUMENTED): undo behaviour, version history, branching, 30-day trash
- Affinity (DOCUMENTED): undo, scripting API "often as a single undoable action"
- Canva (DOCUMENTED): `⌘Y`/`⇧⌘Z`, version history (1000, attributed, unbounded), 30-day trash,
  reference-blocked asset deletion
- Spool (SOURCE-CODE): `History`, `DocumentCommand`, `CommandOperation`, `record()`, `undo()`, `redo()`,
  `shell.rs::ShortcutAction::{Undo, Redo}` in `app/src/canvas.rs`
- Cross-references: `architecture/history.md`, `architecture/profiles.md` (Implication B),
  `architecture/ai-runtime.md` (Implication C), `matrices/feature-matrix.md` §9
