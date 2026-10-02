# Document Model: Bindings

**A binding is a persistent, first-class relationship between two objects that is not containment.**

tldraw is the only product in this research whose binding system is documented at the record level. It is
also the abstraction that a prototype/connector layer, an "AI operation", and an attachment system would
all need. It is therefore worth reading closely even though only one product uses the term.

## The model

[Fully DOCUMENTED — `sdk-features/bindings.mdx`]

> "Bindings create persistent relationships between shapes. When you draw an arrow to a rectangle, a binding
> stores that connection so the arrow stays attached when you move the rectangle. Bindings power features
> like arrows that follow shapes, stickers that stick to other shapes, and **layout constraints** that keep
> shapes aligned."

### Record shape

```ts
interface TLBaseBinding<Type, Props> {
  id: TLBindingId
  typeName: 'binding'
  type: Type
  fromId: TLShapeId
  toId: TLShapeId
  props: Props
  meta: JsonObject
}
```

Arrow bindings store:

```ts
props: {
  terminal: 'end',
  normalizedAnchor: { x: 0.5, y: 0.5 },   // ← normalized position on the target
  isPrecise: false,
  isExact: false,
  snap: 'none',
}
```

[DOCUMENTED]

### Directionality

> "Every binding has direction. The `fromId` points to the source shape, and the `toId` points to the target
> shape. For arrows, the arrow is always the 'from' shape and the shape it points to is the 'to' shape.
> This directionality determines which lifecycle hooks fire and lets the system know which shape 'owns' the
> relationship."
>
> "If you move a rectangle that an arrow points to, the arrow binding's `onAfterChangeToShape` hook fires.
> If you move the arrow itself, `onAfterChangeFromShape` fires instead."

[INFERRED] `normalizedAnchor` is the key idea: the attachment point is stored **as a fraction of the
target's local bounds**, not as a world or parent coordinate. That makes the attachment survive the target
resizing.

## The BindingUtil lifecycle

[DOCUMENTED — the most complete published lifecycle in this research]

| Hooks | When they fire |
|---|---|
| `onBeforeCreate`, `onAfterCreate`, `onBeforeChange`, `onAfterChange` | The **binding record itself** is created or modified. `onBefore*` can return a replacement record |
| `onAfterChangeFromShape`, `onAfterChangeToShape` | **A bound shape changes.** "These are the most common hooks for keeping shapes synchronized." Arrow bindings use them to update the arrow's position and parent when the target moves |
| `onBeforeDelete`, `onAfterDelete` | The binding record is removed |
| `onBeforeDeleteFromShape`, `onBeforeDeleteToShape` | A bound shape is about to be deleted |
| `onBeforeIsolateFromShape`, `onBeforeIsolateToShape` | **The bound shapes are about to be separated** (one is deleted, copied, or duplicated without the other). "Use these to 'bake in' the binding's current state before it disappears." |
| `onOperationComplete` | "All binding operations in a transaction have finished. Use it to compute aggregate updates across many related bindings." |

## Isolation vs. deletion — the key distinction

[DOCUMENTED, quoted]

> "Isolation callbacks handle a specific problem: when an arrow's target shape is deleted, the arrow
> shouldn't suddenly point to empty space. The `onBeforeIsolateFromShape` hook receives the binding and the
> `removedShape`, and lets the arrow update its terminal position to match the current attachment point
> before the binding is removed. **The arrow then appears to 'let go' of the shape naturally.**
>
> "Isolation also occurs during copy and duplicate operations. **If you copy an arrow but not its target,
> the copied arrow needs to convert its binding into a fixed position.** The isolation callback handles
> this transformation."

> "Use isolation callbacks for consistency updates that should happen whenever shapes separate. Use
> `onBeforeDeleteFromShape` and `onBeforeDeleteToShape` for actions specific to deletion, like removing a
> sticker when its parent shape is deleted."

[INFERRED] **This is the most important architectural idea in the document that appears in none of the
other three products.** A relationship system that only models "the target was deleted" will be wrong in
at least these cases:

1. Target deleted.
2. Target copied without the source.
3. Source copied without the target.
4. Source and target moved to different parents/pages.
5. One of them converted (e.g. flattened, or detached from a component instance).

All five are *separation* events. tldraw's naming — "isolation" — makes that visible. A model with only
`onDelete` hooks will silently produce dangling relationships in four of five cases.

## Automatic bookkeeping

[DOCUMENTED]

