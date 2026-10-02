# Document Model: Objects

What an "object" is, in each product, and what that implies for Spool.

## The fundamental disagreement

The four products do not agree on what a document contains.

| Product | The unit of the document | Vocabulary |
|---|---|---|
| **Figma** | **Layer** — "Every shape, text object, or image you add to the canvas has its own layer" | layer, frame, group, section, component, instance |
| **tldraw** | **Record** — "Everything in the store is a record. A record is a JSON object with an `id` and a `typeName`" | record (shape, page, asset, binding, + custom), shape |
| **Affinity** | **Layer** — with type-specific layer kinds | layer, container layer, pixel layer, curve, shape, text frame, group |
| **Canva** | **Element** | element, page, group |

[INFERRED] "Layer" is the user-facing word but it is *not* the right internal term in any product —
Figma uses a group as a layer, and tldraw's "record" covers pages and assets which are not layers at all.
**Spool should avoid the word "layer" for the internal object type** and reserve it for the layers-panel
presentation.

## Identity

| Product | Identity form | Notes |
|---|---|---|
| Figma | Server-assigned node id | Stable across sessions |
| tldraw | **Branded string with a type prefix**: `'shape:abc123'`, `'page:…'`, `'binding:…'` | "This prevents accidentally mixing up IDs from different record types" — survives JSON serialisation |
| Affinity | Unknown | — |
| Canva | Unknown | — |

[DOCUMENTED — `sdk-features/store.mdx`]

[INFERRED] The type-prefixed id is a small, high-value decision: it makes "pass a shape id where a page id
belongs" a type error rather than a runtime bug, and it survives serialisation without a wrapper type. For
a Rust document model this maps naturally to newtypes — `ObjectId`, `PageId`, `AssetId`, `BindingId` —
which is the correct equivalent.

## Object types and how they are organised

### Figma: types are separate kinds

Frame, Section, Group, Rectangle, Ellipse, Line, Polygon, Star, Vector, Text, Image, Component, Component
set, Instance, Slice, Ellipse/Line inside a shape, Boolean group (a frame holding the operands plus a
`booleanOperation` property — the exact internal form is **Unknown**), Sticky, Table, Connector, Code block,
Text, Video.

[INFERRED] Figma has one object kind per shape type, which means every visitor (hit test, render, snap,
serialise, agent operation) must handle each kind.

### tldraw: one shape type with a discriminator

```ts
{
  id: 'shape:abc123',
  typeName: 'shape',
  type: 'geo',                       // ← the kind
  x: 100, y: 200,
  props: { geo: 'rectangle', w: 300, h: 150, color: 'blue' },  // ← kind-specific props
}
```

[INFERRED] `type` selects a `ShapeUtil` class; `props` is validated against that util's schema. The
consequences are large:
- Adding a new shape type adds a class, not a variant in every enum.
- All shared behaviour (transform, snap, cull, bind, tab) is implemented once.
- Serialisation is uniform.
- Rendering is per-util.

This is the single most transferable structural idea in the whole research for a Rust implementation:
**a `ShapeUtil`-equivalent trait** with `get_geometry`, `can_snap`, `can_cull`, `can_bind`, `get_text`,
`component`.

### Affinity: type-specific layers

Container layer, pixel layer, curve, shape, text frame, text box, image frame, group, selection,
adjustment layer, brush stroke. [DOCUMENTED]

### Canva: one element type, many element kinds

[DOCUMENTED] Canva's UI speaks of "elements" uniformly.

## Universal object properties

Derived by comparing all four products:

| Property | Figma | tldraw | Affinity | Canva |
|---|---|---|---|---|
| id | Yes | Yes | Yes | Yes |
| name | Yes (rename `⇧⌘R` in Affinity) | Not a first-class shape field (metadata) | Yes | Yes |
| type | Yes | `type` + `typeName` | Layer kind | Element kind |
| x, y | Yes (bounds top-left) | Yes | Yes | Yes |
| width, height | Yes | `props.w`, `props.h` | Yes | Yes |
| rotation | Yes | Yes | Yes | Yes |
| visible | Yes | Yes | Yes | Yes |
| locked | Yes | Yes | Yes | Yes |
| opacity | Yes | Yes (per-shape alpha) | Yes | Yes |
| blend mode | Yes | Documented | **27 modes** | Unknown |
| effects | Yes (separate from fill) | Yes | Yes (`⌃⌘V` paste FX — separate from style) | Yes |
| fill | Yes | `props.color` (named palette) | Yes (fill/stroke) | Yes |
| stroke | Yes | Yes | Yes | Yes |
| parent | Yes | `parentId` | Yes | Yes |
| index (z-order) | Layer order | **Fractional string index** | Layer order | Layer order |
| meta | Yes | `meta: JsonObject` on records | Unknown | Unknown |

[DOCUMENTED where cited]

[INFERRED] `meta: JsonObject` in tldraw is worth copying: a typed core plus a free-form per-object
extension slot. It lets features ship without a schema migration, and it gives an agent somewhere to
record provenance ("this object was inferred by Magic Layers") or editor-only annotations.

