# Keyboard Matrix

> §21. Keyboard system across products. Focus on the *system*, not just the bindings: customisation,
> layouts, chords, context scoping, discoverability, and accessibility.
>
> Bindings are as documented at the time of research. Where a product documents a different set per
> keyboard layout, that is noted rather than collapsed.

---

## 1. The system-level comparison — the part that matters architecturally

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| User-remappable individual shortcuts | **—** | ✔ (host-defined registry) | ✔ **full** | **—** | **—** |
| Keyboard *layouts* | ✔ **16 documented** | ✔ | ✔ ("international and selected") | ✔ | **—** |
| Layout affects physical key only | ✔ | ✔ | ✔ | ✔ | ✔ (implicitly) |
| Chords (multi-key sequences) | ◐ | ✔ | ✔ | ✔ | **—** |
| Context-scoped bindings | ? internal | ✔ (state chart path) | ✔ (Studio-scoped) | ✔ (focus modes) | **— (`context: None` everywhere)** |
| Action/command registry | ? internal | ✔ **~100 actions with `kbd`** | ✔ scripting API | ✔ `/` palette | **12 text actions** |
| Binding availability gating | ? | ✔ **declared rules** | ◐ | ? | **— (handlers branch internally)** |
| Generated menus from bindings | ◐ | ✔ (`availableActions`) | ✔ | ✔ | **—** |
| Cheat sheet / binding discovery | ✔ | ✔ | ✔ | ✔ | **—** |
| Shortcut conflict resolution | fixed | by registration order | user-defined | fixed | n/a (none to conflict) |
| Framework support for JSON keymaps | ? | ✔ | ✔ | ? | ✔ **GPUI supports it; unused** |

[INFERRED] **The market splits cleanly**: Affinity and tldraw expose customisation; Figma and Canva do
not. And in the two that do, customisation is *per-key*, never *per-gesture-semantics*. Nobody
configures what a modifier means.

---

