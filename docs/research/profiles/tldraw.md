# Interaction Profile Evidence — tldraw

> **What this file is.** A catalogue of tldraw's interaction conventions, written so that a future
> `Interaction Profile` for Spool could be authored from it.
>
> **What this file is not.** A design of that profile. tldraw ships no profiles at all — that absence
> is itself the most important finding here.
>
> Read with: `architecture/profiles.md` §2.2, `architecture/interaction-runtime.md`,
> `products/tldraw.md`.

---

## 1. Summary

tldraw is the **only product in the corpus whose interaction architecture is inspectable**. That makes
it the highest-value source for profile design and the *worst* source for profile content — because
the tools are the consumer's, not the library's.

Three consequences that recur throughout this file:

1. **`@tldraw/editor` has no built-in tools.** [SOURCE-CODE] Every convention below is a convention of
   a particular *consumer*, not of tldraw.
2. **Interaction is a hierarchical state chart.** [SOURCE-CODE] Conventions differ per tool because
   each tool is its own chart, not because there is a global rule table.
3. **Tools check whether they are locked rather than being forced to honour it.** [SOURCE-CODE] A
   profile that adds a lock has to change tool code, not settings.

[INFERRED] Therefore a `tldraw` profile for Spool would be an **opinionated reconstruction of the
tldraw.com demo**, reconstructed from architecture rather than from a documented specification. Its
fidelity would be lower than a Figma profile's and its architectural value would be higher.

---

## 2. Tool vocabulary

[SOURCE-CODE] The default UI registers tools whose behaviour is:

| Tool class | Persists after use? | Notes |
|---|---|---|
| Select | yes | multi-select, lasso |
| Hand / pan | yes | |
| Rectangle, ellipse, diamond, triangle, parallelogram, hexagon, … | **no — reverts to Select** | shape tools |
| Draw (freehand) | no | produces a simplified polyline shape |
| Text | **yes — stays active** for continued typing | |
| Note | no | |
| Arrow | no | |
| Line | no | |
| Frame | no | |
| Image | no | |
| Highlighter | no | |
| Laser | no | ephemeral, presence only |
| Eraser | no | |
| Draw-on-top / frame-on-top | no | |

Notable absences [SOURCE-CODE]: **no pen tool, no node editor, no components, no layout engine, no
boolean path operations.** tldraw is an infinite-canvas whiteboard with a shape vocabulary, not a
vector editor.

[INFERRED] The *persist-after-use* rule is the Figma rule, and tldraw inherits it. The `Text` exception
is also Figma's. This is evidence that the convention is genuinely standardised rather than a Figma
quirk.

---

## 3. Contexts and focus modes

[SOURCE-CODE] Contexts are implicit, expressed by depth in the state chart:

| Context | Entered by | Escape behaviour |
|---|---|---|
| Root | — | clears selection |
| Selecting | Select tool / click | to root |
| Editing a shape (text) | double-click a text shape | back to selecting, shape stays selected |
| Creating a shape | pointer-down with a shape tool | **discards the in-progress shape** |
| Moving / Scaling / Resizing | drag from a handle or the body | restores the pre-gesture geometry |
| Panning | space / middle / hand tool | — |
| Focused shape group | double-click into a group | ascends one level |

- `Escape` **always** does one thing: move one step up the chart, or restore. [SOURCE-CODE]
- `focusedGroupId` is instance state; `selectAdjacentShape` scores candidates by distance plus an
  off-axis term, so `Tab`/`⇧Tab` traverse *geometrically*, not by tree order. [SOURCE-CODE]

[INFERRED] `Tab` traversal by geometric proximity rather than tree order is a genuinely different
convention from Figma's, and it is one of the few places tldraw's *feel* differs from the industry
default. Worth knowing before deciding to emulate either.

---

## 4. Modifier semantics by operation

[SOURCE-CODE] Modifier handling is **per state node**, not global:

