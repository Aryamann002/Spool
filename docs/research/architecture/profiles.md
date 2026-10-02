# Architecture — Interaction Profiles

> Research note. Layer 2 of three. §23 and §24 of the brief ask whether the concept
> `Interaction Profile { Spool | Figma | tldraw | Affinity | Canva }` is architecturally reasonable,
> and which behaviours belong in a profile versus the core editor.
> This note answers that question **as evidence**, and stops before a decision.

---

## 1. The question, restated precisely

An interaction profile is a *named bundle* that changes how the same document is manipulated. For it
to be worth building, three things must be separable:

1. **Invocation** — which keys and gestures start an operation.
2. **The operation itself** — what `MoveObjects` means.
3. **The available vocabulary** — which tools, modes, and modifiers exist at all.

Most of what people call "feel" lives in (1) and (3). Almost nothing lives in (2). That is the
finding that makes profiles tractable.

---

## 2. What the products actually expose

### 2.1 Figma — layouts, not profiles

[DOCUMENTED] Figma ships **16 keyboard layouts**, selectable in Preferences → Keyboard shortcuts.
They are regional input conventions (`QWERTY`/`AZERTY`/`QWERTZ`, `WASD`-style arrow clusters, and so
on). They change *which physical keys* trigger actions. They do not change:
- the tool set,
- modifier semantics,
- gesture behaviour,
- menu structure.

Figma does **not** document any user rebinding of individual shortcuts.

[INFERRED] So "Figma-like" as a profile is, for Figma itself, a *locale* concept rather than an
*ergonomics* concept. A Spool profile called `figma` would have to be authored from Figma's observed
behaviour, not imported from it.

### 2.2 tldraw — a configurable architecture, no shipped profile

[SOURCE-CODE] tldraw is the opposite case, and the most useful one:

- `@tldraw/editor` has **no built-in tools at all**. Every tool lives in the consumer.
- Tools declare a static config table: `id`, `initial` state, `children`, `isLockable`,
  `useCoalescedEvents`, `trackPerformance`.
- Actions are a registry of ~100 entries each with a `kbd` string, plus three key-matching strategies
  and explicit **gating rules** (an action declares what state/selection it requires).
- Tools check `isToolLocked` themselves; the editor does not enforce it.
- `editor.run(fn, { history: 'record' | 'ignore' | 'squash' })` lets a caller wrap any behaviour in a
  history boundary.

[INFERRED] tldraw's answer to "which profile is active?" is: **the host application decides, by
deciding which tools to register.** A profile in tldraw is a plugin composition problem, not a
settings problem. That is the cleanest possible separation, but it also means tldraw ships nothing
that looks like a profile to a user.

### 2.3 Affinity — Studios: the concept is already shipped

[DOCUMENTED] Affinity's workspaces are called **Studios**. Documented studios include Vector, Pixel,
Layout, Canva AI, Slice, Retouching, Color Grading, Typography, Compositing, Astrophotography, and
Scripting. Crucially, the Personas page documents that studios can be **fully customised** — the user
can create their own, choose which panels appear, and reorder them.

