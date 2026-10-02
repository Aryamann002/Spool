# Interaction: Keyboard System

The keyboard is where the four products differ most and where Spool has the clearest opportunity, because
each product solved a *different* problem and none of the four solutions covers the whole space.

## The four models

| Product | Model | Customisable? | Layout-correct? |
|---|---|---|---|
| **Figma** | **Layout selection.** A fixed action set with a 16-layout remapping table | Layouts yes; bindings no | Yes, via discrete layout tables |
| **tldraw** | **Per-action `kbd` strings** with programmatic matching | Yes, programmatically (`overrides.actions`) | **Yes — three documented matching strategies** |
| **Affinity** | **Full user customisation.** "you can customize them to suit your way of working" | Yes, in Settings | Partially — some bindings are keyboard-qualified |
| **Canva** | **Fixed documented set** | No | Not documented |

## Figma: layout selection, not binding customisation

[DOCUMENTED — "Select keyboard layout"]

- Default shortcuts assume **US QWERTY**.
- 16 supported layouts: Chinese (Pinyin), Danish, Finnish, French AZERTY, German QWERTZ, Italian,
  Japanese (Kana), Korean, Norwegian, Portuguese, Spanish, Spanish (Latin America), Swedish, U.K.
  (Mac/PC), U.S. QWERTY / Generic.
