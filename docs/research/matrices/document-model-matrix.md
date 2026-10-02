# Document Model Matrix

> Cross-product comparison of document structure. Read **across** rows, not down columns: the value of
> this matrix is showing which concepts appear in one product and nowhere else.
>
> Spool column is [SOURCE-CODE] against the prototype and is a baseline, not a target.
> Legend: **✔** · **◐** partial/constrained · **—** absent · **?** undocumented

---

## 1. Root structure

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Document root | ✔ | ✔ (`document` scope) | ✔ | ✔ | ✔ `Document` |
| Multiple documents / files | ✔ | ✔ | ✔ | ✔ | **—** |
| Pages | ✔ | ✔ | ✔ | ✔ **bounded stack** | ◐ 4 id ranges, not pages |
| Page as a container of objects | ✔ | ✔ | ✔ | ✔ | **— (no page object)** |
| Frames / artboards as top-level containers | ✔ **universal** | ◐ | ✔ | ◐ | ✔ |
| Groups | ✔ **derived bounds** | ✔ | ✔ | ✔ | **—** |
| Nested groups | ✔ | ✔ | ✔ | ✔ | **—** |
| Flipping (hierarchy → geometry) | ✔ | ✔ | ✔ **3 scopes** | ✔ | **—** |
| Reparenting | ✔ | ✔ | ✔ | ✔ | **—** |

---

## 2. Identity

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Stable object id | ✔ | ✔ | ✔ | ✔ | ✔ `ObjectId(pub u64)` |
| **Branded / typed ids** | — | ✔ **`shape:abc`** | — | — | **—** |
| Name uniqueness | ✔ (layer names) | ◐ | ✔ | ◐ | ✔ per type counter |
| **Id allocated by the document** | ✔ | ✔ | ✔ | ✔ | ✔ `next_id` |
| **Id stable across serialise/parse** | ✔ | ✔ | ✔ | ✔ | **— (no serialisation)** |

[INFERRED] tldraw's branded ids (`shape:abc`, distinct type namespaces) are the cheapest available
protection against the classic AI-agent failure of passing a `shape:` id where a `page:` id was
expected. Worth weighing when the operation API is designed (`architecture/ai-runtime.md`).

---

## 3. Z-order

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Ordering model | explicit list | **fractional string index** | layer list | layer list | **vector index** |
| Gaps / reordering cost | reorder | **no rewrite — insert between** | reorder | reorder | insert at index |
| Stable under concurrent insert | ◐ | ✔ **by construction** | ? | ? | **—** |
| Send forward/back | ✔ | ✔ | ✔ | ✔ | **—** |
| Send to front/back | ✔ | ✔ | ✔ | ✔ | **—** |

[INFERRED] tldraw's fractional index is the only ordering scheme in the corpus designed for
*concurrent* insertion — which is exactly what presence and AI agents produce. Spool's vector index
requires an O(n) shift per insert and cannot merge two independently-edited documents without a
full re-order. See `architecture/document-model.md`.

---

## 4. Objects

| Object type | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Rectangle / frame | ✔ | ✔ | ✔ | ✔ | ✔ |
| Ellipse | ✔ | ✔ | ✔ | ✔ | ✔ |
| Line | ✔ | ✔ | ✔ | ✔ | **—** |
| Polygon / star | ✔ | ✔ | ✔ | ✔ | **—** |
| **Vector path (nodes)** | ◐ pen | **—** | ✔ **Node Tool** | **—** | **—** |
| Freehand / brush | — | ✔ simplified | ✔ pressure | — | **—** |
| Arrow / connector | ✔ | ✔ | ✔ | ◐ | **—** |
| Text | ✔ | ✔ | ✔ | ✔ | ✔ |
| Image | ✔ | ✔ | ✔ | ✔ | **—** |
| Video | ✔ | — | — | ✔ | **—** |
| Component / instance | ✔ | — | — | — | **—** |
| Table | ✔ | — | — | ✔ | **—** |
| Slice | ✔ | — | ✔ | ◐ | **—** |
| **Extensible by third parties** | plugins | ✔ **SDK** | scripting | apps | **—** |

[INFERRED] Only tldraw's object model is documented as extensible by a third party. For a product
positioned as "VS Code for visual design", that row is the one that most directly addresses the
ambition — and it is also the row where Spool is furthest from all four products.

---