| Operation | Modifier | Behaviour |
|---|---|---|
| Drag to move | `Alt` | **bail** — switch from live mutation to preview, release to commit |
| Drag to move | `⇧` | constrain axis (only where the shape supports it) |
| Drag to move | `⌘` | constrain to a 45° angle |
| Draw / brush | `⌥` | erase instead of draw |
| Resize | `⇧` | preserve aspect |
| Text editing | `Esc` | exit editing |
| Tool selection | double-click tool | **tool lock** |
| Multi-select | `⇧`-click | additive |
| `⌥`-click | — | select the shape behind |

[INFERRED] `Alt`-as-bail is the most distinctive convention in the corpus. It means the in-flight
gesture has a *reversible* and a *live* mode, and switching mid-gesture does not discard the gesture.
This is architecturally expensive — see `architecture/history.md` — and it is the single strongest
argument that a "tldraw-like" profile is not a cheap add-on.

---

## 5. Creation semantics

[SOURCE-CODE]

- Click with a shape tool creates a shape with a **default size**; drag creates a drag-defined size.
  (Same rule as Figma.)
- Dragging from the handle of a **group** creates a **new group around it** (tldraw-specific; [OBSERVED]
  in the product).
- `createShape` returns an id, and the tool is responsible for positioning the result.

[INFERRED] "Drag out of a group creates a new group" is a container-model decision masquerading as a
gesture rule. A profile reproducing it must have the same group semantics, or it will produce a
gesture that has no meaning.

---

## 6. Navigation

[SOURCE-CODE] / [DOCUMENTED]

| Action | Binding |
|---|---|
| Pan | space-drag, middle-drag, two-finger scroll |
| Zoom | wheel + `⌘`/`Ctrl`, pinch |
| Zoom to fit | `⇧1` |
| Zoom to selection | `⇧2` |
| Zoom to 100% | `⇧0` |
| Zoom in / out | `=` / `-` |
| Reset zoom | `⇧0` |
| **Jump/navigate to a URL state** | via share links; camera is part of the shared state |

**tldraw's camera is part of the document state for sharing purposes** [SOURCE-CODE] — a shared link
carries the camera, not just the shapes. [INFERRED] This is a meaningful architectural difference from
the other three products, where the camera is purely a session concern.

There is **no minimap** [OBSERVED].

---

## 7. Snapping configuration

[SOURCE-CODE] tldraw has the most precisely specified snapping in the corpus, and the only one with a
stated unit:

- Three snap sources: **bounds** snaps (edges, centres), **handle** snaps (connection points), and
  **grid** snaps.
- **Tolerance is expressed in screen pixels and divided by zoom** — the default is 8 screen px. This
  makes snapping feel identical at every zoom level.
- Snapping returns a **pure nudge** rather than mutating the object: `snapManager.snapShapes(...)`
  yields a delta.
- Indicators **merge collinear snaps** and **dedupe**, so overlapping guides do not stack up.
- Each shape type opts in via `ShapeUtil` hooks: `canSnap`, `getBoundsSnapGeometry`,
  `getHandleSnapGeometry`, `canCull`, `canBind`, `canTabTo`, `getText`, `getGeometry`, `component`.
- **Self-snap opt-out**: a shape does not snap to itself.

[INFERRED] The `ShapeUtil` hook set is the single most reusable abstraction in the entire corpus. It
means snapping, culling, binding, tab traversal, and text extraction are all *per-shape-type policy*
rather than a central switch statement — which is precisely the property Spool would need for a
general-purpose document model. See `architecture/rendering.md` Implication C.

---

## 8. Selection model summary

| Concept | tldraw's answer |
|---|---|
| Selected | `selectedShapeIds` in **instance state**, not document state |
| Editing | a state-chart node, not a flag |
| Behind | `⌥`-click |
| Marquee | lasso (freehand), not a rectangle |
| Additive / subtractive | `⇧` / `⌘` |
| Ancestor filtering | a **side effect** removes children when a parent is selected |
| Groups | real containers with a `parentId` |
| Focus | `focusedGroupId` for scope; `Tab` traversal by geometry |
| Selection in history | **not** captured |

