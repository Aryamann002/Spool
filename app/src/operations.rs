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
//! - [`SemanticOperation::Compound`] — several of the above as one atomic
//!   entry. See "Compound operations" below.
//!
//! Every leaf variant is a reversible value type holding a `before`/`after`
//! pair. Undo replays `before`; redo replays `after`. Nothing is stored as a
//! byte offset or a captured diff, so history records *intentions* rather than
//! net state changes.
//!
//! # Compound operations
//!
//! [`SemanticOperation::Runtime`] wraps exactly one
//! `canvas::DocumentCommand`, which in turn wraps exactly one private
//! `CommandOperation`. One history entry therefore cannot hold an insert *and*
//! a geometry change — the shape a modifier-drag duplicate needs, where copies
//! are created at press time and then follow the pointer.
//!
//! [`SemanticOperation::Compound`] closes that gap without becoming a general
//! transaction framework. It is deliberately narrow:
//!
//! - It holds a list of existing operations. It introduces no new mutation
//!   primitive, no rollback log, and no nesting semantics of its own.
//! - Undo replays its members in **reverse** order; redo replays them in
//!   order. That ordering is the whole reason a compound is not just a list.
//! - Every member is validated before **any** member is applied, so a compound
//!   is all-or-nothing by construction rather than by compensation.
//! - Nested compounds are flattened on construction, so the stack never grows
//!   deeper than the caller's nesting.
//!
//! # Atomicity
//!
//! Research §10 observed that tldraw has no rollback anywhere: a failed
//! mutation is handled by an explicit compensating operation placed by hand.
//! That was tolerable when every entry was a single idempotent command. It is
//! not tolerable for a compound, where a member failing halfway leaves the
//! entry half-applied.
//!
//! Spool does not add a rollback log. It makes the *validate* step exhaustive
//! and runs it over the whole operation first, so [`EditSession::execute`] can
//! reach apply-time only for an operation that cannot fail. The remaining
//! apply-time failures are replay failures on stale state, and those return
//! without touching history.
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
//! - No AI, plugin, import, or automation caller. The boundary is reachable by
//!   them because [`EditSession::execute`] is the only mutation entry point,
//!   and [`Origin`] exists so an entry can say which of them produced it. None
//!   of those callers are implemented here, and [`Origin`] is the only thing
//!   that anticipates them — there is no agent runtime, no plugin registry, and
//!   no MCP surface anywhere in this module.
//!
//! # Who is on the live path
//!
//! `canvas::CanvasView` owns an [`EditSession`] and funnels every mutation
//! through [`EditSession::execute`]. So the live set is [`SemanticOperation`],
//! [`SemanticHistory`], [`Origin`], [`EditSession`], [`OperationTarget::Full`],
//! and [`apply`].
//!
//! What `cargo check` still reports as unused is the *wider* surface that no
//! caller has yet: [`OperationTarget::Runtime`] and
//! [`OperationTarget::Document`] (a bundle-level caller holding only a
//! [`PersistentDocument`]), the gesture helpers
//! ([`EditSession::begin_gesture`] and friends), and the free-standing
//! constructors ([`create_nodes`], [`move_nodes`], and siblings) that build an
//! operation without an [`EditSession`] in hand.
//!
//! Those warnings are the remaining-work list. Do not silence them with a
//! blanket `#![allow(dead_code)]`, which would hide exactly the gap they
//! describe.
//!
//! # MUTATION HARNESS
//!
//! `app/mutate_ops.sh` breaks one rule at a time in this file — the reverse
//! order of a compound undo, the validate checks, the point at which an
//! operation is recorded — and requires the test suite to notice. A rule that
//! can be broken without a test failing is reported as `SURVIVED`, because an
//! untested invariant is the finding. The script refuses to run unless it sees
//! this marker, so it cannot be pointed at an unrelated file.

use gpui::{Point, Size};

use crate::canvas::{
    Document, DocumentCommand, Geometry, GeometryChange, ObjectId, ObjectPlacement, ObjectSnapshot,
    ReplayDirection,
};
use crate::source_document::{NodeId, PersistentDocument, RenameNode};

/// Who asked for an operation.
///
/// Research §17 recorded the structural gap this closes: with a bare
/// `Vec<SemanticOperation>` stack there is nowhere to attach provenance, so an
/// entry cannot say whether a human, an agent, an importer, or a plugin
/// produced it — and every entry is indistinguishable in the undo spine.
///
/// Only [`Origin::User`] has a caller today. The remaining variants exist
/// because the architecture this module serves names those callers, and a
/// stack that cannot carry attribution cannot be extended to them without a
/// second, parallel mechanism. Adding a variant later is additive; this
/// records no agent, registers no plugin, and runs no automation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// A human acting through the editor: a gesture, a keystroke, a menu item.
    #[default]
    User,
    /// A future agent caller. Not implemented.
    Agent,
    /// A future plugin or extension caller. Not implemented.
    Plugin,
    /// A future importer. Not implemented.
    Import,
    /// A future automation or script caller. Not implemented.
    Automation,
}

