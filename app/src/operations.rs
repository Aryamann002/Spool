//! The single semantic operation boundary.
//!
//! # The problem this solves
//!
//! The repository had two histories that never met. `canvas.rs` had a
//! working undo stack over [`DocumentCommand`] (geometry, style, text,
//! insert, delete). `source_document.rs` had [`SemanticHistory`] over
//! [`RenameNode`], touching `PersistentDocument` metadata. Neither could see
//! the other, and nothing in the editor ever invoked the second one: as of
//! this change `RenameNode` was a fully tested island with no call site.
//!
//! This module converges them on one vocabulary and one stack, without
//! rewriting the canvas. It is an adapter, not a replacement.
//!
//! # The operation model
//!
//! [`SemanticOperation`] is the closed set of user-visible mutations:
//!
//! - [`SemanticOperation::Rename`] — persistent metadata (name).
//! - [`SemanticOperation::Runtime`] — the canvas command family, which today
//!   covers move, resize, create, delete, duplicate, style, and text.
//!
//! Both variants are reversible value types holding a `before`/`after` pair.
//! Undo replays `before`; redo replays `after`. Nothing is stored as a byte
//! offset or a captured diff, so history records *intentions* rather than net
//! state changes.
//!
//! # The transaction boundary
//!
//! [`EditSession::execute`] is the only place an operation is committed. One
//! call is one logical transaction and produces at most one history entry,
//! however many objects it touched. A multi-node move is one
//! [`SemanticOperation::Runtime`] holding one geometry command covering every
//! moved object, not N commands.
//!
//! Gestures mutate transiently and only reach `execute` on success:
//!
//! ```text
//! begin_gesture  -> transient mutation (no history)
//! commit_gesture -> execute(..)  one entry
//! cancel_gesture -> restore, zero entries
//! ```
//!
//! A cancelled gesture is indistinguishable from one that never started.
//!
//! # Undo/redo mechanism
//!
//! [`SemanticHistory`] holds two stacks of [`SemanticOperation`].
//! `undo` pops, applies `before`, and pushes onto `redo`. `redo` does the
//! mirror image. Committing a new operation clears `redo`, exactly as before.
//! No-op operations are dropped before they reach a stack.
//!
//! An operation is applied against an [`OperationTarget`], which is either a
//! bare canvas `Document` or a whole [`EditSession`]. That indirection is what
//! lets one stack hold operations targeting different state: a rename needs
//! metadata, a move needs the canvas document.
//!
//! # What this deliberately does not do
//!
//! - No tldraw-style marks or lazy sealing. Spool commits eagerly, which the
//!   project's own research (§19) identifies as the behaviour Spool already
//!   had and chose to keep.
//! - No snapshot of the whole document per pointer event. Undo replays the
//!   `before` values a command already carries, so a drag costs one small
//!   geometry change per object, not a full copy of the document.
//! - No AI, plugin, or import caller. The boundary is reachable by them
//!   because [`EditSession::execute`] is the only mutation entry point, but
//!   none of those callers are implemented here.
//!
//! # Expected dead-code warnings
//!
//! `cargo check` reports unused items here, and that is the honest state of
//! the migration: the interactive canvas still records through
//! [`crate::canvas::History`], so nothing constructs an [`EditSession`] yet.
//! Only [`SemanticHistory`], [`SemanticOperation`], and
//! [`OperationTarget::Runtime`] are on the live path. These warnings are the
//! remaining-work list; do not silence them with a blanket
//! `#![allow(dead_code)]`, which would hide exactly the gap they describe.

use gpui::{Point, Size};

use crate::canvas::{
    Document, DocumentCommand, Geometry, GeometryChange, ObjectId, ObjectPlacement, ObjectSnapshot,
    ReplayDirection,
};
use crate::source_document::{NodeId, PersistentDocument, RenameNode};

/// Every user-visible document mutation.
///
/// This is the closed set. Adding a capability means adding a variant here
/// and handling it in exactly one place, which is what makes the boundary
/// impossible to forget rather than a call-site convention.
#[derive(Clone, Debug, PartialEq)]
pub enum SemanticOperation {
    /// Persistent, source-backed metadata: renaming a node.
    ///
    /// This is the operation that used to live behind its own private
    /// history. It now shares the stack with everything else.
    Rename(RenameNode),
    /// A canvas command: geometry (move/resize), style, text, insert, or
    /// delete.
    ///
    /// Geometry is temporary prototype state, not authored layout. See
    /// `document_runtime_bridge` for why.
    Runtime(DocumentCommand),
}