- Deleting a shape removes its bindings and fires isolation + deletion callbacks.
- **Copying shapes duplicates only bindings between the copied shapes.**
- **Moving shapes to different pages removes cross-page bindings automatically.**
- Both bound shapes copied/duplicated together ⇒ the binding is copied with them.
- `createBinding` checks both shapes' `canBind()` and **skips the binding if either refuses**.

[INFERRED] "Move to a different page removes cross-page bindings" implies a **document-wide scope
constraint**: a relationship cannot cross a page boundary. This becomes important if Spool ever has
multiple pages or documents.

## The bindings index

[DOCUMENTED]

> "The editor maintains an index of all bindings touching each shape. […] **The bindings index is a computed
> value that updates incrementally** as bindings change. Lookups are fast and never scan all records."

API: `getBinding`, `getBindingsFromShape`, `getBindingsToShape`, `getBindingsInvolvingShape`.

[INFERRED] A maintained reverse index (`shape → bindings`) is required for the `onAfterChange*` hooks to be
cheap. Without it, moving one shape requires a full scan.

## Using bindings beyond arrows

[DOCUMENTED — from tldraw's own docs and examples]

1. **Arrows following shapes** — the canonical case.
2. **Stickers attached to frames.**
3. **Layout constraints** ("keep shapes aligned") — third-party patterns built on bindings.
4. **Arrows bound to a frame while dragging** ("arrows bound to a shape" is a documented driver query
   helper). [DOCUMENTED — `docs/driver.mdx`]
5. **Node-based visual programming** — the Workflow starter kit: "Connections between nodes are bindings
   that track relationships as shapes move." [DOCUMENTED — `docs/ai.mdx`]

## Relationship to hierarchy

| | Containment (`parentId`) | Binding |
|---|---|---|
| Cardinality | 0 or 1 parent | Many-to-many, directional |
| Owns the child? | **Yes** — the container controls the child's coordinate space | No |
| Survives reparenting? | No — reparenting changes the coordinate basis | Usually yes |
| Affects layout? | Yes | Only if a binding type implements it |
| Affects hit testing? | No (children hit-tested independently) | No |
| Deleted when one end is deleted? | Yes | No — triggers isolation |

[DOCUMENTED for tldraw; INFERRED for the comparison]

[INFERRED] **These are genuinely orthogonal relationships and conflating them is a common error.** A shape
can be a child of a frame and bound to a shape in a different frame.

## Bindings and history

[DOCUMENTED]

- Bindings are records, so they "participate in undo/redo when written through the usual store APIs".
- The `onAfterChange*` hooks fire when a bound shape changes, which means a single move can cascade into
  binding-driven writes.
- `onOperationComplete` exists precisely to let a binding type **collapse a cascade into an aggregate
  update** at the end of a transaction.

[INFERRED] The cascade problem: moving shape A, where 50 arrows are bound to A, produces 50 writes. Without
`onOperationComplete`, that is 50 history entries or one transaction with 50 diffs. With it, the binding type
can compute the aggregate and apply one update per arrow at the end. This is a real and subtle requirement.

## Bindings and collaboration

[DOCUMENTED]

- Bindings are `document`-scoped records, so they **sync**.
- Copy semantics ("only bindings between the copied shapes") must be deterministic for merge correctness.
- Cross-page bindings are removed on move, which is a scope rule, not a conflict-resolution rule.

## Where the other products have this concept

| Product | Equivalent | Naming |
|---|---|---|
| tldraw | Bindings | Documented record type |
| Figma | **Prototype connections** (a "noodle" from one frame to another) | Yes — "drag out a noodle and link it to the object" |
| Figma | **Instance swap** (a reference to another component) | Not a binding; an override value |
| Affinity | **Layers panel parent/containment** only; **smart guides** are transient | No persistent non-containment relationship documented |
| Canva | **Element-anchored comments** (`⌥⌘J` jump to comment on selected element) | Implies a persistent comment→element reference |

[DOCUMENTED for Figma's prototype noodles and Canva's element-anchored comments]

[INFERRED] **Figma's prototype connection is a binding**, described with different vocabulary. It has:
- a source node,
- a target node,
- a direction (normally source → destination),
- and it is drawn as a "noodle".

[INFERRED] **Canva's element-anchored comment is a binding** from a comment record to an element. The fact
that Canva can "jump to the comment on the selected element" requires that relationship to be stored.

So although only tldraw uses the word, the concept exists in at least three products.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

- **Nothing.** No binding, no relationship, no reference between objects.
- `Tool::Comment` is declared but `creates_object()` returns `None` — a placeholder with no backing model.
- `Document` has no reference table and `DesignObject` has no id fields other than its own.
- `shell.rs` has `prototype_inspector()` — UI chrome only.

[INFERRED] Bindings are entirely ahead. However — the prototype *does* have the operation that would need
them: `MarqueeGesture` selects objects, and a future "connect these two" tool would need exactly this
abstraction.

## Candidate architectural implication

**Evidence:**

1. A binding is a **persistent record with `fromId`/`toId`/`props`/`meta`**, not a pointer stored on either
   end. [DOCUMENTED]
2. Attachments store a **normalized anchor** (fraction of the target's local bounds) so they survive resize.
   [DOCUMENTED]
3. Direction is meaningful and determines **which lifecycle hooks fire**. [DOCUMENTED]
4. **Separation (isolation) is distinct from deletion**, and covers copy, duplicate, page moves, and
   conversion — not just delete. [DOCUMENTED]
5. Cascading updates need an **`onOperationComplete` aggregation hook** to avoid N-write explosions.
   [DOCUMENTED]
6. Bookkeeping rules are non-obvious and must be specified: copy only internal bindings, drop cross-page
   bindings, clean up on delete. [DOCUMENTED]
7. A **reverse index** is required for cheap hook dispatch. [DOCUMENTED]
8. Bindings participate in undo/redo because they are records. [DOCUMENTED]
9. A **relationship cannot cross a page/document boundary.** [DOCUMENTED for tldraw]
10. Figma's prototype connections and Canva's element-anchored comments are the same concept under
    different names. [DOCUMENTED + INFERRED]
11. Containment and binding are **orthogonal**. [INFERRED]

**Why it matters:** Bindings are what a prototype/connector system, an attachment system, a comment system,
a variable-driven binding system, and any "these two things are related" AI operation need. They are also
where history cascades get expensive.

**Potential Spool approaches:**

- **A. No bindings.** Store the relationship inside the arrow object (`target_id` on the arrow). Simpler;
  makes cleanup on target deletion manual; asymmetric cleanup.
- **B. Bidirectional pointers** (`source.bound_to`, `target.bound_from`). Simple lookups; duplicated state;
  deletion requires scanning.
- **C. A binding record table** (tldraw's model) with a reverse index and lifecycle hooks.
- **D. C + a normalised-anchor prop convention** and per-binding-type behaviour objects.

[INFERRED] **C/D is the right answer if Spool ever wants prototyping, connectors, or comments.** A is
acceptable if arrows are the only use case and are arrow-like in spirit. The cost difference is small now
and large later, because retrofitting a binding table means migrating relationships out of object fields.

[INFERRED] The isolation/deletion distinction should be adopted **even if bindings are stored as fields**,
because it changes how the delete, copy, and reparent code paths are written.

**Decision: TBD — requires architecture review.**

## Open questions

1. Bindings as records, or as fields on the source object?
2. Are bindings typed (arrow, sticker, prototype-link, comment, layout-constraint)?
3. Can a binding cross a page boundary? (tldraw says no.)
4. What is the anchor representation — normalised fraction, edge index, or free point?
5. What is the delete policy — cascade, isolate, or both? (tldraw: isolate the relationship, keep the
   object.)
6. How are binding-driven updates aggregated for history?
7. Does reparenting preserve or break a binding?
8. Are prototype connections the same record type as arrows, or a different one?

## Sources

- tldraw: `sdk-features/bindings.mdx` ⭐⭐ (record shape, directionality, lifecycle hooks, isolation,
  bookkeeping, index, API), `sdk-features/handles.mdx`, `sdk-features/arrow-shape.mdx`,
  `sdk-features/store.mdx` (records participate in history), `docs/ai.mdx` (workflow bindings),
  `docs/driver.mdx` (arrows bound to a shape)
- Figma: "Prototype actions" (/360040035874) ⭐ (Scroll to, "drag out a noodle and link it",
  Open/Close/Swap overlay); "Connect your prototype" (/360040315773)
- Affinity: "Node Tool" (/tools-tools-node/); "Keyboard shortcuts for general editing"
  (/workspace-shortcuts-editing/)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/) (jump to comment on selected element)
- Spool prototype: `app/src/canvas.rs` (`Tool::Comment`, `Document`, `DesignObject`), `app/src/shell.rs`
  (`prototype_inspector`)