/// One committed entry: the operation plus who asked for it.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub operation: SemanticOperation,
    pub origin: Origin,
}

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
    /// Several operations applied and reverted as one unit.
    ///
    /// Undo replays members in reverse order; redo replays them in order.
    /// A compound is validated in full before any member is applied.
    Compound(Vec<SemanticOperation>),
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
            // A compound is a no-op only if every member is. An empty
            // compound is vacuously a no-op, which is the honest answer: it
            // changes nothing and must not open a history entry.
            Self::Compound(operations) => operations.iter().all(SemanticOperation::is_noop),
        }
    }

    /// Build a compound from intent, flattening any nested compounds.
    ///
    /// Flattening keeps the stack shallow and makes reverse-order undo a
    /// property of one flat list rather than of every nesting depth. An empty
    /// input yields a compound with no members, which `is_noop` reports as a
    /// no-op, so it records nothing.
    pub fn compound(operations: Vec<SemanticOperation>) -> Self {
        let mut flattened = Vec::with_capacity(operations.len());
        for operation in operations {
            match operation {
                Self::Compound(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }
        Self::Compound(flattened)
    }

    /// The members of a compound, or this operation alone.
    ///
    /// Lets the replay paths iterate one shape without caring whether the
    /// entry happened to be compound.
    fn members(&self) -> Vec<&SemanticOperation> {
        match self {
            Self::Compound(operations) => operations.iter().collect(),
            other => vec![other],
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
    /// The operation would give a node a name another node already holds.
    ///
    /// `source_document` enforces node-name uniqueness, so this is a real
    /// rejection and not a style preference. It used to surface as
    /// [`OperationError::MissingNode`], which made a legitimate refusal
    /// indistinguishable from a broken identity and let it reach the
    /// apply-time panic in [`EditSession::execute`].
    DuplicateName(String),
    /// The operation's recorded `before` value no longer matches the state,
    /// so replaying it would silently overwrite a change made since.
    ///
    /// This is the replay-failure case: the node exists and the name is free,
    /// but the document has moved on.
    StaleEdit(NodeId),
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNode(id) => write!(f, "no node with identity {}", id.as_str()),
            Self::MissingObject(id) => write!(f, "no runtime object {id:?}"),
            Self::WrongTarget(what) => write!(f, "{what} cannot be applied to this target"),
            Self::DuplicateName(name) => write!(f, "another node is already named {name:?}"),
            Self::StaleEdit(id) => {
                write!(
                    f,
                    "the recorded value for {} no longer matches the document",
                    id.as_str()
                )
            }
        }
    }
}

impl std::error::Error for OperationError {}

impl OperationError {
    /// Translate a `source_document` failure for a specific rename.
    ///
    /// `source_document` reports a stale rename as a generic
    /// `InvalidOperation`, and a missing node by its raw id string. Mapping
    /// through the operation's own identity keeps the two apart, which is what
    /// let a duplicate-name refusal masquerade as a missing node.
    fn from_rename(rename: &RenameNode, error: crate::source_document::ModelError) -> Self {
        use crate::source_document::ModelError;
        match error {
            ModelError::DuplicateName(name) => Self::DuplicateName(name),
            ModelError::InvalidOperation(_) => Self::StaleEdit(rename.id.clone()),
            // Anything else from this call site is a missing node: `apply` is
            // the only operation that touches names, and it looks the node up
            // by this id.
            _ => Self::MissingNode(rename.id.clone()),
        }
    }
}

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
/// Entries are [`HistoryEntry`] rather than bare operations so that provenance
/// travels with the operation. See [`Origin`] for why that matters to callers
/// that do not exist yet.
#[derive(Clone, Debug, Default)]
pub struct SemanticHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl SemanticHistory {
    /// Commit an operation as a human action: drop it if it changes nothing,
    /// otherwise push it and clear redo.
    ///
    /// Returns whether anything was recorded.
    pub fn record(&mut self, operation: SemanticOperation) -> bool {
        self.record_from(Origin::User, operation)
    }

    /// Commit an operation attributed to a named caller.
    ///
    /// The no-op check and the redo clear are identical to [`SemanticHistory::record`];
    /// only the attribution differs.
    pub fn record_from(&mut self, origin: Origin, operation: SemanticOperation) -> bool {
        if operation.is_noop() {
            return false;
        }
        self.undo.push(HistoryEntry { operation, origin });
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
        self.undo.last().map(|entry| &entry.operation)
    }

    /// The most recently committed entry, operation and origin together.
    ///
    /// This is what a caller needs in order to label an undo step, and what a
    /// caller cannot reconstruct from [`SemanticHistory::peek_undo`] alone.
    pub fn peek_undo_entry(&self) -> Option<&HistoryEntry> {
        self.undo.last()
    }

    /// Who asked for the operation the next undo would reverse.
    pub fn peek_undo_origin(&self) -> Option<Origin> {
        self.undo.last().map(|entry| entry.origin)
    }

    /// Who asked for the operation the next redo would reapply.
    ///
    /// The counterpart to [`SemanticHistory::peek_undo_origin`]. Attribution
    /// has to be readable on both sides to be trustworthy: after an undo the
    /// entry is on the redo stack, so a caller that could only read the undo
    /// side would report the wrong author.
    pub fn peek_redo_origin(&self) -> Option<Origin> {
        self.redo.last().map(|entry| entry.origin)
    }

    /// Undo the most recent committed operation.
    pub fn undo(&mut self, target: &mut OperationTarget<'_>) -> Result<bool, OperationError> {
        let Some(entry) = self.undo.pop() else {
            return Ok(false);
        };
        match apply(&entry.operation, target, ReplayDirection::Undo) {
            Ok(()) => {
                self.redo.push(entry);
                Ok(true)
            }
            Err(error) => {
                // Put it back so a failed undo does not lose the entry.
                self.undo.push(entry);
                Err(error)
            }
        }
    }

    /// Redo the most recently undone operation.
    pub fn redo(&mut self, target: &mut OperationTarget<'_>) -> Result<bool, OperationError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(false);
        };
        match apply(&entry.operation, target, ReplayDirection::Redo) {
            Ok(()) => {
                self.undo.push(entry);
                Ok(true)
            }
            Err(error) => {
                self.redo.push(entry);
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
    // A compound applies in forward order on redo and reverse order on undo,
    // because the members were applied in forward order. Flattened on
    // construction, so this is one ordering rule rather than one per nesting
    // depth.
    let members = operation.members();
    let ordered = match direction {
        ReplayDirection::Redo => members.iter().copied().collect::<Vec<_>>(),
        ReplayDirection::Undo => members.iter().rev().copied().collect::<Vec<_>>(),
    };
    for member in ordered {
        apply_one(member, target, direction)?;
    }
    Ok(())
}

/// Apply one non-compound operation in the given direction.
fn apply_one(
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
                    .map_err(|error| OperationError::from_rename(rename, error))
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
        // `members` guarantees a compound never reaches here; `compound`
        // flattens on construction.
        SemanticOperation::Compound(_) => Err(OperationError::WrongTarget("a nested compound")),
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
    /// The in-flight gesture, if any.
    in_flight: Option<InFlight>,
}

/// What a gesture must remember so that cancelling it is indistinguishable
/// from never having started it.
///
/// Geometry snapshots alone are not enough. A gesture that creates objects —
/// a modifier-drag duplicate — has inserted rows into the runtime that a
/// geometry restore cannot take back, because a snapshot records a position,
/// not an existence. So the gesture also accumulates the operations it applied
/// and replays them in reverse on cancel.
struct InFlight {
    /// Geometry captured when the gesture began.
    geometry: Vec<ObjectSnapshot>,
    /// Operations applied while the gesture was in flight, in application
    /// order. Replayed in reverse by [`EditSession::cancel_gesture`].
    applied: Vec<SemanticOperation>,
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
    ///
    /// A failure changes neither state nor history: validation is exhaustive
    /// and runs first, and the operation is only recorded once it has applied.
    pub fn execute(&mut self, operation: SemanticOperation) -> Result<bool, OperationError> {
        self.execute_from(Origin::User, operation)
    }

    /// [`EditSession::execute`], attributed to a named caller.
    pub fn execute_from(
        &mut self,
        origin: Origin,
        operation: SemanticOperation,
    ) -> Result<bool, OperationError> {
        // Validate before mutating so a failure changes neither state nor
        // history. This walks the whole operation, compounds included, so a
        // compound whose fourth member would be rejected never applies its
        // first three.
        validate(&operation, &self.document, &self.runtime)?;
        if operation.is_noop() {
            return Ok(false);
        }
        let Self {
            document, runtime, ..
        } = self;
        // Apply before recording. The constructors already applied eagerly, and
        // every replay is idempotent, so this is normally a no-op — but doing
        // it first means an apply-time failure cannot leave an entry behind or
        // clear the redo branch.
        apply(
            &operation,
            &mut OperationTarget::Full { document, runtime },
            ReplayDirection::Redo,
        )?;
        self.history.record_from(origin, operation);
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
    /// runtime document while a gesture is in flight. A second call replaces
    /// any gesture already in flight, which mirrors the canvas dropping a
    /// superseded interaction.
    pub fn begin_gesture(&mut self, ids: &[ObjectId]) {
        let geometry = self.runtime.snapshot_objects(ids);
        self.in_flight = Some(InFlight {
            geometry,
            applied: Vec::new(),
        });
    }

    /// Run one operation as part of the in-flight gesture.
    ///
    /// This is the entry point for a gesture that mutates structurally rather
    /// than only moving things — a modifier-drag duplicate inserts objects.
    /// The operation is validated and applied, and remembered, but **not**
    /// recorded: nothing a gesture does becomes undoable until the gesture
    /// commits. That is what keeps "one gesture is one history entry" true
    /// when the gesture is not a single geometry change.
    ///
    /// On commit these operations become part of the one entry, so the
    /// structure and the drag it belongs to undo together. On cancel they are
    /// reversed and nothing is left behind. Outside a gesture this is exactly
    /// [`EditSession::execute`].
    pub fn gesture_execute(
        &mut self,
        operation: SemanticOperation,
    ) -> Result<bool, OperationError> {
        if !self.gesture_in_flight() {
            return self.execute(operation);
        }
        validate(&operation, &self.document, &self.runtime)?;
        if operation.is_noop() {
            return Ok(false);
        }
        let Self {
            document,
            runtime,
            in_flight,
            ..
        } = self;
        apply(
            &operation,
            &mut OperationTarget::Full { document, runtime },
            ReplayDirection::Redo,
        )?;
        if let Some(in_flight) = in_flight {
            in_flight.applied.push(operation);
        }
        Ok(true)
    }

    /// Commit the in-flight gesture as exactly one history entry.
    ///
    /// The entry is the gesture's structural operations composed with the
    /// geometry it produced, so a gesture that duplicated and dragged commits
    /// as one undoable action rather than two.
    ///
    /// Returns false, recording nothing, when there is no gesture or when it
    /// changed nothing at all.
    pub fn commit_gesture(&mut self) -> Result<bool, OperationError> {
        let Some(in_flight) = self.in_flight.take() else {
            return Ok(false);
        };
        let mut members = in_flight.applied;
        let command = geometry_command(&self.runtime, &in_flight.geometry);
        if !command.is_noop() {
            members.push(SemanticOperation::Runtime(command));
        }
        if members.is_empty() {
            return Ok(false);
        }
        self.execute(SemanticOperation::compound(members))
    }

    /// Abandon the in-flight gesture.
    ///
    /// Geometry returns to its pre-gesture value, anything the gesture
    /// inserted or otherwise changed is taken back, and **no history entry is
    /// created**. A cancelled gesture is indistinguishable from one that never
    /// started, which is the property the project's own research identifies as
    /// the opposite of tldraw's destructive bail.
    pub fn cancel_gesture(&mut self) {
        let Some(in_flight) = self.in_flight.take() else {
            return;
        };
        self.runtime.restore_snapshots(&in_flight.geometry);
        // Reverse order, matching how the operations were applied.
        for operation in in_flight.applied.iter().rev() {
            let Self {
                document, runtime, ..
            } = self;
            apply(
                operation,
                &mut OperationTarget::Full { document, runtime },
                ReplayDirection::Undo,
            )
            .expect("a gesture operation that applied once reverses cleanly");
        }
    }

    /// Whether a gesture is currently in flight.
    pub fn gesture_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
}

/// Check that an operation can apply, before any state changes.
///
/// This must mirror every rejection [`apply`] can produce, because
/// [`EditSession::execute`] relies on that: an operation that reaches
/// apply-time having passed validation is expected to succeed, and a refusal
/// that slips through here would otherwise be applied only to fail.
///
/// The rename branch is the reason this function is worth writing out rather
/// than delegating. `RenameNode::apply` rejects three distinct conditions, and
/// checking only node existence let a duplicate-name rename reach apply-time,
/// where it used to panic.
fn validate(
    operation: &SemanticOperation,
    document: &PersistentDocument,
    runtime: &Document,
) -> Result<(), OperationError> {
    match operation {
        SemanticOperation::Rename(rename) => {
            let Some(node) = document
                .structure
                .nodes
                .iter()
                .find(|node| node.id == rename.id)
            else {
                return Err(OperationError::MissingNode(rename.id.clone()));
            };
            if rename.before == rename.after {
                return Ok(());
            }
            if document
                .structure
                .nodes
                .iter()
                .any(|other| other.id != rename.id && other.name == rename.after)
            {
                return Err(OperationError::DuplicateName(rename.after.clone()));
            }
            if node.name != rename.before {
                return Err(OperationError::StaleEdit(rename.id.clone()));
            }
            Ok(())
        }
        SemanticOperation::Runtime(command) => command.validate(runtime),
        // Every member, before any member is applied. A compound is therefore
        // all-or-nothing by construction instead of by compensation, which is
        // what a multi-member entry needs once undo can no longer reverse a
        // half-applied state.
        SemanticOperation::Compound(operations) => {
            for member in operations {
                validate(member, document, runtime)?;
            }
            Ok(())
        }
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
///
/// Applies eagerly; see [`plan_move_nodes`] for the form that does not, which
/// is what a compound needs so that a refused compound leaves no partial state
/// behind.
pub fn move_nodes(runtime: &mut Document, ids: &[ObjectId], dx: f32, dy: f32) -> SemanticOperation {
    let operation = plan_move_nodes(runtime, ids, dx, dy);
    apply_planned_moves(runtime, &operation);
    operation
}

/// Describe a move by a delta without applying it.
///
/// The constructors that mutate as they build cannot be composed into an
/// all-or-nothing compound, because by the time the compound is validated each
/// member has already changed the runtime. This one only reads, so the whole
/// compound can be refused with nothing applied.
///
/// Prefer this when building a compound, and
/// [`EditSession::execute`] applies it.
pub fn plan_move_nodes(
    runtime: &Document,
    ids: &[ObjectId],
    dx: f32,
    dy: f32,
) -> SemanticOperation {
    let changes: Vec<GeometryChange> = runtime
        .snapshot_objects(ids)
        .iter()
        .filter_map(|snapshot| {
            let after = Geometry {
                position: Point {
                    x: snapshot.geometry.position.x + dx,
                    y: snapshot.geometry.position.y + dy,
                },
                size: snapshot.geometry.size,
            };
            (after != snapshot.geometry).then_some(GeometryChange {
                id: snapshot.id,
                before: snapshot.geometry,
                after,
            })
        })
        .collect();
    SemanticOperation::Runtime(DocumentCommand::geometry(changes))
}

/// Apply a geometry operation built by [`plan_move_nodes`].
fn apply_planned_moves(runtime: &mut Document, operation: &SemanticOperation) {
    if let SemanticOperation::Runtime(command) = operation {
        command.replay(runtime, ReplayDirection::Redo);
    }
}

/// Resize one runtime object. Geometry is temporary prototype state.
pub fn resize_node(runtime: &mut Document, id: ObjectId, geometry: Geometry) -> SemanticOperation {
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

/// The offset `Document::duplicate_objects` applies to every copy it mints.
///
/// Exposed so a caller positioning copies by its own rule can cancel the nudge
/// out rather than rediscover it.
pub const DUPLICATE_NUDGE: f32 = 16.0;

/// Create copies and place them by a delta, as **one** history entry.
///
/// This is the shape a modifier-drag duplicate needs, and the reason
/// [`SemanticOperation::Compound`] exists. The copies are inserted at the
/// position [`duplicate_nodes`] gives them — the original's position plus
/// [`DUPLICATE_NUDGE`] — and then translated by `delta`. Expressing that as two
/// entries would make one gesture cost two undos, and undoing the movement
/// first would leave the copies sitting where they were dropped before the
/// second undo removed them.
///
/// The copies start at the nudged position, so the *net* placement of each copy
/// is `original + DUPLICATE_NUDGE + delta`. A caller that wants a copy to land
/// exactly at the pointer should pass `delta - DUPLICATE_NUDGE` on each axis.
/// That is stated rather than hidden because the nudge belongs to
/// `Document::duplicate_objects`, not to this operation.
///
/// The originals are untouched: this moves the copies, never the sources.
/// Duplicating `ids` that do not exist yields an empty compound, which
/// [`SemanticOperation::is_noop`] reports as a no-op, so it records nothing.
pub fn duplicate_and_move_nodes(
    runtime: &mut Document,
    ids: &[ObjectId],
    delta: Point<f32>,
) -> SemanticOperation {
    // The insert half has to mutate: a copy's identity and name can only be
    // minted by the allocator. The move half is planned from a shared read, so
    // it is described rather than applied.
    let placements = runtime.duplicate_objects(ids);
    let copy_ids: Vec<ObjectId> = placements
        .iter()
        .map(|placement| placement.object.id)
        .collect();
    let moved = plan_move_nodes(runtime, &copy_ids, delta.x, delta.y);
    apply_planned_moves(runtime, &moved);
    SemanticOperation::compound(vec![
        SemanticOperation::Runtime(DocumentCommand::insert(placements)),
        moved,
    ])
}

/// Build the geometry command describing what a gesture changed.
///
/// Objects that did not actually move are dropped. That is not only tidiness:
/// without the filter, a multi-object gesture in which one object was pinned
/// would carry a `before == after` entry for it, so the command's `is_noop`
/// would be decided by whether *any* object moved while the entry still
/// described all of them.
fn geometry_command(runtime: &Document, snapshots: &[ObjectSnapshot]) -> DocumentCommand {
    let changes: Vec<GeometryChange> = snapshots
        .iter()
        .filter_map(|snapshot| {
            let after = runtime.geometry_of(snapshot.id)?;
            (after != snapshot.geometry).then_some(GeometryChange {
                id: snapshot.id,
                before: snapshot.geometry,
                after,
            })
        })
        .collect();
    DocumentCommand::geometry(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Geometry, ObjectType};
    use crate::source_document::{
        EditorRuntimeState, LamineStructure, SourceBinding, StructuralNode,
    };
    use gpui::{point, size};

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

    /// A two-node persistent document, shared by the session helper and by the
    /// replay tests that need metadata without a canvas runtime.
    fn persistent_document() -> PersistentDocument {
        PersistentDocument {
            structure: LamineStructure {
                nodes: vec![
                    node("spool-a", "Alpha", "frame", None),
                    node("spool-b", "Beta", "frame", None),
                ],
            },
            sources: Default::default(),
        }
    }

    fn session() -> EditSession {
        // The runtime starts empty; operations create what they need. Note
        // that `Document::default()` is the canvas starter scene, not empty.
        EditSession::new(persistent_document(), Document::empty())
    }

    fn geometry_of(session: &EditSession, id: ObjectId) -> Geometry {
        session
            .runtime
            .geometry_of(id)
            .expect("object exists in runtime")
    }

    fn seed_rect(session: &mut EditSession) -> ObjectId {
        let operation = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
        );
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

        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Alpha 2".into(),
        )
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
        let resized = Geometry {
            position: start.position,
            size: size(80.0, 60.0),
        };

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

        let operation = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(20.0, 20.0),
        );
        session.execute(operation).unwrap();
        assert_eq!(session.runtime.objects().len(), empty + 1);
        let created = session.runtime.objects()[0].id;

        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), empty);
        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.objects().len(), empty + 1);
        assert!(
            session.runtime.object(created).is_some(),
            "stable id after redo"
        );
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
        assert!(
            session.runtime.object(id).is_some(),
            "deleted id restored by undo"
        );
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
        session.runtime.set_geometry(
            id,
            Geometry {
                position: point(500.0, 500.0),
                size: start.size,
            },
        );
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
        session.runtime.set_geometry(
            id,
            Geometry {
                position: start.position,
                size: size(999.0, 999.0),
            },
        );
        session.cancel_gesture();

        assert_eq!(geometry_of(&session, id), start);
        assert_eq!(session.history.undo_len(), depth, "no entry was created");
    }

    // -- Transaction granularity.

    #[test]
    fn multi_node_move_is_one_history_transaction() {
        let mut session = session();
        let first = seed_rect(&mut session);
        let operation = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
        );
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
        session.runtime.set_geometry(
            id,
            Geometry {
                position: point(12.0, 0.0),
                size: start.size,
            },
        );
        assert!(session.commit_gesture().unwrap());
        assert_eq!(session.history.undo_len(), depth + 1);
        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
    }

    // -- Refusals: an operation that cannot apply must not panic, record, or
    // touch the redo branch.

    #[test]
    fn a_rename_to_a_taken_name_is_refused_without_panicking() {
        // Regression: `validate` used to check only that the node existed, so
        // this reached the apply-time `.expect` and took the editor down. The
        // refusal has to happen before anything is recorded.
        let mut session = session();
        let depth = session.history.undo_len();
        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Beta".into(),
        )
        .unwrap();

        let error = session
            .execute(operation)
            .expect_err("a taken name must be refused");
        assert_eq!(error, OperationError::DuplicateName("Beta".into()));
        assert_eq!(
            session.history.undo_len(),
            depth,
            "a refusal records nothing"
        );
        assert_eq!(
            session.document.structure.nodes[0].name, "Alpha",
            "state unchanged"
        );
    }

    #[test]
    fn a_refused_operation_preserves_the_redo_branch() {
        // The documented invariant is that a failed operation changes neither
        // state nor history. The redo branch is history.
        let mut session = session();
        let id = seed_rect(&mut session);
        let operation = move_nodes(&mut session.runtime, &[id], 5.0, 0.0);
        session.execute(operation).unwrap();
        session.undo().unwrap();
        assert!(session.history.can_redo());

        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Beta".into(),
        )
        .unwrap();
        assert!(session.execute(operation).is_err());

        assert!(session.history.can_redo(), "a refusal must not clear redo");
    }

    #[test]
    fn a_rename_whose_before_value_is_stale_is_refused() {
        // The replay-failure case: the node exists and the name is free, but
        // the recorded `before` no longer matches, so replaying would silently
        // overwrite an intervening change.
        let mut session = session();
        session
            .execute(
                rename_node_in(
                    &session.document,
                    NodeId::new("spool-a").unwrap(),
                    "Renamed".into(),
                )
                .unwrap(),
            )
            .unwrap();

        // Hand-built, so its `before` is now wrong. The constructor cannot
        // produce this — only a stale replay or a second caller can.
        let stale = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-a").unwrap(),
            before: "Alpha".into(),
            after: "Third".into(),
        });
        let error = session
            .execute(stale)
            .expect_err("a stale rename must be refused");
        assert_eq!(
            error,
            OperationError::StaleEdit(NodeId::new("spool-a").unwrap())
        );
        assert_eq!(session.document.structure.nodes[0].name, "Renamed");
    }

    #[test]
    fn every_rename_refusal_is_distinguishable() {
        // The three rejections must not collapse into one variant. They did
        // once, which is how a duplicate name was reported as a missing node.
        let mut session = session();

        let missing = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-nope").unwrap(),
            before: "A".into(),
            after: "B".into(),
        });
        assert_eq!(
            session.execute(missing).unwrap_err(),
            OperationError::MissingNode(NodeId::new("spool-nope").unwrap())
        );

        let taken = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-a").unwrap(),
            before: "Alpha".into(),
            after: "Beta".into(),
        });
        assert_eq!(
            session.execute(taken).unwrap_err(),
            OperationError::DuplicateName("Beta".into())
        );

        // The name is free, so staleness is the only thing left to catch it.
        let stale = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-a").unwrap(),
            before: "Wrong".into(),
            after: "Fresh".into(),
        });
        assert_eq!(
            session.execute(stale).unwrap_err(),
            OperationError::StaleEdit(NodeId::new("spool-a").unwrap())
        );
    }

    // -- Compound operations.

    #[test]
    fn a_compound_is_one_history_entry() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let depth = session.history.undo_len();

        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(40.0, 25.0));
        assert!(session.execute(operation).unwrap());

        assert_eq!(
            session.history.undo_len(),
            depth + 1,
            "two members, one entry"
        );
        assert!(session.undo().unwrap());
        assert_eq!(session.history.undo_len(), depth);
        assert!(session.redo().unwrap());
        assert_eq!(session.history.undo_len(), depth + 1);
    }

    #[test]
    fn a_compound_undo_removes_the_copies_and_reverses_their_move_together() {
        // The property that makes a compound worth having. Two entries would
        // undo the movement first and leave the copies stranded where they were
        // dropped until a second undo.
        let mut session = session();
        let id = seed_rect(&mut session);
        let original_position = geometry_of(&session, id).position;

        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(40.0, 25.0));
        session.execute(operation).unwrap();

        let copy = session
            .runtime
            .objects()
            .iter()
            .find(|object| object.id != id)
            .expect("a copy exists")
            .id;
        assert_eq!(session.runtime.objects().len(), 2);
        assert_ne!(geometry_of(&session, copy).position, original_position);

        // One undo: the copy is gone. Not merely put back where it started.
        assert!(session.undo().unwrap());
        assert!(
            session.runtime.object(copy).is_none(),
            "the copy is removed"
        );
        assert_eq!(
            session.runtime.objects().len(),
            1,
            "only the original remains"
        );
        assert_eq!(
            geometry_of(&session, id).position,
            original_position,
            "the original never moved"
        );

        assert!(session.redo().unwrap());
        assert_eq!(session.runtime.objects().len(), 2);
        assert!(
            session.runtime.object(copy).is_some(),
            "the copy returns with its identity"
        );
    }

    #[test]
    fn a_compound_undo_reverses_members_in_reverse_order() {
        // Order is the whole reason a compound is not just a list, and it is
        // only observable when a later member's inverse depends on the earlier
        // member having been applied. Two chained renames of one node are that
        // case: each records the value the previous one produced.
        //
        // Forward-order undo would apply `Mid -> Alpha` while the node still
        // reads `End`, which the staleness check rejects outright. Reverse
        // order unwinds the chain and the node lands back on its original name.
        let mut document = persistent_document();
        let name = |document: &PersistentDocument| document.structure.nodes[0].name.clone();
        let id = NodeId::new("spool-a").unwrap();

        let compound = SemanticOperation::compound(vec![
            SemanticOperation::Rename(RenameNode {
                id: id.clone(),
                before: "Alpha".into(),
                after: "Mid".into(),
            }),
            SemanticOperation::Rename(RenameNode {
                id: id.clone(),
                before: "Mid".into(),
                after: "End".into(),
            }),
        ]);

        let mut target = |document: &mut PersistentDocument, direction| {
            apply(
                &compound,
                &mut OperationTarget::Document(document),
                direction,
            )
            .unwrap();
        };
        target(&mut document, ReplayDirection::Redo);
        assert_eq!(name(&document), "End", "applied forward");

        // Reverse order is required, not incidental: applied forward, the
        // first inverse would ask the node to read `Mid` while it reads `End`,
        // and `RenameNode::apply` refuses that as stale.
        target(&mut document, ReplayDirection::Undo);
        assert_eq!(name(&document), "Alpha", "reversed");

        target(&mut document, ReplayDirection::Redo);
        assert_eq!(name(&document), "End", "re-applied forward");
    }

    #[test]
    fn a_compound_whose_members_chain_cannot_be_committed_as_one_entry() {
        // The limit the previous test deliberately steps around, recorded so it
        // is not rediscovered as a bug.
        //
        // `validate` checks every member against one pre-transaction snapshot.
        // Two members that depend on each other cannot both satisfy that: the
        // second expects the value the first has not produced yet. So the replay
        // rule handles chained members, but `execute` refuses to commit them.
        //
        // This is the price of refusing to record a half-applied entry, and it
        // is the right trade while compound members are built from intent. It
        // is the first thing to revisit if a caller needs it.
        let mut session = session();
        let id = NodeId::new("spool-a").unwrap();
        let compound = SemanticOperation::compound(vec![
            SemanticOperation::Rename(RenameNode {
                id: id.clone(),
                before: "Alpha".into(),
                after: "Mid".into(),
            }),
            SemanticOperation::Rename(RenameNode {
                id: id.clone(),
                before: "Mid".into(),
                after: "End".into(),
            }),
        ]);

        let error = session
            .execute(compound)
            .expect_err("chained members are refused");
        assert_eq!(error, OperationError::StaleEdit(id));
        assert_eq!(
            session.document.structure.nodes[0].name, "Alpha",
            "and nothing applied"
        );
    }

    #[test]
    fn the_gesture_command_describes_only_the_objects_that_moved() {
        // `DocumentCommand`'s payload is private to `canvas.rs`, so the entry is
        // inspected through its `Debug` rendering rather than by matching on
        // variants. It is the only observation available from this module, and
        // without this test the filter in `geometry_command` could be deleted
        // with nothing failing.
        let mut session = session();
        let first = seed_rect(&mut session);
        let created = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(500.0, 500.0),
            size(10.0, 10.0),
        );
        session.execute(created).unwrap();
        let second = session
            .runtime
            .objects()
            .iter()
            .map(|object| object.id)
            .find(|candidate| *candidate != first)
            .expect("a second object");

        let snapshots = session.runtime.snapshot_objects(&[first, second]);
        let moved = geometry_of(&session, first);
        // Only the first object moves, as a drag of a partly-pinned selection
        // would.
        session.runtime.set_geometry(
            first,
            Geometry {
                position: point(30.0, 40.0),
                size: moved.size,
            },
        );

        let command = geometry_command(&session.runtime, &snapshots);
        assert!(!command.is_noop());
        let rendered = format!("{command:?}");
        assert_eq!(
            rendered.matches("GeometryChange").count(),
            1,
            "only the object that moved is described: {rendered}"
        );

        // And a gesture that has already settled describes nothing, because
        // fresh snapshots match the state they are compared against.
        let settled_snapshots = session.runtime.snapshot_objects(&[first, second]);
        let settled = geometry_command(&session.runtime, &settled_snapshots);
        assert!(settled.is_noop(), "a still gesture changes nothing");
        assert_eq!(format!("{settled:?}").matches("GeometryChange").count(), 0);
    }

    #[test]
    fn a_planned_move_is_relative_to_where_the_object_actually_is() {
        // Pinned with a non-zero origin on purpose. At the origin, "position
        // plus delta" and "the delta alone" are the same number, so a planner
        // that ignored the object's position would still pass every other test
        // in this module. This is the only assertion that separates them.
        let mut session = session();
        let created = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(100.0, 200.0),
            size(10.0, 10.0),
        );
        session.execute(created).unwrap();
        let id = session.runtime.objects()[0].id;
        let start = geometry_of(&session, id);
        assert_eq!(start.position, point(100.0, 200.0));

        let planned = plan_move_nodes(&session.runtime, &[id], 5.0, -3.0);
        assert_eq!(geometry_of(&session, id), start, "planning moved nothing");

        session.execute(planned).unwrap();
        assert_eq!(
            geometry_of(&session, id).position,
            point(105.0, 197.0),
            "the delta is applied to the object's real position"
        );

        // And the eager constructor agrees, so the two forms differ only in
        // when they mutate.
        session.undo().unwrap();
        let eager = move_nodes(&mut session.runtime, &[id], 5.0, -3.0);
        session.execute(eager).unwrap();
        assert_eq!(geometry_of(&session, id).position, point(105.0, 197.0));
    }

    #[test]
    fn two_planned_moves_of_one_object_do_not_accumulate() {
        // Worth pinning because it is a real constraint on building compounds,
        // not an accident. `plan_move_nodes` measures from the state it reads,
        // so two planned moves of the same object both start from the same
        // place and the second wins. Composition across objects works; stacking
        // deltas on one object needs the deltas summed by the caller.
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);

        let first = plan_move_nodes(&session.runtime, &[id], 10.0, 0.0);
        let second = plan_move_nodes(&session.runtime, &[id], 0.0, 20.0);
        assert_ne!(first, second, "the two plans differ");

        session
            .execute(SemanticOperation::compound(vec![first, second]))
            .unwrap();

        // The second plan wins rather than summing with the first: both read
        // the same starting geometry, so this is not the sum of the deltas.
        assert_eq!(
            geometry_of(&session, id).position,
            point(start.position.x, start.position.y + 20.0),
            "planned moves do not accumulate"
        );
        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
    }

    #[test]
    fn a_compound_of_moves_on_distinct_objects_accumulates() {
        // The realistic compound: one gesture touching several objects, each
        // planned against the state it read.
        let mut session = session();
        let first = seed_rect(&mut session);
        let created = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
        );
        session.execute(created).unwrap();
        let second = session
            .runtime
            .objects()
            .iter()
            .map(|object| object.id)
            .find(|candidate| *candidate != first)
            .expect("a second object");
        let depth = session.history.undo_len();

        let operation = SemanticOperation::compound(vec![
            plan_move_nodes(&session.runtime, &[first], 10.0, 0.0),
            plan_move_nodes(&session.runtime, &[second], 0.0, 20.0),
        ]);
        assert!(session.execute(operation).unwrap());
        assert_eq!(session.history.undo_len(), depth + 1);

        assert_eq!(geometry_of(&session, first).position, point(10.0, 0.0));
        assert_eq!(geometry_of(&session, second).position, point(0.0, 20.0));

        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, first).position, point(0.0, 0.0));
        assert_eq!(geometry_of(&session, second).position, point(0.0, 0.0));
    }

    #[test]
    fn a_compound_replays_identically_over_many_undo_redo_cycles() {
        // Deterministic replay. If the recorded members were not symmetric
        // under the reverse-order undo rule, the state would drift each cycle.
        let mut session = session();
        let id = seed_rect(&mut session);
        let original_position = geometry_of(&session, id).position;

        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(33.0, 44.0));
        session.execute(operation).unwrap();
        let committed: Vec<(ObjectId, Geometry)> = session
            .runtime
            .objects()
            .iter()
            .filter_map(|object| {
                session
                    .runtime
                    .geometry_of(object.id)
                    .map(|g| (object.id, g))
            })
            .collect();

        for _ in 0..5 {
            assert!(session.undo().unwrap());
            assert_eq!(session.runtime.objects().len(), 1, "one object after undo");
            assert_eq!(geometry_of(&session, id).position, original_position);

            assert!(session.redo().unwrap());
            let replayed: Vec<(ObjectId, Geometry)> = session
                .runtime
                .objects()
                .iter()
                .filter_map(|object| {
                    session
                        .runtime
                        .geometry_of(object.id)
                        .map(|g| (object.id, g))
                })
                .collect();
            assert_eq!(
                replayed, committed,
                "each cycle reproduces the state exactly"
            );
        }
    }

    #[test]
    fn a_compound_whose_last_member_is_invalid_applies_nothing() {
        // All-or-nothing. If members were validated as they were applied, the
        // first two would land and the entry would be half-applied — with no
        // way to undo it, because reversing the applied prefix is not the same
        // operation as reversing the compound.
        let mut session = session();
        let id = seed_rect(&mut session);
        let position = geometry_of(&session, id);
        let depth = session.history.undo_len();

        // `plan_move_nodes` only reads, so the member has not moved yet when
        // validation runs. An eager `move_nodes` here would have already moved
        // it, which is exactly the leak this test exists to rule out.
        let good = plan_move_nodes(&session.runtime, &[id], 100.0, 100.0);
        let also_good = plan_move_nodes(&session.runtime, &[id], 0.0, 100.0);
        let bad = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-a").unwrap(),
            before: "Alpha".into(),
            after: "Beta".into(),
        });
        let operation = SemanticOperation::compound(vec![good, also_good, bad]);

        assert!(
            session.execute(operation).is_err(),
            "the compound is refused"
        );
        assert_eq!(
            geometry_of(&session, id),
            position,
            "the valid member never applied"
        );
        assert_eq!(
            session.history.undo_len(),
            depth,
            "and nothing was recorded"
        );
    }

    #[test]
    fn nested_compounds_flatten_so_replay_order_is_unambiguous() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);

        let inner = SemanticOperation::compound(vec![
            plan_move_nodes(&session.runtime, &[id], 5.0, 0.0),
            plan_move_nodes(&session.runtime, &[id], 0.0, 5.0),
        ]);
        let outer = SemanticOperation::compound(vec![
            inner,
            plan_move_nodes(&session.runtime, &[id], 7.0, 0.0),
        ]);

        // Flattened to three members, not a nested tree.
        let SemanticOperation::Compound(members) = &outer else {
            panic!("expected a compound")
        };
        assert_eq!(members.len(), 3);
        assert!(matches!(members[0], SemanticOperation::Runtime(_)));

        assert!(session.execute(outer).unwrap());
        assert!(session.undo().unwrap());
        assert_eq!(geometry_of(&session, id), start);
    }

    #[test]
    fn a_compound_is_a_noop_only_when_every_member_is() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let position = geometry_of(&session, id);

        let idle = plan_move_nodes(&session.runtime, &[id], 0.0, 0.0);
        assert!(SemanticOperation::compound(vec![idle.clone()]).is_noop());
        assert!(
            SemanticOperation::compound(vec![]).is_noop(),
            "an empty compound changes nothing"
        );

        let real = plan_move_nodes(&session.runtime, &[id], 12.0, 0.0);
        assert!(!SemanticOperation::compound(vec![idle, real]).is_noop());
        assert_eq!(
            geometry_of(&session, id),
            position,
            "planning moved nothing"
        );

        // And a no-op compound records nothing.
        let depth = session.history.undo_len();
        let idle = plan_move_nodes(&session.runtime, &[id], 0.0, 0.0);
        assert!(!session
            .execute(SemanticOperation::compound(vec![idle]))
            .unwrap());
        assert_eq!(session.history.undo_len(), depth);
        assert_eq!(geometry_of(&session, id), position);
    }

    #[test]
    fn a_new_operation_after_a_compound_undo_invalidates_the_whole_branch() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(10.0, 10.0));
        session.execute(operation).unwrap();
        session.undo().unwrap();
        assert!(session.history.can_redo());

        let operation = move_nodes(&mut session.runtime, &[id], 3.0, 0.0);
        session.execute(operation).unwrap();
        assert!(
            !session.history.can_redo(),
            "redo branch is dropped, not truncated"
        );
    }

    #[test]
    fn a_compound_mixing_metadata_and_runtime_undoes_as_one_lifo_entry() {
        // A compound is the only way a rename and a runtime change land
        // together, which is the case an agent caller needs: one action, one
        // undo step, touching both kinds of state.
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        let rename = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Alpha 2".into(),
        )
        .unwrap();
        let move_op = move_nodes(&mut session.runtime, &[id], 15.0, 15.0);
        assert!(session
            .execute(SemanticOperation::compound(vec![rename, move_op]))
            .unwrap());

        assert_eq!(session.document.structure.nodes[0].name, "Alpha 2");
        assert_ne!(geometry_of(&session, id), start);
        assert_eq!(session.history.undo_len(), depth + 1);

        assert!(session.undo().unwrap());
        assert_eq!(
            session.document.structure.nodes[0].name, "Alpha",
            "metadata reverted"
        );
        assert_eq!(geometry_of(&session, id), start, "runtime reverted");

        assert!(session.redo().unwrap());
        assert_eq!(session.document.structure.nodes[0].name, "Alpha 2");
        assert_ne!(geometry_of(&session, id), start);
    }

    #[test]
    fn a_duplicate_and_move_of_nothing_records_nothing() {
        let mut session = session();
        let depth = session.history.undo_len();
        let operation =
            duplicate_and_move_nodes(&mut session.runtime, &[ObjectId(9_999)], point(5.0, 5.0));
        assert!(SemanticOperation::compound(vec![operation.clone()]).is_noop());
        assert!(!session.execute(operation).unwrap());
        assert_eq!(session.history.undo_len(), depth);
    }

    // -- Cancellation of a gesture that inserted objects.

    #[test]
    fn cancelling_a_gesture_removes_what_it_inserted() {
        // The debt a geometry-only cancel could not pay: a modifier-drag
        // duplicate adds rows to the runtime, and restoring positions cannot
        // un-add them. A cancelled gesture must be indistinguishable from one
        // that never started.
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        session.begin_gesture(&[id]);
        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(60.0, 60.0));
        session.gesture_execute(operation).unwrap();
        assert_eq!(
            session.runtime.objects().len(),
            2,
            "the copy exists mid-gesture"
        );

        session.cancel_gesture();

        assert_eq!(session.runtime.objects().len(), 1, "the copy is gone");
        assert_eq!(geometry_of(&session, id), start, "geometry restored");
        assert_eq!(
            session.history.undo_len(),
            depth,
            "the gesture recorded nothing"
        );
        assert!(!session.gesture_in_flight());

        // And the discarded gesture left nothing undoable to replay.
        assert!(!session.history.can_undo() || session.history.undo_len() == depth);
    }

    #[test]
    fn cancelling_a_gesture_reverses_every_operation_it_applied() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        session.begin_gesture(&[id]);
        // Two structural operations in one gesture: copy, then rename.
        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(20.0, 0.0));
        session.gesture_execute(operation).unwrap();
        let rename = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Renamed".into(),
        )
        .unwrap();
        session.gesture_execute(rename).unwrap();
        assert_eq!(session.document.structure.nodes[0].name, "Renamed");

        session.cancel_gesture();

        assert_eq!(
            session.document.structure.nodes[0].name, "Alpha",
            "metadata restored"
        );
        assert_eq!(session.runtime.objects().len(), 1, "copy removed");
        assert_eq!(geometry_of(&session, id), start);
        assert_eq!(session.history.undo_len(), depth, "nothing recorded");
    }

    #[test]
    fn committing_a_gesture_that_inserted_keeps_the_copy_and_records_one_entry() {
        // The counterpart: a gesture that both duplicates and drags commits the
        // whole thing, not just the geometry.
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let depth = session.history.undo_len();

        session.begin_gesture(&[id]);
        let operation = duplicate_and_move_nodes(&mut session.runtime, &[id], point(30.0, 0.0));
        session.gesture_execute(operation).unwrap();

        // The original never moved, so the geometry command alone would have
        // been a no-op. The gesture still commits, because the copy it created
        // is real work and belongs to the same undo step as the drag.
        assert!(session.commit_gesture().unwrap());
        assert_eq!(session.history.undo_len(), depth + 1, "one entry, not two");
        assert_eq!(
            session.runtime.objects().len(),
            2,
            "the copy survives a commit"
        );

        // One undo takes back both the copy and its placement.
        assert!(session.undo().unwrap());
        assert_eq!(session.runtime.objects().len(), 1);
        assert_eq!(
            geometry_of(&session, id).position,
            start.position,
            "the original stayed put"
        );
    }

    #[test]
    fn gesture_execute_outside_a_gesture_is_just_execute() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let depth = session.history.undo_len();
        let operation = move_nodes(&mut session.runtime, &[id], 8.0, 0.0);
        assert!(session.gesture_execute(operation).unwrap());
        assert_eq!(session.history.undo_len(), depth + 1);
        assert!(session.undo().unwrap());
    }

    // -- Provenance.

    #[test]
    fn an_entry_remembers_who_asked_for_it_through_undo_and_redo() {
        let mut session = session();
        let id = seed_rect(&mut session);

        // No origin given: a human action.
        let operation = move_nodes(&mut session.runtime, &[id], 1.0, 0.0);
        session.execute(operation).unwrap();
        assert_eq!(session.history.peek_undo_origin(), Some(Origin::User));

        // An attributed caller. Same object, so this is about attribution
        // rather than about geometry.
        let operation = plan_move_nodes(&session.runtime, &[id], 0.0, 1.0);
        session.execute_from(Origin::Agent, operation).unwrap();
        assert_eq!(session.history.peek_undo_origin(), Some(Origin::Agent));

        // Attribution survives the move to the redo stack and back, which is
        // the only way it can still be trusted when a caller reads it. After
        // the undo the entry is on the redo side, and the next thing to undo is
        // the older human action.
        assert!(session.undo().unwrap());
        assert_eq!(session.history.peek_redo_origin(), Some(Origin::Agent));
        assert_eq!(session.history.peek_undo_origin(), Some(Origin::User));

        assert!(session.redo().unwrap());
        assert_eq!(session.history.peek_undo_origin(), Some(Origin::Agent));

        assert_eq!(
            session.history.peek_undo_entry().map(|entry| entry.origin),
            Some(Origin::Agent)
        );
        assert!(
            session.history.peek_undo().is_some(),
            "the operation is still readable"
        );
    }

    #[test]
    fn attribution_does_not_change_undo_semantics() {
        let mut session = session();
        let id = seed_rect(&mut session);
        let start = geometry_of(&session, id);
        let operation = move_nodes(&mut session.runtime, &[id], 4.0, 4.0);

        session.execute_from(Origin::Import, operation).unwrap();
        assert_ne!(geometry_of(&session, id), start);
        assert!(session.undo().unwrap());
        assert_eq!(
            geometry_of(&session, id),
            start,
            "attribution is metadata, not behaviour"
        );
    }

    #[test]
    fn origins_are_distinct_so_a_caller_can_tell_them_apart() {
        // If these collapsed, the attribution would carry no information.
        let all = [
            Origin::User,
            Origin::Agent,
            Origin::Plugin,
            Origin::Import,
            Origin::Automation,
        ];
        for (index, left) in all.iter().enumerate() {
            for right in all.iter().skip(index + 1) {
                assert_ne!(
                    left, right,
                    "{left:?} and {right:?} must be distinguishable"
                );
            }
        }
        assert_eq!(
            Origin::default(),
            Origin::User,
            "an unattributed call is a human one"
        );
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
        assert_eq!(
            session.runtime.object(id).expect("object").spool_id,
            spool_id
        );

        // A rename is persistent metadata but does not change identity.
        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Alpha X".into(),
        )
        .unwrap();
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
        let mut other = EditorRuntimeState {
            selection: vec![],
            camera_offset: (0, 0),
            active_tool: String::new(),
        };
        other.camera_offset = (999, 999);
        other.selection.push(NodeId::new("spool-b").unwrap());
        assert_eq!(
            session.history.undo_len(),
            before,
            "camera/selection must not record"
        );

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
        let names: Vec<String> = session
            .document
            .structure
            .nodes
            .iter()
            .map(|node| node.name.clone())
            .collect();

        // Renaming a node that does not exist must fail cleanly.
        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Renamed".into(),
        )
        .unwrap();
        session.execute(operation).unwrap();
        let depth_after_rename = session.history.undo_len();

        let bogus = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-missing").unwrap(),
            before: "Nope".into(),
            after: "Other".into(),
        });
        assert!(session.execute(bogus).is_err());

        assert_eq!(
            session.history.undo_len(),
            depth_after_rename,
            "no entry for a failure"
        );
        let after: Vec<String> = session
            .document
            .structure
            .nodes
            .iter()
            .map(|node| node.name.clone())
            .collect();
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
                rename_node_in(
                    &session.document,
                    NodeId::new("spool-a").unwrap(),
                    "Alpha X".into(),
                )
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
        let created = create_nodes(
            &mut session.runtime,
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
        );
        let after_build = session.runtime.objects().len();
        session.execute(created).unwrap();
        assert_eq!(
            session.runtime.objects().len(),
            after_build,
            "create replayed twice"
        );
        assert!(session.undo().unwrap());
        assert!(
            session.runtime.objects().is_empty(),
            "undo removed the created object"
        );
        assert!(session.redo().unwrap());
        assert_eq!(
            session.runtime.objects().len(),
            after_build,
            "redo restored exactly one"
        );

        // Delete: removing an absent id twice removes nothing.
        let id = session.runtime.objects()[0].id;
        let deleted = delete_nodes(&mut session.runtime, &[id]);
        assert!(session.execute(deleted).unwrap());
        assert!(
            session.runtime.objects().is_empty(),
            "delete applied twice is stable"
        );
        assert!(session.undo().unwrap());
        assert_eq!(
            session.runtime.objects().len(),
            1,
            "undo restored exactly one"
        );
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
            resize_node(
                &mut session.runtime,
                id,
                Geometry {
                    position: point(1.0, 1.0),
                    size: size(20.0, 20.0),
                },
            ),
            duplicate_nodes(&mut session.runtime, &[id]),
            {
                let duplicated = session
                    .runtime
                    .objects()
                    .last()
                    .expect("duplicate exists")
                    .id;
                delete_nodes(&mut session.runtime, &[duplicated])
            },
        ];
        for operation in operations {
            session.execute(operation).expect("applies");
        }
        let operation = rename_node_in(
            &session.document,
            NodeId::new("spool-a").unwrap(),
            "Alpha 2".into(),
        )
        .unwrap();
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