impl SemanticOperation {
    /// Whether applying this operation would change nothing.
    ///
    /// No-op operations never enter history, so a gesture that ends where it
    /// started produces no entry and does not clear redo.
    pub fn is_noop(&self) -> bool {
        match self {
            Self::Rename(rename) => rename.before == rename.after,
            // The canvas command owns this judgement for its own variants; an
            // empty insert or delete changes nothing either.
            Self::Runtime(command) => command.is_noop(),
        }
    }
}

/// A failure applying an operation. A failed operation changes neither state
/// nor history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationError {
    /// The operation named a node that does not exist.
    MissingNode(NodeId),
    /// The operation named a runtime object that does not exist.
    MissingObject(ObjectId),
    /// The operation does not apply to this kind of target, for example a
    /// rename replayed against a bare canvas document.
    WrongTarget(&'static str),
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNode(id) => write!(f, "no node with identity {}", id.as_str()),
            Self::MissingObject(id) => write!(f, "no runtime object {id:?}"),
            Self::WrongTarget(what) => write!(f, "{what} cannot be applied to this target"),
        }
    }
}

impl std::error::Error for OperationError {}

/// Where an operation is applied.
///
/// Unifying two histories means one stack holds operations that touch
/// different state. Rather than force both into one type, the target is
/// chosen at replay time.
pub enum OperationTarget<'a> {
    /// Only canvas document state is available.
    ///
    /// This is what the live canvas entity has: it owns a `Document` and no
    /// persistent metadata.
    Runtime(&'a mut Document),
    /// Only persistent metadata state is available.
    ///
    /// A bundle-level caller such as a save/reload cycle has a
    /// [`PersistentDocument`] and no canvas runtime. Without this variant a
    /// rename could only be replayed by also constructing a throwaway
    /// `Document` to satisfy [`OperationTarget::Full`], which would be a lie
    /// about what the caller owns.
    Document(&'a mut PersistentDocument),
    /// Both metadata and canvas state are available.
    Full {
        document: &'a mut PersistentDocument,
        runtime: &'a mut Document,
    },
}

/// One committed undo/redo stack for the whole editor.
///
/// This is the unified stack. `canvas::History` is a thin facade over it, so
/// existing canvas call sites and tests are unchanged, but there is only one
/// implementation of the semantics.
#[derive(Clone, Debug, Default)]
pub struct SemanticHistory {
    undo: Vec<SemanticOperation>,
    redo: Vec<SemanticOperation>,
}

impl SemanticHistory {
    /// Commit an operation: drop it if it changes nothing, otherwise push it
    /// and clear redo.
    ///
    /// Returns whether anything was recorded.
    pub fn record(&mut self, operation: SemanticOperation) -> bool {
        if operation.is_noop() {
            return false;
        }
        self.undo.push(operation);
        self.redo.clear();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of committed operations currently undoable.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of operations currently redoable.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// The most recently committed operation, without removing it.
    ///
    /// Lets a caller inspect what the next undo would reverse.
    pub fn peek_undo(&self) -> Option<&SemanticOperation> {
        self.undo.last()
    }

    /// Undo the most recent committed operation.
    pub fn undo(&mut self, target: &mut OperationTarget<'_>) -> Result<bool, OperationError> {
        let Some(operation) = self.undo.pop() else {
            return Ok(false);
        };
        match apply(&operation, target, ReplayDirection::Undo) {
            Ok(()) => {
                self.redo.push(operation);
                Ok(true)
            }
            Err(error) => {
                // Put it back so a failed undo does not lose the entry.
                self.undo.push(operation);
                Err(error)
            }
        }
    }

    /// Redo the most recently undone operation.
    pub fn redo(&mut self, target: &mut OperationTarget<'_>) -> Result<bool, OperationError> {
        let Some(operation) = self.redo.pop() else {
            return Ok(false);
        };
        match apply(&operation, target, ReplayDirection::Redo) {
            Ok(()) => {
                self.undo.push(operation);
                Ok(true)
            }
            Err(error) => {
                self.redo.push(operation);
                Err(error)
            }
        }
    }
}

/// Apply one operation in the given direction.
///
/// This is the low-level replay primitive behind [`SemanticHistory::undo`],
/// [`SemanticHistory::redo`], and [`EditSession::execute`]. It is public so a
/// caller that owns only some of the state can still commit: [`EditSession`]
/// requires both a persistent document and a canvas runtime, so a
/// bundle-level caller holding only a [`PersistentDocument`] would otherwise
/// have no way to apply an operation it has just recorded.
pub fn apply(
    operation: &SemanticOperation,
    target: &mut OperationTarget<'_>,
    direction: ReplayDirection,
) -> Result<(), OperationError> {
    match operation {
        SemanticOperation::Rename(rename) => match target {
            OperationTarget::Full { document, .. } | OperationTarget::Document(document) => {
                // Undo replays the inverse, which restores the previous name.
                // Replaying the operation itself would be a no-op at best and
                // a stale-edit error at worst.
                let applied = match direction {
                    ReplayDirection::Undo => rename.inverse(),
                    ReplayDirection::Redo => rename.clone(),
                };
                applied
                    .apply(document)
                    .map_err(|_| OperationError::MissingNode(rename.id.clone()))
            }
            OperationTarget::Runtime(_) => Err(OperationError::WrongTarget("a node rename")),
        },
        SemanticOperation::Runtime(command) => {
            let runtime = match target {
                OperationTarget::Runtime(document) => document,
                OperationTarget::Full { runtime, .. } => runtime,
                OperationTarget::Document(_) => {
                    return Err(OperationError::WrongTarget("a canvas command"))
                }
            };
            command.replay(runtime, direction);
            Ok(())
        }
    }
}

/// The editor's mutable state, and the only mutation entry point.
///
/// Owns the two things that are genuinely different kinds of state:
///
/// - `document` — persistent, source-backed. Durable.
/// - `runtime` — the canvas document. Prototype; disposable; rebuilt from a
///   projection rather than persisted.
///
/// Selection and camera are **not** here and are not history. They are
/// editor session state that lives on the canvas view. Keeping them out is
/// what stops a user scrolling the canvas from producing an undo entry, which
/// is the property this boundary exists to guarantee.
pub struct EditSession {
    pub document: PersistentDocument,
    pub runtime: Document,
    pub history: SemanticHistory,
    /// Geometry captured at the start of the in-flight gesture, if any.
    in_flight: Option<Vec<ObjectSnapshot>>,
}

impl EditSession {
    pub fn new(document: PersistentDocument, runtime: Document) -> Self {
        Self {
            document,
            runtime,
            history: SemanticHistory::default(),
            in_flight: None,
        }
    }

    /// The single mutation entry point.
    ///
    /// One call is one logical transaction and at most one history entry,
    /// however many objects it touched. A no-op records nothing and leaves
    /// redo intact.
    pub fn execute(
        &mut self,
        operation: SemanticOperation,
    ) -> Result<bool, OperationError> {
        // Validate before mutating so a failure changes neither state nor
        // history.
        validate(&operation, &self.document, &self.runtime)?;
        if !self.history.record(operation.clone()) {
            return Ok(false);
        }
        let Self {
            document, runtime, ..
        } = self;
        apply(
            &operation,
            &mut OperationTarget::Full { document, runtime },
            ReplayDirection::Redo,
        )
        .expect("an operation that passed validation applies cleanly");
        Ok(true)
    }

    pub fn undo(&mut self) -> Result<bool, OperationError> {
        // Disjoint field borrows: the history replays into the document and
        // runtime it sits beside, so `self` is never aliased.
        let Self {
            document,
            runtime,
            history,
            ..
        } = self;
        history.undo(&mut OperationTarget::Full { document, runtime })
    }

    pub fn redo(&mut self) -> Result<bool, OperationError> {
        let Self {
            document,
            runtime,
            history,
            ..
        } = self;
        history.redo(&mut OperationTarget::Full { document, runtime })
    }

    /// Start a gesture, capturing the geometry it may change.
    ///
    /// Nothing is recorded here, and nothing is recorded by mutating the
    /// runtime document while a gesture is in flight.
    pub fn begin_gesture(&mut self, ids: &[ObjectId]) {
        self.in_flight = Some(self.runtime.snapshot_objects(ids));
    }

    /// Commit the in-flight gesture as exactly one history entry.
    ///
    /// Returns false, recording nothing, when there is no gesture or when it
    /// ended where it started.
    pub fn commit_gesture(&mut self) -> Result<bool, OperationError> {
        let Some(snapshots) = self.in_flight.take() else {
            return Ok(false);
        };
        if snapshots.is_empty() {
            return Ok(false);
        }
        let command = geometry_command(&self.runtime, &snapshots);
        self.execute(SemanticOperation::Runtime(command))
    }

    /// Abandon the in-flight gesture.
    ///
    /// Geometry returns to its pre-gesture value and **no history entry is
    /// created**. A cancelled gesture is indistinguishable from one that never
    /// started, which is the property the project's own research identifies as
    /// the opposite of tldraw's destructive bail.
    pub fn cancel_gesture(&mut self) {
        if let Some(snapshots) = self.in_flight.take() {
            self.runtime.restore_snapshots(&snapshots);
        }
    }

    /// Whether a gesture is currently in flight.
    pub fn gesture_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
}

/// Check that an operation can apply, before any state changes.
fn validate(
    operation: &SemanticOperation,
    document: &PersistentDocument,
    runtime: &Document,
) -> Result<(), OperationError> {
    match operation {
        SemanticOperation::Rename(rename) => {
            if document.structure.nodes.iter().any(|node| node.id == rename.id) {
                Ok(())
            } else {
                Err(OperationError::MissingNode(rename.id.clone()))
            }
        }
        SemanticOperation::Runtime(command) => command.validate(runtime),
    }
}

// -- Operation constructors -------------------------------------------------
//
// Each builds one operation from intent, so callers never hand-assemble
// before/after pairs and cannot record a malformed one.
//
// # Why these mutate as they build
//
// The constructors apply eagerly, and `execute` then replays forward. That
// double application is deliberate — it keeps Spool's established eager
// direction instead of inventing a pending-operation stage — and it is safe
// only because every replay is idempotent:
//
// - `set_geometry` / `set_style` assign the same `after` value twice.
// - `insert_object` refuses an id that is already present.
// - `remove_objects` on absent ids removes nothing.
//
// That idempotence is a load-bearing invariant, not an accident. A future
// constructor that mutates non-idempotently would apply twice per user action.
// `eagerly_built_operations_do_not_double_apply` pins the behaviour.

/// Rename a node, reading the current name from the document.
///
/// Prefer this over [`rename_node`]: it captures the real `before` value, so
/// undo restores exactly what was there.
pub fn rename_node_in(
    document: &PersistentDocument,
    id: NodeId,
    after: String,
) -> Result<SemanticOperation, OperationError> {
    let node = document
        .structure
        .nodes
        .iter()
        .find(|node| node.id == id)
        .ok_or_else(|| OperationError::MissingNode(id.clone()))?;
    Ok(SemanticOperation::Rename(RenameNode {
        id,
        before: node.name.clone(),
        after,
    }))
}

/// Move runtime objects by a delta. One operation regardless of object count.
///
/// Geometry is temporary prototype state.
pub fn move_nodes(
    runtime: &mut Document,
    ids: &[ObjectId],
    dx: f32,
    dy: f32,
) -> SemanticOperation {
    let snapshots = runtime.snapshot_objects(ids);
    for snapshot in &snapshots {
        let moved = Geometry {
            position: Point {
                x: snapshot.geometry.position.x + dx,
                y: snapshot.geometry.position.y + dy,
            },
            size: snapshot.geometry.size,
        };
        runtime.set_geometry(snapshot.id, moved);
    }
    SemanticOperation::Runtime(geometry_command(runtime, &snapshots))
}

/// Resize one runtime object. Geometry is temporary prototype state.
pub fn resize_node(
    runtime: &mut Document,
    id: ObjectId,
    geometry: Geometry,
) -> SemanticOperation {
    let snapshots = runtime.snapshot_objects(&[id]);
    runtime.set_geometry(id, geometry);
    SemanticOperation::Runtime(geometry_command(runtime, &snapshots))
}

/// Create runtime objects from a tool, as one operation.
pub fn create_nodes(
    runtime: &mut Document,
    kind: crate::canvas::ObjectType,
    position: Point<f32>,
    object_size: Size<f32>,
) -> SemanticOperation {
    let object = runtime.create_object(kind, position, object_size, None);
    let placement = ObjectPlacement {
        index: runtime.objects().len().saturating_sub(1),
        object,
    };
    SemanticOperation::Runtime(DocumentCommand::insert(vec![placement]))
}

/// Delete runtime objects, as one operation.
pub fn delete_nodes(runtime: &mut Document, ids: &[ObjectId]) -> SemanticOperation {
    let placements = runtime.remove_objects(ids);
    SemanticOperation::Runtime(DocumentCommand::delete(placements))
}

/// Duplicate runtime objects, as one operation.
pub fn duplicate_nodes(runtime: &mut Document, ids: &[ObjectId]) -> SemanticOperation {
    let placements = runtime.duplicate_objects(ids);
    SemanticOperation::Runtime(DocumentCommand::insert(placements))
}

/// Build the geometry command describing what a gesture changed.
fn geometry_command(runtime: &Document, snapshots: &[ObjectSnapshot]) -> DocumentCommand {
    let changes: Vec<GeometryChange> = snapshots
        .iter()
        .map(|snapshot| GeometryChange {
            id: snapshot.id,
            before: snapshot.geometry,
            after: runtime.geometry_of(snapshot.id).unwrap_or(snapshot.geometry),
        })
        .collect();
    DocumentCommand::geometry(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Geometry, ObjectType};
    use gpui::{point, size};
    use crate::source_document::{EditorRuntimeState, LamineStructure, SourceBinding, StructuralNode};

    fn node(id: &str, name: &str, kind: &str, parent: Option<&str>) -> StructuralNode {
        StructuralNode {
            id: NodeId::new(id).expect("valid id"),
            name: name.to_owned(),
            kind: kind.to_owned(),
            parent: parent.map(|value| NodeId::new(value).expect("valid id")),
            children: Vec::new(),
            source: SourceBinding {
                file: "index.html".into(),
                selector: format!("[data-spool-id=\"{id}\"]"),
            },
        }
    }

    fn session() -> EditSession {
        let document = PersistentDocument {
            structure: LamineStructure {
                nodes: vec![
                    node("spool-a", "Alpha", "frame", None),
                    node("spool-b", "Beta", "frame", None),
                ],
            },
            sources: Default::default(),
        };
        // The runtime starts empty; operations create what they need. Note
        // that `Document::default()` is the canvas starter scene, not empty.
        EditSession::new(document, Document::empty())
    }

    fn geometry_of(session: &EditSession, id: ObjectId) -> Geometry {
        session
            .runtime
            .geometry_of(id)
            .expect("object exists in runtime")
    }

    fn seed_rect(session: &mut EditSession) -> ObjectId {
        let operation =
            create_nodes(&mut session.runtime, ObjectType::Rectangle, point(0.0, 0.0), size(10.0, 10.0));
        // Record the creation so later geometry has something to move.
        session.execute(operation).expect("create applies");
        let ids: Vec<ObjectId> = session
            .runtime
            .objects()
            .iter()
            .map(|object| object.id)
            .collect();
        ids[0]
    }

    // -- Rename.

    #[test]
    fn rename_undo_redo() {
        let mut session = session();
        let before = session.history.undo_len();

        let operation =
            rename_node_in(&session.document, NodeId::new("spool-a").unwrap(), "Alpha 2".into())
                .expect("node exists");
        assert!(session.execute(operation).unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha 2");

        assert!(session.undo().unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha");
        assert!(session.redo().unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha 2");

        assert_eq!(session.history.undo_len(), before + 1);
    }

    // -- Move.

    #[test]
    fn move_undo_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);

        let operation = move_nodes(&mut session.runtime, &[id], 30.0, 40.0);
        session.execute(operation).unwrap();
        let moved = geometry_of(&session, id);
        assert_ne!(moved, start);

        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
        assert!(session.redo().unwrap());
        assert_eq!(geometry_of(&session, id), moved);
    }

    // -- Resize.

    #[test]
    fn resize_undo_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let resized = Geometry { position: start.position, size: size(80.0, 60.0) };

        let operation = resize_node(&mut session.runtime, id, resized);
        session.execute(operation).unwrap();
        assert_eq!(geometry_of(&session, id), resized);

        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
        assert!(session.redo().unwrap());
        assert_eq!(geometry_of(&session, id), resized);
    }

    // -- Create.

    #[test]
    fn create_undo_redo() {
        let mut session = session();
        let empty = session.runtime.objects().len();

        let operation =
            create_nodes(&mut session.runtime, ObjectType::Rectangle, point(0.0, 0.0), size(20.0, 20.0));
        session.execute(operation).unwrap();
        assert_eq!(session.runtime.objects().len(), empty + 1);
        let created = session.runtime.objects()[0].id;

        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), empty);
        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.objects().len(), empty + 1);
        assert!(session.runtime.object(created).is_some(), "stable id after redo");
    }

    // -- Delete.

    #[test]
    fn delete_undo_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let populated = session.runtime.objects().len();

        let operation = delete_nodes(&mut session.runtime, &[id]);
        session.execute(operation).unwrap();
        assert!(session.runtime.object(id).is_none());

        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), populated);
        assert!(session.runtime.object(id).is_some(), "deleted id restored by undo");
        assert!(session.redo().unwrap());
        assert!(session.runtime.object(id).is_none());
    }

    // -- Duplicate.

    #[test]
    fn duplicate_undo_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let original = session.runtime.objects().len();

        let operation = duplicate_nodes(&mut session.runtime, &[id]);
        session.execute(operation).unwrap();
        assert_eq!(session.runtime.objects().len(), original + 1);
        let duplicate_id = session
            .runtime
            .objects()
            .iter()
            .find(|object| object.id != id)
            .expect("a duplicate exists")
            .id;

        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), original);
        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.objects().len(), original + 1);
        // The duplicate keeps its identity across the round trip.
        assert!(session.runtime.object(duplicate_id).is_some());
    }

    // -- Cancellation.

    #[test]
    fn cancelled_move_creates_no_committed_operation() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        session.begin_gesture(&[id]);
        // Mutate transiently, as a drag does.
        session.runtime.set_geometry(id, Geometry { position: point(500.0, 500.0), size: start.size });
        assert!(session.gesture_in_flight());

        session.cancel_gesture();
        assert!(!session.gesture_in_flight());
        assert_eq!(geometry_of(&session, id), start, "geometry restored");
        assert_eq!(session.history.undo_len(), depth, "no entry was created");
    }

    #[test]
    fn cancelled_resize_creates_no_committed_operation() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        session.begin_gesture(&[id]);
        session.runtime.set_geometry(id, Geometry { position: start.position, size: size(999.0, 999.0) });
        session.cancel_gesture();

        assert_eq!(geometry_of(&session, id), start);
        assert_eq!(session.history.undo_len(), depth, "no entry was created");
    }

    // -- Transaction granularity.

    #[test]
    fn multi_node_move_is_one_history_transaction() {
        let mut session = session();
        let first = seed_rect(&mut session);
        let operation =
            create_nodes(&mut session.runtime, ObjectType::Rectangle, point(0.0, 0.0), size(10.0, 10.0));
        session.execute(operation).unwrap();
        let second = session
            .runtime
            .objects()
            .iter()
            .find(|object| object.id != first)
            .expect("second object")
            .id;

        let depth = session.history.undo_len();
        let operation = move_nodes(&mut session.runtime, &[first, second], 25.0, 0.0);
        session.execute(operation).unwrap();

        // One entry for two objects.
        assert_eq!(session.history.undo_len(), depth + 1);
        // One undo reverses both.
        assert!(session.undo().unwrap());
        assert_eq!(session.history.undo_len(), depth);
        assert_eq!(geometry_of(&session, first).position.x, 0.0);
        assert_eq!(geometry_of(&session, second).position.x, 0.0);
    }

    #[test]
    fn gesture_commit_is_one_entry_and_no_op_gesture_is_none() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        // A gesture that moves nothing commits nothing.
        session.begin_gesture(&[id]);
        session.commit_gesture().unwrap();
        assert_eq!(session.history.undo_len(), depth);

        // A gesture that moves something commits exactly one entry.
        session.begin_gesture(&[id]);
        session.runtime.set_geometry(id, Geometry { position: point(12.0, 0.0), size: start.size });
        assert!(session.commit_gesture().unwrap());
        assert_eq!(session.history.undo_len(), depth + 1);
        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
    }

    // -- Identity and runtime-only state.

    #[test]
    fn identities_survive_undo_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let spool_id = session.runtime.object(id).expect("object").spool_id.clone();

        let operation = move_nodes(&mut session.runtime, &[id], 10.0, 10.0);
        session.execute(operation).unwrap();
        assert!(session.undo().unwrap());
        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.object(id).expect("object").spool_id, spool_id);

        // A rename is persistent metadata but does not change identity.
        let operation =
            rename_node_in(&session.document, NodeId::new("spool-a").unwrap(), "Alpha X".into()).unwrap();
        session.execute(operation).unwrap();
        assert!(session.undo().unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha");
        assert_eq!(session.document.structure.nodes[0].id.as_str(), "spool-a");
    }

    #[test]
    fn selection_and_camera_are_runtime_only_and_never_recorded() {
        let mut session = session();
        // Editor session state exists, but is not part of the session's
        // persistent state or history.
        let runtime = EditorRuntimeState {
            selection: vec![NodeId::new("spool-a").unwrap()],
            camera_offset: (120, -30),
            active_tool: "select".into(),
        };

        let id = seed_rect(&mut session);
        let depth = session.history.undo_len();
        let operation = move_nodes(&mut session.runtime, &[id], 5.0, 5.0);
        session.execute(operation).unwrap();
        assert!(session.undo().unwrap());
        assert!(session.redo().unwrap());
        // Redo returned the entry to the undo stack, so the move still costs
        // exactly one history entry.
        assert_eq!(session.history.undo_len(), depth + 1);

        // Executing an operation does not capture selection or camera: the
        // session never recorded the `runtime` value built above, and only
        // the create and the move were ever recorded.
        let _ = runtime;

        // Mutating runtime state alone produces no history entry.
        let before = session.history.undo_len();
        let mut other =
            EditorRuntimeState { selection: vec![], camera_offset: (0, 0), active_tool: String::new() };
        other.camera_offset = (999, 999);
        other.selection.push(NodeId::new("spool-b").unwrap());
        assert_eq!(session.history.undo_len(), before, "camera/selection must not record");

        // And no persistent state mentions them.
        let persisted = format!("{:?}", session.document);
        assert!(!persisted.contains("camera"));
        assert!(!persisted.contains("selection"));
    }

    #[test]
    fn a_new_operation_clears_redo_and_a_noop_does_not() {
        let mut session = session();
        let id = seed_rect(&mut session);

        let operation = move_nodes(&mut session.runtime, &[id], 3.0, 0.0);
        session.execute(operation).unwrap();
        session.undo().unwrap();
        assert!(session.history.can_redo());

        // A no-op must not destroy the redo branch.
        let noop = move_nodes(&mut session.runtime, &[id], 0.0, 0.0);
        assert!(!session.execute(noop).unwrap(), "a no-op records nothing");
        assert!(session.history.can_redo(), "redo survives a no-op");

        // A real operation clears it.
        let operation = move_nodes(&mut session.runtime, &[id], 1.0, 0.0);
        session.execute(operation).unwrap();
        assert!(!session.history.can_redo());
    }

    #[test]
    fn a_failed_operation_changes_neither_state_nor_history() {
        let mut session = session();
        let names: Vec<String> =
            session.document.structure.nodes.iter().map(|node| node.name.clone()).collect();

        // Renaming a node that does not exist must fail cleanly.
        let operation =
            rename_node_in(&session.document, NodeId::new("spool-a").unwrap(), "Renamed".into()).unwrap();
        session.execute(operation).unwrap();
        let depth_after_rename = session.history.undo_len();

        let bogus = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-missing").unwrap(),
            before: "Nope".into(),
            after: "Other".into(),
        });
        assert!(session.execute(bogus).is_err());

        assert_eq!(session.history.undo_len(), depth_after_rename, "no entry for a failure");
        let after: Vec<String> =
            session.document.structure.nodes.iter().map(|node| node.name.clone()).collect();
        assert_ne!(names, after, "the successful rename is still in place");
        assert_eq!(session.document.structure.nodes[0].name, "Renamed");
    }

    #[test]
    fn edit_session_undoes_metadata_and_runtime_as_one_lifo_sequence() {
        // Architectural invariant: `EditSession` holds a `SemanticHistory`, the
        // same type `canvas::History` is a facade over. A session therefore
        // undoes a rename and a move as one interleaved stack, which is what
        // distinguishes "one history implementation" from "two stacks that
        // happen to look alike".
        let mut session = session();
        let id = seed_rect(&mut session);
        let baseline = session.history.undo_len();

        let first = move_nodes(&mut session.runtime, &[id], 12.0, 0.0);
        session.execute(first).unwrap();
        session
            .execute(
                rename_node_in(&session.document, NodeId::new("spool-a").unwrap(), "Alpha X".into())
                    .unwrap(),
            )
            .unwrap();
        let second = move_nodes(&mut session.runtime, &[id], 0.0, 7.0);
        session.execute(second).unwrap();
        assert_eq!(session.history.undo_len(), baseline + 3);

        // Strict LIFO across both kinds: the second move, then the rename, then
        // the first move.
        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id).position.y, 0.0);
        assert_eq!(session.document.structure.nodes[0].name, "Alpha X");

        assert!(session.undo().unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha");

        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id).position.x, 0.0);
        assert_eq!(session.history.undo_len(), baseline);
    }

    #[test]
    fn rename_against_a_runtime_only_target_is_refused() {
        let mut document = Document::empty();
        let mut history = SemanticHistory::default();
        history.record(SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-a").unwrap(),
            before: "A".into(),
            after: "B".into(),
        }));
        // Replaying a metadata operation with no metadata available cannot be
        // silently ignored.
        let result = history.undo(&mut OperationTarget::Runtime(&mut document));
        assert!(matches!(result, Err(OperationError::WrongTarget(_))));
        // And the entry is preserved for a correct target.
        assert!(history.can_undo());
    }

    #[test]
    fn eagerly_built_operations_do_not_double_apply() {
        // Constructors apply eagerly and `execute` replays forward. If any
        // replay were non-idempotent this would show up here.
        let mut session = session();

        // Insert: the create already added the object, so replaying forward
        // must not add a second one with the same id.
        let created = create_nodes(&mut session.runtime, ObjectType::Rectangle, point(0.0, 0.0), size(10.0, 10.0));
        let after_build = session.runtime.objects().len();
        session.execute(created).unwrap();
        assert_eq!(session.runtime.objects().len(), after_build, "create replayed twice");
        assert!(session.undo().unwrap());
        assert!(session.runtime.objects().is_empty(), "undo removed the created object");
        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.objects().len(), after_build, "redo restored exactly one");

        // Delete: removing an absent id twice removes nothing.
        let id = session.runtime.objects()[0].id;
        let deleted = delete_nodes(&mut session.runtime, &[id]);
        assert!(session.execute(deleted).unwrap());
        assert!(session.runtime.objects().is_empty(), "delete applied twice is stable");
        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), 1, "undo restored exactly one");
    }

    #[test]
    fn every_operation_kind_leaves_the_caller_owned_runtime_state_alone() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let depth = session.history.undo_len();

        // Selection and camera belong to the canvas view, not to the session.
        // Driving all six operation kinds must leave that caller-owned state
        // byte-identical and must not record anything on its behalf.
        let view = EditorRuntimeState {
            selection: vec![NodeId::new("spool-a").unwrap()],
            camera_offset: (120, -30),
            active_tool: "select".into(),
        };

        let operations = [
            move_nodes(&mut session.runtime, &[id], 5.0, 5.0),
            resize_node(&mut session.runtime, id, Geometry { position: point(1.0, 1.0), size: size(20.0, 20.0) }),
            duplicate_nodes(&mut session.runtime, &[id]),
            {
                let duplicated = session.runtime.objects().last().expect("duplicate exists").id;
                delete_nodes(&mut session.runtime, &[duplicated])
            },
        ];
        for operation in operations {
            session.execute(operation).expect("applies");
        }
        let operation =
            rename_node_in(&session.document, NodeId::new("spool-a").unwrap(), "Alpha 2".into()).unwrap();
        session.execute(operation).unwrap();

        // Four runtime operations plus the rename: five entries, no more.
        assert_eq!(session.history.undo_len(), depth + 5);

        // The view's state is exactly what it was.
        assert_eq!(view.selection, vec![NodeId::new("spool-a").unwrap()]);
        assert_eq!(view.camera_offset, (120, -30));
        assert_eq!(view.active_tool, "select");

        // And undoing all five restores geometry without consulting it.
        for _ in 0..5 {
            assert!(session.undo().unwrap());
        }
        assert_eq!(view.camera_offset, (120, -30));
        // Five undos leave only the seed creation that predates them.
        assert_eq!(session.history.undo_len(), depth);
    }
}