## 5. Reference and reuse

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Component definition | ✔ | — | — | — | **—** |
| Instance = reference + overrides | ✔ | — | — | — | **—** |
| Variants / variant properties | ✔ | — | — | — | **—** |
| Nested components | ✔ | — | — | — | **—** |
| Libraries (local / remote) | ✔ | — | — | — | **—** |
| Publishing | ✔ | — | — | — | **—** |
| Detach instance | ✔ | — | — | — | **—** |
| **Any reuse primitive at all** | ✔ | **—** | **—** | **—** | **—** |

[INFERRED] **Three of the four products have no reuse primitive whatsoever.** Reuse in Figma is a
composed feature over hierarchy + styles; in tldraw, Affinity and Canva, duplication is a copy. This
matters for the brief's "general-purpose environment that can be extended and adapted": if reuse is
wanted, it must be designed from first principles, because there is no incremental path from copying.

---

## 6. Styles and appearance

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Per-object fill / stroke | ✔ | ✔ | ✔ | ✔ | ✔ `ObjectStyle` |
| Embedded (not referenced) styles | ✔ | ✔ | ✔ | ✔ | ✔ |
| Named reusable styles | ✔ | ◐ | ✔ | ✔ (text) | **—** |
| Colour variables / tokens | ✔ **6 typed kinds** | — | — | **—** | **—** |
| Style **references** (indirection) | ✔ via variables | — | — | **—** | **—** |
| Variables driving paint | ✔ | — | — | — | **—** |
| Variables driving typography | ✔ | — | — | — | **—** |
| Variables driving **layout metrics** | ✔ | — | — | — | **—** |
| Variables driving **visibility** | ✔ | — | — | — | **—** |
| Variables driving **variant selection** | ✔ | — | — | — | **—** |
| **Governance by copy, not reference** | — | — | — | ✔ **Brand Controls** | **—** |
| Opacity / blend modes | ✔ | ◐ | ✔ **27 modes** | ✔ | **—** |
| Shadows, blur | ✔ | — | ✔ | ✔ | **—** |

[INFERRED] The Figma-vs-Canva contrast is the most useful pair in this table: **Figma references,
Canva copies.** Both solve "keep a design on-brand". Their failure modes are opposite — Figma's
failures are indirection and edit-time surprise; Canva's are drift. There is no third option in the
corpus, and Spool will have to invent one or pick a side.

---

## 7. Layout

| Capability | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Absolute positioning | ✔ | ✔ | ✔ | ✔ | ✔ |
| Constraints (resize behaviour) | ✔ | — | ✔ | ◐ relative panel | **—** |
| Auto layout / stack | ✔ | **—** | ✔ (Layout Studio) | ◐ | **—** |
| Gap / padding | ✔ | — | ✔ | ◐ | **—** |
| Alignment in a container | ✔ | — | ✔ | ✔ | **—** |
| Fill / hug / fixed sizing | ✔ | — | — | — | **—** |
| Min / max size | ✔ | — | ✔ | — | **—** |
| Wrapping | ✔ | — | ✔ | ✔ | **—** |
| Nested layout | ✔ | — | ✔ | ◐ | **—** |
| Escape hatch from layout | ✔ `Ignore auto layout` | — | — | — | — |
| Layout driven by variables | ✔ | — | — | — | — |

[INFERRED] **tldraw — the most architecturally sophisticated product in the corpus — has no layout
engine at all.** Every other product in the corpus that has one has a different one. Layout is the
area where the corpus has the least convergence and Spool has the most to decide.

---

## 8. Assets

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Image assets | ✔ | ✔ | ✔ | ✔ | **—** |
| Vector (SVG) assets | ✔ | ✔ | ✔ | ✔ | **—** |
| Font assets | ✔ | — | ✔ | ✔ | **—** |
| Media library | ◐ | — | — | ✔ **large** | **—** |
| Stock assets | ✔ | — | — | ✔ **large** | **—** |
| Embedded vs linked | ✔ linked | ✔ | ✔ | ✔ | **—** |
| **Missing-asset behaviour** | ? | ? | ? | ? | **[all four undocumented]** |
| Asset replacement without reflow | ✔ | ✔ | ✔ | ✔ | — |
| Asset referenced by other objects | ✔ | ✔ | ✔ | ✔ **deletion blocked** | **—** |
| Import (file → document) | ✔ SVG/PDF | ✔ SVG | ✔ many | ✔ many | **—** |

[INFERRED] **Missing-asset behaviour is undocumented in all four products.** This is the single largest
pure gap in the corpus: an asset is a reference, references dangle, and no product documents what
happens. Spool will have to invent the answer, and it belongs in the document model rather than in a
renderer.

