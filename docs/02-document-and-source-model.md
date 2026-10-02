# Spool document and source model

## Authorities

The project source consists of HTML, CSS, and SVG files. Those files define authored content and visual implementation. `lamine.yaml` indexes Spool concepts that markup alone cannot reliably supply. The native Rust document/runtime is compiled from those files and the index and may be discarded and rebuilt.

```text
HTML / CSS / SVG ─┐
                  ├─ load + validate ─> native runtime ─> GPUI renderer
lamine.yaml ──────┘
        semantic operation ─> source + metadata updates ─> history
```

## `lamine.yaml`

One record per managed node. The required record is: immutable `id`, unique human `name`, semantic `kind`, optional `parent`, ordered `children`, and `source.file` plus `source.selector` (or a source range when a selector is not stable). IDs are opaque and never derived from names, DOM order, or CSS classes. Renaming changes only the name. Reordering changes hierarchy, not identity. Duplicate IDs, names, dangling links, and ambiguous source bindings fail validation.

The file may contain Spool-only facts such as frame identity, mask/group semantics, or cached child count when a feature proves it needs them. It must not own text, geometry, fills, typography, or other values already authored in HTML/CSS/SVG. Derived counts and runtime caches are rebuildable and must not become competing truth.

## Source ownership and provenance

Each runtime node retains its stable Spool ID and a source binding. A semantic edit records which source file and authored construct it changes. Source ranges are derived from the current parse and are never durable identity. External source edits trigger re-parse and re-binding; missing or ambiguous identities are reported, not guessed. Source preservation is a correctness property: edits must leave bytes outside their intended source span unchanged.

## Runtime boundary

Persistent `Document` contains source references, metadata, and source-derived semantic nodes. `EditorState` contains selection, camera, tools, hover, gestures, caret, and temporary guides. Renderer structures, indexes, layout results, and caches are runtime-only. No GPUI entity or view state is serialized.

## Initial format contract

The first implementation supports explicit Spool IDs and source bindings for a deliberately small HTML subset. It does not claim general CSS cascade support. Unsupported, external, or ambiguous CSS must remain untouched until a dedicated ownership rule exists. Any cache is optional; deleting it cannot lose user work.