## 2. Tools and modes

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Select tool | `V` | `V` | `V` | *n/a — no tools* | `V` |
| Frame | `F` | `F` | — | *n/a* | `F` |
| Rectangle | `R` | `R` | `R` | `R` (one-shot) | `R` |
| Ellipse / circle | `O` | `O` | `O` | `C` (one-shot) | `O` |
| Line | `L` | `L` | `L` | `L` (one-shot) | **—** |
| Polygon / star | via shape selector | ✔ | ✔ | via `S` | **—** |
| Pen | **— (no binding)** | **—** | ✔ | — | `P` (**declared, unimplemented**) |
| Pencil / brush | — | ✔ | ✔ | — | — |
| Text | `T` | `T` | `T` | `T` (one-shot) | `T` |
| Hand / pan | `H` | `H` | ✔ | — | *only via space/middle* |
| Scale | `K` | — | ✔ | — | — |
| Slice | `⌘⌥K` | — | ✔ | — | — |
| Comment | `C` | — | ✔ | — | `C` (**declared, unimplemented**) |
| Frame / slice selector combo | `⇧R` | — | — | — | — |
| **Node tool / direct select** | `⌘F2` | ? | **`A`** | **`⌘F2`** | **—** |
| **Select mode** | **`⌘F1`** | — | — | **`⌘F1`** | **—** |
| **Text edit mode** | **`⌘F3`** | `Esc` from edit | — | **`⌘F3`** | double-click |
| **Tool lock** | — | **double-click tool** | — | n/a | **—** |
| Quick actions | `/` (layers) | — | `/` (**limited layouts**) | `/` or `⌘E` | **—** |
| Jump to layer | `⌘\` then name | — | — | — | **—** |

[INFERRED] `⌘F1`/`⌘F2`/`⌘F3` is identical in Figma and Canva, which is strong evidence it is a
*convention* rather than two independent choices. Spool has neither, and the double-click-only path to
text editing is the one place where Figma, tldraw, Affinity and Canva all differ from it.

---

## 3. History and clipboard

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Undo | `⌘Z` | `⌘Z` | `⌘Z` | `⌘Z` | ✔ **raw path** |
| Redo | `⌘⇧Z` | `⌘⇧Z` | `⌘⇧Z` | `⌘Y` / `⇧⌘Z` | ✔ both |
| Cut / Copy / Paste | ✔ | ✔ | ✔ | ✔ | ✔ **12 GPUI actions** |
| Paste in place / at cursor | ✔ | ✔ | ✔ | ✔ | **—** |
| Copy as PNG / SVG / CSS | ✔ | ✔ | ✔ | ✔ | **—** |
| Duplicate | `⌘D` | ✔ | `⌘D` | `⌘D` | ✔ raw path |

[INFERRED] Spool is the only product in this matrix where **the same user-visible verb is bound in two
different subsystems**: clipboard via GPUI actions, undo/duplicate/delete via a raw string matcher.
See `architecture/input-system.md` §3.

---

## 4. Object operations

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Delete | `⌫`/`⌦` | ✔ | ✔ | ✔ | ✔ raw path |
| Select all | `⌘A` | ✔ | ✔ | ✔ | ✔ (text action) |
| Deselect | `Esc` | `Esc` | `Esc` | `Esc` | ✔ |
| Group | `⌘G` | — | `⌘G` | ✔ | **—** |
| Ungroup | `⇧⌘G` | — | `⌘J` | ✔ | **—** |
| Move into / out of group | — | — | `⌥⌘G` / `⌥⇧⌘G` | — | — |
| Flatten | — | — | ✔ | — | **—** |
| Bring forward / back | `]` / `[` | ✔ | ✔ | ✔ | **—** |
| Bring to front / back | `⌘]` / `⌘[` | ✔ | ✔ | ✔ | **—** |
| Align left/centre/right | `⌥A` etc. | ✔ | ✔ | ✔ | **—** |
| Distribute | ✔ | ✔ | ✔ | ✔ | **—** |
| Flip horizontal / vertical | `⇧H` / `⇧V` | ✔ | ✔ | ✔ | **—** |
| Rotate 90° | `⇧>` | — | ✔ | ✔ | **—** |
| Lock / unlock | `⌘⇧L` | — | ✔ | ✔ | **—** |
| Hide / show | `⌘⇧H` | — | ✔ | ✔ | **—** |

---

## 5. Movement and nudging

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Nudge 1 unit | arrows | arrows | arrows | arrows | ✔ |
| Nudge 10 units | `⇧`+arrow | `⇧`+arrow | `⇧`+arrow | `⇧`+arrow | ✔ |
| Nudge 0.1 unit | `⌥`+arrow | `⌥`+arrow | `⌥`+arrow | panel | **—** |
| Nudge in local/rotated space | — | ✔ | ✔ | — | — |
| Nudge selection only | `⌥` | — | — | — | — |

---

## 6. Camera

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Zoom in / out | `+` / `-` | `=` / `-` | ✔ | ✔ | **—** |
| Zoom to fit | `⇧1` | `⇧1` | ✔ | `⌥⌘0` | ✔ |
| Zoom to selection | `⇧2` | `⇧2` | ✔ | — | **—** |
| Zoom to 100% | `⇧0` | `⇧0` | ✔ | `⌘0` | **—** |
| Zoom to fill | — | — | — | `⇧⌘0` | — |
| Go to page | — | — | — | `⌥⌘G` | **—** |
| Page view modes | — | — | — | `⌥⌘1/2/3/P` | **—** |
| Reset zoom | `⇧0` | `⇧0` | ✔ | — | **—** |

[INFERRED] `⇧1`/`⇧2`/`⇧0` is shared by Figma and tldraw verbatim. This is the clearest case of a
camera convention having converged across two independent codebases, and Spool already matches one
third of it.

---

## 7. Text editing

| Action | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Backspace / delete forward | `⌫` / `⌦` | ✔ | ✔ | ✔ | ✔ |
| Select all text | `⌘A` | ✔ | ✔ | ✔ | ✔ |
| Select left / right | `⇧←` / `⇧→` | ✔ | ✔ | ✔ | ✔ |
| Word left / right | `⌥←` / `⌥→` | ✔ | ✔ | ✔ | ✔ |
| Line start / end | `Home`/`End`, `⌘←`/`⌘→` | ✔ | ✔ | ✔ | ✔ |
| Newline | `↵` | ✔ | ✔ | ✔ | ✔ |
| Apply text style 1–5 | `⌥⌘0`–`⌥⌘5` | — | ✔ | ✔ `⌥⌘0`–`⌥⌘5` | **—** |
| Copy / paste text style | `⌥⌘C` / `⌥⌘V` | — | ✔ | ✔ | **—** |

[INFERRED] Spool's 12 text actions cover Tiers 1–2 of the text-editing keyboard completely and
Tier 3 not at all. That is a clean, defensible scope for a prototype.

---

## 8. Accessibility

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Focus-mode ladder documented | ✔ | — | ◐ | ✔ **with a11y rationale** | **—** |
| Screen-reader support documented | ◐ | ? | ◐ | ✔ | **—** |
| All transforms via panel (no chord, no drag) | ✔ | ◐ | ✔ | ✔ **best** | partial (X/Y/W/H fields) |
| Selection without a pointer | ✔ arrows | ✔ | ✔ | ✔ **directional `⇧W/A/S/D`** | **—** |
| IME-safe single-letter shortcuts | ? | ✔ | ? | ? | **— hook unused** |
| Reduced-motion support | ? | ? | ? | ? | **?** |
| Full keyboard operability of the app | ◐ | ◐ | ✔ | ◐ | **partial** |

[INFERRED] Canva's directional multi-select and its "everything in a panel" transform model are the two
accessibility conventions in the corpus that cost Spool almost nothing to adopt — one is a selection
operation, the other is a panel. Both are recorded in `profiles/canva.md` §10 as Tier 1.

---

## 9. Spool's two keyboard paths — the concrete problem

[SOURCE-CODE] Spool currently resolves keys in two places that do not know about each other.

**Path A — GPUI actions (`app/src/main.rs`, `app/src/canvas.rs`)**

```
spool_text::{ Backspace, Delete, Left, Right, SelectLeft, SelectRight,
              SelectAll, Home, End, Paste, Copy, Cut }
```
16 `KeyBinding`s, **every one with `context: None`**, all clipboard + caret operations.

**Path B — raw `on_key_down` string matcher (`app/src/shell.rs:44-76`)**

```rust
if (platform || control) && key == "z"  { Undo | Redo }
if control && key == "y"                 { Redo }
if (platform || control) && key == "d"  { Duplicate }
match key { "delete" | "backspace" => Delete,
            "v" => Tool(Select), "f" => Tool(Frame), "r" => Tool(Rectangle),
            "o" => Tool(Ellipse),  "t" => Tool(Text), "p" => Tool(Pen),
            "c" => Tool(Comment),  "escape" => …, "1" if shift => fit }
```

| Consequence | Evidence |
|---|---|
| `backspace`/`delete` are bound in **both** paths | `main.rs:15-16` and `shell.rs:64` |
| Undo/redo/duplicate are unreachable through the action system | `shell.rs` only |
| A user remapping would remap only Path A | structural |
| Tool guards check `!platform && !control && !shift` but **not `alt`** | `shell.rs:65-73` |
| No `prefers_ime_for_printable_keys` — `v f r o t p c` will fight a CJK IME | `shell.rs` raw path |
| `cmd-x`/`ctrl-x` both registered by hand instead of using `secondary-` | `main.rs:21-30` |
| No undo/redo in the registry means `available_actions` cannot report them | framework capability unused |

[INFERRED] This is the concrete, verifiable form of `architecture/input-system.md` Implication B. It is
recorded here rather than only in the architecture note because the keyboard matrix is where an
engineer implementing "how should this key behave" will look.

---

## 10. Conventions worth treating as settled

[INFERRED] Based on convergence across products, these are the bindings Spool should feel familiar for
**regardless of profile**:

| Convention | Convergence |
|---|---|
| `⌘Z` / `⌘⇧Z` | 4 of 4 |
| `Esc` cancels, then exits, then deselects | 4 of 4 |
| `V` select, `R` rectangle, `O`/`C` ellipse, `T` text | 3 of 4 with tools; 4 of 4 one-shot |
| `⇧1` zoom to fit, `⇧0` zoom to 100% | 2 of 4 verbatim; 4 of 4 in some form |
| `⌘D` duplicate | 3 of 4 documented explicitly |
| `⌘G` group | 2 of 4 (absent where there are no groups) |
| `+`/`-` zoom | 3 of 4 |
| `⇧`-arrow = coarse nudge, `⌥`-arrow = fine nudge | 3 of 4 |
| `⌘A` select all | 4 of 4 |
| `⌘F1/F2/F3` focus ladder | 2 of 4 verbatim, and both are the accessibility leaders |

## 11. Open questions

1. Should Spool adopt `secondary-` immediately, or keep explicit `cmd-`/`ctrl-` pairs for clarity?
2. Is `⌥⌘0`–`⌥⌘5` for text styles worth adopting, given Spool has no text styles?
3. What is the right key for "open quick actions"? `/`, `⌘E`, or `⌘\`?
4. Should zoom-to-selection (`⇧2`) be added before zoom-to-100% (`⇧0`)?
5. Is `=` or `+` the correct zoom-in binding across platforms?
6. Does a profile override the *bindings* only, or also the *set of bindings that exist*?
   (`architecture/profiles.md` Implication A)
7. What should happen to the tool shortcuts when an IME is active — suppress, or route to the IME?
   (`architecture/input-system.md` §2.8)

## 12. Sources

- Figma (DOCUMENTED): Keyboard shortcuts pages across the 16 layouts — help.figma.com ⭐⭐⭐
- tldraw (SOURCE-CODE/DOCUMENTED): action registry `kbd` strings, key-matching strategies, gating
  rules, `isToolLocked` ⭐⭐⭐
- Affinity (DOCUMENTED): Keyboard shortcuts, Personas/Studios, `/` availability note ⭐⭐
- Canva (DOCUMENTED): Keyboard shortcuts — canva.com/help ⭐⭐
- Spool (SOURCE-CODE): `app/src/main.rs`, `app/src/canvas.rs:10-26`, `app/src/shell.rs:44-76, 1288-1391`
- GPUI (SOURCE-CODE): `keymap.rs`, `keymap/binding.rs`, `keymap/context.rs`, `key_dispatch.rs`,
  `platform/keystroke.rs` — see `architecture/input-system.md` §2
- Cross-references: `architecture/input-system.md`, `architecture/profiles.md`,
  `matrices/interaction-matrix.md`, `profiles/*.md`