- Preference is **account-wide**, applies to all files, revertible to "Generic".
- **A subset of ~25 actions is remapped per layout**: zoom in/out, show/hide UI, show left sidebar, pixel
  grid, snap to pixel grid, multiplayer cursors, layout guides, show outlines, remove fill, remove
  stroke, send to back, send backward, bring to front, bring forward, adjust list indentation, font size,
  font weight, letter spacing, line height, type ellipsis, type `@`, type `\`, add code block, use cursor
  chat, open quick actions, paste to replace, select parent.
- **Figma notifies the user of a mismatch** between the OS layout and the Figma setting.
- The Figma desktop app reads the OS layout identifier: `com.apple.keylayout.ABC`, `00004009`,
  `org.sil.ukelele`. [ENGINEERING-DISCLOSED — "Behind the scenes: international keyboard shortcuts"]

**Critical negative finding:** [INFERRED from the absence of any rebinding UI in the docs] **Figma does
not offer user-assignable shortcuts.** Users cannot rebind `⌘D`. The Figma forum has a long-running
request thread confirming this.

[INFERRED] The Figma model is *maintenance-driven*: every new layout requires a human to produce and
verify a new remapping table. tldraw's model is *algorithm-driven*: one matcher handles arbitrary
layouts.

## tldraw: the reference `kbd` system

[DOCUMENTED — `sdk-features/actions.mdx`]

### Structure

```ts
{
  id: 'undo',
  label: 'action.undo',            // or a context→key map
  icon: 'undo',
  kbd: 'cmd+z,ctrl+z',
  readonlyOk: false,
  checkbox: false,
  isRequiredA11yAction: false,
  onSelect(source) { /* 'kbd' | 'menu' | 'toolbar' | … */ }
}
```

- ~100 default actions across editing, grouping, arrangement, export, zoom, preferences.
- "Menus and toolbars look up actions by ID and render them with their labels, icons, and keyboard
  shortcuts." **One registry drives keys, menus, toolbars, and the shortcuts dialog.**
- `onSelect(source)` receives where it was triggered from — used for analytics.
- `label` may be a context→key map for context-sensitive labels.

### Format

- Modifiers `+`-separated: `cmd` (alias `meta`), `ctrl`, `shift`, `alt` (alias `option`).
- Special keys: `del`, `backspace`, `enter`, `escape`, `space`, `left/right/up/down`.
- Commas bind several combinations: `'cmd+g,ctrl+g'`. **"Every combination is active on every platform"**
  — only the *displayed hint* is platform-specific.
- To bind `+` itself: `cmd++`. On layouts where `+` is shift+another key: `cmd+shift++`.

### The three matching strategies — the most valuable content in this file

[DOCUMENTED, quoted]

> "Shortcuts match the character the browser reports for the key press, not the physical keycap, with two
> exceptions: **shifted number-row keys match by position**, so write them as `shift+<digit>` on every
> layout, and **when the reported character is non-ASCII (Cyrillic, Greek, macOS Option dead keys)
> matching falls back to the physical key's US QWERTY character**, so `cmd+z` still works on a Russian
> layout."
>
> "With shift held, the shifted and unshifted US spellings are the same binding, so `shift+:` and `shift+;`
> both fire when the browser reports `:` with shift held. **Bind what the keyboard emits, not what the
> keycap says**: on French AZERTY the `:` key emits `/` with shift held, so bind `shift+/`; on Japanese JIS
> the `^` key emits `~`, so bind `shift+~`. The characters `!`, `?` and `$` are reserved as legacy
> modifier markers and cannot be used as keys."

[INFERRED] Three distinct matchers are needed:

| # | Strategy | Handles |
|---|---|---|
| 1 | **Emitted character** | AZERTY, JIS, Dvorak, remapped layouts |
| 2 | **Physical US-QWERTY fallback** | Cyrillic, Greek, macOS Option dead keys |
| 3 | **Positional match on the number row** | Shifted number keys where the character differs by layout |

A design tool with international users needs all three. This is a concrete, implementable specification.

### Activation conditions

[DOCUMENTED]

> "Shortcuts only fire while the editor is focused and the key event does not target a text input. They
> are also disabled when a menu is open, a shape is being edited, the editor has a crashing error, or the
> user has disabled keyboard shortcuts in preferences. In readonly mode, only actions with `readonlyOk` are
> bound. Actions marked with `isRequiredA11yAction: true` bypass the disabled check for accessibility
> purposes."

[INFERRED] Four *independent* disable conditions, each of which is a real-world necessity:
- focus (obviously),
- text input (otherwise every shortcut types letters),
- menu open (menus have their own key handling),
- **crashing error** (a robustness escape hatch),
- and a bypass for a11y-critical actions.

The `isRequiredA11yAction` bypass is a genuinely thoughtful pattern: some actions (navigate, select, escape)
must work even when shortcuts are "disabled", or disabling shortcuts breaks keyboard access entirely.

### Customisation

```ts
const overrides: TLUiOverrides = {
  actions(editor, actions, helpers) {
    actions['duplicate'].kbd = 'cmd+shift+d,ctrl+shift+d'
    delete actions['print']
    actions['duplicate'].onSelect = async (source) => { /* wrap the original */ }
    return actions
  },
}
```

**Documented limitation:** "`copy`, `cut`, and `paste` shortcuts are handled by native clipboard events
rather than the `kbd` system, so changing their `kbd` has no effect."

## Affinity: full customisation

[DOCUMENTED — "Keyboard shortcuts for general editing"]

> "Many shortcuts are the same as those for equivalent features in other apps, and **you can customize them
> to suit your way of working**. In the tables below, a **blank entry means no shortcut is assigned by
> default—you can add one in Affinity's settings**. N/A means the action is unavailable on the
> corresponding platform."

Also documented:

- Shortcut reference is **split by Studio**: Workspace, General editing, Vector Studio, Pixel Studio,
  Layout Studio. [INFERRED: shortcuts are scoped per Studio]
- Nudge distances are configurable under **Settings ▸ Tools**. [DOCUMENTED]
- Some bindings are explicitly layout-qualified: "Set No fill … `/` (International and selected keyboards
  only)". [DOCUMENTED]

[INFERRED] Affinity's answer is *full user rebinding with a layout caveat*: they bind to physical keys and
disable bindings that don't work on some layouts, rather than implementing tldraw's character-matching
fallbacks. Simpler to implement, less correct for users.

## Canva: fixed, heuristically typed

[DOCUMENTED — "Canva keyboard shortcuts"]

### The grammar

Canva's shortcut set follows a consistent, learnable pattern:

| Prefix | Meaning | Examples |
|---|---|---|
| *(none)* | Element creation, view toggles | `T` `R` `L` `C` `S` `⇧R` `⇧;` |
| `⌘/Ctrl` | View, zoom, file, text formatting | `⌘+` `⌘0` `⌘G` `⌘F` `⌘B` |
| `⌥/Alt` + `⌘` | Structural / navigation | `⌥⌘0` zoom-to-fit, `⌥⌘1/2/3` views, `⌥⌘P` present, `⌥⌘G` go-to-page |
| `⇧⌘` | Text style / font size / list | `⇧⌘L/C/R` align, `⇧⌘,`/`⇧⌘.` size, `⇧⌘7/8` lists |
| `⌥` + arrow | Large nudge; rotate | `⌥,` `⌥.` rotate, `⇧⌘←` large resize |

### Accessibility-specific bindings [DOCUMENTED]

- `⌘F1` — move focus to the Editor toolbar
- `⌘F2` — move focus to Canvas
- `⌘F3` — move focus to the Side panel
- `⌘F6` / `⇧⌘F6` — next / previous surface
- `/` or `⌘E` — quick actions
- `⇧10` — open More actions menu for the selected element

[INFERRED] **`⌘F1/F2/F3` is the most accessibility-conscious focus model of the four products.** Explicit,
documented focus targets between chrome and canvas is the prerequisite for a usable keyboard-only
workflow. tldraw has `isRequiredA11yAction`; Figma has `Keyboard box selection` and `F6`-focused toolbar;
only Canva documents focus movement as a first-class binding.

## Cross-product binding comparison (a sample)

| Action | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| Undo | `⌘Z` | `cmd+z` action | `⌘Z` | `⌘Z` |
| Redo | `⌘⇧Z` | `cmd+shift+z` | `⇧⌘Z` | **`⌘Y` or `⇧⌘Z`** |
| Duplicate | `⌘D` | `duplicate` action | **`⌘J`** | `⌘D` |
| Deselect | `Esc` | — | **`⌘D`** | `Esc` |
| Delete | `Del` / `Backspace` | `delete` / `backspace` | `Del` | `Del` |
| Select all | `⌘A` | `cmd+a` | `⌘A` | `⌘A` |
| Inverse selection | `⌘⇧A` | — | — | — |
| Match selection | `⌥⌘A` | — | — | — |
| Group / Ungroup | `⌘G` / `⇧⌘G` | `cmd+g` / `cmd+shift+g` | `⌘G` / `⇧⌘G` | `⌘G` / `⇧⌘G` |
| Bring forward | `⌘]` | `bring-forward` action | `⌘]` | `⌘]` |
| Bring to front | `⌘⌥]` (Mac) / `⌘⇧]` | `bring-to-front` | `⇧⌘]` | `⌥⌘]` |
| Send to back | `⌘⌥[` | `send-to-back` | `⇧⌘[` | `⌥⌘[` |
| Select parent | `\` | `cmd+shift+up` | `⌘↑` | — |
| Select child | `Enter` | `cmd+shift+down` | — | `Tab` |
| Next sibling | `Tab` | `Tab` | `⌥]` | `Tab` |
| Zoom to fit | `⇧1` | `zoom-to-fit` action | — | `⌥⌘0` |
| Zoom to selection | `⇧2` | `zoom-to-selection` action | — | — |
| Actual size | `⇧0` | `zoom-to-100` | — | `⌘0` |
| Show/hide UI | `⌘\` | `toggle-focus-mode` | — | `⌘/` (side panel only) |
| New line in text | `Return` | `enter` | `Return` | `Return` |

[DOCUMENTED where cited; Figma `⇧1`/`⇧2`/`⇧0` are THIRD-PARTY/community-documented]

### The three real disagreements

1. **`⌘D`**: Duplicate in Figma/tldraw/Canva; **Deselect in Affinity**; Duplicate is `⌘J` there.
2. **Redo**: `⌘⇧Z` everywhere except **Canva, which also accepts `⌘Y`**.
3. **Bring-to-front**: Figma uses `⌘⌥]` on Mac and `⌘⇧]` on Windows — the same logical action has different
   modifiers per platform. Affinity and Canva use `⇧⌘]` / `⌥⌘]` respectively. [DOCUMENTED]

[INFERRED] Disagreement 3 is instructive: it shows that even within one product, a modifier convention
can drift between platforms. Figma's Mac/Win split exists because macOS uses `⌥` for "to front" while
Windows convention uses `⇧`.

## Command palettes / quick actions

| Product | Mechanism | Binding |
|---|---|---|
| Figma | **Actions menu** — searches every action by name | `⌘/` |
| Canva | Quick actions | `/` or `⌘E` |
| tldraw | No palette documented; ~100 actions are documented in the SDK reference instead | — |
| Affinity | Context toolbar + menus; no palette documented | — |

[INFERRED] **A command palette is the highest-leverage accessibility affordance**, because it makes every
action reachable without a keybinding. Figma's is the most developed (it also hosts "Add code block",
"Use cursor chat", "Paste to replace", "Pixel grid", "Snap to pixel grid", "Multiplayer cursors",
"Layout guides", "Show outlines", "Select parent" — actions that have no key or a conflicting one).

It is also the mechanism by which Figma offers "keyboard controls … cannot be disabled" while still
letting users reach every command.

## Spool prototype: what exists

From `app/src/main.rs` and `app/src/canvas.rs` [OBSERVED in source]:

```rust
cx.bind_keys([
    KeyBinding::new("backspace", canvas::Backspace, None),
    KeyBinding::new("delete", canvas::Delete, None),
    KeyBinding::new("left", canvas::Right, None),   // …
    KeyBinding::new("shift-left", canvas::SelectLeft, None),
    KeyBinding::new("shift-right", canvas::SelectRight, None),
    KeyBinding::new("cmd-a", canvas::SelectAll, None),
    KeyBinding::new("ctrl-a", canvas::SelectAll, None),
    KeyBinding::new("home", canvas::Home, None),
    KeyBinding::new("end", canvas::End, None),
    KeyBinding::new("cmd-v", canvas::Paste, None), …
]);
```

- **16 hard-coded `KeyBinding`s**, registered once at startup, each with an explicit `cmd-` and `ctrl-`
  duplicate.
- **No action registry.** Each binding maps directly to a canvas enum variant.
- `shell.rs` has a `ShortcutAction` enum and `shortcut_action(...)` for *displaying* shortcut hints in the
  UI. [OBSERVED]
- **No `undo`/`redo` binding at all** despite `History` existing with `can_undo`/`can_redo`.
  [OBSERVED: `main.rs` has no `cmd-z`]
- No `cmd-d` (duplicate), no `cmd-g` (group), no ordering bindings, no zoom bindings, no escape binding
  (Escape is handled inside the canvas's mouse/interaction logic, not as a key binding).
- **No `cmd/ctrl-shift-z` for redo.**

[OBSERVED INFERENCE] The architecture implied is: **bindings → enum variants → ad-hoc handlers**. This is
the anti-pattern the research brief warns about ("I needed a resize feature, so I shoved 400 lines into
canvas.rs"). An action registry is the single highest-value structural change to Spool's keyboard system,
and it is cheap.

## Candidate architectural implication

**Evidence:**

1. An **action registry** (id + label + kbd + handler) drives keys, menus, toolbars, and the help dialog in
   one place. [tldraw DOCUMENTED]
2. Matching needs **three strategies**: emitted character, physical-US fallback for non-ASCII, and
   positional matching for the number row. [tldraw DOCUMENTED]
3. Shortcut firing must be gated on focus, text-input target, menu state, and error state — with a
   bypass for a11y-critical actions. [tldraw DOCUMENTED]
4. Layout *selection* (Figma) is high-maintenance; *algorithmic matching* (tldraw) is not. [INFERRED]
5. Full user rebinding (Affinity) is achievable but requires layout caveats. [Affinity DOCUMENTED]
6. A command palette (Figma, Canva) is the universal accessibility escape hatch. [DOCUMENTED]
7. Explicit focus-target bindings (Canva `⌘F1/F2/F3`) are the highest-value a11y bindings. [DOCUMENTED]
8. Modifiers are **context-dependent**: `⌘D` means different things in different products; bring-to-front
   uses different modifiers per platform within one product. [DOCUMENTED]

**Why it matters:** A design editor's keyboard system is its primary accessibility surface and its
primary agent input surface (tldraw's driver dispatches *key* events through the same system). Rebuilding
it later means touching every feature.

**Potential Spool approaches:**

- **A. Keep hard-coded bindings.** Fastest; unmaintainable; no a11y story; blocks profiles and agents.
- **B. Action registry with `kbd` strings + tldraw's three matchers + focus/error gating.** The reference
  implementation. Portable to native input stacks (the strategies are about key events, not DOM).
- **C. B + user rebinding UI.** Adds a settings surface; needs conflict detection and persistence.
- **D. B + command palette.** Highest accessibility return per unit of effort.
- **E. B + profile layer** that can swap `kbd` sets per interaction profile (see `profiles/`).

**Tradeoffs:** B is clearly correct and cheap. C is the difference between "nice" and "professional" but
costs a settings UI, conflict resolution, and a migration story for default bindings. D is small and
transformative for accessibility. E is the enabling mechanism for the whole interaction-profile concept
Spool is considering.

[INFERRED] The order is: B → D → E → C. An action registry is a prerequisite for all three of the others.

**Note:** the profile mechanism in `profiles/` only works if actions exist as named units with
replaceable `kbd` and handler. So **the action registry is a hard prerequisite for interaction profiles,
not merely a nicety.**

**Decision: TBD — requires architecture review.**

## Open questions

1. Does Spool's input layer report both the physical key and the emitted character? (Needed for tldraw's
   strategies 2 and 3. GPUI's answer is Unknown from this research.)
2. Will Spool support user rebinding at launch? If not, is the action model at least rebinding-*ready*?
3. Is there a command palette?
4. What are the explicit focus targets?
5. Should redo be `⌘⇧Z` only, or also `⌘Y`?
6. Should `⌘D` be duplicate or deselect? (Figma/tldraw/Canva say duplicate; Affinity says deselect.)
7. Can a shortcut be bound to *two* actions depending on selection state? (Figma's `⇧1`/`⇧2` don't, but
   arrow keys do: pan vs. move.)

## Sources

- Figma: "Select keyboard layout" (/5665442977431) ⭐; "Use Figma products with a keyboard"
  (/360040328653) ⭐; "Adjust alignment, rotation, position, and dimensions" (/360039956914); "Guide to
  auto layout" (/360040451373); "Select layers and objects" (/360040449873);
  "Behind the scenes: international keyboard shortcuts" (https://www.figma.com/blog/behind-the-scenes-international-keyboard-shortcuts/)
  — ENGINEERING-DISCLOSED for OS layout identifier reading.
- tldraw: `sdk-features/actions.mdx` ⭐ (kbd format, three matching strategies, activation conditions,
  overrides), `sdk-features/tools.mdx` (`kbd` on tool items), `sdk-features/user-preferences.mdx`,
  `docs/driver.mdx` (synthetic key events)
- Affinity: "Keyboard shortcuts for general editing" (/workspace-shortcuts-editing/) ⭐;
  "About Studios" (/workspace-about-studios/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/) ⭐
- Spool prototype: `app/src/main.rs` (`cx.bind_keys([...])`), `app/src/shell.rs` (`ShortcutAction`,
  `shortcut_action`)
