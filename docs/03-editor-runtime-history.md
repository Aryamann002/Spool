# Editor, runtime, operations, and history

## Layers

1. **Source project:** HTML/CSS/SVG and `lamine.yaml`; saved and human-readable.
2. **Document model:** validated persistent semantic nodes with stable IDs and source provenance.
3. **Operation layer:** typed commands with validation, before/after values, and explicit failure. This is the only mutation entry point.
4. **History:** committed operations, inverse application for undo, forward application for redo. No-op operations do not enter history; a new committed edit clears redo.
5. **Editor state:** transient selection, camera, active tool, gestures, text caret, and guides.
6. **Runtime/renderer:** disposable geometry, hit-test data, layout, and GPUI drawing. It observes document revisions and rebuilds only affected derived state.

## Operation contract

An operation resolves a stable Spool ID, validates its target and source ownership, computes a source/metadata patch, applies it as one unit, and returns a reversible semantic record. The same operation must be callable from a pointer gesture, keyboard action, menu, automation, or future agent. A failed operation changes neither project source nor metadata nor history.

For the first slice, implement identity/binding plus one semantic edit that exercises the whole path (rename a node in metadata); HTML remains the content authority. Subsequent operations may edit HTML/CSS only after source-preserving parsing and ownership are established.

## Interaction and history

Continuous gestures may update transient previews. Pointer-up commits one operation; Escape restores the pre-gesture value without creating an entry. Undo applies the operation's `before` state; redo applies `after`. History stores IDs and semantic values, never byte offsets. Re-resolve source locations at application time.

## Runtime and renderer

The runtime is constructed from validated source and metadata, and may cache resolved layout and paint data. It does not rewrite canonical source merely by loading. GPUI is the current frontend, not the document model. Keep renderer-independent operations and model tests possible without opening a window.

## Failure and external edits

Parsing, validation, and source patching return structured errors. Do not partially apply multi-file edits. External edits invalidate affected bindings and derived data; preserve the user's bytes and report conflicts for explicit resolution. Never silently pick one of multiple matching CSS declarations.