---

## 9. Bindings, constraints, prototype

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Binding records (`fromId`/`toId`/`anchor`) | ✔ | ✔ **normalised** | ✔ | — | **—** |
| Binding survives deletion of one end | ◐ isolation | ✔ **isolation vs deletion modelled** | ◐ | — | — |
| Binding aggregation into one op | ✔ | ✔ `onOperationComplete` | ? | — | — |
| Prototype triggers + actions | ✔ **14 documented actions** | — | — | ◐ | **—** |
| Conditional logic in prototype | ✔ | — | — | — | — |
| Variables in prototype | ✔ | — | — | — | — |

---

## 10. Serialisation

| Property | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Native format | ✔ `.fig` | ✔ JSON | ✔ | ✔ | **—** |
| Schema migrations | ✔ | ✔ **per-type `up`/`down`** | ✔ | ? | **—** |
| Headless read | ✔ | ✔ | ◐ | ✔ | **—** |
| Headless write / export | ✔ | ✔ | ✔ | ✔ | **—** |
| **Document separable from the view** | ✔ | ✔ | ✔ | ✔ | **✘ — owned by `CanvasView`** |

---

## 11. Ownership: persistent vs transient

| Concept | Figma | tldraw | Affinity | Canva | Spool |
|---|---|---|---|---|---|
| Selection is transient | ? | ✔ **instance state** | ? | ? | ✔ (a field, not a scope type) |
| Hover is transient | ✔ | ✔ | ✔ | ✔ | ◐ |
| Active tool is transient | ✔ | ✔ | ✔ | n/a | ✔ |
| Camera is transient | ✔ | ✔ **but carried in share links** | ✔ | ✔ | ✔ |
| Text caret / range transient | ✔ | ✔ | ✔ | ✔ | ✔ |
| Snap preview transient | ✔ | ✔ | ✔ | ? | **—** |
| IME composition transient | ✔ | ✔ | ✔ | ✔ | ✔ |
| Snapping opt-out is persistent | — | — | ✔ **`Exclude From Snapping`** | — | **—** |
| Guides are persistent | ◐ | — | ✔ | — | **—** |
| Instance state is a distinct tier | ? | ✔ **`document` / `session` / `presence`** | ? | ? | **✘** |

[INFERRED] **tldraw's three scopes are the only explicit tiering in the corpus**, and they are also
multiplayer scopes, which means the tiering was not invented for clarity — it was forced by presence.
Spool has no tiers and no presence, and would be choosing the taxonomy without the pressure that
produced it.

---

## 12. What the matrix exposes

[INFERRED] Six concepts have exactly one carrier:

| Concept | Sole carrier | Consequence for Spool |
|---|---|---|
| Vector path / node model | Affinity | Needed for "more than UI mockups" |
| Components / instances / variants | Figma | No incremental path exists; must be designed from scratch |
| Typed variables | Figma | Same |
| Branded ids + fractional z-index | tldraw | Cheapest wins available; mostly a naming decision |
| Brand governance by copy | Canva | The counter-model to variables |
| Per-layer snapping opt-out | Affinity | A *document* field, not a preference — easy to miss |

And four concepts have **no carrier at all**: missing-asset behaviour, reparenting
coordinate-preservation, variable fonts, and per-operation AI approval.

[INFERRED] The columns are also asymmetric in a way that matters: tldraw is the only product with an
extensible object model and the only one with no layout engine or components. Spool's ambition
("VS Code for visual design") points toward tldraw's shape; its reference set points toward Figma's
content. Those are compatible, but only if the object model is designed to be open.

## 13. Sources

- tldraw (SOURCE-CODE) ⭐⭐⭐: store scopes, branded ids, query indexes, migrations, fractional string
  index, bindings, `ShapeUtil` — see `architecture/document-model.md`
- Figma (DOCUMENTED): frames, groups, components, variables, auto layout, constraints, assets, prototype
- Affinity (DOCUMENTED): personas, layers, node tool, snapping options, document formats
- Canva (DOCUMENTED): pages, templates, Brand Kit, assets, version history
- Spool (SOURCE-CODE): `app/src/canvas.rs` (`DesignObject`, `Document`, `ObjectId`, `ObjectStyle`,
  `Selection`, `History`), `app/src/layers.rs`, `app/src/shell.rs`
- Cross-references: `architecture/document-model.md`, `architecture/editor-runtime.md`,
  `document/*.md`, `profiles/spool.md`