## The object envelope question

Two structurally different envelopes:

**Type-per-object (Figma, Affinity):**
```
Object = { id, kind: Frame | Text | Path | Image | …, …kind-specific fields }
```

**Record-with-props (tldraw):**
```
Record = { id, typeName, type, ...common fields, props: validated map, meta }
```

[INFERRED] In Rust the tldraw model maps to a trait-object approach:

```rust
trait ObjectKind {
    fn kind(&self) -> ObjectKindId;
    fn geometry(&self) -> Geometry;          // bounds, rotation, transforms
    fn hit_test(&self, p: Point) -> bool;
    fn text(&self) -> Option<&str>;
    fn can_snap(&self) -> bool;
    fn can_cull(&self) -> bool;
    fn serialize(&self) -> serde_json::Value;
}
```
with `type_name` ↔ `typeName` and `props` ↔ a `serde_json::Map` validated at load.

The Figma/Affinity model maps to an enum with per-variant fields. That is easier to make exhaustive in
Rust (the compiler finds every missing arm) but requires editing the enum and every match site to add a
kind.

## Geometry representation

| Product | Model |
|---|---|
| Figma | Bounds (x, y, w, h) + rotation + per-object rotation origin; flips stored in a matrix; export uses CSS `transform`/`matrix()` |
| tldraw | `x`, `y`, `rotation`, `props.w`, `props.h`; a `Geometry2d` layer (Rectangle2d, Polyline2d, …) per util |
| Affinity | Unknown; a Transform panel exists in Layout Studio |
| Canva | Position panel X/Y/W/H + relative positioning modes |

[INFERRED] Nobody stores a general affine matrix as the primary representation, and all three that support
flip end up expressing it in CSS `matrix()` at export. That is evidence that a matrix is a good
*interchange* form and a poor *authoring* form.

## Persistence and serialisation

| Product | Format | Migrations |
|---|---|---|
| Figma | `.fig` — a documented scene-graph-like JSON; the Cornell CS 5152 lab uses it as a scene-graph teaching format [THIRD-PARTY] | Unknown |
| tldraw | **`getSnapshot(store) → { document, session }`**, JSON-serialisable; `loadSnapshot`; schema version in the snapshot; **per-shape and per-record-type migration sequences** (`TLPropsMigrations`, `MigrationSequence`) | **First-class, per-type, `up`/`down` functions** |
| Affinity | `.afdesign` etc.; preset bundles | Unknown |
| Canva | Proprietary | Unknown |

[DOCUMENTED — tldraw; SOURCE-CODE for the migration shape]

[INFERRED] tldraw's migration system is the strongest published model here: a named, ordered sequence of
`{ id, up, down }` functions per record type, run automatically when loading an older snapshot. For an
open-source product whose files will be shared, **schema migration is a foundational concern, not a
later concern** — and it constrains the document model (ids must be stable, records must be
self-describing, `props` must be open enough to extend).

## Custom record types

tldraw allows an application to register new record types in the store:

```ts
const store = createTLStore({
  records: {
    comment: {
      scope: 'document',
      validator: T.object({ id: T.string, typeName: T.literal('comment'), shapeId: T.string, … }),
      createDefaultProperties: () => ({ createdAt: Date.now() }),
      migrations: commentMigrations,
    },
  },
})
```

[DOCUMENTED — `sdk-features/store.mdx`]

"Custom records appear in `store.listen` diffs and **participate in undo/redo** when written through the
usual store APIs."

[INFERRED] This is the extensibility mechanism a "VS Code for design" product needs: comments, annotations,
plugins' data, and AI provenance can all be first-class records without forking the core. Any Spool
architecture review should ask whether adding "a new persistent record type" requires editing the core
document model. If yes, extensibility is compromised.

## Text as an object vs. a property

| Product | Model |
|---|---|
| Figma | Text is its own layer type |
| tldraw | Text is a **shape type** (`type: 'text'`) with `props.richText` |
| Affinity | Text Frame / Text Box are their own layer kinds |
| Canva | Text is an element kind |

[DOCUMENTED]

[INFERRED] All four treat text as a distinct object type rather than a property of a shape. That is
consistent and probably right: text needs caret state, editing mode, and typographic bounds.

## Spool prototype: what exists

From `app/src/canvas.rs` [OBSERVED in source]:

```rust
pub struct ObjectId(pub u64);

pub enum ObjectType { Frame, Rectangle, Ellipse, Text }

pub struct DesignObject {
    pub id: ObjectId,
    pub name: String,
    pub position: Point<f32>,
    pub size: Size<f32>,
    pub object_type: ObjectType,
    pub text_content: Option<String>,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

pub struct Document {
    objects: Vec<DesignObject>,
    next_id: u64,
    next_names: [u64; 4],
    layer_structure_revision: u64,
}
```

Observations:

