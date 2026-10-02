# Spool product and architecture rules

This document governs implementation. Research in `docs/research/` is evidence and background; it does not override these decisions.

## Product

Spool is a native visual editor for source-authored designs. A person must be able to inspect and edit the project as ordinary HTML, CSS, and SVG, while the editor provides direct manipulation, semantic layers, and undoable actions.

## Non-negotiable boundaries

1. HTML, CSS, and SVG are the authoritative implementation and visual source. Spool must preserve source authorship when it changes source.
2. `lamine.yaml` is a structural and semantic index: stable Spool identity, naming, hierarchy, source bindings, and Spool-only facts. It is not a second project database and must not duplicate CSS values or HTML content.
3. The Rust runtime is a disposable, optimized interpretation of source plus metadata. It can be rebuilt at any time and is never a persistence authority.
4. Persistent document state and editor/session state have distinct types. Selection, camera, active tool, hover, pointer/gesture state, and text caret never enter the saved document.
5. Every user-visible persistent mutation goes through a typed semantic operation. Input devices, menus, automation, and later agents call the same operation path.
6. A committed semantic action is one undo step. A cancelled interaction restores its starting state and contributes no history entry. History stores semantic before/after intent, not parser offsets.
7. Keep source edits minimal and traceable. Every node resolves to a source file and selector/range; ambiguity or unsupported CSS ownership must be surfaced rather than silently rewriting shared rules.
8. The native renderer owns rendering and interaction. Do not embed a browser DOM as the editor's persistent or runtime scene graph.

## Change discipline

Add capabilities in vertical slices that preserve current editor behavior. No new subsystem is justified by hypothetical future scale alone; first establish a measured bottleneck and an acceptance test.