[INFERRED] An Affinity Studio is *not* primarily an interaction profile. It is closer to:
- a **capability gate** (Pixel Studio's tools do not exist in Vector Studio),
- plus a **panel layout**,
- plus a **default tool**.

That combination is the important part: it shows the product treats "what exists" and "where it
lives" as one decision, while leaving the gestures alone.

[DOCUMENTED] Affinity *also* has full shortcut customisation, and accepts `/` only on
"International and selected keyboards". So Affinity separates:
- **Studio** = which tools + panels + defaults,
- **Shortcut customisation** = which keys,
- **Snapping presets** = which snapping rules,
and exposes each independently.

[DOCUMENTED] Snapping presets are a striking precedent: 7 named presets (including *UI design* and
*Pixel work*), each selecting a bundle of snapping behaviours. **This is an interaction profile,
scoped to one subsystem, shipped in a shipping product.** It is the clearest evidence in the corpus
that the concept is reasonable.

### 2.4 Canva — profiles exist as product tiers, not as settings

[INFERRED] Canva's effective profiles are: Free design, Pro design, Presentation, Print, Whiteboard,
and the Magic Studio AI surfaces. They differ in *capability*, not in *feel*. Canva does not appear
to expose a control that changes how the same operation responds to modifiers.

[DOCUMENTED] Canva's one genuine feel-level convention is the `⌘F1`/`⌘F2`/`⌘F3` focus ladder, which is
a state distinction rather than a setting.

### 2.5 Summary table

| | Invocation (keys) | Modifier semantics | Tool vocabulary | Panels/layout | Gestures |
|---|---|---|---|---|---|
| Figma | 16 layouts, no rebinding | fixed | fixed | fixed | fixed |
| tldraw | host-defined `kbd` registry | host-defined | host-defined | host-defined | host-defined |
| Affinity | **fully customisable** | fixed | **by Studio** | **by Studio** | fixed |
| Canva | fixed | fixed | fixed | fixed | fixed |

**No product exposes modifier semantics or gesture behaviour as user configuration.** This is the
single most important negative finding in the corpus.

---

## 3. Why "profile = keymap" is insufficient, and what it actually covers

Reading the evidence, a profile has five separable axes:

| Axis | Who varies it | Evidence | Configurable today? |
|---|---|---|---|
| **A. Keymap** | user | Affinity full customisation; Figma 16 layouts; tldraw registry | yes (Affinity), partly (others) |
| **B. Tool set** | profile | Affinity Studios | yes (Affinity), implicitly (tldraw host) |
| **C. Contexts / focus modes** | profile | Canva `⌘F1/F2/F3`; GPUI `KeyContext` | no (as a user concept) |
| **D. Snapping rule bundle** | user | Affinity 7 presets | yes (Affinity only) |
| **E. Modifier semantics / gestures** | nobody | no product exposes this | **no** |

[INFERRED] Axes A and D are **user preferences**. Axes B and C are **profile identity**. Axis E is
**architecture**, and is the one that would have to be built before any of this works.

[INFERRED] The tempting promise — "choose `Affinity` and Alt-drag mirrors" — requires axis E, which no
product has built, and which Spool cannot retrofit cheaply because those behaviours are compiled into
gesture handlers (see `interaction/input.md` for Affinity's specific `⌃`-drag-mirrors behaviour).

A realistic first profile therefore changes **what exists and which keys reach it**, not **how a
gesture interprets a modifier**. That is a much smaller promise, and it is the one the evidence
supports.

---

## 4. What must be core regardless of profile

[INFERRED] From the corpus, the following are *not* profile material, because all four products agree
on them and a profile that changed them would simply feel broken:

- Escape cancels the in-flight interaction and returns to the previous state.
- Undo is one user-perceived action and is always available.
- Selection is a set of identities that survives focus changes.
- The document is never mutated by a gesture that is later cancelled.
- Text is entered by a text-editing session with its own caret, range, and IME state.
- The camera is transient; it is never part of the document.
- A drag threshold distinguishes click from drag.

And these are *core mechanisms* that profiles merely configure:

- snapping tolerance expressed in **screen** pixels, divided by zoom (tldraw),
- the drag threshold,
- whether history captures at all.

---

## 5. Where the architecture must be cut, in Spool's terms

### 5.1 Points that must not be profile-aware

[INFERRED] A profile must never reach:

- the document model,
- the operation set,
- history capture,
- rendering.

If a profile can change history granularity, then undo semantics depend on which profile is loaded,
which makes bugs unreproducible and makes "one undoable action" undefined. [PROPOSED] History capture
scope should be fixed in the operation layer, with profiles able only to *choose operations*, not to
change how they are recorded.

### 5.2 Points where a profile hook is cheap and valuable

| Hook | Why cheap | Evidence it is needed |
|---|---|---|
| Keymap source (built-in vs file) | GPUI loads JSON keymaps already [SOURCE-CODE] | Affinity customisation, Figma layouts |
| Context predicate set | GPUI has `KeyContext` + a predicate language [SOURCE-CODE] | Canva focus ladder; Figma's select/direct-select |
| Enabled tool ids | `Tool` is already an enum with `creates_object()` | Affinity Studios |
| Snapping preset id | snapping does not exist in the prototype yet | Affinity's 7 presets |
| Cursor per tool/state | currently unmodelled | every product |

### 5.3 The one hard case

[INFERRED] **Modifier-as-mode versus modifier-as-toggle** cannot be a keymap setting. Figma uses
`⇧`-drag to constrain; Affinity uses `⌃`-drag in one place to *select by intersection* and in another
to *mirror-shear*, i.e. the same physical modifier means different things depending on gesture state.
A profile that wanted to reproduce this must parameterise the *state machine*, not the keymap.

[PROPOSED-QUESTION] Whether Spool should permit modifier meaning to be per-state is an open design
question, and it is the single largest obstacle to faithful profile emulation.

---

## 6. Candidate architectural implications

> §46 format. No decisions.

### Implication A — Model a profile as a *bundle of named axes*, not a single blob

**Evidence:** [DOCUMENTED] Affinity separates Studios (tools + panels), shortcut customisation, and
snapping presets. [DOCUMENTED] Figma separates layouts from everything else. [SOURCE-CODE] tldraw
separates the action registry from tool registration.

**Why it matters:** A single `Profile` enum with baked-in behaviour is what makes profiles
unmaintainable — every divergence becomes another branch. Named axes let a user set axes
independently, which is what Affinity actually permits.

**Approaches:**
- **A1.** `Profile { keymap, tool_set, contexts, snap_preset, cursor_set }`; profiles are presets
  that populate those five fields; users may override any field.
- **A2.** A1, but the axes are the *only* user-facing surface — there is no "Figma profile" object at
  all, only named presets that happen to bundle defaults.
- **A3.** Axis-per-entity (each action carries its own profile applicability), which is the most
  flexible and the most complex.

**Tradeoffs:** A1 is discoverable. A2 avoids the false promise that a profile is a single thing, and
matches Affinity's actual behaviour best. A3 is what a plugin ecosystem would need eventually.

**Decision: TBD — requires architecture review.**

---

### Implication B — Decide now that profiles may not touch history

**Evidence:** [INFERRED] from `architecture/history.md` — tldraw's capture modes are chosen by the
*call site* (`editor.run(..., { history })`), not by user configuration. [DOCUMENTED] no product
exposes history granularity to users.

**Why it matters:** It is cheap to state now and expensive to discover later. If a profile can change
capture scope, "one undoable action" stops being a property of the operation and becomes a property
of the session, which also makes undo tests profile-dependent.

**Approaches:**
- **B1.** Hard rule: profiles configure only *which operations exist*; capture scope is fixed per
  operation.
- **B2.** Allow capture scope to be an operation parameter, but not a profile parameter — callers
  (including tools and agents) choose, profiles do not.
- **B3.** Defer; decide when profiles are actually implemented.

**Tradeoffs:** B1 is the safest and costs nothing now. B2 is what tldraw actually does and is more
honest about where the decision belongs. B3 is the only option that costs nothing today and is
therefore a trap.

**Decision: TBD — requires architecture review.**

---

### Implication C — Treat "profile = preset" and "profile = plugin" as different milestones

**Evidence:** [SOURCE-CODE] tldraw's mechanism is host composition; no product ships presets of it.
[DOCUMENTED] Affinity's user-customisable Studios are presets that can also be edited.

**Why it matters:** A preset profile (Figma-like, Canva-like) is a data file and can ship early. A
plugin profile (third-party tools) requires a stable extension API, which is a much later commitment.
Conflating them leads to either a too-small API or a too-early one.

**Approaches:**
- **C1.** Ship presets only; never promise plugins.
- **C2.** Presets now, but design the axes so a later plugin API is a superset.
- **C3.** Design the plugin API first, since "VS Code for visual design" arguably implies it.

**Tradeoffs:** C1 is cheapest and honest. C2 is the usual compromise. C3 best matches the stated
ambition but front-loads the hardest problem in the brief (and would need the operation registry to
be stable first).

**Decision: TBD — requires architecture review.**

---

### Implication D — Record modifier-semantics divergence as a known unsolved gap

**Evidence:** [DOCUMENTED] Affinity's `⌃`-drag-mirrors vs `⌃`-drag-selects-by-intersection;
[DOCUMENTED] Figma's `⇧`-constrain; [INFERRED] no product parameterises these.

**Why it matters:** Spool's positioning statement promises Affinity-class expressiveness. If modifier
semantics are compiled into gesture handlers, that promise can only be met by forking the handlers per
profile, which is the maintenance trap the whole profile concept is meant to avoid.

**Approaches:**
- **D1.** Accept that profiles change *what exists and which keys*, not modifier meaning; document it.
- **D2.** Make each gesture's modifier interpretation a declared, profile-supplied table from day one.
- **D3.** Keep modifier semantics fixed in core; let profiles only add tools whose modifiers are
  tool-local (e.g. a Pen tool's own modifiers), which bounds the surface.

**Tradeoffs:** D1 is honest and cheap but limits what a profile can promise. D2 is the only route to
true emulation and is a large refactor of every gesture handler. D3 is a plausible middle: it makes
tool-local modifiers possible without making *core* modifier semantics configurable, and it is
compatible with A2 and C1.

**Decision: TBD — requires architecture review.**

---

## 7. Open questions

1. Does a Spool profile change the document at all — e.g. a "Pixel work" profile with a different grid?
2. Should snapping presets be a profile axis or a standalone user preference? (Affinity treats them as
   standalone.)
3. Are tool sets per-profile, or per-document? (A `.psd`-imported document arguably wants Pixel tools.)
4. How does a profile interact with a keyboard layout? Two axes that both change key meaning.
5. Is there a Spool-default profile, and what is it called? (`spool` in the brief's list implies yes.)
6. Does a profile change the default tool?
7. What is the migration story when a profile changes and a saved keymap references a now-disabled action?
8. Can a document record which profile it was last edited under? (Hint: probably not — it is session state.)
9. How much of Affinity's node-tool behaviour could a hypothetical `affinity` profile actually
   reproduce, given that it depends on both modifier semantics *and* a state machine Spool does not have?

## 8. Sources

- Affinity (DOCUMENTED): Personas / Studios pages, keyboard shortcuts, snapping pages — affin.co/help ⭐⭐
- Figma (DOCUMENTED): Keyboard shortcuts pages (16 layouts), Preferences — help.figma.com
- tldraw (SOURCE-CODE): tool registration/config table, actions registry (`kbd` strings + gating),
  `editor.run` capture modes, `isToolLocked`
- Canva (DOCUMENTED): Keyboard shortcuts, focus ladder — canva.com/help ⭐⭐
- GPUI (SOURCE-CODE): `Keymap` / `KeyBinding` / `KeyBindingContextPredicate` / `KeyContext`,
  `Action::build(json)`, `is_action_available` — see `architecture/input-system.md` §2
- Spool prototype (SOURCE-CODE): `Tool` enum + `creates_object()` (`canvas.rs`), `TOOLS` in
  `shell.rs`, 16 hard-coded `KeyBinding`s in `main.rs`
- Related notes: `interaction/keyboard.md`, `interaction/input.md`, `interaction/snapping.md`,
  `architecture/editor-runtime.md`, `architecture/input-system.md`, `architecture/history.md`