- **Flat `Vec<DesignObject>`; no parent, no hierarchy, no index, no page.** Z-order is vector position.
- `ObjectId` is a newtype over `u64` — good (the type-safety intent matches tldraw's).
- `next_id` is monotonically increasing; `ObjectId::LANDING/EDITOR/FEATURES/MOBILE` are fixed constants 1–4
  seeding the default document.
- `next_names: [u64; 4]` — one counter per object type for auto-naming ("Rectangle 2"). Implies auto-naming
  is a known requirement.
- `layer_structure_revision: u64` — a monotonic counter bumped on structural change, used to invalidate the
  layers projection without diffing. **This is a useful pattern**: cheap invalidation for a derived view.
- `hit_test` is `self.objects.iter().rev().find(|o| o.contains(world_point))` — linear scan, last-wins
  (topmost), AABB containment only. No spatial index.
- `objects_in(bounds)` filters the whole vector with `WorldRect::contains_object`.
- No rotation, no visible/locked, no opacity, no blend mode, no effects, no meta, no serialisation, no
  migrations, no asset references.
- No custom-type extension point.

## Candidate architectural implication

**Evidence:**

1. **One shape type with a discriminator + per-kind behaviour object** (tldraw) reduces the cost of adding
   a shape kind from O(all match sites) to O(1 new impl). [SOURCE-CODE]
2. **Type-prefixed / newtype ids** prevent cross-type id confusion and survive serialisation.
   [DOCUMENTED]
3. **A free-form `meta` slot** on every record lets features ship without schema migration.
   [DOCUMENTED]
4. **Registerable record types** let an application add persistent concepts without forking the core.
   [DOCUMENTED]
5. **Per-type, named, `up`/`down` migration sequences** are the published approach to schema evolution.
   [DOCUMENTED]
6. **All four products treat text as a distinct object type.** [DOCUMENTED]
7. **Nobody stores an affine matrix as the authoring representation.** [INFERRED from three products]
8. A **flat vector with a structure revision counter** is a perfectly good interim representation, but
   linear hit testing and linear queries will not scale. [OBSERVED in Spool's source]

**Why it matters:** The object envelope is the hardest thing to change later. Every system — renderer,
hit test, snap engine, layers panel, serialiser, operation API, agent API — is written against it.

**Potential Spool approaches:**

- **A. Enum-per-kind (Figma/Affinity style).** Rust-idiomatic; exhaustive matching; expensive to add a
  kind.
- **B. Discriminator + trait object (tldraw style).** Cheap to add a kind; requires dynamic dispatch in hot
  paths; loses compile-time exhaustiveness.
- **C. Enum-per-kind + a `ShapeBehaviour` trait per kind** — a hybrid: the enum gives exhaustive
  compile-time matching, the trait gives uniform access to geometry/hit-test/text/snap/cull. Adding a kind
  requires a new enum arm plus a new impl, and the compiler finds every site that needs updating.

**Tradeoffs:** A is simplest and best for a small fixed set. B is best for extensibility. C costs one
extra indirection and is a reasonable middle ground for Rust; it preserves compile-time safety *and*
behavioural uniformity, but does not by itself allow third parties to add kinds at runtime (which requires
a registry — an enum and a dynamic registry are mutually exclusive).

[INFERRED] **A runtime registry of object kinds implies option B.** Since Spool's stated goal is
extensibility ("VS Code for visual design"), the tension between compile-time exhaustiveness and runtime
extensibility is fundamental and must be decided explicitly, not deferred.

**Decision: TBD — requires architecture review.**

## Open questions

1. Enum-per-kind or discriminator + registry?
2. Can Spool files be shared between versions? If yes, migrations are foundational from day one.
3. Is `meta` available on every object? (Needed for AI provenance and plugin data.)
4. What is the naming scheme for auto-generated names?
5. How does z-order work before hierarchy exists? (Array position works; it does not survive hierarchy.)
6. Are fixed ids (`ObjectId::LANDING`) a seeding mechanism or a hard-coded demo? [OBSERVED: hard-coded
   demo]

## Sources

- Figma: "Select layers and objects" (/360040449873); "Guide to components in Figma" (/360038662654);
  "Adjust alignment, rotation, position, and dimensions" (/360039956914)
- tldraw: `sdk-features/store.mdx` ⭐ (records, ids, scopes, custom records, migrations, snapshots,
  queries), `sdk-features/shapes.mdx` ⭐ (ShapeUtil hooks), `sdk-features/geometry.mdx`,
  `sdk-features/shape-transforms.mdx`, `sdk-features/shape-indexing.mdx`, `docs/editor.mdx`
- Affinity: "Node Tool" (/tools-tools-node/); "Keyboard shortcuts for general editing"
  (/workspace-shortcuts-editing/)
- Canva: "Add, duplicate, and delete elements" (/add-elements/); "Canva keyboard shortcuts"
  (/canva-keyboard-shortcuts/)
- Spool prototype: `app/src/canvas.rs` (`ObjectId`, `ObjectType`, `DesignObject`, `Document`, `hit_test`,
  `objects_in`, `next_names`, `layer_structure_revision`)
- Cornell CS 5152 lab (`.fig` as a scene-graph JSON format) — https://www.cs.cornell.edu/courses/cs5152/2024sp/labs/design2/ — THIRD-PARTY