[INFERRED] The lasso is the second-biggest feel difference from the rest of the corpus: a freehand
enclosure rather than a rectangle. It changes what "selecting" means under occlusion and near curves.

---

## 9. History perception

[SOURCE-CODE] — see `architecture/history.md` for the full treatment.

- Undo stack of **diffs against marks**, not before/after snapshots.
- **Bail** (`Alt`-switch, `Escape`) marks the starting state so a gesture can be undone as one entry
  even though the document was mutated live.
- **Squash** (`crop`) collapses a range into one entry.
- Three capture modes: record / ignore / squash-and-ignore.
- `editor.run(fn, opts)` is how any caller, including a plugin or an agent, opens a capture boundary.
- Selection is **not** part of history.

[INFERRED] Bail + squash is what makes tldraw's live-mutation feel identical to Figma's
commit-on-release feel. Reproducing the *feel* without reproducing bail would produce a profile that
looks right and behaves wrong on cancel.

---

## 10. What a `tldraw` profile would have to reproduce

**Tier 1 — the conventions that define the feel**
1. Lasso selection rather than rectangular marquee.
2. Alt-as-bail: switch live mutation to preview mid-gesture without losing the gesture.
3. `Tab` traversal by geometric proximity rather than tree order.
4. Parent selection filters out descendants (via a side effect, not a rule).
5. Shapes produce simplified polylines; no pen tool, no node editing.
6. Anchor snapping to connection handles, with merged indicators.
7. Zoom-independent snapping via screen-px ÷ zoom.
8. Camera travels in share links.
9. Text tool stays active after use.
10. Escape = one step up the chart, or restore.

**Tier 2 — architectural**
11. Per-shape-type policy hooks rather than central switches.
12. Selection as transient editor state.
13. Marks-based history with explicit bail and squash.

**Tier 3 — not reproducible**
14. The state-chart architecture itself cannot be "reproduced by a profile"; it *is* the substrate
    the profile would run on.

---

## 11. What cannot be reproduced from the available evidence

| Item | Why not |
|---|---|
| The exact tool list of tldraw.com | the library ships none; the consumer decides |
| Which tools revert to Select | consumer behaviour, not documented in the library |
| The exact `ShapeUtil` set of any shape | per-consumer source |
| Lasso hit-testing tolerance | not documented |
| Geometric `Tab` scoring weights | source shows the shape of the score, not the constants |
| Default snap distance as a *product* decision | the library default is 8 screen px; the site may differ |

[PROPOSED] Where this file says "the consumer decides", the honest conclusion is that the Spool
profile author is making a design choice, not recovering a specification. Record it as such.

---

## 12. Evidence gaps specific to this profile

- tldraw's docs describe the *SDK*, not the *product*. Most product-level conventions in this file are
  [OBSERVED] or [INFERRED] from the demo, not [DOCUMENTED].
- The demo's source is not part of the public repository in a way this research consulted.
- Tool-lock is documented as a property but its interaction with bail is not described.

## 13. Sources

- tldraw (SOURCE-CODE) ⭐⭐⭐: state chart (`StateNode`, handlers, `onEnter`/`onExit`, `target`
  re-dispatch, unhandled-event fallthrough), tool config table, store scopes (`document`/`session`/
  `presence`), `ShapeUtil` hooks, snap manager and tolerance, `editor.run` capture modes,
  `selectedShapeIds`, `focusedGroupId`, `selectAdjacentShape`, camera/share state
- tldraw (DOCUMENTED): `docs/editor.mdx`, `docs/shapes.mdx`, `docs/selection.mdx`, `docs/snapping.mdx`,
  `sdk-features/performance.mdx`, `sdk-features/culling.mdx`, `llms.txt`
- Related notes: `products/tldraw.md`, `architecture/interaction-runtime.md`, `architecture/history.md`,
  `architecture/rendering.md` (Implication C), `architecture/editor-runtime.md`,
  `interaction/snapping.md`, `interaction/selection.md`
