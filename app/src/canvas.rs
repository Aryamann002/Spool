use gpui::{
    div, fill, point, prelude::*, px as gpui_px, relative, rgb, rgba, size, App, Bounds,
    ClipboardItem, Context, DispatchPhase, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, GlobalElementId, HitboxBehavior, HitboxId, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, PinchEvent, Pixels,
    Point, Render, ScrollWheelEvent, SharedString, Size, Style, TextRun, UTF16Selection, Window,
};
use std::{cell::Cell, collections::BTreeMap, ops::Range, rc::Rc};

gpui::actions!(
    spool_text,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Copy,
        Cut,
    ]
);

use crate::{
    diagnostics,
    operations::{EditSession, OperationError, SemanticOperation},
    snap,
    source_document::{NodeId, PersistentDocument},
    theme,
};

#[cfg(debug_assertions)]
#[path = "canvas_workloads.rs"]
mod workloads;

#[cfg(test)]
#[path = "canvas_benchmarks.rs"]
mod benchmarks;

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
const WORLD_BOUNDS: Size<f32> = size(764.0, 688.0);
const WORLD_CENTER: Point<f32> = point(382.0, 344.0);
const LABEL_HEIGHT: f32 = 24.0;
const MIN_OBJECT_SIZE: f32 = 20.0;
const DRAG_THRESHOLD: f32 = 4.0;
/// Screen pixels left around content on each side when fitting the viewport.
const FIT_MARGIN: f32 = 48.0;
const RESIZE_HANDLE_SIZE: f32 = 8.0;
const RESIZE_HANDLE_HIT_RADIUS: f32 = 7.0;

/// Conservative world-space padding applied to every side of an object's
/// geometry box for Phase 14 viewport culling. An object's element tree is
/// constructed only when its padded box intersects the viewport (inclusive
/// edges, the same axis-aligned model as `diagnostics::intersects`).
///
/// The padding must cover visual overflow beyond the geometry box: frame and
/// starter-artboard labels paint `LABEL_HEIGHT` (24) world units above the
/// box, strokes add a few world units on each side, and text content can
/// overflow its box. All of that overflow is specified in world units scaled
/// by zoom (`px!(value, zoom)`), so a world-space padding stays conservative
/// at every zoom level from `MIN_ZOOM` to `MAX_ZOOM`. 64 world units covers
/// the 24-unit label plus roughly 40 world units (about two to three 14-unit
/// text lines at zoom 1) of additional margin. Objects whose box lies
/// entirely beyond this padding are skipped; everything else is constructed.
const CULL_PADDING: f32 = 64.0;

macro_rules! px {
    ($value:expr, $zoom:ident) => {
        gpui_px($value * $zoom)
    };
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    offset: Point<f32>,
    zoom: f32,
    viewport: Size<f32>,
    initialized: bool,
    /// The bounds of the most recent fit request, waiting for a viewport.
    ///
    /// A project is loaded before the first frame, so "frame my content" is
    /// asked while the camera still has a zero-sized viewport. Answering then
    /// means dividing by zero and clamping to the minimum zoom — which is how
    /// opening a project used to leave the user staring at a 10%-zoom speck.
    /// The request is kept and applied the moment a real viewport arrives.
    pending_fit: Option<WorldRect>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset: point(0.0, 0.0),
            zoom: 1.0,
            viewport: size(0.0, 0.0),
            initialized: false,
            pending_fit: None,
        }
    }
}

impl Camera {
    pub fn world_to_screen(&self, world: Point<f32>) -> Point<f32> {
        point(
            (world.x - self.offset.x) * self.zoom,
            (world.y - self.offset.y) * self.zoom,
        )
    }

    pub fn screen_to_world(&self, screen: Point<f32>) -> Point<f32> {
        point(
            screen.x / self.zoom + self.offset.x,
            screen.y / self.zoom + self.offset.y,
        )
    }

    fn resize(&mut self, viewport: Size<f32>) -> bool {
        if viewport == self.viewport {
            return false;
        }
        if self.initialized {
            self.offset.x -= (viewport.width - self.viewport.width) / (2.0 * self.zoom);
            self.offset.y -= (viewport.height - self.viewport.height) / (2.0 * self.zoom);
        } else {
            self.offset = point(
                WORLD_CENTER.x - viewport.width / (2.0 * self.zoom),
                WORLD_CENTER.y - viewport.height / (2.0 * self.zoom),
            );
            self.initialized = true;
        }
        self.viewport = viewport;
        if let Some(bounds) = self.pending_fit.take() {
            self.apply_fit(bounds);
        }
        true
    }

    fn zoom_at(&mut self, factor: f32, screen: Point<f32>) {
        let world_anchor = self.screen_to_world(screen);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = point(
            world_anchor.x - screen.x / self.zoom,
            world_anchor.y - screen.y / self.zoom,
        );
    }

    /// Set an absolute zoom, keeping `screen` pinned to the same world point.
    fn set_zoom_at(&mut self, zoom: f32, screen: Point<f32>) {
        self.zoom_at(zoom / self.zoom, screen);
    }

    fn set_zoom_at_center(&mut self, zoom: f32) {
        self.zoom_at(
            zoom / self.zoom,
            point(self.viewport.width / 2.0, self.viewport.height / 2.0),
        );
    }

    /// Fit a world-space box into the viewport.
    ///
    /// This is the camera's only fit rule, and it takes the box as an argument
    /// rather than reading a constant. The previous version fitted a hard-coded
    /// `WORLD_BOUNDS`, which was an accidental bound: every product in the
    /// research corpus is either explicitly infinite (Figma, tldraw) or
    /// explicitly bounded (Canva), and a fixed 764x688 is neither — it made
    /// zoom-to-fit mean "show me the prototype's placeholder artboard" instead
    /// of "show me what is in this document".
    fn fit_bounds(&mut self, bounds: WorldRect) {
        self.pending_fit = Some(bounds);
        if self.initialized {
            self.apply_fit(bounds);
            self.pending_fit = None;
        }
    }

    fn apply_fit(&mut self, bounds: WorldRect) {
        let available = size(
            (self.viewport.width - FIT_MARGIN * 2.0).max(1.0),
            (self.viewport.height - FIT_MARGIN * 2.0).max(1.0),
        );
        self.zoom = (available.width / bounds.width().max(1.0))
            .min(available.height / bounds.height().max(1.0))
            // No cap at 100%: fitting a small frame does magnify it, in Figma
            // and in tldraw, because "fit" means fit. `⇧0` is the key for
            // actual size. The floor only stops content becoming a speck.
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = point(
            bounds.center().x - self.viewport.width / (2.0 * self.zoom),
            bounds.center().y - self.viewport.height / (2.0 * self.zoom),
        );
    }

    /// Fit the placeholder starter scene, for a document with no real content.
    fn fit(&mut self) {
        self.fit_bounds(WorldRect {
            min: point(
                WORLD_CENTER.x - WORLD_BOUNDS.width / 2.0,
                WORLD_CENTER.y - WORLD_BOUNDS.height / 2.0,
            ),
            max: point(
                WORLD_CENTER.x + WORLD_BOUNDS.width / 2.0,
                WORLD_CENTER.y + WORLD_BOUNDS.height / 2.0,
            ),
        });
    }

    fn pan_from(
        &mut self,
        start_offset: Point<f32>,
        start_pointer: Point<f32>,
        pointer: Point<f32>,
    ) {
        let delta = point(pointer.x - start_pointer.x, pointer.y - start_pointer.y);
        self.offset = point(
            start_offset.x - delta.x / self.zoom,
            start_offset.y - delta.y / self.zoom,
        );
    }

    /// Phase 14 conservative culling predicate: true when the object's
    /// geometry can affect the current viewport, in which case its element
    /// tree must be constructed.
    ///
    /// This is exactly the `diagnostics::intersects` model — inclusive
    /// axis-aligned geometry-box intersection with the viewport in screen
    /// space — applied to the object's box expanded by `CULL_PADDING` world
    /// units on every side, so the constructed set is always a superset of
    /// the diagnostic visibility model. Padding is world space, so the
    /// predicate follows the actual camera zoom and offset rather than
    /// assuming zoom = 1.
    ///
    /// Conservative on uncertainty: any input the intersection model cannot
    /// evaluate (non-finite geometry, camera offset, zoom or viewport;
    /// non-positive zoom or viewport; negative size) returns true so that
    /// unevaluable state constructs the object instead of culling it. This
    /// includes the pre-prepaint render where the viewport is still empty.
    fn affects_viewport(&self, position: Point<f32>, object_size: Size<f32>) -> bool {
        let padded_position = point(position.x - CULL_PADDING, position.y - CULL_PADDING);
        let padded_size = size(
            object_size.width + 2.0 * CULL_PADDING,
            object_size.height + 2.0 * CULL_PADDING,
        );
        let evaluable = self.zoom > 0.0
            && self.viewport.width > 0.0
            && self.viewport.height > 0.0
            && object_size.width >= 0.0
            && object_size.height >= 0.0
            && [
                self.zoom,
                self.viewport.width,
                self.viewport.height,
                self.offset.x,
                self.offset.y,
                padded_position.x,
                padded_position.y,
                padded_size.width,
                padded_size.height,
            ]
            .iter()
            .all(|value| value.is_finite()); // Not evaluable -> construct (conservative); otherwise the padded
                                             // intersection decides.
        !evaluable
            || diagnostics::intersects(
                padded_position,
                padded_size,
                self.offset,
                self.viewport,
                self.zoom,
            )
    }
}

// `Ord` is additive: it lets the runtime projection key a `BTreeMap` by
// `ObjectId` so projection order is deterministic. No existing behaviour
// depends on this, and `ObjectId` remains a runtime lookup key, not identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

impl ObjectId {
    pub const LANDING: Self = Self(1);
    pub const EDITOR: Self = Self(2);
    pub const FEATURES: Self = Self(3);
    pub const MOBILE: Self = Self(4);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    Frame,
    Rectangle,
    Ellipse,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Frame,
    Rectangle,
    Ellipse,
    Pen,
    Text,
    Comment,
}

impl Tool {
    fn creates_object(self) -> Option<ObjectType> {
        match self {
            Self::Frame => Some(ObjectType::Frame),
            Self::Rectangle => Some(ObjectType::Rectangle),
            Self::Ellipse => Some(ObjectType::Ellipse),
            Self::Text => Some(ObjectType::Text),
            Self::Select | Self::Pen | Self::Comment => None,
        }
    }
}

impl ObjectType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Frame => "Frame",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Text => "Text",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const fn from_rgb(value: u32) -> Self {
        Self {
            red: ((value >> 16) & 0xff) as u8,
            green: ((value >> 8) & 0xff) as u8,
            blue: (value & 0xff) as u8,
        }
    }

    pub const fn to_rgb(self) -> u32 {
        ((self.red as u32) << 16) | ((self.green as u32) << 8) | self.blue as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectStyle {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    /// Corner radius in pixels; zero means square.
    ///
    /// Mirrors CSS `border-radius`. Carried in the style rather than folded
    /// into geometry because it is paint, not layout.
    pub border_radius: f32,
    /// Alpha from 0 to 1. Mirrors CSS `opacity`.
    pub opacity: f32,
}

impl Default for ObjectStyle {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }
}

/// Everything about one object's appearance that the editor can change.
///
/// One snapshot per object, so undo restores the appearance as it was rather
/// than trying to reverse individual properties. Keeping text colour and font
/// size here — rather than beside it on the object — is what lets one style edit
/// cover "the label got smaller and darker" as a single history entry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Appearance {
    pub style: ObjectStyle,
    pub text_color: Option<Color>,
    /// Authored CSS `font-size` in pixels, when one was authored.
    pub font_size: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleEdit {
    Fill(Option<Color>),
    Stroke(Option<Color>),
    StrokeWidth(f32),
    /// Text colour. `None` removes an explicit colour and lets the renderer
    /// choose one again.
    TextColor(Option<Color>),
    /// Font size in pixels.
    FontSize(f32),
    /// Corner radius in pixels.
    BorderRadius(f32),
    /// Alpha from 0 to 1, clamped.
    Opacity(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesignObject {
    /// Runtime lookup key. Never durable; allocated locally from `next_id`.
    pub id: ObjectId,
    /// Carries a persistent identity, but only sometimes.
    ///
    /// Two distinct cases, and conflating them would let a runtime-local id be
    /// mistaken for durable identity:
    ///
    /// - **Projected.** Objects built by
    ///   `document_runtime_bridge::RuntimeProjection::canvas_objects` carry the
    ///   `NodeId` of a real `lamine.yaml` structural node.
    /// - **Locally minted.** Objects this `Document` creates or duplicates get
    ///   an id from `allocate_node_id` (`spool-node-<16 hex>`). No structural
    ///   node backs them yet and no save path writes them anywhere, so today
    ///   they are runtime-local values that merely share the `NodeId` type.
    ///
    /// `NodeId` here means "opaque, well-formed identifier", not "exists in
    /// the persistent document". Only the projected case may be persisted.
    pub spool_id: NodeId,
    pub name: String,
    pub position: Point<f32>,
    pub size: Size<f32>,
    pub object_type: ObjectType,
    pub text_content: Option<String>,
    /// Authored CSS `color` for this object's text, when source declared one.
    ///
    /// `None` means "nobody authored a colour", which is different from a
    /// painted colour: the renderer picks its own in that case. Keeping the
    /// distinction is what stops a guess from silently overriding source.
    pub text_color: Option<Color>,
    /// Authored CSS `font-size` in pixels, when source declared one.
    pub font_size: Option<f32>,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    /// CSS `border-radius` in pixels; zero means square corners.
    pub border_radius: f32,
    /// CSS `opacity`, 0 to 1.
    pub opacity: f32,
}

impl DesignObject {
    fn contains(&self, point: Point<f32>) -> bool {
        point.x >= self.position.x
            && point.y >= self.position.y
            && point.x <= self.position.x + self.size.width
            && point.y <= self.position.y + self.size.height
    }

    fn geometry(&self) -> ObjectGeometry {
        ObjectGeometry {
            position: self.position,
            size: self.size,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub position: Point<f32>,
    pub size: Size<f32>,
}

type ObjectGeometry = Geometry;

/// An Inspector geometry change that has not been recorded yet.
///
/// The Inspector owns the widget; the canvas owns the document and the history
/// boundary. Keeping the boundary here is what stops a continuous control from
/// becoming one history entry per pointer movement: the shell drags, and the
/// whole drag is one operation when it ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryScrub {
    pub id: ObjectId,
    /// The geometry the object had before the scrub started.
    pub before: Geometry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeometryChange {
    pub id: ObjectId,
    pub before: Geometry,
    pub after: Geometry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleChange {
    pub id: ObjectId,
    pub before: Appearance,
    pub after: Appearance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChange {
    pub id: ObjectId,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectPlacement {
    pub object: DesignObject,
    pub index: usize,
}

/// Which way a command is being replayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDirection {
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq)]
enum CommandOperation {
    Geometry(Vec<GeometryChange>),
    Style(Vec<StyleChange>),
    Text(Vec<TextChange>),
    Insert(Vec<ObjectPlacement>),
    Delete(Vec<ObjectPlacement>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentCommand {
    operation: CommandOperation,
}

impl DocumentCommand {
    pub fn geometry(changes: Vec<GeometryChange>) -> Self {
        Self {
            operation: CommandOperation::Geometry(changes),
        }
    }

    pub fn style(changes: Vec<StyleChange>) -> Self {
        Self {
            operation: CommandOperation::Style(changes),
        }
    }

    pub fn text(changes: Vec<TextChange>) -> Self {
        Self {
            operation: CommandOperation::Text(changes),
        }
    }

    pub fn insert(objects: Vec<ObjectPlacement>) -> Self {
        Self {
            operation: CommandOperation::Insert(objects),
        }
    }

    pub fn delete(objects: Vec<ObjectPlacement>) -> Self {
        Self {
            operation: CommandOperation::Delete(objects),
        }
    }
}

impl DocumentCommand {
    /// Whether applying this command would change nothing.
    ///
    /// A no-op command never enters history, so a gesture that ends where it
    /// started produces no entry and does not clear redo.
    pub fn is_noop(&self) -> bool {
        match &self.operation {
            CommandOperation::Geometry(changes) => changes.iter().all(|c| c.before == c.after),
            CommandOperation::Style(changes) => changes.iter().all(|c| c.before == c.after),
            CommandOperation::Text(changes) => changes.iter().all(|c| c.before == c.after),
            CommandOperation::Insert(objects) | CommandOperation::Delete(objects) => {
                objects.is_empty()
            }
        }
    }

    /// Check the command can apply, before any state changes.
    pub fn validate(&self, document: &Document) -> Result<(), OperationError> {
        // Only in-place edits need their target present. An insert adds the
        // objects, and a delete has already removed them, so requiring
        // presence there would reject every legitimate command.
        let ids: Vec<ObjectId> = match &self.operation {
            CommandOperation::Geometry(changes) => changes.iter().map(|c| c.id).collect(),
            CommandOperation::Style(changes) => changes.iter().map(|c| c.id).collect(),
            CommandOperation::Text(changes) => changes.iter().map(|c| c.id).collect(),
            CommandOperation::Insert(_) | CommandOperation::Delete(_) => Vec::new(),
        };
        for id in ids {
            if document.object(id).is_none() {
                return Err(OperationError::MissingObject(id));
            }
        }
        Ok(())
    }

    /// Apply the command in one direction. This is the undo/redo mechanism:
    /// each variant replays `before` or `after`, and insert/delete are
    /// symmetric inverses by construction.
    pub fn replay(&self, document: &mut Document, direction: ReplayDirection) {
        let forward = matches!(direction, ReplayDirection::Redo);
        match &self.operation {
            CommandOperation::Geometry(changes) => {
                for change in changes {
                    document.set_geometry(
                        change.id,
                        if forward { change.after } else { change.before },
                    );
                }
            }
            CommandOperation::Style(changes) => {
                for change in changes {
                    document.set_appearance(
                        change.id,
                        if forward { change.after } else { change.before },
                    );
                }
            }
            CommandOperation::Text(changes) => {
                for change in changes {
                    document.set_text_content(
                        change.id,
                        if forward {
                            change.after.clone()
                        } else {
                            change.before.clone()
                        },
                    );
                }
            }
            CommandOperation::Insert(objects) => {
                // Undoing an insert removes what it added; redoing restores it.
                if forward {
                    document.insert_objects(objects);
                } else {
                    let ids: Vec<ObjectId> = objects
                        .iter()
                        .map(|placement| placement.object.id)
                        .collect();
                    document.remove_objects(&ids);
                }
            }
            CommandOperation::Delete(objects) => {
                // Undoing a delete puts the removed objects back.
                if forward {
                    let ids: Vec<ObjectId> = objects
                        .iter()
                        .map(|placement| placement.object.id)
                        .collect();
                    document.remove_objects(&ids);
                } else {
                    document.insert_objects(objects);
                }
            }
        }
    }

    /// A copy with the no-op entries removed.
    ///
    /// Public because the semantic boundary is the right place to trim a
    /// partially-changed command: a multi-object gesture where some objects
    /// did not actually move should record only the ones that did.
    pub fn normalized(mut self) -> Self {
        match &mut self.operation {
            CommandOperation::Geometry(changes) => changes.retain(|c| c.before != c.after),
            CommandOperation::Style(changes) => changes.retain(|c| c.before != c.after),
            CommandOperation::Text(changes) => changes.retain(|c| c.before != c.after),
            CommandOperation::Insert(_) | CommandOperation::Delete(_) => {}
        }
        self
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    objects: Vec<DesignObject>,
    next_id: u64,
    next_node_id: u64,
    next_names: [u64; 4],
    layer_structure_revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            objects: vec![
                frame(
                    ObjectId::LANDING,
                    node_id("spool-node-landing"),
                    "Landing",
                    0.0,
                    24.0,
                    430.0,
                    286.0,
                ),
                frame(
                    ObjectId::EDITOR,
                    node_id("spool-node-editor"),
                    "Editor",
                    454.0,
                    24.0,
                    310.0,
                    252.0,
                ),
                frame(
                    ObjectId::FEATURES,
                    node_id("spool-node-features"),
                    "Features",
                    106.0,
                    366.0,
                    394.0,
                    280.0,
                ),
                frame(
                    ObjectId::MOBILE,
                    node_id("spool-node-mobile"),
                    "Mobile",
                    524.0,
                    366.0,
                    192.0,
                    322.0,
                ),
            ],
            next_id: 5,
            next_node_id: 1,
            next_names: [1; 4],
            layer_structure_revision: 0,
        }
    }
}

impl Document {
    /// A document with no objects.
    ///
    /// `Document::default()` is the canvas starter scene, which is populated
    /// with four demonstration frames. Anything that needs a blank document —
    /// a test, or a runtime built purely from a projection — wants this.
    // Test-only: `Document::from_design_objects` is how a runtime built purely
    // from a projection gets its object list, and nothing in the editor needs a
    // blank starter scene. Compiled out of the binary rather than silenced, so
    // "unused" cannot quietly become "shipped".
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            objects: Vec::new(),
            next_id: 1,
            next_node_id: 1,
            next_names: [1; 4],
            layer_structure_revision: 0,
        }
    }

    /// Build a runtime document from already-projected canvas objects.
    ///
    /// This is the bridge between `RuntimeProjection::canvas_objects` and the
    /// live editor. It is deliberately one-way: the runtime document is a
    /// disposable interpretation, and nothing here writes projected geometry
    /// back into the persistent document.
    ///
    /// The counters are seeded so that anything the user draws afterwards gets
    /// a key above every projected key. Projected keys come from the reserved
    /// range owned by `document_runtime_bridge`, so this only has to clear the
    /// highest one actually present rather than assume a base.
    ///
    /// `next_node_id` restarts at 1 because projected objects keep their real
    /// `NodeId`s; the counter only mints ids for objects created later in this
    /// session.
    pub fn from_design_objects(objects: Vec<DesignObject>) -> Self {
        let next_id = objects
            .iter()
            .map(|object| object.id.0)
            .max()
            .map(|highest| highest + 1)
            .unwrap_or(1);
        // Provisional visibility bridge.
        //
        // A projection currently supplies geometry but no paint: fill and
        // stroke are `None` because the authored CSS has not been read yet. An
        // object with neither is drawn as nothing, so a correctly projected
        // project would be present in the document, in the layers panel, and
        // hit-testable, yet invisible on the canvas.
        //
        // Falling back to the canvas default style keeps the projected scene
        // visible while CSS ownership is still unimplemented. This is
        // deliberately NOT a style claim: it is a placeholder so the pipeline
        // can be seen end to end. When CSS ownership lands it must replace this
        // fallback, not sit beside it.
        let objects = objects
            .into_iter()
            .map(|mut object| {
                if object.fill.is_none() && object.stroke.is_none() {
                    let default = default_style(object.object_type);
                    object.fill = default.fill;
                    object.stroke = default.stroke;
                }
                object
            })
            .collect::<Vec<_>>();
        let mut document = Self {
            objects,
            next_id,
            next_node_id: 1,
            next_names: [1; 4],
            layer_structure_revision: 0,
        };
        // Assign a layer-structure revision per inserted object so layers and
        // other observers see the document as freshly built rather than empty.
        document.layer_structure_revision = document.objects.len() as u64;
        document
    }

    pub fn layer_structure_revision(&self) -> u64 {
        self.layer_structure_revision
    }

    pub fn objects(&self) -> &[DesignObject] {
        &self.objects
    }

    pub fn object(&self, id: ObjectId) -> Option<&DesignObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn text_content(&self, id: ObjectId) -> Option<&str> {
        self.object(id)?.text_content.as_deref()
    }

    /// Mirror a new name from the persistent document onto the runtime object.
    ///
    /// Not a second place a name can be edited: the persistent document is the
    /// authority, and this only refreshes the copy the Inspector and the layers
    /// list read. Called after a rename, never instead of one.
    pub fn set_object_name(&mut self, id: ObjectId, name: String) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        if object.name == name {
            return false;
        }
        object.name = name;
        true
    }

    pub fn set_text_content(&mut self, id: ObjectId, text: String) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        if object.object_type != ObjectType::Text {
            return false;
        }
        object.text_content = Some(text);
        true
    }

    fn allocate_id(&mut self) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Mint an identity for a newly created or duplicated object.
    ///
    /// The counter alone is not sufficient. A loaded project brings its own
    /// `NodeId`s from `lamine.yaml`, and those are arbitrary authored strings —
    /// not necessarily produced by this counter. Restarting the counter at 1 in
    /// [`Document::from_design_objects`] therefore risked minting an identity a
    /// live node already owns, which would make two objects share one durable
    /// identity.
    ///
    /// So the allocator checks against what is actually present and skips
    /// collisions. That is authoritative and deterministic regardless of how
    /// the document was built.
    fn allocate_node_id(&mut self) -> NodeId {
        loop {
            let candidate = node_id(format!("spool-node-{:016x}", self.next_node_id));
            self.next_node_id += 1;
            if !self
                .objects
                .iter()
                .any(|object| object.spool_id == candidate)
            {
                return candidate;
            }
        }
    }

    fn allocate_name(&mut self, object_type: ObjectType) -> String {
        let index = match object_type {
            ObjectType::Frame => 0,
            ObjectType::Rectangle => 1,
            ObjectType::Ellipse => 2,
            ObjectType::Text => 3,
        };
        let number = self.next_names[index];
        self.next_names[index] += 1;
        format!("{} {number}", object_type.label())
    }

    pub fn create_object(
        &mut self,
        object_type: ObjectType,
        position: Point<f32>,
        object_size: Size<f32>,
        text_content: Option<String>,
    ) -> DesignObject {
        let id = self.allocate_id();
        let spool_id = self.allocate_node_id();
        let object = DesignObject {
            id,
            spool_id,
            name: self.allocate_name(object_type),
            position,
            size: size(
                object_size.width.max(MIN_OBJECT_SIZE),
                object_size.height.max(MIN_OBJECT_SIZE),
            ),
            object_type,
            text_content,
            text_color: None,
            font_size: None,
            fill: default_style(object_type).fill,
            stroke: default_style(object_type).stroke,
            border_radius: default_style(object_type).border_radius,
            opacity: 1.0,
        };
        self.insert_object(object.clone(), self.objects.len());
        object
    }

    fn insert_object(&mut self, object: DesignObject, index: usize) -> bool {
        if self.object(object.id).is_some() {
            return false;
        }
        self.objects.insert(index.min(self.objects.len()), object);
        self.layer_structure_revision += 1;
        true
    }

    pub fn insert_objects(&mut self, placements: &[ObjectPlacement]) {
        let mut placements = placements.to_vec();
        placements.sort_by_key(|placement| placement.index);
        for placement in placements {
            self.insert_object(placement.object, placement.index);
        }
    }

    fn placement(&self, id: ObjectId) -> Option<ObjectPlacement> {
        let index = self.objects.iter().position(|object| object.id == id)?;
        Some(ObjectPlacement {
            object: self.objects[index].clone(),
            index,
        })
    }

    pub fn remove_objects(&mut self, ids: &[ObjectId]) -> Vec<ObjectPlacement> {
        let removed: Vec<_> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, object)| ids.contains(&object.id))
            .map(|(index, object)| ObjectPlacement {
                object: object.clone(),
                index,
            })
            .collect();
        self.objects.retain(|object| !ids.contains(&object.id));
        self.layer_structure_revision += removed.len() as u64;
        removed
    }

    pub fn duplicate_objects(&mut self, ids: &[ObjectId]) -> Vec<ObjectPlacement> {
        let originals: Vec<_> = self
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .cloned()
            .collect();
        let mut duplicates = Vec::with_capacity(originals.len());
        for mut object in originals {
            object.id = self.allocate_id();
            object.spool_id = self.allocate_node_id();
            object.name = self.allocate_name(object.object_type);
            object.position = point(object.position.x + 16.0, object.position.y + 16.0);
            let index = self.objects.len();
            self.insert_object(object.clone(), index);
            duplicates.push(ObjectPlacement { object, index });
        }
        duplicates
    }

    // Test-only reading of an object's paint, used to assert that a replayed
    // style entry restores exactly what it recorded. Production style edits go
    // through `DocumentCommand::style` -> `set_appearance`, which is the single
    // write path, so there is deliberately no second read path to keep in step.
    #[cfg(test)]
    fn style(&self, id: ObjectId) -> Option<ObjectStyle> {
        self.appearance(id).map(|appearance| appearance.style)
    }

    /// The full appearance of an object: paint plus its text styling.
    pub fn appearance(&self, id: ObjectId) -> Option<Appearance> {
        self.object(id).map(|object| Appearance {
            style: ObjectStyle {
                fill: object.fill,
                stroke: object.stroke,
                border_radius: object.border_radius,
                opacity: object.opacity,
            },
            text_color: object.text_color,
            font_size: object.font_size,
        })
    }

    // Test-only convenience over `set_appearance`, for the tests that set paint
    // without a history entry behind it. Production writes style only by
    // replaying `DocumentCommand::style`.
    #[cfg(test)]
    pub fn set_style(&mut self, id: ObjectId, style: ObjectStyle) -> bool {
        let Some(current) = self.appearance(id) else {
            return false;
        };
        self.set_appearance(id, Appearance { style, ..current })
    }

    /// Apply a whole appearance. The single write path for every style edit,
    /// so a replayed history entry restores exactly what it recorded.
    pub fn set_appearance(&mut self, id: ObjectId, appearance: Appearance) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        object.fill = appearance.style.fill;
        object.stroke = appearance.style.stroke;
        object.border_radius = appearance.style.border_radius;
        object.opacity = appearance.style.opacity;
        object.text_color = appearance.text_color;
        object.font_size = appearance.font_size;
        true
    }

    pub fn set_position(&mut self, id: ObjectId, position: Point<f32>) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        object.position = position;
        true
    }

    pub fn set_size(&mut self, id: ObjectId, object_size: Size<f32>) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        object.size = size(
            object_size.width.max(MIN_OBJECT_SIZE),
            object_size.height.max(MIN_OBJECT_SIZE),
        );
        true
    }

    pub fn geometry(&self, id: ObjectId) -> Option<Geometry> {
        self.object(id).map(DesignObject::geometry)
    }

    pub fn set_geometry(&mut self, id: ObjectId, geometry: Geometry) -> bool {
        if !self.set_position(id, geometry.position) {
            return false;
        }
        self.set_size(id, geometry.size)
    }

    pub fn hit_test(&self, world_point: Point<f32>) -> Option<ObjectId> {
        self.objects
            .iter()
            .rev()
            .find(|object| object.contains(world_point))
            .map(|object| object.id)
    }

    fn objects_in(&self, bounds: WorldRect) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|object| bounds.contains_object(object))
            .map(|object| object.id)
            .collect()
    }
}

fn frame(
    id: ObjectId,
    spool_id: NodeId,
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> DesignObject {
    DesignObject {
        id,
        spool_id,
        name: name.to_string(),
        position: point(x, y),
        size: size(width, height),
        object_type: ObjectType::Frame,
        text_content: None,
        text_color: None,
        font_size: None,
        fill: default_style(ObjectType::Frame).fill,
        stroke: default_style(ObjectType::Frame).stroke,
        border_radius: default_style(ObjectType::Frame).border_radius,
        opacity: 1.0,
    }
}

fn node_id(value: impl Into<String>) -> NodeId {
    NodeId::new(value).expect("generated Spool node IDs use the validated identifier alphabet")
}

fn edited_style(mut appearance: Appearance, edit: StyleEdit) -> Appearance {
    let style = &mut appearance.style;
    match edit {
        StyleEdit::TextColor(color) => appearance.text_color = color,
        StyleEdit::FontSize(size) => appearance.font_size = Some(size.max(1.0)),
        StyleEdit::BorderRadius(radius) => style.border_radius = radius.max(0.0),
        StyleEdit::Opacity(alpha) => style.opacity = alpha.clamp(0.0, 1.0),
        StyleEdit::Fill(color) => style.fill = color.map(|color| Fill { color }),
        StyleEdit::Stroke(color) => {
            style.stroke = color.map(|color| Stroke {
                color,
                width: style.stroke.map_or(1.0, |stroke| stroke.width),
            });
        }
        StyleEdit::StrokeWidth(width) => {
            let color = style
                .stroke
                .map_or(Color::from_rgb(theme::BORDER), |stroke| stroke.color);
            style.stroke = Some(Stroke { color, width });
        }
    }
    appearance
}

fn default_style(object_type: ObjectType) -> ObjectStyle {
    ObjectStyle {
        fill: match object_type {
            ObjectType::Frame => Some(Fill {
                color: Color::from_rgb(theme::PAPER),
            }),
            ObjectType::Rectangle | ObjectType::Ellipse => Some(Fill {
                color: Color::from_rgb(theme::SURFACE_RAISED),
            }),
            ObjectType::Text => None,
        },
        stroke: match object_type {
            ObjectType::Text => None,
            _ => Some(Stroke {
                color: Color::from_rgb(theme::BORDER),
                width: 1.0,
            }),
        },
        // Only a shape gets a default radius; giving every object one would
        // look like a decision the editor made on the author's behalf.
        border_radius: match object_type {
            ObjectType::Rectangle | ObjectType::Ellipse => 4.0,
            _ => 0.0,
        },
        opacity: 1.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorldRect {
    min: Point<f32>,
    max: Point<f32>,
}

impl WorldRect {
    fn from_object(object: &DesignObject) -> Self {
        Self {
            min: object.position,
            max: point(
                object.position.x + object.size.width,
                object.position.y + object.size.height,
            ),
        }
    }

    /// The smallest box containing every object, if there is at least one.
    ///
    /// `None` for an empty document, because "fit nothing" has no honest
    /// answer: the caller falls back to the placeholder scene rather than
    /// inventing a box around the origin.
    fn around(objects: &[DesignObject]) -> Option<Self> {
        let mut bounds: Option<Self> = None;
        for object in objects {
            let object_bounds = Self::from_object(object);
            bounds = Some(match bounds {
                Some(current) => Self {
                    min: point(
                        current.min.x.min(object_bounds.min.x),
                        current.min.y.min(object_bounds.min.y),
                    ),
                    max: point(
                        current.max.x.max(object_bounds.max.x),
                        current.max.y.max(object_bounds.max.y),
                    ),
                },
                None => object_bounds,
            });
        }
        bounds
    }

    fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    fn center(self) -> Point<f32> {
        point(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    fn from_points(start: Point<f32>, end: Point<f32>) -> Self {
        Self {
            min: point(start.x.min(end.x), start.y.min(end.y)),
            max: point(start.x.max(end.x), start.y.max(end.y)),
        }
    }

    fn contains_object(self, object: &DesignObject) -> bool {
        object.position.x >= self.min.x
            && object.position.y >= self.min.y
            && object.position.x + object.size.width <= self.max.x
            && object.position.y + object.size.height <= self.max.y
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    selected: Vec<ObjectId>,
}

impl Selection {
    pub fn ids(&self) -> &[ObjectId] {
        &self.selected
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.selected.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    fn click(&mut self, target: Option<ObjectId>, additive: bool) {
        let Some(id) = target else {
            self.selected.clear();
            return;
        };
        if !additive {
            self.selected.clear();
            self.selected.push(id);
        } else if let Some(index) = self.selected.iter().position(|selected| *selected == id) {
            self.selected.remove(index);
        } else {
            self.selected.push(id);
        }
    }

    fn replace(&mut self, ids: Vec<ObjectId>) {
        self.selected = ids;
    }

    fn add_all(&mut self, ids: impl IntoIterator<Item = ObjectId>) {
        for id in ids {
            if !self.contains(id) {
                self.selected.push(id);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct CanvasHitbox {
    id: HitboxId,
    origin: Point<Pixels>,
}

#[derive(Clone, Copy)]
struct PanGesture {
    button: MouseButton,
    pointer_start: Point<f32>,
    offset_start: Point<f32>,
}

#[derive(Clone, Copy)]
pub struct ObjectSnapshot {
    pub id: ObjectId,
    pub geometry: ObjectGeometry,
}

#[derive(Clone, Copy)]
enum ClickSelection {
    SelectOnly(ObjectId),
    Toggle(ObjectId),
}

struct MoveGesture {
    pointer_start_screen: Point<f32>,
    pointer_start_world: Point<f32>,
    objects: Vec<ObjectSnapshot>,
    selected_ids: Vec<ObjectId>,
    click_selection: ClickSelection,
    /// `⌘`/`Ctrl` held at press time: place this object freely, ignoring
    /// alignment for the whole gesture.
    ///
    /// Read once, when the gesture starts, rather than sampled per pointer
    /// movement. Figma's `⌘`-drag is one continuous "not this time", and a
    /// magnet that switches off halfway through a drag is the single most
    /// disorienting thing a snapping implementation can do.
    suspend_snap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl ResizeHandle {
    const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    fn moves_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
    }

    fn screen_position(self, camera: Camera, object: &DesignObject) -> Point<f32> {
        let origin = camera.world_to_screen(object.position);
        let right = origin.x + object.size.width * camera.zoom;
        let bottom = origin.y + object.size.height * camera.zoom;
        match self {
            Self::TopLeft => point(origin.x, origin.y),
            Self::Top => point((origin.x + right) / 2.0, origin.y),
            Self::TopRight => point(right, origin.y),
            Self::Right => point(right, (origin.y + bottom) / 2.0),
            Self::BottomRight => point(right, bottom),
            Self::Bottom => point((origin.x + right) / 2.0, bottom),
            Self::BottomLeft => point(origin.x, bottom),
            Self::Left => point(origin.x, (origin.y + bottom) / 2.0),
        }
    }
}

struct ResizeGesture {
    pointer_start_screen: Point<f32>,
    pointer_start_world: Point<f32>,
    object: ObjectSnapshot,
    handle: ResizeHandle,
}

#[derive(Clone, Copy)]
struct CreateGesture {
    object_type: ObjectType,
    pointer_start_screen: Point<f32>,
    pointer_start_world: Point<f32>,
    current_world: Point<f32>,
}

#[derive(Clone, Copy)]
struct CreationPreview {
    object_type: ObjectType,
    geometry: Geometry,
}

enum Interaction {
    None,
    PotentialMove(MoveGesture),
    Moving(MoveGesture),
    PotentialResize(ResizeGesture),
    Resizing(ResizeGesture),
    PotentialCreate(CreateGesture),
    Creating(CreateGesture),
}

impl Interaction {
    fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn restore(&self, document: &mut Document) {
        match self {
            Self::PotentialMove(gesture) | Self::Moving(gesture) => {
                for object in &gesture.objects {
                    document.set_geometry(object.id, object.geometry);
                }
            }
            Self::PotentialResize(gesture) | Self::Resizing(gesture) => {
                document.set_geometry(gesture.object.id, gesture.object.geometry);
            }
            Self::None | Self::PotentialCreate(_) | Self::Creating(_) => {}
        }
    }

    fn preview(&self) -> Option<CreationPreview> {
        let Self::Creating(gesture) = self else {
            return None;
        };
        Some(CreationPreview {
            object_type: gesture.object_type,
            geometry: creation_geometry(gesture.pointer_start_world, gesture.current_world),
        })
    }
}

fn creation_geometry(start: Point<f32>, current: Point<f32>) -> Geometry {
    let bounds = WorldRect::from_points(start, current);
    Geometry {
        position: bounds.min,
        size: size(
            (bounds.max.x - bounds.min.x).max(MIN_OBJECT_SIZE),
            (bounds.max.y - bounds.min.y).max(MIN_OBJECT_SIZE),
        ),
    }
}

/// Does this gesture ignore alignment for its whole duration?
///
/// `⌘` on macOS, `Ctrl` everywhere else — the same key that means "precise" to
/// every product in the corpus. Read once when the gesture starts rather than
/// sampled per movement: a magnet that switches off halfway through a drag is
/// the most disorienting thing a snapping implementation can do, and the user
/// who wants to place something freely says so before they press.
///
/// Split out from the gesture literal so the rule can be tested without a
/// window. It is one boolean and one `||`, but it is a *convention*, and
/// conventions are exactly the things that rot silently.
fn suspends_snap(modifiers: gpui::Modifiers) -> bool {
    modifiers.platform || modifiers.control
}

fn drag_threshold_crossed(start: Point<f32>, current: Point<f32>) -> bool {
    let dx = current.x - start.x;
    let dy = current.y - start.y;
    dx * dx + dy * dy >= DRAG_THRESHOLD * DRAG_THRESHOLD
}

fn movement_delta(camera: Camera, start_world: Point<f32>, screen: Point<f32>) -> Point<f32> {
    let current_world = camera.screen_to_world(screen);
    point(
        current_world.x - start_world.x,
        current_world.y - start_world.y,
    )
}

fn apply_move(document: &mut Document, objects: &[ObjectSnapshot], delta: Point<f32>) {
    for object in objects {
        document.set_position(
            object.id,
            point(
                object.geometry.position.x + delta.x,
                object.geometry.position.y + delta.y,
            ),
        );
    }
}

fn resized_geometry(
    start: ObjectGeometry,
    handle: ResizeHandle,
    delta: Point<f32>,
    proportional: bool,
) -> ObjectGeometry {
    let left = start.position.x;
    let top = start.position.y;
    let right = left + start.size.width;
    let bottom = top + start.size.height;

    // `⇧` preserves the aspect ratio. The pointer's dominant axis decides the
    // scale, which is the same rule the move uses for its dominant axis — one
    // gesture grammar for both, rather than `⇧` meaning something different
    // depending on which handle the pointer happened to grab.
    let delta = if proportional {
        // The pointer picks one axis; that axis's *resulting size* is what the
        // user asked for, and the other axis follows by scale. Scaling the drag
        // vector itself instead would compound the error, because the vector is
        // what already moved the box.
        let width = start.size.width.max(f32::EPSILON);
        let height = start.size.height.max(f32::EPSILON);
        let (driven, other) = if delta.x.abs() >= delta.y.abs() {
            (width, height)
        } else {
            (height, width)
        };
        let requested = driven
            + if delta.x.abs() >= delta.y.abs() {
                delta.x
            } else {
                delta.y
            };
        let scale = (requested / driven).max(MIN_OBJECT_SIZE / other.max(f32::EPSILON));
        point((width * scale) - width, (height * scale) - height)
    } else {
        delta
    };

    let new_left = if handle.moves_left() {
        (left + delta.x).min(right - MIN_OBJECT_SIZE)
    } else {
        left
    };
    let new_right = if handle.moves_right() {
        (right + delta.x).max(new_left + MIN_OBJECT_SIZE)
    } else {
        right
    };
    let new_top = if handle.moves_top() {
        (top + delta.y).min(bottom - MIN_OBJECT_SIZE)
    } else {
        top
    };
    let new_bottom = if handle.moves_bottom() {
        (bottom + delta.y).max(new_top + MIN_OBJECT_SIZE)
    } else {
        bottom
    };

    ObjectGeometry {
        position: point(new_left, new_top),
        size: size(new_right - new_left, new_bottom - new_top),
    }
}

fn apply_resize(
    document: &mut Document,
    gesture: &ResizeGesture,
    camera: Camera,
    screen: Point<f32>,
    proportional: bool,
) {
    let delta = movement_delta(camera, gesture.pointer_start_world, screen);
    let geometry = resized_geometry(gesture.object.geometry, gesture.handle, delta, proportional);
    document.set_geometry(gesture.object.id, geometry);
}

fn geometry_command(document: &Document, snapshots: &[ObjectSnapshot]) -> DocumentCommand {
    let changes = snapshots
        .iter()
        .filter_map(|snapshot| {
            let after = document.geometry(snapshot.id)?;
            (snapshot.geometry != after).then_some(GeometryChange {
                id: snapshot.id,
                before: snapshot.geometry,
                after,
            })
        })
        .collect();
    DocumentCommand::geometry(changes)
}

#[derive(Clone)]
struct MarqueeGesture {
    start: Point<f32>,
    current: Point<f32>,
    additive: bool,
    initial_selection: Vec<ObjectId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextEditState {
    id: ObjectId,
    original_text: String,
    editing_text: String,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    pointer_anchor: Option<usize>,
}

/// The runtime appearance an object had the moment its project was opened.
///
/// Used only to decide what a save needs to write. Keeping it derived state on
/// the view — not in the persistent document — is what lets save answer "did the
/// user change this?" without recording anything new in the document.
#[derive(Clone, Debug, PartialEq)]
struct SourceSnapshot {
    position: Point<f32>,
    /// Where the object sat relative to its parent's origin when it was opened.
    ///
    /// The baseline a flow element's move is measured from. A flow box has no
    /// authored position of its own — the flow decides it — so the editor's only
    /// honest way to express "the user moved this" is as a change from the
    /// position the author wrote, not as an absolute coordinate.
    parent_relative: Point<f32>,
    size: Size<f32>,
    text: Option<String>,
    /// How the object looked when it was opened, so save can tell a user edit
    /// from an authored value. The same shape undo restores, which keeps the two
    /// paths from drifting apart.
    appearance: Appearance,
}

/// Component-wise difference between two points.
///
/// Used to express a world position relative to a containing block, which is
/// the coordinate system an authored `left`/`top` lives in.
fn sub_point(a: Point<f32>, b: Point<f32>) -> Point<f32> {
    point(a.x - b.x, a.y - b.y)
}

/// Format a colour the way source spells it.
///
/// Written back into CSS as `#rrggbb`. Alpha is not supported by the style
/// module, so a colour with one is left to the reader rather than written as a
/// value that means something narrower than what the editor holds.
fn css_color(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

pub struct CanvasView {
    camera: Camera,
    /// The single mutation and history boundary for the live editor.
    ///
    /// This used to be two fields here: `document: Document` and
    /// `history: History`, which meant the canvas owned a runtime document
    /// and a history stack beside it while the persistent document and the
    /// semantic operation layer sat unused. Now every committed canvas edit
    /// goes through `EditSession::execute`, so the live path is
    /// canvas -> EditSession -> SemanticOperation -> SemanticHistory.
    ///
    /// No project is loaded yet, so `session.document` is empty metadata.
    /// That is the honest state: there is no `lamine.yaml` behind the starter
    /// scene. When project opening lands, that document is populated here and
    /// metadata operations become reachable from the same stack.
    session: EditSession,
    selection: Selection,
    pan: Option<PanGesture>,
    interaction: Interaction,
    marquee: Option<MarqueeGesture>,
    tool: Tool,
    space_held: bool,
    hitbox: Rc<Cell<Option<CanvasHitbox>>>,
    focus_handle: Option<FocusHandle>,
    text_edit: Option<TextEditState>,
    #[cfg(debug_assertions)]
    workload_started: bool,
    #[cfg(debug_assertions)]
    workload_running: bool,
    /// Where the open project lives on disk, if one is open.
    ///
    /// Kept beside the document rather than inside it: the persistent document
    /// describes content, not where it came from.
    project_root: Option<std::path::PathBuf>,
    /// What each loaded object looked like when the project was opened.
    ///
    /// Save diffs against this. Without it, saving an untouched project would
    /// rewrite every element with absolute geometry — the file would change on
    /// disk for a session in which the user did nothing, which is exactly the
    /// behaviour source-preserving save exists to prevent.
    source_snapshot: BTreeMap<NodeId, SourceSnapshot>,
    /// `⇧` is held, so this movement is constrained to one axis.
    ///
    /// Sampled per pointer movement rather than at press time, which is the
    /// opposite of the snap suspension on purpose: a user presses `⇧` halfway
    /// through a drag to straighten it, and expects it to take effect there and
    /// then. It is reset the moment the gesture ends.
    constrain_drag: bool,
    /// The object under the pointer, when the pointer is over one.
    ///
    /// Runtime only, like everything else about hover: not in the document, not
    /// in history, not restored by undo. Cleared whenever the pointer stops
    /// being over an object, so a stale outline can never outlive the cursor.
    hovered: Option<ObjectId>,
    /// Alignment lines currently holding, one per axis at most.
    ///
    /// Pure runtime feedback: they are not in the document, not in history, and
    /// they are cleared the moment nothing is moving. A snap the user cannot
    /// see is the same as an object that jumped for no reason, so the guide is
    /// part of the interaction, not a decoration added afterwards.
    snap_guides: Vec<snap::Guide>,
    /// Debug-only: true pre-gesture positions for the runtime history probe.
    #[cfg(debug_assertions)]
    drag_start_positions: Vec<(f32, f32)>,
}

/// Turn a changed appearance into one source edit per property that moved.
///
/// Only what actually changed becomes an edit. A change the editor cannot
/// express in the supported CSS subset — removing a fill, clearing an authored
/// text colour — produces no edit at all rather than a guessed one.
fn style_edits(
    node: &NodeId,
    before: &Appearance,
    after: &Appearance,
) -> Vec<crate::project_save::SourceEdit> {
    let mut edits = Vec::new();
    let mut push = |property: &str, value: String| {
        edits.push(crate::project_save::SourceEdit::Style {
            node: node.clone(),
            property: property.to_owned(),
            value,
        });
    };

    if before.style.fill != after.style.fill {
        if let Some(fill) = after.style.fill {
            push("background-color", css_color(fill.color));
        }
    }
    if before.style.stroke != after.style.stroke {
        if let Some(stroke) = after.style.stroke {
            push(
                "border",
                format!(
                    "{}px {}",
                    crate::project_save::css_length(stroke.width),
                    css_color(stroke.color)
                ),
            );
        }
    }
    if before.text_color != after.text_color {
        if let Some(color) = after.text_color {
            push("color", css_color(color));
        }
    }
    if before.font_size != after.font_size {
        if let Some(size) = after.font_size {
            push(
                "font-size",
                format!("{}px", crate::project_save::css_length(size)),
            );
        }
    }
    if before.style.border_radius != after.style.border_radius {
        push(
            "border-radius",
            format!(
                "{}px",
                crate::project_save::css_length(after.style.border_radius)
            ),
        );
    }
    if before.style.opacity != after.style.opacity {
        push(
            "opacity",
            crate::project_save::css_length(after.style.opacity),
        );
    }
    edits
}

impl CanvasView {
    pub(crate) fn workload_controls_input(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            self.workload_running
        }
        #[cfg(not(debug_assertions))]
        {
            false
        }
    }

    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            session: EditSession::new(PersistentDocument::default(), Document::default()),
            selection: Selection::default(),
            pan: None,
            interaction: Interaction::None,
            marquee: None,
            tool: Tool::Select,
            space_held: false,
            hitbox: Rc::new(Cell::new(None)),
            focus_handle: None,
            text_edit: None,
            #[cfg(debug_assertions)]
            workload_started: false,
            #[cfg(debug_assertions)]
            workload_running: false,
            project_root: None,
            source_snapshot: BTreeMap::new(),
            constrain_drag: false,
            hovered: None,
            snap_guides: Vec::new(),
            #[cfg(debug_assertions)]
            drag_start_positions: Vec::new(),
        }
    }

    pub fn new_with_context(cx: &mut Context<Self>) -> Self {
        let mut view = Self::new();
        view.focus_handle = Some(cx.focus_handle());
        #[cfg(debug_assertions)]
        view.install_workload_fixture();
        view
    }

    /// Replace this canvas's contents with a source-backed project.
    ///
    /// This is the seam between opening a project and editing it. The canvas
    /// takes the already-derived runtime document plus the canonical persistent
    /// document and does no parsing itself, so there is exactly one place in
    /// the editor that knows how a project becomes a scene.
    ///
    /// History is reset deliberately: the new document has no relationship to
    /// whatever was open before, and replaying an old operation against it
    /// would be meaningless rather than merely stale.
    pub fn load_project(&mut self, loaded: crate::project_open::LoadedProject) {
        self.project_root = Some(loaded.root.clone());
        self.session = EditSession::new(loaded.document, loaded.runtime);
        // Snapshot the runtime state before any edit, so save can tell an
        // authored value from a change the user made in this session. Taken
        // after the session exists because the snapshot needs each object's
        // parent origin, which is resolved through the runtime.
        self.source_snapshot = self.opened_state();
        // A freshly loaded document has no selection, and keeping stale ids
        // would let hit-testing and layers refer to objects that no longer
        // exist.
        self.selection = Selection::default();
        self.interaction = Interaction::None;
        self.marquee = None;
        self.text_edit = None;
        // Projected objects are laid out by the projection's own placeholder
        // rule, so fitting the camera is what actually brings them on screen.
        self.camera.fit();
    }

    /// The runtime state the project was opened with, for every managed object.
    ///
    /// Derived state on the view, never in the document: this is how save
    /// answers "did the user change this?" without recording anything new
    /// anywhere.
    fn opened_state(&self) -> BTreeMap<NodeId, SourceSnapshot> {
        self.session
            .runtime
            .objects()
            .iter()
            .map(|object| {
                let parent_origin = self.parent_origin(&object.spool_id);
                (
                    object.spool_id.clone(),
                    SourceSnapshot {
                        position: object.position,
                        parent_relative: sub_point(object.position, parent_origin),
                        size: object.size,
                        text: object.text_content.clone(),
                        appearance: Appearance {
                            style: ObjectStyle {
                                fill: object.fill,
                                stroke: object.stroke,
                                border_radius: object.border_radius,
                                opacity: object.opacity,
                            },
                            text_color: object.text_color,
                            font_size: object.font_size,
                        },
                    },
                )
            })
            .collect()
    }

    /// Where a node's containing block starts, in world coordinates.
    ///
    /// Zero for a root, because a root's containing block is the page. Resolved
    /// through the persistent document rather than the runtime object list so
    /// it is the authored hierarchy that decides, not the order things happen
    /// to be drawn in.
    fn parent_origin(&self, node: &NodeId) -> Point<f32> {
        let parent = self
            .session
            .document
            .structure
            .nodes
            .iter()
            .find(|candidate| &candidate.id == node)
            .and_then(|candidate| candidate.parent.clone());
        let Some(parent) = parent else {
            return point(0.0, 0.0);
        };
        let object = self
            .session
            .runtime
            .objects()
            .iter()
            .find(|object| object.spool_id == parent);
        object
            .and_then(|object| self.session.runtime.geometry(object.id))
            .map(|geometry| geometry.position)
            .unwrap_or(point(0.0, 0.0))
    }

    /// Write the current editor state back to the project's authored source.
    ///
    /// This is the save half of the product loop. It diffs each managed object
    /// against [`Self::source_snapshot`] and emits one [`SourceEdit`] per real
    /// change, and the save layer turns each into the smallest authored edit it
    /// can:
    ///
    /// - geometry becomes a `style` attribute on the element: a size outright,
    ///   and a position as `left`/`top` for a box already out of the flow or
    ///   `transform: translate(...)` for one that is in it
    /// - text replaces the element's authored text
    /// - style rewrites the CSS declaration that already owns the property
    /// - metadata is written by the existing bundle writer
    ///
    /// Objects created during the session have no authored element yet, so they
    /// are reported as unsupported rather than silently dropped. That is a real
    /// limitation of this milestone, not a silent success.
    ///
    /// Returns the outcome, including what could not be written.
    pub fn save_project(
        &self,
    ) -> Result<crate::project_save::SaveOutcome, crate::project_bundle::BundleError> {
        let mut outcome = crate::project_save::SaveOutcome::default();
        let Some(root) = self.project_root.as_deref() else {
            // No project open: nothing to write, and nothing invented.
            return Ok(outcome);
        };
        let mut edits = Vec::new();
        for object in self.session.runtime.objects() {
            // Only objects that came from authored source can be written back.
            let bound = self
                .session
                .document
                .structure
                .nodes
                .iter()
                .any(|node| node.id == object.spool_id);
            if !bound {
                outcome
                    .unsupported
                    .push(crate::project_save::UnsupportedEdit {
                        node: object.spool_id.clone(),
                        kind: "object",
                        reason: "created in this session, so it has no authored element yet".into(),
                    });
                continue;
            }
            // No snapshot means this object was not the one that was loaded —
            // an object recreated under the same identity. Writing it would be a
            // guess, so it is reported instead.
            let Some(before) = self.source_snapshot.get(&object.spool_id) else {
                outcome
                    .unsupported
                    .push(crate::project_save::UnsupportedEdit {
                        node: object.spool_id.clone(),
                        kind: "object",
                        reason: "no opened state to compare against".into(),
                    });
                continue;
            };

            let geometry = self.session.runtime.geometry(object.id);
            if let Some(geometry) = geometry {
                // Only what actually changed. Writing an unchanged size back
                // would pin the element to an explicit box the author never
                // wrote, which stops following the stylesheet from then on.
                let moved = before.position != geometry.position;
                let resized = before.size != geometry.size;
                if moved || resized {
                    let parent_origin = self.parent_origin(&object.spool_id);
                    let placement = moved.then(|| {
                        let Some(structure) = self
                            .session
                            .document
                            .structure
                            .nodes
                            .iter()
                            .find(|node| node.id == object.spool_id)
                        else {
                            return crate::project_save::Placement::Flow { dx: 0.0, dy: 0.0 };
                        };
                        // Already out of the flow: keep it that way and write a
                        // position from the containing block, which is the
                        // parent element. The editor works in world
                        // coordinates, so a child has to be written relative to
                        // where its parent now sits or it would jump on reopen.
                        if crate::project_save::is_out_of_flow(&self.session.document, structure)
                            .unwrap_or(false)
                        {
                            return crate::project_save::Placement::ContainingBlock {
                                x: geometry.position.x - parent_origin.x,
                                y: geometry.position.y - parent_origin.y,
                            };
                        }
                        // In the flow: the authored position is wherever the
                        // flow puts this element, so the edit is the change from
                        // that — never an absolute coordinate, which would
                        // delete the author's layout. Measured against the
                        // parent origin *as it is now*, so a child that only
                        // moved because its parent did writes nothing: in the
                        // flow a child's position is its parent's business.
                        let now = sub_point(geometry.position, parent_origin);
                        crate::project_save::Placement::Flow {
                            dx: now.x - before.parent_relative.x,
                            dy: now.y - before.parent_relative.y,
                        }
                    });
                    edits.push(crate::project_save::SourceEdit::Geometry {
                        node: object.spool_id.clone(),
                        placement,
                        width: resized.then_some(geometry.size.width),
                        height: resized.then_some(geometry.size.height),
                    });
                }
            }
            let text = object.text_content.as_ref().filter(|t| !t.is_empty());
            if let Some(text) = text.filter(|text| Some(*text) != before.text.as_ref()) {
                edits.push(crate::project_save::SourceEdit::Text {
                    node: object.spool_id.clone(),
                    text: text.clone(),
                });
            }
            let now = Appearance {
                style: ObjectStyle {
                    fill: object.fill,
                    stroke: object.stroke,
                    border_radius: object.border_radius,
                    opacity: object.opacity,
                },
                text_color: object.text_color,
                font_size: object.font_size,
            };
            edits.extend(style_edits(&object.spool_id, &before.appearance, &now));
        }
        let mut written = crate::project_save::save_project(root, &self.session.document, &edits)?;
        written.unsupported.append(&mut outcome.unsupported);
        Ok(written)
    }

    /// The live commit path for every canvas mutation.
    ///
    /// One call is one history entry, however many objects it touched. The
    /// command is normalized first so a gesture that changed nothing, or that
    /// changed only some of a multi-selection, records only the real change.
    /// The canvas has already applied the mutation eagerly, and replaying the
    /// command forward here is safe because every replay is idempotent.
    fn commit(&mut self, command: DocumentCommand) -> bool {
        self.session
            .execute(SemanticOperation::Runtime(command.normalized()))
            .unwrap_or(false)
    }

    pub fn is_text_editing(&self) -> bool {
        self.text_edit.is_some()
    }

    /// The object under a world point that has text a user would expect to
    /// edit.
    ///
    /// Not "anything typed as text": a source-backed `<a class="cta">` is
    /// declared as a frame in `lamine.yaml` yet carries a label, and refusing to
    /// edit it would make a correctly loaded project uneditable. The rule is
    /// what the user sees — a non-empty text run — and it is the same rule the
    /// renderer draws by.
    fn editable_text_at(&self, world: Point<f32>) -> Option<ObjectId> {
        let id = self.session.runtime.hit_test(world)?;
        self.session
            .runtime
            .object(id)
            .filter(|object| {
                object
                    .text_content
                    .as_deref()
                    .is_some_and(|text| !text.is_empty())
            })
            .map(|object| object.id)
    }

    fn begin_text_edit(&mut self, id: ObjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.session.runtime.text_content(id).map(str::to_owned) else {
            return;
        };
        let end = text.len();
        self.selection.click(Some(id), false);
        self.interaction = Interaction::None;
        self.text_edit = Some(TextEditState {
            id,
            original_text: text.clone(),
            editing_text: text,
            selected_range: end..end,
            selection_reversed: false,
            marked_range: None,
            pointer_anchor: None,
        });
        if let Some(focus_handle) = &self.focus_handle {
            window.focus(focus_handle, cx);
        }
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    pub fn cancel_text_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let cancelled = self.discard_text_edit();
        if cancelled {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        cancelled
    }

    fn discard_text_edit(&mut self) -> bool {
        self.text_edit.take().is_some()
    }

    pub fn commit_text_edit_session(&mut self, cx: &mut Context<Self>) -> bool {
        let committed = self.commit_text_edit();
        if committed {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        committed
    }

    fn commit_text_edit(&mut self) -> bool {
        let Some(edit) = self.text_edit.take() else {
            return false;
        };
        if edit.original_text == edit.editing_text {
            return false;
        }
        if !self
            .session
            .runtime
            .set_text_content(edit.id, edit.editing_text.clone())
        {
            return false;
        }
        self.commit(DocumentCommand::text(vec![TextChange {
            id: edit.id,
            before: edit.original_text,
            after: edit.editing_text,
        }]));
        true
    }

    fn edit_cursor(&self) -> Option<usize> {
        let edit = self.text_edit.as_ref()?;
        Some(if edit.selection_reversed {
            edit.selected_range.start
        } else {
            edit.selected_range.end
        })
    }

    fn set_text_selection(&mut self, anchor: usize, caret: usize, cx: &mut Context<Self>) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let (range, reversed) = selection_from_anchor_and_caret(&edit.editing_text, anchor, caret);
        edit.selected_range = range;
        edit.selection_reversed = reversed;
        edit.marked_range = None;
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    fn move_text_cursor(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let offset = utf8_boundary(&edit.editing_text, offset.min(edit.editing_text.len()));
        edit.selected_range = offset..offset;
        edit.selection_reversed = false;
        edit.marked_range = None;
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    fn select_text_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let offset = utf8_boundary(&edit.editing_text, offset.min(edit.editing_text.len()));
        if edit.selection_reversed {
            edit.selected_range.start = offset;
        } else {
            edit.selected_range.end = offset;
        }
        if edit.selected_range.end < edit.selected_range.start {
            edit.selection_reversed = !edit.selection_reversed;
            edit.selected_range = edit.selected_range.end..edit.selected_range.start;
        }
        edit.marked_range = None;
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    fn replace_editing_text(
        &mut self,
        range: Option<Range<usize>>,
        replacement: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let range = range
            .map(|range| utf16_range_to_utf8(&edit.editing_text, range))
            .or_else(|| edit.marked_range.clone())
            .unwrap_or_else(|| edit.selected_range.clone());
        let start = utf8_boundary(&edit.editing_text, range.start);
        let end = utf8_boundary(&edit.editing_text, range.end);
        edit.editing_text.replace_range(start..end, replacement);
        let cursor = start + replacement.len();
        edit.selected_range = cursor..cursor;
        edit.selection_reversed = false;
        edit.marked_range = None;
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    fn text_backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            if edit.selected_range.is_empty() {
                let previous = previous_char_boundary(&edit.editing_text, cursor);
                self.text_edit.as_mut().unwrap().selected_range = previous..cursor;
            }
            self.replace_editing_text(None, "", cx);
        } else {
            window.play_system_bell();
        }
    }

    fn text_delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            if edit.selected_range.is_empty() {
                let next = next_char_boundary(&edit.editing_text, cursor);
                self.text_edit.as_mut().unwrap().selected_range = cursor..next;
            }
            self.replace_editing_text(None, "", cx);
        } else {
            window.play_system_bell();
        }
    }

    fn text_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            let offset = if edit.selected_range.is_empty() {
                previous_char_boundary(&edit.editing_text, cursor)
            } else {
                edit.selected_range.start
            };
            self.move_text_cursor(offset, cx);
        }
    }

    fn text_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            let offset = if edit.selected_range.is_empty() {
                next_char_boundary(&edit.editing_text, cursor)
            } else {
                edit.selected_range.end
            };
            self.move_text_cursor(offset, cx);
        }
    }

    fn text_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            self.select_text_to(previous_char_boundary(&edit.editing_text, cursor), cx);
        }
    }

    fn text_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(edit), Some(cursor)) = (self.text_edit.as_ref(), self.edit_cursor()) {
            self.select_text_to(next_char_boundary(&edit.editing_text, cursor), cx);
        }
    }

    fn text_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = self.text_edit.as_mut() {
            edit.selected_range = 0..edit.editing_text.len();
            edit.selection_reversed = false;
            edit.marked_range = None;
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
    }

    fn text_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_text_cursor(0, cx);
    }

    fn text_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &self.text_edit {
            self.move_text_cursor(edit.editing_text.len(), cx);
        }
    }

    fn text_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_editing_text(None, &text, cx);
        }
    }

    fn text_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &self.text_edit {
            if !edit.selected_range.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    edit.editing_text[edit.selected_range.clone()].to_string(),
                ));
            }
        }
    }

    fn text_cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &self.text_edit {
            if !edit.selected_range.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    edit.editing_text[edit.selected_range.clone()].to_string(),
                ));
                self.replace_editing_text(None, "", cx);
            }
        }
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn layer_structure_revision(&self) -> u64 {
        self.session.runtime.layer_structure_revision()
    }

    pub fn document_objects(&self) -> &[DesignObject] {
        self.session.runtime.objects()
    }

    /// The canonical, source-backed document.
    ///
    /// Read-only on purpose: metadata is durable and is changed through
    /// semantic operations, never by poking at it from a view.
    pub fn persistent_document(&self) -> &PersistentDocument {
        &self.session.document
    }

    /// The disposable runtime document the editor draws and hit-tests.
    ///
    /// Exposed so a project can be checked against what the editor actually
    /// holds, rather than against a separate copy.
    // Test-only today: the open-path tests use it to assert that a loaded
    // project is really hittable and that an unsupported kind is really absent.
    // The editor reads `document_objects` instead. Compiled out of the binary
    // rather than silenced, so it cannot drift into being a second accessor the
    // editor depends on.
    #[cfg(test)]
    pub fn runtime_document(&self) -> &Document {
        &self.session.runtime
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.commit_text_edit();
        self.interaction.restore(&mut self.session.runtime);
        self.tool = tool;
        self.interaction = Interaction::None;
        self.marquee = None;
        self.clear_gesture_feedback();
    }

    /// Undo the last committed semantic operation.
    ///
    /// This is the whole live undo path minus repainting, split out so the
    /// integration tests can drive it without a GPUI window. Everything that
    /// decides *what* changes happens here; [`CanvasView::undo`] only adds the
    /// notification.
    fn undo_history(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.session.runtime);
            self.interaction = Interaction::None;
        }
        let changed = self.session.undo().unwrap_or(false);
        if changed {
            self.retain_existing_selection();
            self.refresh_object_names();
        }
        changed
    }

    /// Redo the last undone operation. See [`CanvasView::undo_history`].
    fn redo_history(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.session.runtime);
            self.interaction = Interaction::None;
        }
        let changed = self.session.redo().unwrap_or(false);
        if changed {
            self.retain_existing_selection();
            self.refresh_object_names();
        }
        changed
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        let changed = self.undo_history();
        if changed {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        changed
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        let changed = self.redo_history();
        if changed {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        changed
    }

    pub fn selected_objects(&self) -> Vec<DesignObject> {
        self.selection
            .ids()
            .iter()
            .filter_map(|id| self.session.runtime.object(*id).cloned())
            .collect()
    }

    pub fn set_selected_style(&mut self, edit: StyleEdit, cx: &mut Context<Self>) -> bool {
        self.commit_text_edit();
        let had_interaction = self.interaction.is_active();
        let changed = self.apply_selected_style(edit);
        if changed || had_interaction {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        changed
    }

    fn apply_selected_style(&mut self, edit: StyleEdit) -> bool {
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.session.runtime);
            self.interaction = Interaction::None;
        }
        let changes: Vec<_> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| {
                let before = self.session.runtime.appearance(*id)?;
                let after = edited_style(before, edit);
                (before != after).then_some(StyleChange {
                    id: *id,
                    before,
                    after,
                })
            })
            .collect();
        if changes.is_empty() {
            return false;
        }
        for change in &changes {
            self.session.runtime.set_appearance(change.id, change.after);
        }
        self.commit(DocumentCommand::style(changes));
        true
    }

    /// Replace one object's text as a single semantic operation.
    ///
    /// The same command a committed caret edit produces, for callers that
    /// already know the text. It goes through the same history boundary rather
    /// than around it: one call is one undo step, undo restores the exact
    /// previous run of text, and redo re-applies this one. Used by the runtime
    /// probe and by anything that needs to set text without a caret.
    #[cfg(any(test, debug_assertions))]
    pub fn set_object_text(&mut self, id: ObjectId, text: String) -> bool {
        self.commit_text_edit();
        let Some(before) = self.session.runtime.text_content(id).map(str::to_owned) else {
            return false;
        };
        if before == text {
            return false;
        }
        let Some(change) = self
            .session
            .runtime
            .set_text_content(id, text.clone())
            .then_some(TextChange {
                id,
                before,
                after: text,
            })
        else {
            return false;
        };
        self.commit(DocumentCommand::text(vec![change]));
        true
    }

    /// One object's current geometry.
    ///
    /// Read through this rather than reaching into the document from the shell:
    /// the Inspector shows what the runtime holds, which is the same value the
    /// canvas draws and the same one save diffs against.
    pub fn object_geometry(&self, id: ObjectId) -> Option<Geometry> {
        self.session.runtime.geometry(id)
    }

    /// Set one object's geometry as a single semantic operation.
    ///
    /// The Inspector's way of asking the same question a canvas drag asks. It
    /// goes through the same history boundary instead of writing geometry
    /// beside it, so one call is one undo step, a value that lands where it
    /// started records nothing, and the document stays the only place a new
    /// position lives.
    pub fn set_object_geometry(&mut self, id: ObjectId, geometry: Geometry) -> bool {
        self.commit_text_edit();
        let Some(before) = self.session.runtime.geometry(id) else {
            return false;
        };
        self.commit_geometry_change(id, before, geometry)
    }

    /// Start an Inspector geometry change that is not recorded yet.
    ///
    /// Returns the geometry the object had, which the caller has to hand back
    /// on commit or cancel: while a continuous control is being dragged the
    /// runtime already shows the new value, and only this remembers where it
    /// started.
    pub fn begin_geometry_scrub(&mut self, id: ObjectId) -> Option<GeometryScrub> {
        self.commit_text_edit();
        let before = self.session.runtime.geometry(id)?;
        Some(GeometryScrub { id, before })
    }

    /// Move a scrubbing object without recording anything.
    pub fn scrub_geometry(&mut self, scrub: &GeometryScrub, geometry: Geometry) -> bool {
        self.session.runtime.set_geometry(scrub.id, geometry)
    }

    /// Finish a scrub as one semantic operation.
    ///
    /// A scrub that ended where it started records nothing, exactly as a canvas
    /// drag that returns to its origin does.
    pub fn commit_geometry_scrub(&mut self, scrub: GeometryScrub) -> bool {
        let Some(after) = self.session.runtime.geometry(scrub.id) else {
            return false;
        };
        self.commit_geometry_change(scrub.id, scrub.before, after)
    }

    /// Abandon a scrub: the runtime goes back to where it started and nothing is
    /// recorded.
    pub fn cancel_geometry_scrub(&mut self, scrub: GeometryScrub) -> bool {
        self.session.runtime.set_geometry(scrub.id, scrub.before)
    }

    fn commit_geometry_change(&mut self, id: ObjectId, before: Geometry, after: Geometry) -> bool {
        if before == after {
            return false;
        }
        self.session.runtime.set_geometry(id, after);
        self.commit(DocumentCommand::geometry(vec![GeometryChange {
            id,
            before,
            after,
        }]));
        true
    }

    /// Rename a source-backed object, as one semantic operation.
    ///
    /// Goes through [`crate::operations::rename_node_in`] rather than writing a
    /// name into the document beside the history, so a rename is the same kind
    /// of undo step as a move and lands in the metadata file on save like every
    /// other structural edit. The runtime object's name is refreshed from the
    /// document afterwards, because the document is the authority and the
    /// runtime only ever mirrors it.
    ///
    /// A created object has no node to rename yet, so nothing is recorded.
    pub fn rename_object(&mut self, id: ObjectId, name: String) -> bool {
        self.commit_text_edit();
        let Some(object) = self.session.runtime.object(id) else {
            return false;
        };
        let node = object.spool_id.clone();
        let current = object.name.clone();
        if current == name {
            return false;
        }
        let Ok(operation) =
            crate::operations::rename_node_in(&self.session.document, node.clone(), name)
        else {
            return false;
        };
        let changed = self.session.execute(operation).unwrap_or(false);
        if !changed {
            return false;
        }
        // The runtime carries a copy of the name so the Inspector and the layers
        // list can read it without walking the persistent document.
        let renamed = self
            .session
            .document
            .structure
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .map(|candidate| candidate.name.clone());
        if let Some(name) = renamed {
            self.session.runtime.set_object_name(id, name);
        }
        true
    }

    /// Re-mirror the document's node names onto the runtime objects.
    ///
    /// The persistent document is the authority for a name; the runtime copy
    /// exists so the Inspector and the layers list can read one without walking
    /// the document. Anything that can change a name has to refresh that copy —
    /// a rename, an undo, a redo — or the copy quietly becomes a second name
    /// store that disagrees with the one that saves.
    fn refresh_object_names(&mut self) {
        let names: Vec<(NodeId, String)> = self
            .session
            .document
            .structure
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.name.clone()))
            .collect();
        for object in self.session.runtime.objects().to_vec() {
            if let Some((_, name)) = names.iter().find(|(id, _)| *id == object.spool_id) {
                self.session
                    .runtime
                    .set_object_name(object.id, name.clone());
            }
        }
    }

    pub fn select_object(&mut self, id: ObjectId, additive: bool, cx: &mut Context<Self>) {
        self.commit_text_edit();
        self.selection.click(Some(id), additive);
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.commit_text_edit();
        let had_marquee = self.marquee.take().is_some();
        if !self.selection.is_empty() {
            self.selection.click(None, false);
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        } else if had_marquee {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
    }

    pub fn zoom_percent(&self) -> u32 {
        (self.camera.zoom * 100.0).round() as u32
    }

    pub fn set_zoom_percent(&mut self, zoom_percent: u32) {
        self.camera.set_zoom_at_center(zoom_percent as f32 / 100.0);
    }

    /// Nudge every selected object by a world-space delta, as one operation.
    ///
    /// Arrow keys, `⇧` for ten times the distance. One press is one history
    /// entry, which is what makes holding an arrow key cheap to undo: the user
    /// presses it four times and presses undo once, not the other way round.
    ///
    /// Deliberately *not* snapped. An arrow key states a position; it does not
    /// ask where the object should be. More concretely, snapping is a magnet
    /// within eight screen pixels, so a nudge of an object that happens to
    /// share a neighbour's edge would be pulled straight back and the key
    /// press would do nothing at all. Every product in the corpus treats the
    /// arrow keys as a direct set for exactly that reason.
    pub fn nudge_selection(&mut self, dx: f32, dy: f32) -> bool {
        self.commit_text_edit();
        let ids = self.selection.ids().to_vec();
        let mut changes = Vec::new();
        for id in &ids {
            if let Some(before) = self.session.runtime.geometry(*id) {
                changes.push((*id, before));
            }
        }
        if changes.is_empty() {
            return false;
        }
        let recorded: Vec<GeometryChange> = changes
            .iter()
            .filter_map(|(id, before)| {
                let after = Geometry {
                    position: point(before.position.x + dx, before.position.y + dy),
                    size: before.size,
                };
                (after != *before).then_some(GeometryChange {
                    id: *id,
                    before: *before,
                    after,
                })
            })
            .collect();
        if recorded.is_empty() {
            return false;
        }
        for change in &recorded {
            self.session.runtime.set_geometry(change.id, change.after);
        }
        self.commit(DocumentCommand::geometry(recorded));
        true
    }

    /// Move a gesture's objects to where the pointer is, snapping unless the
    /// gesture suspended it.
    ///
    /// The one place a drag's geometry is computed. Updating and releasing both
    /// come through here, which is what makes the object land in the same place
    /// whether the user lets go mid-drag or drops it on the final pixel — a
    /// release that re-derives its own position is how objects used to jump by
    /// a snap width on mouse-up.
    fn drag_gesture_objects(&mut self, gesture: &MoveGesture, screen: Point<f32>) {
        let raw = movement_delta(self.camera, gesture.pointer_start_world, screen);
        let raw = if self.constrain_drag {
            // `⇧` constrains a move to one axis, on whichever the pointer has
            // travelled furthest. Figma, tldraw and Affinity all resolve the
            // same way, and the rule is not "horizontal or vertical" but "the
            // one you clearly meant" — so it is the dominant component that
            // survives, not a fixed preference.
            if raw.x.abs() >= raw.y.abs() {
                point(raw.x, 0.0)
            } else {
                point(0.0, raw.y)
            }
        } else {
            raw
        };
        let (dx, dy) = if gesture.suspend_snap {
            self.snap_guides.clear();
            (raw.x, raw.y)
        } else {
            // The bounds come from the gesture's own snapshots, never from the
            // live geometry. Reading live geometry would make every pointer
            // movement after the first add the whole drag again on top of the
            // position the previous movement already produced — so the object
            // would accelerate away from the pointer, and the snap would be
            // measured from a place the user never dragged it to.
            let bounds = snap::bounds_of(
                &gesture
                    .objects
                    .iter()
                    .map(|object| {
                        snap::Rect::new(
                            object.geometry.position.x,
                            object.geometry.position.y,
                            object.geometry.size.width,
                            object.geometry.size.height,
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            match bounds {
                Some(bounds) => {
                    let ids: Vec<ObjectId> = gesture.objects.iter().map(|o| o.id).collect();
                    let (delta, _) = self.snap_delta(bounds, &ids, (raw.x, raw.y));
                    (delta.0, delta.1)
                }
                None => (raw.x, raw.y),
            }
        };
        apply_move(&mut self.session.runtime, &gesture.objects, point(dx, dy));
    }

    /// Snap a proposed world-space translation of `moving` against everything
    /// else on the canvas, and remember the guides that explain the result.
    ///
    /// The whole selection is one rectangle: a multi-selection moves as a unit
    /// and snaps as a unit, because snapping each object independently makes a
    /// group fly apart. `suspended` is the `⌘`/`Ctrl` override, which every
    /// product in the corpus documents as "place it freely this once".
    fn snap_delta(
        &mut self,
        bounds: snap::Rect,
        moving: &[ObjectId],
        delta: (f32, f32),
    ) -> ((f32, f32), Vec<snap::Guide>) {
        if moving.is_empty() {
            self.snap_guides.clear();
            return (delta, Vec::new());
        }
        let targets = self.snap_targets(moving);
        let snapped = snap::snap_translation(bounds, delta, &targets, self.camera.zoom);
        self.snap_guides = snapped.guides.clone();
        (snapped.delta, snapped.guides)
    }

    /// Every rectangle a moving selection may snap to.
    ///
    /// An object is never a target for itself, and neither is anything inside
    /// it: a child's edges move with the parent, so aligning to them would
    /// fight the move instead of explaining it.
    fn snap_targets(&self, moving: &[ObjectId]) -> Vec<snap::Rect> {
        let moving_nodes: Vec<&NodeId> = moving
            .iter()
            .filter_map(|id| self.session.runtime.object(*id))
            .map(|object| &object.spool_id)
            .collect();
        self.snap_rects_all()
            .into_iter()
            .filter(|(_rect, node)| {
                !moving_nodes
                    .iter()
                    .any(|candidate| self.is_within(node, candidate))
            })
            .map(|(rect, _)| rect)
            .collect()
    }

    /// Every drawn object as a rectangle plus the document node it came from.
    fn snap_rects_all(&self) -> Vec<(snap::Rect, &NodeId)> {
        self.session
            .runtime
            .objects()
            .iter()
            .map(|object| {
                (
                    snap::Rect::new(
                        object.position.x,
                        object.position.y,
                        object.size.width,
                        object.size.height,
                    ),
                    &object.spool_id,
                )
            })
            .collect()
    }

    /// Is `node` inside `ancestor`, at any depth?
    fn is_within(&self, node: &NodeId, ancestor: &NodeId) -> bool {
        let mut cursor = Some(node.clone());
        while let Some(current) = cursor {
            if &current == ancestor {
                return true;
            }
            cursor = self
                .session
                .document
                .structure
                .nodes
                .iter()
                .find(|candidate| candidate.id == current)
                .and_then(|candidate| candidate.parent.clone());
        }
        false
    }

    /// The alignment lines currently holding, for the renderer.
    #[cfg(any(test, debug_assertions))]
    pub fn snap_guides(&self) -> &[snap::Guide] {
        &self.snap_guides
    }

    /// Frame whatever is in the document, or the placeholder scene when the
    /// document is empty.
    ///
    /// This is what `⇧1` means in every product in the corpus, and it is what a
    /// user means by "zoom to fit": show me my document. It is also what
    /// `load_project` calls, so opening a project frames the project.
    pub fn fit_canvas(&mut self) -> bool {
        match WorldRect::around(self.session.runtime.objects()) {
            Some(bounds) => self.camera.fit_bounds(bounds),
            None => self.camera.fit(),
        }
        true
    }

    /// Frame the current selection — `⇧2` in Figma.
    ///
    /// Falls back to fitting everything when nothing is selected, because
    /// zooming to nothing has no meaning and silently doing nothing is worse.
    pub fn zoom_to_selection(&mut self) -> bool {
        let selected: Vec<DesignObject> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.session.runtime.object(*id).cloned())
            .collect();
        let Some(bounds) = WorldRect::around(&selected) else {
            return self.fit_canvas();
        };
        self.camera.fit_bounds(bounds);
        true
    }

    /// Zoom to 100% — `⇧0` in Figma, `⌘0` in Canva.
    ///
    /// Anchored on the selection when there is one, because that is the thing
    /// the user is looking at; otherwise on the viewport centre.
    pub fn zoom_to_actual_size(&mut self) -> bool {
        let anchor = self.selection_bounds_screen();
        match anchor {
            Some(screen) => self.camera.set_zoom_at(1.0, screen),
            None => self.camera.set_zoom_at_center(1.0),
        }
        true
    }

    /// Where on screen the selection's centre is, if anything is selected.
    fn selection_bounds_screen(&self) -> Option<Point<f32>> {
        let selected: Vec<DesignObject> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.session.runtime.object(*id).cloned())
            .collect();
        let center = WorldRect::around(&selected)?.center();
        Some(self.camera.world_to_screen(center))
    }

    /// Pan by a screen-space delta, for wheel scrolling.
    ///
    /// Exposed so the wheel path and the space/middle-drag path share one
    /// camera rule instead of two.
    pub fn pan_by(&mut self, screen_delta: Point<f32>) -> bool {
        self.camera.offset.x -= screen_delta.x / self.camera.zoom;
        self.camera.offset.y -= screen_delta.y / self.camera.zoom;
        true
    }

    pub fn set_space_held(&mut self, held: bool) {
        self.space_held = held;
    }

    fn begin_pan(&mut self, button: MouseButton, event: &MouseDownEvent, window: &mut Window) {
        if self.workload_controls_input() {
            return;
        }
        let should_pan =
            button == MouseButton::Middle || (button == MouseButton::Left && self.space_held);
        if !should_pan {
            return;
        }
        self.interaction = Interaction::None;
        self.marquee = None;
        self.pan = Some(PanGesture {
            button,
            pointer_start: point(f32::from(event.position.x), f32::from(event.position.y)),
            offset_start: self.camera.offset,
        });
        self.capture_pointer(window);
    }

    fn begin_text_pointer_selection(
        &mut self,
        screen: Point<f32>,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(edit) = self.text_edit.as_ref() else {
            return false;
        };
        let Some(caret) = self.text_offset_at_screen(edit.id, screen, window) else {
            return false;
        };
        let anchor = if shift {
            selection_anchor(&edit.selected_range, edit.selection_reversed)
        } else {
            caret
        };
        self.set_text_selection(anchor, caret, cx);
        if let Some(edit) = self.text_edit.as_mut() {
            edit.pointer_anchor = Some(anchor);
        }
        self.capture_pointer(window);
        true
    }

    fn update_text_pointer_selection(
        &mut self,
        screen: Point<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(anchor) = self.text_edit.as_ref().and_then(|edit| edit.pointer_anchor) else {
            return false;
        };
        let Some(id) = self.text_edit.as_ref().map(|edit| edit.id) else {
            return false;
        };
        let Some(caret) = self.text_offset_at_screen(id, screen, window) else {
            return false;
        };
        self.set_text_selection(anchor, caret, cx);
        true
    }

    fn text_offset_at_screen(
        &self,
        id: ObjectId,
        screen: Point<f32>,
        window: &mut Window,
    ) -> Option<usize> {
        let edit = self.text_edit.as_ref().filter(|edit| edit.id == id)?;
        let object = self.session.runtime.object(id)?;
        let local = screen_to_object_local(self.camera, screen, object.position);
        text_offset_at_local_point(&edit.editing_text, local, self.camera.zoom, window)
    }

    fn begin_left_interaction(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workload_controls_input() {
            return;
        }
        self.begin_pan(MouseButton::Left, event, window);
        if self.pan.is_some() {
            return;
        }

        let screen = self.cursor_in_viewport(event.position);
        let world = self.camera.screen_to_world(screen);
        self.marquee = None;

        if let Some(editing_id) = self.text_edit.as_ref().map(|edit| edit.id) {
            let inside_editing_object =
                self.session
                    .runtime
                    .object(editing_id)
                    .is_some_and(|object| {
                        world.x >= object.position.x
                            && world.x <= object.position.x + object.size.width
                            && world.y >= object.position.y
                            && world.y <= object.position.y + object.size.height
                    });
            if inside_editing_object {
                if let Some(focus_handle) = &self.focus_handle {
                    window.focus(focus_handle, cx);
                }
                self.begin_text_pointer_selection(screen, event.modifiers.shift, window, cx);
                cx.stop_propagation();
                diagnostics::count("canvas_notify", 1);
                cx.notify();
                return;
            }
            self.commit_text_edit();
        }

        if self.tool == Tool::Text {
            if let Some(id) = self.editable_text_at(world) {
                self.begin_text_edit(id, window, cx);
                self.begin_text_pointer_selection(screen, event.modifiers.shift, window, cx);
                cx.stop_propagation();
                return;
            }
        }
        if self.tool == Tool::Select && event.click_count >= 2 {
            if let Some(id) = self.editable_text_at(world) {
                self.begin_text_edit(id, window, cx);
                self.begin_text_pointer_selection(screen, event.modifiers.shift, window, cx);
                cx.stop_propagation();
                return;
            }
        }

        if let Some(object_type) = self.tool.creates_object() {
            self.interaction = Interaction::PotentialCreate(CreateGesture {
                object_type,
                pointer_start_screen: screen,
                pointer_start_world: world,
                current_world: world,
            });
            self.capture_pointer(window);
            return;
        }
        if self.tool != Tool::Select {
            return;
        }

        if let Some(handle) = self.hit_test_resize_handle(screen) {
            let id = self.selection.ids()[0];
            if let Some(object) = self.session.runtime.object(id) {
                self.interaction = Interaction::PotentialResize(ResizeGesture {
                    pointer_start_screen: screen,
                    pointer_start_world: world,
                    object: ObjectSnapshot {
                        id,
                        geometry: object.geometry(),
                    },
                    handle,
                });
                self.capture_pointer(window);
            }
            return;
        }

        if let Some(id) = self.session.runtime.hit_test(world) {
            let target_is_selected = self.selection.contains(id);
            let selected_ids = if target_is_selected {
                self.selection.ids().to_vec()
            } else if event.modifiers.shift {
                let mut ids = self.selection.ids().to_vec();
                ids.push(id);
                ids
            } else {
                vec![id]
            };
            let objects = selected_ids
                .iter()
                .filter_map(|selected_id| {
                    self.session
                        .runtime
                        .object(*selected_id)
                        .map(|object| ObjectSnapshot {
                            id: *selected_id,
                            geometry: object.geometry(),
                        })
                })
                .collect();
            self.interaction = Interaction::PotentialMove(MoveGesture {
                pointer_start_screen: screen,
                pointer_start_world: world,
                objects,
                selected_ids,
                click_selection: if event.modifiers.shift {
                    ClickSelection::Toggle(id)
                } else {
                    ClickSelection::SelectOnly(id)
                },
                suspend_snap: suspends_snap(event.modifiers),
            });
            self.capture_pointer(window);
            return;
        }

        self.interaction = Interaction::None;
        let initial_selection = self.selection.ids().to_vec();
        if !event.modifiers.shift {
            self.selection.click(None, false);
        }
        self.marquee = Some(MarqueeGesture {
            start: world,
            current: world,
            additive: event.modifiers.shift,
            initial_selection,
        });
        self.capture_pointer(window);
        diagnostics::count("canvas_notify", 1);
        cx.notify();
    }

    fn capture_pointer(&self, window: &mut Window) {
        if let Some(hitbox) = self.hitbox.get() {
            window.capture_pointer(hitbox.id);
        }
    }

    /// Track what the pointer is over, and say whether the picture changed.
    ///
    /// Returns false when the hover did not change so the caller can skip a
    /// repaint: a pointer moving over empty canvas, or over the same object,
    /// costs nothing.
    ///
    /// Nothing that is being dragged is hoverable, and neither is a text
    /// editor — an outline flashing on and off as the caret moves would be
    /// noise, not feedback.
    fn update_hover(&mut self, screen: Point<f32>) -> bool {
        let hovered = if self.tool == Tool::Select
            && !self.interaction.is_active()
            && self.pan.is_none()
            && self.marquee.is_none()
            && self.text_edit.is_none()
            && self.hit_test_resize_handle(screen).is_none()
        {
            self.session
                .runtime
                .hit_test(self.camera.screen_to_world(screen))
        } else {
            None
        };
        if hovered == self.hovered {
            return false;
        }
        self.hovered = hovered;
        true
    }

    /// What the pointer is currently over, for the status bar.
    #[cfg(any(test, debug_assertions))]
    pub fn hovered(&self) -> Option<ObjectId> {
        self.hovered
    }

    fn hit_test_resize_handle(&self, screen: Point<f32>) -> Option<ResizeHandle> {
        if self.selection.ids().len() != 1 {
            return None;
        }
        let object = self.session.runtime.object(self.selection.ids()[0])?;
        ResizeHandle::ALL.into_iter().find(|handle| {
            let handle_position = handle.screen_position(self.camera, object);
            (screen.x - handle_position.x).abs() <= RESIZE_HANDLE_HIT_RADIUS
                && (screen.y - handle_position.y).abs() <= RESIZE_HANDLE_HIT_RADIUS
        })
    }

    #[cfg(any(test, debug_assertions))]
    fn update_interaction(&mut self, screen: Point<f32>) -> bool {
        self.update_interaction_with(screen, false)
    }

    /// Advance a gesture, with `⇧` state supplied by the caller.
    ///
    /// `update_interaction` is the modifier-free form the tests drive; the
    /// pointer path passes the real key state so `⇧`-constrain can be sampled
    /// live.
    fn update_interaction_with(&mut self, screen: Point<f32>, constrain: bool) -> bool {
        self.constrain_drag = constrain;
        let interaction = std::mem::replace(&mut self.interaction, Interaction::None);
        match interaction {
            Interaction::None => false,
            Interaction::PotentialMove(gesture) => {
                if !drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    self.interaction = Interaction::PotentialMove(gesture);
                    return false;
                }
                self.selection.replace(gesture.selected_ids.clone());
                self.drag_gesture_objects(&gesture, screen);
                self.interaction = Interaction::Moving(gesture);
                true
            }
            Interaction::Moving(gesture) => {
                self.drag_gesture_objects(&gesture, screen);
                self.interaction = Interaction::Moving(gesture);
                true
            }
            Interaction::PotentialResize(gesture) => {
                if !drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    self.interaction = Interaction::PotentialResize(gesture);
                    return false;
                }
                apply_resize(
                    &mut self.session.runtime,
                    &gesture,
                    self.camera,
                    screen,
                    self.constrain_drag,
                );
                self.interaction = Interaction::Resizing(gesture);
                true
            }
            Interaction::Resizing(gesture) => {
                apply_resize(
                    &mut self.session.runtime,
                    &gesture,
                    self.camera,
                    screen,
                    self.constrain_drag,
                );
                self.interaction = Interaction::Resizing(gesture);
                true
            }
            Interaction::PotentialCreate(mut gesture) => {
                if !drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    self.interaction = Interaction::PotentialCreate(gesture);
                    return false;
                }
                gesture.current_world = self.camera.screen_to_world(screen);
                self.interaction = Interaction::Creating(gesture);
                true
            }
            Interaction::Creating(mut gesture) => {
                gesture.current_world = self.camera.screen_to_world(screen);
                self.interaction = Interaction::Creating(gesture);
                true
            }
        }
    }

    /// Drop every piece of feedback that only exists during a gesture.
    ///
    /// Guides and hover are runtime state with no document meaning, so the one
    /// thing they all need is a single place that ends them. Clearing them at
    /// each call site instead is how a guide ends up outliving the drag that
    /// drew it — a magenta line across the artwork that nothing will remove
    /// until the next gesture happens to overwrite it.
    fn clear_gesture_feedback(&mut self) {
        self.snap_guides.clear();
        self.hovered = None;
        self.constrain_drag = false;
    }

    fn finish_interaction(&mut self, screen: Point<f32>) {
        let interaction = std::mem::replace(&mut self.interaction, Interaction::None);
        match interaction {
            Interaction::PotentialMove(gesture) => match gesture.click_selection {
                ClickSelection::SelectOnly(id) => self.selection.click(Some(id), false),
                ClickSelection::Toggle(id) => self.selection.click(Some(id), true),
            },
            Interaction::Moving(gesture) => {
                self.drag_gesture_objects(&gesture, screen);
                let command = geometry_command(&self.session.runtime, &gesture.objects);
                self.commit(command);
            }
            Interaction::PotentialResize(_) => {}
            Interaction::Resizing(gesture) => {
                apply_resize(
                    &mut self.session.runtime,
                    &gesture,
                    self.camera,
                    screen,
                    self.constrain_drag,
                );
                let command =
                    geometry_command(&self.session.runtime, std::slice::from_ref(&gesture.object));
                self.commit(command);
            }
            Interaction::PotentialCreate(gesture) => {
                if gesture.object_type == ObjectType::Text {
                    self.commit_creation(
                        gesture.object_type,
                        gesture.pointer_start_world,
                        size(180.0, 48.0),
                    );
                } else if drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    let current_world = self.camera.screen_to_world(screen);
                    let geometry = creation_geometry(gesture.pointer_start_world, current_world);
                    self.commit_creation(gesture.object_type, geometry.position, geometry.size);
                }
            }
            Interaction::Creating(gesture) => {
                let current_world = self.camera.screen_to_world(screen);
                let geometry = creation_geometry(gesture.pointer_start_world, current_world);
                self.commit_creation(gesture.object_type, geometry.position, geometry.size);
            }
            Interaction::None => {}
        }
        self.clear_gesture_feedback();
    }

    fn commit_creation(
        &mut self,
        object_type: ObjectType,
        position: Point<f32>,
        object_size: Size<f32>,
    ) {
        let text_content = (object_type == ObjectType::Text).then(|| "Type something".to_string());
        let object =
            self.session
                .runtime
                .create_object(object_type, position, object_size, text_content);
        let placement = self.session.runtime.placement(object.id).unwrap();
        self.selection.click(Some(object.id), false);
        self.commit(DocumentCommand::insert(vec![placement]));
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had_interaction = self.interaction.is_active();
        let changed = self.delete_selected_objects();
        if changed || had_interaction {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        changed
    }

    fn delete_selected_objects(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.session.runtime);
            self.interaction = Interaction::None;
        }
        let ids = self.selection.ids().to_vec();
        let deleted = self.session.runtime.remove_objects(&ids);
        if deleted.is_empty() {
            self.retain_existing_selection();
            return false;
        }
        self.commit(DocumentCommand::delete(deleted));
        self.retain_existing_selection();
        true
    }

    pub fn duplicate_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had_interaction = self.interaction.is_active();
        let changed = self.duplicate_selected_objects();
        if changed || had_interaction {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        changed
    }

    fn duplicate_selected_objects(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.session.runtime);
            self.interaction = Interaction::None;
        }
        let ids = self.selection.ids().to_vec();
        let duplicates = self.session.runtime.duplicate_objects(&ids);
        if duplicates.is_empty() {
            self.retain_existing_selection();
            return false;
        }
        let duplicate_ids = duplicates
            .iter()
            .map(|placement| placement.object.id)
            .collect();
        self.selection.replace(duplicate_ids);
        self.commit(DocumentCommand::insert(duplicates));
        true
    }

    fn retain_existing_selection(&mut self) {
        let existing = self
            .selection
            .ids()
            .iter()
            .copied()
            .filter(|id| self.session.runtime.object(*id).is_some())
            .collect();
        self.selection.replace(existing);
    }

    fn cancel_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }
        self.interaction.restore(&mut self.session.runtime);
        self.interaction = Interaction::None;
        self.clear_gesture_feedback();
        true
    }

    pub fn cancel_manipulation(&mut self, cx: &mut Context<Self>) -> bool {
        let cancelled = self.cancel_interaction();
        if cancelled {
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
        cancelled
    }

    fn finish_marquee(&mut self, screen: Point<f32>) {
        let Some(marquee) = self.marquee.take() else {
            return;
        };
        let end = self.camera.screen_to_world(screen);
        let screen_delta = point(
            (end.x - marquee.start.x).abs() * self.camera.zoom,
            (end.y - marquee.start.y).abs() * self.camera.zoom,
        );
        if screen_delta.x.max(screen_delta.y) < 3.0 {
            if !marquee.additive {
                self.selection.click(None, false);
            }
            return;
        }

        let contained = self
            .session
            .runtime
            .objects_in(WorldRect::from_points(marquee.start, end));
        if marquee.additive {
            self.selection.replace(marquee.initial_selection);
            self.selection.add_all(contained);
        } else {
            self.selection.replace(contained);
        }
    }

    fn cursor_in_viewport(&self, position: Point<Pixels>) -> Point<f32> {
        let origin = self
            .hitbox
            .get()
            .map_or(point(gpui_px(0.0), gpui_px(0.0)), |hitbox| hitbox.origin);
        point(
            f32::from(position.x - origin.x),
            f32::from(position.y - origin.y),
        )
    }

    fn render_grid(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let mut spacing = 32.0;
        while spacing * self.camera.zoom < 24.0 {
            spacing *= 2.0;
        }
        while spacing * self.camera.zoom > 48.0 {
            spacing *= 0.5;
        }
        let min_x = self.camera.offset.x;
        let min_y = self.camera.offset.y;
        let max_x = min_x + self.camera.viewport.width / self.camera.zoom;
        let max_y = min_y + self.camera.viewport.height / self.camera.zoom;
        let first_x = (min_x / spacing).ceil() as i32;
        let last_x = (max_x / spacing).floor() as i32;
        let first_y = (min_y / spacing).ceil() as i32;
        let last_y = (max_y / spacing).floor() as i32;
        let dot_size = (1.5 * self.camera.zoom).clamp(1.0, 2.0);

        for grid_y in first_y..=last_y {
            let y = (grid_y as f32 * spacing - self.camera.offset.y) * self.camera.zoom;
            for grid_x in first_x..=last_x {
                let x = (grid_x as f32 * spacing - self.camera.offset.x) * self.camera.zoom;
                let dot_bounds = Bounds {
                    origin: point(bounds.origin.x + gpui_px(x), bounds.origin.y + gpui_px(y)),
                    size: size(gpui_px(dot_size), gpui_px(dot_size)),
                };
                window.paint_quad(gpui::fill(dot_bounds, rgb(0x171a1e)));
            }
        }
    }
}

impl Render for CanvasView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(debug_assertions)]
        self.start_workload(_window, cx);
        diagnostics::count("canvas_render", 1);
        diagnostics::count(
            "viewport_width_sum_px",
            self.camera.viewport.width.round() as u64,
        );
        diagnostics::count(
            "viewport_height_sum_px",
            self.camera.viewport.height.round() as u64,
        );
        diagnostics::count(
            "camera_zoom_sum_milli",
            (self.camera.zoom * 1000.0).round() as u64,
        );
        let render_start = diagnostics::start();
        let entity = cx.entity();
        let entity_for_prepaint = entity.clone();
        let entity_for_paint = entity.clone();
        let hitbox_slot = self.hitbox.clone();
        let current_camera = self.camera;
        let document = &self.session.runtime;
        let selection = &self.selection;
        let marquee = self.marquee.clone();
        let preview = self.interaction.preview();
        let text_edit = self.text_edit.clone();
        let focus_handle = self.focus_handle.clone();
        let input_entity = entity.clone();
        let mut viewport = div()
            .id("canvas-viewport")
            .relative()
            .flex_1()
            .overflow_hidden()
            .bg(rgb(theme::CANVAS))
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseDownEvent, window, _| {
                    this.begin_pan(MouseButton::Middle, event, window);
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.begin_left_interaction(event, window, cx);
                }),
            )
            .on_action(cx.listener(Self::text_backspace))
            .on_action(cx.listener(Self::text_delete))
            .on_action(cx.listener(Self::text_left))
            .on_action(cx.listener(Self::text_right))
            .on_action(cx.listener(Self::text_select_left))
            .on_action(cx.listener(Self::text_select_right))
            .on_action(cx.listener(Self::text_select_all))
            .on_action(cx.listener(Self::text_home))
            .on_action(cx.listener(Self::text_end))
            .on_action(cx.listener(Self::text_paste))
            .on_action(cx.listener(Self::text_copy))
            .on_action(cx.listener(Self::text_cut))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                if this.workload_controls_input() {
                    return;
                }
                let cursor = this.cursor_in_viewport(event.position);
                // Figma: the wheel scrolls the canvas, `⇧`+wheel scrolls
                // sideways, and `⌘`/`Ctrl`+wheel (or a trackpad pinch) zooms.
                // Spool accepted the wheel only as a zoom gesture, which left a
                // trackpad user with no way to pan at all.
                if event.modifiers.control || event.modifiers.platform {
                    let delta = f32::from(event.delta.pixel_delta(gpui_px(24.0)).y);
                    this.camera.zoom_at((delta * 0.002).exp(), cursor);
                } else {
                    let line = gpui_px(24.0);
                    let delta = event.delta.pixel_delta(line);
                    let mut screen_delta = point(f32::from(delta.x), f32::from(delta.y));
                    if event.modifiers.shift {
                        // `⇧` turns a vertical scroll into a horizontal one,
                        // which is what every editor does because a trackpad
                        // only scrolls vertically by default.
                        std::mem::swap(&mut screen_delta.x, &mut screen_delta.y);
                    }
                    this.pan_by(screen_delta);
                }
                diagnostics::count("canvas_notify", 1);
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                if this.workload_controls_input() {
                    return;
                }
                let cursor = this.cursor_in_viewport(event.position);
                this.camera.zoom_at(1.0 + event.delta, cursor);
                diagnostics::count("canvas_notify", 1);
                cx.notify();
            }))
            .child(
                gpui::canvas(
                    move |bounds, window, cx| {
                        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                        hitbox_slot.set(Some(CanvasHitbox {
                            id: hitbox.id,
                            origin: bounds.origin,
                        }));
                        entity_for_prepaint.update(cx, |this, cx| {
                            let viewport =
                                size(f32::from(bounds.size.width), f32::from(bounds.size.height));
                            if this.camera.resize(viewport) {
                                diagnostics::count("camera_resize_notify", 1);
                                diagnostics::count("canvas_notify", 1);
                                cx.notify();
                            }
                        });
                        hitbox
                    },
                    move |bounds, hitbox, window, cx| {
                        let view_state = entity_for_paint.read(cx);
                        if !view_state.workload_controls_input()
                            && (view_state.pan.is_some()
                                || view_state.interaction.is_active()
                                || view_state.marquee.is_some()
                                || view_state
                                    .text_edit
                                    .as_ref()
                                    .is_some_and(|edit| edit.pointer_anchor.is_some()))
                        {
                            window.capture_pointer(hitbox.id);
                        }
                        view_state.render_grid(bounds, window);
                        let view = entity_for_paint.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            view.update(cx, |this, cx| {
                                if this.workload_controls_input() {
                                    return;
                                }
                                if let Some(pan) = this.pan {
                                    this.camera.pan_from(
                                        pan.offset_start,
                                        pan.pointer_start,
                                        point(
                                            f32::from(event.position.x),
                                            f32::from(event.position.y),
                                        ),
                                    );
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                } else if this
                                    .text_edit
                                    .as_ref()
                                    .is_some_and(|edit| edit.pointer_anchor.is_some())
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.update_text_pointer_selection(screen, window, cx);
                                } else if this.interaction.is_active() {
                                    let screen = this.cursor_in_viewport(event.position);
                                    if this.update_interaction_with(screen, event.modifiers.shift) {
                                        diagnostics::count("canvas_notify", 1);
                                        cx.notify();
                                    }
                                } else if this.marquee.is_some() {
                                    let screen = this.cursor_in_viewport(event.position);
                                    let world = this.camera.screen_to_world(screen);
                                    if let Some(marquee) = this.marquee.as_mut() {
                                        marquee.current = world;
                                    }
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                } else if this.update_hover(this.cursor_in_viewport(event.position))
                                {
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                }
                            });
                        });
                        let view = entity_for_paint.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            view.update(cx, |this, cx| {
                                if this.workload_controls_input() {
                                    return;
                                }
                                if event.button == MouseButton::Left
                                    && this
                                        .text_edit
                                        .as_ref()
                                        .is_some_and(|edit| edit.pointer_anchor.is_some())
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.update_text_pointer_selection(screen, window, cx);
                                    if let Some(edit) = this.text_edit.as_mut() {
                                        edit.pointer_anchor = None;
                                    }
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                } else if this.pan.is_some_and(|pan| pan.button == event.button) {
                                    this.pan = None;
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                } else if event.button == MouseButton::Left
                                    && this.interaction.is_active()
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.finish_interaction(screen);
                                    if this.tool == Tool::Text {
                                        if let Some(id) = this.selection.ids().last().copied() {
                                            if this.session.runtime.object(id).is_some_and(
                                                |object| object.object_type == ObjectType::Text,
                                            ) {
                                                this.begin_text_edit(id, window, cx);
                                            }
                                        }
                                    }
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                } else if event.button == MouseButton::Left
                                    && this.marquee.is_some()
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.finish_marquee(screen);
                                    diagnostics::count("canvas_notify", 1);
                                    cx.notify();
                                }
                            });
                        });
                    },
                )
                .absolute()
                .top(gpui_px(0.0))
                .left(gpui_px(0.0))
                .right(gpui_px(0.0))
                .bottom(gpui_px(0.0)),
            )
            .child(artboards(
                current_camera,
                document,
                selection,
                marquee,
                preview,
                GestureFeedback {
                    snap_guides: self.snap_guides.clone(),
                    hovered: self.hovered,
                },
                TextInputRenderContext {
                    edit: text_edit,
                    focus_handle: focus_handle.clone(),
                    entity: input_entity,
                },
            ));
        if let Some(focus_handle) = focus_handle.as_ref() {
            viewport = viewport.track_focus(focus_handle);
        }
        diagnostics::record("canvas_render_build", render_start);
        viewport
    }
}

/// The runtime-only things an in-flight gesture is drawing.
///
/// Grouped rather than passed as two more parameters because they share a
/// lifetime exactly: both exist only while something is being dragged, and
/// neither means anything once it is over.
#[derive(Clone, Debug, Default)]
struct GestureFeedback {
    snap_guides: Vec<snap::Guide>,
    hovered: Option<ObjectId>,
}

struct TextInputRenderContext {
    edit: Option<TextEditState>,
    focus_handle: Option<FocusHandle>,
    entity: Entity<CanvasView>,
}

struct CanvasTextInput {
    view: Entity<CanvasView>,
    focus_handle: FocusHandle,
    text: String,
    selection: Range<usize>,
    cursor: usize,
    zoom: f32,
}

struct CanvasTextPrepaint {
    lines: Vec<(gpui::ShapedLine, usize)>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for CanvasTextInput {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CanvasTextInput {
    type RequestLayoutState = ();
    type PrepaintState = CanvasTextPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let text_style = window.text_style();
        let font_size = gpui_px(14.0 * self.zoom);
        let line_height = gpui_px(18.0 * self.zoom);
        let mut lines = Vec::new();
        let mut selection = Vec::new();
        let mut line_start = 0;
        let mut cursor_quad = None;

        for (line_index, line_text) in self.text.split('\n').enumerate() {
            let shared_text = SharedString::from(line_text.to_owned());
            let run = TextRun {
                len: line_text.len(),
                font: text_style.font(),
                color: rgb(theme::TEXT).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(shared_text, font_size, &[run], None);
            let line_end = line_start + line_text.len();
            let line_y = bounds.top() + line_height * line_index as f32;

            if !self.selection.is_empty() {
                let start = self.selection.start.max(line_start).min(line_end);
                let end = self.selection.end.max(line_start).min(line_end);
                if start < end {
                    selection.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + line.x_for_index(start - line_start), line_y),
                            point(
                                bounds.left() + line.x_for_index(end - line_start),
                                line_y + line_height,
                            ),
                        ),
                        rgba(0x553d74c8),
                    ));
                }
            } else if self.cursor >= line_start && self.cursor <= line_end {
                let x = bounds.left() + line.x_for_index(self.cursor - line_start);
                cursor_quad = Some(fill(
                    Bounds::new(point(x, line_y), size(gpui_px(1.5), line_height)),
                    rgb(theme::ACCENT),
                ));
            }

            lines.push((line, line_index));
            line_start = line_end + 1;
        }

        CanvasTextPrepaint {
            lines,
            selection,
            cursor: cursor_quad,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.handle_input(
            &self.focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let line_height = gpui_px(18.0 * self.zoom);
        for (line, line_index) in prepaint.lines.drain(..) {
            let origin = point(
                bounds.left(),
                bounds.top() + line_height * line_index as f32,
            );
            let _ = line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
        }
        if self.focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }
    }
}

impl EntityInputHandler for CanvasView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let edit = self.text_edit.as_ref()?;
        let range = utf16_range_to_utf8(&edit.editing_text, range_utf16);
        adjusted_range.replace(utf8_range_to_utf16(&edit.editing_text, range.clone()));
        Some(edit.editing_text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let edit = self.text_edit.as_ref()?;
        Some(UTF16Selection {
            range: utf8_range_to_utf16(&edit.editing_text, edit.selected_range.clone()),
            reversed: edit.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let edit = self.text_edit.as_ref()?;
        edit.marked_range
            .as_ref()
            .map(|range| utf8_range_to_utf16(&edit.editing_text, range.clone()))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = self.text_edit.as_mut() {
            edit.marked_range = None;
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_editing_text(range, text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_editing_text(range, new_text, cx);
        if let Some(edit) = self.text_edit.as_mut() {
            let marked_end = edit.selected_range.end;
            let marked_start = marked_end.saturating_sub(new_text.len());
            edit.marked_range = (!new_text.is_empty()).then_some(marked_start..marked_end);
            if let Some(selected) = new_selected_range {
                let selected = utf16_range_to_utf8(new_text, selected);
                edit.selected_range = marked_start + selected.start..marked_start + selected.end;
            }
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let edit = self.text_edit.as_ref()?;
        let range = utf16_range_to_utf8(&edit.editing_text, range_utf16);
        let prefix = &edit.editing_text[..range.start];
        let line_index = prefix.bytes().filter(|byte| *byte == b'\n').count();
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = prefix[line_start..].chars().count();
        let line_height = gpui_px(18.0 * self.camera.zoom);
        let x = element_bounds.left() + gpui_px(column as f32 * 8.0 * self.camera.zoom);
        let y = element_bounds.top() + line_height * line_index as f32;
        Some(Bounds::new(point(x, y), size(gpui_px(1.5), line_height)))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.text_edit.as_mut() {
            edit.selected_range = utf16_range_to_utf8(&edit.editing_text, range_utf16);
            edit.selection_reversed = false;
            diagnostics::count("canvas_notify", 1);
            cx.notify();
        }
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.text_edit.as_ref()?.editing_text.encode_utf16().count())
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.text_edit.is_some()
    }
}

fn screen_to_object_local(
    camera: Camera,
    screen: Point<f32>,
    object_position: Point<f32>,
) -> Point<f32> {
    let world = camera.screen_to_world(screen);
    point(
        (world.x - object_position.x) * camera.zoom,
        (world.y - object_position.y) * camera.zoom,
    )
}

fn selection_from_anchor_and_caret(
    text: &str,
    anchor: usize,
    caret: usize,
) -> (Range<usize>, bool) {
    let anchor = utf8_boundary(text, anchor.min(text.len()));
    let caret = utf8_boundary(text, caret.min(text.len()));
    (anchor.min(caret)..anchor.max(caret), caret < anchor)
}

fn selection_anchor(range: &Range<usize>, reversed: bool) -> usize {
    if reversed {
        range.end
    } else {
        range.start
    }
}

fn nearest_boundary_from_positions(text: &str, positions: &[(usize, f32)], x: f32) -> usize {
    positions
        .iter()
        .filter(|(index, _)| text.is_char_boundary(*index))
        .min_by(|(left_index, left_x), (right_index, right_x)| {
            (left_x - x)
                .abs()
                .total_cmp(&(right_x - x).abs())
                .then_with(|| left_index.cmp(right_index))
        })
        .map_or(0, |(index, _)| *index)
}

fn text_offset_at_local_point(
    text: &str,
    local: Point<f32>,
    zoom: f32,
    window: &mut Window,
) -> Option<usize> {
    let line_height = 18.0 * zoom;
    let lines = text.split('\n').collect::<Vec<_>>();
    let line_index = if line_height <= 0.0 {
        0
    } else {
        ((local.y / line_height).round() as isize).clamp(0, lines.len() as isize - 1) as usize
    };
    let text_style = window.text_style();
    let line_text = lines[line_index];
    let run = TextRun {
        len: line_text.len(),
        font: text_style.font(),
        color: rgb(theme::TEXT).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(
        SharedString::from(line_text.to_owned()),
        gpui_px(14.0 * zoom),
        &[run],
        None,
    );
    let positions = line_text
        .char_indices()
        .map(|(index, _)| (index, f32::from(line.x_for_index(index))))
        .chain(std::iter::once((
            line_text.len(),
            f32::from(line.x_for_index(line_text.len())),
        )))
        .collect::<Vec<_>>();
    let line_offset = lines[..line_index]
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>();
    Some(line_offset + nearest_boundary_from_positions(line_text, &positions, local.x))
}

fn utf8_boundary(text: &str, offset: usize) -> usize {
    let mut boundary = offset.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn utf16_to_utf8(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_offset = 0;
    for character in text.chars() {
        if utf16_offset + character.len_utf16() > offset {
            break;
        }
        utf16_offset += character.len_utf16();
        utf8_offset += character.len_utf8();
    }
    utf8_offset
}

fn utf8_to_utf16(text: &str, offset: usize) -> usize {
    text[..utf8_boundary(text, offset)].encode_utf16().count()
}

fn utf16_range_to_utf8(text: &str, range: Range<usize>) -> Range<usize> {
    let length = text.encode_utf16().count();
    let start = range.start.min(length);
    let end = range.end.min(length).max(start);
    utf16_to_utf8(text, start)..utf16_to_utf8(text, end)
}

fn utf8_range_to_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    utf8_to_utf16(text, range.start)..utf8_to_utf16(text, range.end)
}

fn previous_char_boundary(text: &str, offset: usize) -> usize {
    text[..utf8_boundary(text, offset)]
        .char_indices()
        .last()
        .map_or(0, |(index, _)| index)
}

fn next_char_boundary(text: &str, offset: usize) -> usize {
    let offset = utf8_boundary(text, offset);
    text[offset..]
        .chars()
        .next()
        .map_or(text.len(), |character| offset + character.len_utf8())
}

fn render_text_input(
    camera: Camera,
    object: &DesignObject,
    edit: &TextEditState,
    focus_handle: FocusHandle,
    view: Entity<CanvasView>,
    zoom: f32,
) -> impl IntoElement {
    let origin = camera.world_to_screen(object.position);
    div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .w(gpui_px(object.size.width * zoom))
        .h(gpui_px(object.size.height * zoom))
        .overflow_hidden()
        .track_focus(&focus_handle)
        .child(CanvasTextInput {
            view,
            focus_handle,
            text: edit.editing_text.clone(),
            selection: edit.selected_range.clone(),
            cursor: if edit.selection_reversed {
                edit.selected_range.start
            } else {
                edit.selected_range.end
            },
            zoom,
        })
}

/// Phase 14 construction decision for one document object.
///
/// The actively edited text object is always constructed so its editing
/// overlay, focus tracking and caret stay coherent regardless of camera
/// position. Everything else is gated by the padded viewport predicate.
///
/// Selection is deliberately not a parameter: selection outlines and resize
/// handles are interaction chrome constructed in their own loops inside
/// `artboards`, independent of the base element, so culling an unselected or
/// selected object's base element never removes its chrome and never requires
/// retaining it.
fn should_construct(
    camera: Camera,
    object: &DesignObject,
    editing_object: Option<ObjectId>,
) -> bool {
    editing_object == Some(object.id) || camera.affects_viewport(object.position, object.size)
}

fn artboards(
    camera: Camera,
    document: &Document,
    selection: &Selection,
    marquee: Option<MarqueeGesture>,
    preview: Option<CreationPreview>,
    feedback: GestureFeedback,
    text_input: TextInputRenderContext,
) -> impl IntoElement {
    // Keep the diagnostic-only visibility scan outside the construction timer.
    let visibility_start = diagnostics::start();
    let visible = if diagnostics::enabled() {
        document
            .objects()
            .iter()
            .filter(|object| {
                diagnostics::intersects(
                    object.position,
                    object.size,
                    camera.offset,
                    camera.viewport,
                    camera.zoom,
                )
            })
            .count() as u64
    } else {
        0
    };
    diagnostics::record("visibility_scan", visibility_start);
    let build_start = diagnostics::start();
    let zoom = camera.zoom;
    let text_edit = text_input.edit;
    let focus_handle = text_input.focus_handle;
    let input_entity = text_input.entity;
    let mut world = div()
        .absolute()
        .top(gpui_px(0.0))
        .left(gpui_px(0.0))
        .right(gpui_px(0.0))
        .bottom(gpui_px(0.0))
        .overflow_hidden();

    let editing_object = text_edit.as_ref().map(|edit| edit.id);
    let mut constructed = 0u64;
    for object in document
        .objects()
        .iter()
        .filter(|object| should_construct(camera, object, editing_object))
    {
        constructed += 1;
        let content = match object.id {
            ObjectId::LANDING => Some(landing_frame(zoom, object.size).into_any_element()),
            ObjectId::EDITOR => Some(editor_frame(zoom, object.size).into_any_element()),
            ObjectId::FEATURES => Some(features_frame(zoom, object.size).into_any_element()),
            ObjectId::MOBILE => Some(mobile_frame(zoom, object.size).into_any_element()),
            _ => None,
        };
        if let Some(content) = content {
            world = world.child(positioned_frame(camera, object, content, zoom));
        } else {
            let editing_this_object = editing_object == Some(object.id);
            world = world.child(render_object(camera, object, zoom, editing_this_object));
            if editing_this_object {
                if let (Some(edit), Some(focus_handle)) =
                    (text_edit.as_ref(), focus_handle.as_ref())
                {
                    world = world.child(render_text_input(
                        camera,
                        object,
                        edit,
                        focus_handle.clone(),
                        input_entity.clone(),
                        zoom,
                    ));
                }
            }
        }
    }

    if let Some(preview) = preview {
        world = world.child(render_preview(camera, preview, zoom));
    }

    // Hover sits *under* selection so that hovering something already selected
    // does not change its appearance at all — the selection outline is the
    // stronger statement and should win.
    if let Some(hovered) = feedback.hovered.filter(|id| !selection.contains(*id)) {
        if let Some(object) = document.object(hovered) {
            world = world.child(hover_outline(camera, object));
        }
    }
    for id in selection.ids() {
        if let Some(object) = document.object(*id) {
            world = world.child(selection_outline(camera, object));
        }
    }
    if selection.ids().len() == 1 {
        if let Some(object) = document.object(selection.ids()[0]) {
            for handle in ResizeHandle::ALL {
                world = world.child(resize_handle_element(camera, object, handle));
            }
        }
    }

    // Guides last, so nothing in the scene can paint over the explanation of a
    // move that is happening right now.
    for guide in feedback.snap_guides {
        world = world.child(render_snap_guide(camera, guide, zoom));
    }

    if let Some(marquee) = marquee {
        let bounds = WorldRect::from_points(marquee.start, marquee.current);
        let origin = camera.world_to_screen(bounds.min);
        let marquee_size = size(
            (bounds.max.x - bounds.min.x) * zoom,
            (bounds.max.y - bounds.min.y) * zoom,
        );
        world = world.child(
            div()
                .absolute()
                .left(gpui_px(origin.x))
                .top(gpui_px(origin.y))
                .w(gpui_px(marquee_size.width))
                .h(gpui_px(marquee_size.height))
                .border_1()
                .border_color(rgb(theme::ACCENT)),
        );
    }

    diagnostics::record("canvas_elements", build_start);
    let considered = document.objects().len() as u64;
    diagnostics::count("objects_considered", considered);
    diagnostics::count("objects_constructed", constructed);
    diagnostics::count("objects_culled", considered - constructed);
    diagnostics::count("geometry_intersecting", visible);
    diagnostics::count("geometry_offscreen", considered - visible);
    world
}

fn render_object(
    camera: Camera,
    object: &DesignObject,
    zoom: f32,
    hide_editing_text: bool,
) -> impl IntoElement {
    let origin = camera.world_to_screen(object.position);
    let mut body = div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .w(px!(object.size.width, zoom))
        .h(px!(object.size.height, zoom));
    // Authored `opacity`, applied to the whole object the way CSS applies it:
    // the element and everything it contains, not just its paint.
    if object.opacity < 1.0 {
        body = body.opacity(object.opacity.clamp(0.0, 1.0));
    }
    if object.object_type != ObjectType::Text {
        // Authored `border-radius`. Applied before the ellipse case below so a
        // round shape still wins over a rectangular radius.
        if object.border_radius > 0.0 {
            body = body.rounded(gpui_px(object.border_radius * zoom));
        }
        if let Some(fill) = object.fill {
            body = body.bg(rgb(fill.color.to_rgb()));
        }
        if let Some(stroke) = object.stroke {
            body = body
                .border_1()
                .border_color(rgb(stroke.color.to_rgb()))
                .border_t(gpui_px(stroke.width * zoom))
                .border_b(gpui_px(stroke.width * zoom))
                .border_l(gpui_px(stroke.width * zoom))
                .border_r(gpui_px(stroke.width * zoom));
        }
    }
    match object.object_type {
        ObjectType::Frame => {
            body = body.child(
                div()
                    .absolute()
                    .left(gpui_px(0.0))
                    .top(gpui_px(-LABEL_HEIGHT * zoom))
                    .text_size(px!(12.0, zoom))
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child(object.name.clone()),
            );
        }
        ObjectType::Rectangle => {}
        ObjectType::Ellipse => {
            body = body.rounded_full();
        }
        ObjectType::Text if !hide_editing_text => {
            body = body
                .flex()
                .items_start()
                .text_size(px!(text_size_of(object), zoom))
                .text_color(ink_of(object))
                .child(object.text_content.clone().unwrap_or_default());
        }
        ObjectType::Text => {}
    }

    // Authored text is drawn for any object that has it, not only for objects
    // typed as text. Metadata declares a button as `frame` while its element
    // is an `<a>` with a label; hiding that label would make a correctly
    // loaded project look emptier than its source.
    if let Some(text) = object.text_content.as_ref().filter(|t| !t.is_empty()) {
        if object.object_type != ObjectType::Text {
            body = body
                .flex()
                .items_start()
                .text_size(px!(text_size_of(object), zoom))
                .text_color(ink_of(object))
                .child(text.clone());
        }
    }
    body
}

/// Text size for an object: the authored one when source declared it.
///
/// Provisional: an authored size is applied as the renderer font size, with no
/// line-height model and no scaling against the element's own box.
fn text_size_of(object: &DesignObject) -> f32 {
    object.font_size.unwrap_or(14.0)
}

/// Text colour for an object: authored first, contrast guess second.
///
/// The authored `color` wins because source is authoritative. Only when no
/// colour was authored does the renderer choose one, because an unreadable
/// label is worse than an arbitrary choice.
fn ink_of(object: &DesignObject) -> gpui::Rgba {
    if let Some(color) = object.text_color {
        return rgb(color.to_rgb());
    }
    object
        .fill
        .map(|fill| rgb(contrasting_ink(fill.color)))
        .unwrap_or_else(|| rgb(theme::TEXT))
}

/// Pick black or white text for legibility against a background.
///
/// Provisional: the authored `color` should win once style resolution is
/// plumbed through to the renderer.
fn contrasting_ink(background: Color) -> u32 {
    let luminance = 0.299 * background.red as f32
        + 0.587 * background.green as f32
        + 0.114 * background.blue as f32;
    if luminance > 140.0 {
        0x1a1a1a
    } else {
        0xffffff
    }
}

fn render_preview(camera: Camera, preview: CreationPreview, zoom: f32) -> impl IntoElement {
    let origin = camera.world_to_screen(preview.geometry.position);
    let mut body = div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .w(px!(preview.geometry.size.width, zoom))
        .h(px!(preview.geometry.size.height, zoom))
        .bg(rgb(theme::SURFACE_RAISED))
        .border_1()
        .border_color(rgb(theme::ACCENT));
    if preview.object_type == ObjectType::Ellipse {
        body = body.rounded_full();
    }
    body
}

fn positioned_frame(
    camera: Camera,
    object: &DesignObject,
    content: impl IntoElement,
    zoom: f32,
) -> impl IntoElement {
    let origin = camera.world_to_screen(object.position);
    div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .child(
            div()
                .absolute()
                .left(gpui_px(0.0))
                .top(gpui_px(-LABEL_HEIGHT * zoom))
                .text_size(px!(12.0, zoom))
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child(format!("{:02}  /  {}", object.id.0, object.name)),
        )
        .child(
            div()
                .absolute()
                .left(gpui_px(0.0))
                .top(gpui_px(0.0))
                .child(content),
        )
}

fn selection_outline(camera: Camera, object: &DesignObject) -> impl IntoElement {
    let origin = camera.world_to_screen(object.position);
    div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .w(gpui_px(object.size.width * camera.zoom))
        .h(gpui_px(object.size.height * camera.zoom))
        .border_1()
        .border_color(rgb(theme::ACCENT))
}

/// The outline for an object under the pointer that is not selected.
///
/// Deliberately weaker than the selection outline: thinner and dimmer, so hover
/// answers "is this clickable?" without claiming the user has chosen it. No
/// fill, because a fill would hide the object's own colours at the exact moment
/// the user is comparing them to something else.
fn hover_outline(camera: Camera, object: &DesignObject) -> impl IntoElement {
    let origin = camera.world_to_screen(object.position);
    div()
        .absolute()
        .left(gpui_px(origin.x))
        .top(gpui_px(origin.y))
        .w(gpui_px(object.size.width * camera.zoom))
        .h(gpui_px(object.size.height * camera.zoom))
        .border_1()
        .border_color(rgba((theme::ACCENT & 0x00ff_ffff) | (0x66 << 24)))
}

/// The line that explains a snap.
///
/// One screen pixel wide at any zoom — a guide that grows with the world is a
/// wall, and one that shrinks is invisible exactly when the user is zoomed in
/// trying to see it. Magenta is Figma's choice and this is that convention: a
/// colour no object in the document is likely to be, so it reads as editor
/// chrome rather than as content.
fn render_snap_guide(camera: Camera, guide: snap::Guide, zoom: f32) -> impl IntoElement {
    let thickness = 1.0;
    match guide.axis {
        snap::Axis::Vertical => {
            let x = (guide.at - camera.offset.x) * zoom;
            let top = (guide.start - camera.offset.y) * zoom;
            let height = (guide.end - guide.start) * zoom;
            div()
                .absolute()
                .left(gpui_px(x - thickness / 2.0))
                .top(gpui_px(top))
                .w(gpui_px(thickness))
                .h(gpui_px(height.max(thickness)))
                .bg(rgb(theme::SNAP_GUIDE))
        }
        snap::Axis::Horizontal => {
            let y = (guide.at - camera.offset.y) * zoom;
            let left = (guide.start - camera.offset.x) * zoom;
            let width = (guide.end - guide.start) * zoom;
            div()
                .absolute()
                .left(gpui_px(left))
                .top(gpui_px(y - thickness / 2.0))
                .w(gpui_px(width.max(thickness)))
                .h(gpui_px(thickness))
                .bg(rgb(theme::SNAP_GUIDE))
        }
    }
}

fn resize_handle_element(
    camera: Camera,
    object: &DesignObject,
    handle: ResizeHandle,
) -> impl IntoElement {
    let position = handle.screen_position(camera, object);
    let half_size = RESIZE_HANDLE_SIZE / 2.0;
    div()
        .absolute()
        .left(gpui_px(position.x - half_size))
        .top(gpui_px(position.y - half_size))
        .size(gpui_px(RESIZE_HANDLE_SIZE))
        .rounded_sm()
        .bg(rgb(theme::ACCENT))
        .border_1()
        .border_color(rgb(theme::WINDOW))
}

fn landing_frame(zoom: f32, frame_size: Size<f32>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px!(frame_size.width, zoom))
        .h(px!(frame_size.height, zoom))
        .p(px!(22.0, zoom))
        .gap(px!(24.0, zoom))
        .bg(rgb(theme::PAPER))
        .text_color(rgb(theme::INK))
        .border_1()
        .border_color(rgb(0xd8d5ce))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px!(8.0, zoom))
                        .child(div().size(px!(15.0, zoom)).rounded_sm().bg(rgb(theme::INK)))
                        .child(
                            div()
                                .text_size(px!(14.0, zoom))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("Forma"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px!(16.0, zoom))
                        .text_size(px!(12.0, zoom))
                        .text_color(rgb(0x686963))
                        .child("Studio")
                        .child("Journal")
                        .child("About"),
                )
                .child(
                    div()
                        .px(px!(13.0, zoom))
                        .py(px!(7.0, zoom))
                        .rounded_md()
                        .bg(rgb(theme::INK))
                        .text_size(px!(12.0, zoom))
                        .text_color(rgb(theme::PAPER))
                        .child("Explore"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_between()
                .gap(px!(16.0, zoom))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(px!(12.0, zoom))
                        .w(px!(205.0, zoom))
                        .child(
                            div()
                                .text_size(px!(12.0, zoom))
                                .text_color(rgb(0x6c7068))
                                .child("A PLACE TO BEGIN AGAIN"),
                        )
                        .child(
                            div()
                                .text_color(rgb(theme::INK))
                                .text_size(px!(29.0, zoom))
                                .line_height(px!(32.0, zoom))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child("Make room for\nwhat matters."),
                        )
                        .child(
                            div()
                                .text_size(px!(12.0, zoom))
                                .text_color(rgb(0x676b65))
                                .child("Thoughtful objects for slower days."),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px!(8.0, zoom))
                                .mt(px!(4.0, zoom))
                                .child(
                                    div()
                                        .px(px!(13.0, zoom))
                                        .py(px!(8.0, zoom))
                                        .rounded_md()
                                        .bg(rgb(theme::INK))
                                        .text_size(px!(12.0, zoom))
                                        .text_color(rgb(theme::PAPER))
                                        .child("Discover the collection"),
                                )
                                .child(
                                    div()
                                        .text_size(px!(14.0, zoom))
                                        .text_color(rgb(0x5a5e58))
                                        .child("↗"),
                                ),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_end()
                        .justify_center()
                        .w(px!(156.0, zoom))
                        .h(px!(170.0, zoom))
                        .overflow_hidden()
                        .rounded_md()
                        .bg(rgb(theme::SAGE))
                        .child(
                            div()
                                .absolute()
                                .top(px!(19.0, zoom))
                                .left(px!(18.0, zoom))
                                .size(px!(88.0, zoom))
                                .rounded_full()
                                .bg(rgb(0xb9bda9)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(px!(-14.0, zoom))
                                .right(px!(14.0, zoom))
                                .w(px!(73.0, zoom))
                                .h(px!(120.0, zoom))
                                .rounded_t_full()
                                .bg(rgb(0x565e50)),
                        )
                        .child(
                            div()
                                .relative()
                                .mb(px!(20.0, zoom))
                                .w(px!(52.0, zoom))
                                .h(px!(78.0, zoom))
                                .rounded_md()
                                .bg(rgb(theme::SAND))
                                .border_1()
                                .border_color(rgb(0xeee4d4)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .border_t_1()
                .border_color(rgb(0xd7d3ca))
                .pt(px!(10.0, zoom))
                .text_size(px!(12.0, zoom))
                .text_color(rgb(0x74766f))
                .child("Designed for the everyday")
                .child("01 — 04"),
        )
}

fn editor_frame(zoom: f32, frame_size: Size<f32>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px!(frame_size.width, zoom))
        .h(px!(frame_size.height, zoom))
        .p(px!(12.0, zoom))
        .gap(px!(12.0, zoom))
        .bg(rgb(0xf0efeb))
        .border_1()
        .border_color(rgb(0xd8d5ce))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px!(12.0, zoom))
                .text_color(rgb(0x4a4e4b))
                .child("◈  spool")
                .child("Landing page     100%"),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .gap(px!(8.0, zoom))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px!(28.0, zoom))
                        .items_center()
                        .gap(px!(12.0, zoom))
                        .py(px!(8.0, zoom))
                        .bg(rgb(0xe6e4df))
                        .text_size(px!(14.0, zoom))
                        .text_color(rgb(0x555955))
                        .child("↖")
                        .child("□")
                        .child("○")
                        .child("T"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .p(px!(13.0, zoom))
                        .gap(px!(12.0, zoom))
                        .bg(rgb(0xe5e3dc))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px!(12.0, zoom))
                                        .text_color(rgb(0x555953))
                                        .child("Forma"),
                                )
                                .child(
                                    div()
                                        .text_size(px!(12.0, zoom))
                                        .text_color(rgb(0x777a73))
                                        .child("Studio    Journal"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .justify_center()
                                .items_start()
                                .gap(px!(8.0, zoom))
                                .p(px!(12.0, zoom))
                                .bg(rgb(0xd5d4cc))
                                .child(
                                    div()
                                        .text_size(px!(12.0, zoom))
                                        .text_color(rgb(0x666b62))
                                        .child("A PLACE TO BEGIN AGAIN"),
                                )
                                .child(
                                    div()
                                        .text_size(px!(18.0, zoom))
                                        .line_height(px!(20.0, zoom))
                                        .text_color(rgb(0x262923))
                                        .child("Make room for\nwhat matters."),
                                )
                                .child(
                                    div()
                                        .size(px!(70.0, zoom))
                                        .rounded_md()
                                        .bg(rgb(theme::SAGE)),
                                ),
                        ),
                ),
        )
}

fn features_frame(zoom: f32, frame_size: Size<f32>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px!(frame_size.width, zoom))
        .h(px!(frame_size.height, zoom))
        .p(px!(20.0, zoom))
        .gap(px!(16.0, zoom))
        .bg(rgb(0x202421))
        .text_color(rgb(0xf0efe8))
        .border_1()
        .border_color(rgb(0x333a34))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px!(12.0, zoom))
                .text_color(rgb(0xb2b8ac))
                .child("FORMA  /  OUR APPROACH")
                .child("A quieter kind of good"),
        )
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px!(8.0, zoom))
                        .w(px!(190.0, zoom))
                        .child(
                            div()
                                .text_size(px!(24.0, zoom))
                                .line_height(px!(27.0, zoom))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child("Made to stay.\nMade with care."),
                        )
                        .child(
                            div()
                                .text_size(px!(12.0, zoom))
                                .text_color(rgb(0xb7bcb1))
                                .child("Considered forms, honest materials, fewer better things."),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap(px!(8.0, zoom))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .justify_center()
                                .w(px!(62.0, zoom))
                                .h(px!(100.0, zoom))
                                .pb(px!(9.0, zoom))
                                .bg(rgb(0x666f5f))
                                .text_size(px!(12.0, zoom))
                                .text_color(rgb(0xe9e8df))
                                .child("01"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .justify_center()
                                .w(px!(62.0, zoom))
                                .h(px!(122.0, zoom))
                                .pb(px!(9.0, zoom))
                                .bg(rgb(0x88877a))
                                .text_size(px!(12.0, zoom))
                                .text_color(rgb(0xf0eee7))
                                .child("02"),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap(px!(8.0, zoom))
                .child(feature_pill("01", "Thoughtful form", zoom))
                .child(feature_pill("02", "Lasting materials", zoom))
                .child(feature_pill("03", "Less, but better", zoom)),
        )
}

fn feature_pill(number: &'static str, label: &'static str, zoom: f32) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(px!(8.0, zoom))
        .p(px!(10.0, zoom))
        .border_1()
        .border_color(rgb(0x3a413a))
        .child(
            div()
                .text_size(px!(12.0, zoom))
                .text_color(rgb(theme::ACCENT))
                .child(number),
        )
        .child(
            div()
                .text_size(px!(12.0, zoom))
                .text_color(rgb(0xe3e4dd))
                .child(label),
        )
}

fn mobile_frame(zoom: f32, frame_size: Size<f32>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px!(frame_size.width, zoom))
        .h(px!(frame_size.height, zoom))
        .p(px!(10.0, zoom))
        .gap(px!(12.0, zoom))
        .rounded_lg()
        .bg(rgb(0x171a1d))
        .border_1()
        .border_color(rgb(0x45494a))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px!(5.0, zoom))
                .text_size(px!(12.0, zoom))
                .text_color(rgb(0xe7e7e2))
                .child("9:41")
                .child("●  ▮"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px!(12.0, zoom))
                .p(px!(12.0, zoom))
                .bg(rgb(0xe9e6de))
                .text_color(rgb(0x252722))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px!(12.0, zoom))
                        .child("Forma")
                        .child("☰"),
                )
                .child(
                    div()
                        .text_size(px!(21.0, zoom))
                        .line_height(px!(23.0, zoom))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("Good design\nlives well."),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .items_end()
                        .justify_end()
                        .p(px!(9.0, zoom))
                        .bg(rgb(theme::SAGE))
                        .child(
                            div()
                                .w(px!(54.0, zoom))
                                .h(px!(92.0, zoom))
                                .rounded_t_full()
                                .bg(rgb(0x68715f)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px!(12.0, zoom))
                        .text_color(rgb(0x676b64))
                        .child("Objects for living")
                        .child("↗"),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::test_support::RetainedLayers;
    use crate::operations::{OperationTarget, SemanticHistory};

    fn starter_layer_rows() -> Vec<(ObjectId, SharedString)> {
        vec![
            (ObjectId::LANDING, "Landing".into()),
            (ObjectId::EDITOR, "Editor".into()),
            (ObjectId::FEATURES, "Features".into()),
            (ObjectId::MOBILE, "Mobile".into()),
        ]
    }

    #[test]
    fn retained_layer_rows_follow_create_delete_duplicate_and_history() {
        let mut canvas = CanvasView::new();
        let mut layers = RetainedLayers::default();
        let starter = starter_layer_rows();
        assert_eq!(layers.synchronize(&canvas), starter);

        let text = canvas.session.runtime.create_object(
            ObjectType::Text,
            point(10.0, 20.0),
            size(80.0, 30.0),
            Some("Content, not the layer name".into()),
        );
        canvas.commit(DocumentCommand::insert(vec![canvas
            .session
            .runtime
            .placement(text.id)
            .unwrap()]));
        let mut created = starter.clone();
        created.push((text.id, "Text 1".into()));
        assert_eq!(layers.synchronize(&canvas), created);
        assert!(canvas.session.undo().unwrap());
        assert_eq!(layers.synchronize(&canvas), starter);
        assert!(canvas.session.redo().unwrap());
        assert_eq!(layers.synchronize(&canvas), created);

        // Reverse selection order must not reverse document/projected order.
        let duplicates = canvas
            .session
            .runtime
            .duplicate_objects(&[text.id, ObjectId::EDITOR]);
        assert_eq!(duplicates.len(), 2);
        canvas.commit(DocumentCommand::insert(duplicates.clone()));
        let mut duplicated = created.clone();
        duplicated.extend([
            (duplicates[0].object.id, "Frame 1".into()),
            (duplicates[1].object.id, "Text 2".into()),
        ]);
        assert_eq!(layers.synchronize(&canvas), duplicated);
        assert!(canvas.session.undo().unwrap());
        assert_eq!(layers.synchronize(&canvas), created);
        assert!(canvas.session.redo().unwrap());
        assert_eq!(layers.synchronize(&canvas), duplicated);

        // Non-adjacent deletions must restore their original positions and names.
        let removed = canvas
            .session
            .runtime
            .remove_objects(&[text.id, ObjectId::EDITOR]);
        canvas.commit(DocumentCommand::delete(removed));
        let deleted: Vec<_> = duplicated
            .iter()
            .filter(|(id, _)| *id != text.id && *id != ObjectId::EDITOR)
            .cloned()
            .collect();
        assert_eq!(layers.synchronize(&canvas), deleted);
        assert!(canvas.session.undo().unwrap());
        assert_eq!(layers.synchronize(&canvas), duplicated);
        assert!(canvas.session.redo().unwrap());
        assert_eq!(layers.synchronize(&canvas), deleted);
        assert_eq!(layers.document_walks(), 10);
    }

    #[test]
    fn retained_layer_rows_keep_names_and_skip_walks_for_selection_and_transient_changes() {
        let mut canvas = CanvasView::new();
        let text = canvas.session.runtime.create_object(
            ObjectType::Text,
            point(10.0, 20.0),
            size(80.0, 30.0),
            Some("hello".into()),
        );
        let mut layers = RetainedLayers::default();
        let mut expected = starter_layer_rows();
        expected.push((text.id, "Text 1".into()));
        assert_eq!(layers.synchronize(&canvas), expected);

        canvas.selection.replace(vec![text.id, ObjectId::EDITOR]);
        assert_eq!(layers.synchronize(&canvas), expected);
        assert_eq!(layers.selected(), &[text.id, ObjectId::EDITOR]);
        // Selection changes report only the rows whose presentation changed,
        // in row order; unchanged rows are not touched.
        assert_eq!(
            layers.presentation_updates(),
            &[(ObjectId::EDITOR, true), (text.id, true)]
        );
        canvas.selection.replace(vec![ObjectId::LANDING]);
        assert_eq!(layers.synchronize(&canvas), expected);
        assert_eq!(layers.selected(), &[ObjectId::LANDING]);
        assert_eq!(
            layers.presentation_updates(),
            &[
                (ObjectId::LANDING, true),
                (ObjectId::EDITOR, false),
                (text.id, false),
            ]
        );

        let before = canvas.session.runtime.geometry(text.id).unwrap();
        canvas
            .session
            .runtime
            .set_position(text.id, point(100.0, 200.0));
        canvas.session.runtime.set_size(text.id, size(120.0, 40.0));
        assert_eq!(layers.synchronize(&canvas), expected);
        let after = canvas.session.runtime.geometry(text.id).unwrap();
        canvas
            .session
            .history
            .record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
                GeometryChange {
                    id: text.id,
                    before,
                    after,
                },
            ])));
        assert!(canvas.session.undo().unwrap());
        assert_eq!(layers.synchronize(&canvas), expected);
        assert!(canvas.session.redo().unwrap());
        assert_eq!(layers.synchronize(&canvas), expected);

        canvas.session.runtime.set_style(
            text.id,
            ObjectStyle {
                fill: None,
                stroke: None,
                ..ObjectStyle::default()
            },
        );
        assert_eq!(layers.synchronize(&canvas), expected);
        canvas
            .session
            .runtime
            .set_text_content(text.id, "Changed content must not replace Text 1".into());
        assert_eq!(layers.synchronize(&canvas), expected);
        canvas.commit(DocumentCommand::text(vec![TextChange {
            id: text.id,
            before: "hello".into(),
            after: "Changed content must not replace Text 1".into(),
        }]));
        assert!(canvas.session.undo().unwrap());
        assert_eq!(layers.synchronize(&canvas), expected);
        assert!(canvas.session.redo().unwrap());
        assert_eq!(layers.synchronize(&canvas), expected);

        canvas.camera.offset = point(300.0, 400.0);
        canvas.set_zoom_percent(200);
        assert_eq!(layers.synchronize(&canvas), expected);
        canvas.selection.replace(vec![]);
        assert_eq!(layers.synchronize(&canvas), expected);
        assert!(layers.selected().is_empty());
        assert_eq!(layers.presentation_updates(), &[(ObjectId::LANDING, false)]);
        assert_eq!(layers.document_walks(), 1);
    }

    #[test]
    fn layer_structure_revision_tracks_successful_mutations_and_history_order() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        assert_eq!(document.layer_structure_revision(), 0);
        let object = document.create_object(
            ObjectType::Text,
            point(1.0, 2.0),
            size(80.0, 30.0),
            Some("hello".into()),
        );
        assert_eq!(document.layer_structure_revision(), 1);
        assert!(!document.insert_object(object.clone(), 0));
        assert!(document.remove_objects(&[ObjectId(9999)]).is_empty());
        assert_eq!(document.layer_structure_revision(), 1);
        let original_order: Vec<_> = document.objects().iter().map(|object| object.id).collect();
        let duplicates = document.duplicate_objects(&[ObjectId::EDITOR, object.id]);
        assert_eq!(document.layer_structure_revision(), 3);
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(
            duplicates.clone(),
        )));
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.layer_structure_revision(), 5);
        assert_eq!(
            document
                .objects()
                .iter()
                .map(|object| object.id)
                .collect::<Vec<_>>(),
            original_order
        );
        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.layer_structure_revision(), 7);
        let order: Vec<_> = document.objects().iter().map(|object| object.id).collect();
        assert_eq!(
            &order[original_order.len()..],
            &duplicates
                .iter()
                .map(|placement| placement.object.id)
                .collect::<Vec<_>>()
        );
        let removed = document.remove_objects(&[ObjectId::EDITOR, object.id]);
        assert_eq!(document.layer_structure_revision(), 9);
        history.record(SemanticOperation::Runtime(DocumentCommand::delete(removed)));
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.layer_structure_revision(), 11);
        assert_eq!(
            document
                .objects()
                .iter()
                .map(|object| object.id)
                .collect::<Vec<_>>(),
            order
        );
        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.layer_structure_revision(), 13);
    }

    #[test]
    fn layer_structure_revision_ignores_geometry_style_text_selection_and_camera() {
        let mut canvas = CanvasView::new();
        let object = canvas.session.runtime.create_object(
            ObjectType::Text,
            point(1.0, 2.0),
            size(80.0, 30.0),
            Some("hello".into()),
        );
        let revision = canvas.layer_structure_revision();
        let before = canvas.session.runtime.geometry(object.id).unwrap();
        canvas
            .session
            .runtime
            .set_position(object.id, point(20.0, 30.0));
        let after = canvas.session.runtime.geometry(object.id).unwrap();
        canvas
            .session
            .history
            .record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
                GeometryChange {
                    id: object.id,
                    before,
                    after,
                },
            ])));
        canvas.session.undo().unwrap();
        canvas.session.redo().unwrap();
        canvas
            .session
            .runtime
            .set_size(object.id, size(100.0, 50.0));
        canvas.session.runtime.set_style(
            object.id,
            ObjectStyle {
                fill: None,
                stroke: None,
                ..ObjectStyle::default()
            },
        );
        canvas
            .session
            .runtime
            .set_text_content(object.id, "changed".into());
        canvas.commit(DocumentCommand::text(vec![TextChange {
            id: object.id,
            before: "hello".into(),
            after: "changed".into(),
        }]));
        canvas.session.undo().unwrap();
        canvas.session.redo().unwrap();
        canvas.selection.click(Some(object.id), false);
        canvas.set_zoom_percent(200);
        canvas.camera.offset = point(100.0, 200.0);
        assert_eq!(canvas.layer_structure_revision(), revision);
    }

    #[test]
    fn borrowed_object_element_construction_preserves_document_at_all_zoom_limits() {
        let mut document = Document::default();
        for object_type in [
            ObjectType::Frame,
            ObjectType::Rectangle,
            ObjectType::Ellipse,
            ObjectType::Text,
        ] {
            document.create_object(
                object_type,
                point(20.0, 40.0),
                size(100.0, 80.0),
                (object_type == ObjectType::Text).then(|| "Unicode 🧵 text".to_string()),
            );
        }
        let before = document.objects().to_vec();
        for zoom in [MIN_ZOOM, 1.0, MAX_ZOOM] {
            let camera = Camera {
                zoom,
                ..Camera::default()
            };
            for object in document.objects() {
                let _element = render_object(camera, object, zoom, false).into_any_element();
                let _outline = selection_outline(camera, object).into_any_element();
                for handle in ResizeHandle::ALL {
                    let _handle = resize_handle_element(camera, object, handle).into_any_element();
                }
            }
        }
        assert_eq!(document.objects(), before);
    }

    #[test]
    fn camera_coordinates_round_trip() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        camera.zoom_at(1.7, point(317.0, 246.0));

        let world = point(183.0, 512.0);
        let screen = camera.world_to_screen(world);
        let round_trip = camera.screen_to_world(screen);
        assert!((round_trip.x - world.x).abs() < 0.001);
        assert!((round_trip.y - world.y).abs() < 0.001);
    }

    #[test]
    fn zoom_keeps_the_world_point_under_the_cursor() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        let cursor = point(723.0, 151.0);
        let world_anchor = camera.screen_to_world(cursor);

        camera.zoom_at(2.4, cursor);

        let screen_anchor = camera.world_to_screen(world_anchor);
        assert!((screen_anchor.x - cursor.x).abs() < 0.001);
        assert!((screen_anchor.y - cursor.y).abs() < 0.001);
    }

    #[test]
    fn pan_tracks_pointer_delta_at_current_zoom() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        camera.zoom_at(2.0, point(480.0, 360.0));

        let world_point = point(350.0, 280.0);
        let start_screen = camera.world_to_screen(world_point);
        let offset_start = camera.offset;
        camera.pan_from(offset_start, point(20.0, 30.0), point(84.0, 6.0));
        let end_screen = camera.world_to_screen(world_point);

        assert!((end_screen.x - start_screen.x - 64.0).abs() < 0.001);
        assert!((end_screen.y - start_screen.y + 24.0).abs() < 0.001);
    }

    #[test]
    fn zoom_is_clamped_and_fit_centers_world_bounds() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));

        camera.zoom_at(0.001, point(480.0, 360.0));
        assert_eq!(camera.zoom, MIN_ZOOM);
        camera.zoom_at(100.0, point(480.0, 360.0));
        assert_eq!(camera.zoom, MAX_ZOOM);

        camera.fit();
        let center = camera.world_to_screen(WORLD_CENTER);
        assert!((center.x - 480.0).abs() < 0.001);
        assert!((center.y - 360.0).abs() < 0.001);
        let bottom_right = camera.world_to_screen(point(WORLD_BOUNDS.width, WORLD_BOUNDS.height));
        assert!(bottom_right.x <= 912.0);
        assert!(bottom_right.y <= 672.0);
    }

    #[test]
    fn document_allocates_unique_ids_and_deterministic_names() {
        let mut document = Document::default();
        let first = document.create_object(
            ObjectType::Rectangle,
            point(10.0, 20.0),
            size(120.0, 80.0),
            None,
        );
        let second = document.create_object(
            ObjectType::Rectangle,
            point(30.0, 40.0),
            size(90.0, 60.0),
            None,
        );

        assert_ne!(first.id, second.id);
        assert_eq!(first.name, "Rectangle 1");
        assert_eq!(second.name, "Rectangle 2");
    }

    #[test]
    fn document_creates_each_phase_six_object_type_with_geometry() {
        let mut document = Document::default();
        for (index, object_type) in [
            ObjectType::Frame,
            ObjectType::Rectangle,
            ObjectType::Ellipse,
            ObjectType::Text,
        ]
        .into_iter()
        .enumerate()
        {
            let content = (object_type == ObjectType::Text).then(|| "hello, world".to_string());
            let object = document.create_object(
                object_type,
                point(index as f32 * 10.0, 25.0),
                size(140.0, 90.0),
                content.clone(),
            );
            assert_eq!(object.object_type, object_type);
            assert_eq!(object.position, point(index as f32 * 10.0, 25.0));
            assert_eq!(object.size, size(140.0, 90.0));
            assert_eq!(object.text_content, content);
            assert_eq!(document.object(object.id), Some(&object));
        }
        assert_eq!(document.objects().len(), 8);
    }

    #[test]
    fn created_objects_are_hit_testable_and_newest_object_wins() {
        let mut document = Document::default();
        let first = document.create_object(
            ObjectType::Rectangle,
            point(20.0, 30.0),
            size(100.0, 80.0),
            None,
        );
        let second = document.create_object(
            ObjectType::Ellipse,
            point(20.0, 30.0),
            size(100.0, 80.0),
            None,
        );

        assert_eq!(document.hit_test(point(50.0, 50.0)), Some(second.id));
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn creation_drag_normalizes_reversed_direction_and_never_has_negative_size() {
        let geometry = creation_geometry(point(260.0, 220.0), point(100.0, 100.0));

        assert_eq!(geometry.position, point(100.0, 100.0));
        assert_eq!(geometry.size, size(160.0, 120.0));
    }

    #[test]
    fn sub_threshold_creation_gesture_creates_no_object() {
        let mut canvas = CanvasView::new();
        canvas.tool = Tool::Rectangle;
        canvas.interaction = Interaction::PotentialCreate(CreateGesture {
            object_type: ObjectType::Rectangle,
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(100.0, 100.0),
            current_world: point(100.0, 100.0),
        });

        canvas.update_interaction(point(3.0, 0.0));
        canvas.finish_interaction(point(3.0, 0.0));

        assert_eq!(canvas.session.runtime.objects().len(), 4);
        assert!(canvas.session.history.undo_len() == 0);
    }

    #[test]
    fn frame_rectangle_and_ellipse_gestures_create_selected_document_objects() {
        for (tool, object_type) in [
            (Tool::Frame, ObjectType::Frame),
            (Tool::Rectangle, ObjectType::Rectangle),
            (Tool::Ellipse, ObjectType::Ellipse),
        ] {
            let mut canvas = CanvasView::new();
            canvas.tool = tool;
            let start_screen = point(100.0, 100.0);
            let end_screen = point(160.0, 140.0);
            let start_world = canvas.camera.screen_to_world(start_screen);
            canvas.interaction = Interaction::PotentialCreate(CreateGesture {
                object_type,
                pointer_start_screen: start_screen,
                pointer_start_world: start_world,
                current_world: start_world,
            });

            assert!(canvas.update_interaction(end_screen));
            assert!(canvas.interaction.preview().is_some());
            assert_eq!(canvas.session.runtime.objects().len(), 4);
            canvas.finish_interaction(end_screen);

            let created = canvas.session.runtime.objects().last().unwrap();
            assert_eq!(created.object_type, object_type);
            assert_eq!(created.size, size(60.0, 40.0));
            assert_eq!(canvas.selection.ids(), &[created.id]);
            assert_eq!(canvas.session.history.undo_len(), 1);
        }
    }

    #[test]
    fn escape_cancels_creation_without_history_or_selection_changes() {
        let mut canvas = CanvasView::new();
        canvas.selection.click(Some(ObjectId::LANDING), false);
        canvas.interaction = Interaction::Creating(CreateGesture {
            object_type: ObjectType::Ellipse,
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(100.0, 100.0),
            current_world: point(180.0, 160.0),
        });
        let preview = canvas.interaction.preview();
        assert!(preview.is_some());

        assert!(canvas.cancel_interaction());

        assert_eq!(canvas.session.runtime.objects().len(), 4);
        assert!(canvas.session.history.undo_len() == 0);
        assert_eq!(canvas.selection.ids(), &[ObjectId::LANDING]);
        assert!(canvas.interaction.preview().is_none());
    }

    #[test]
    fn text_click_creates_and_selects_a_text_object_immediately() {
        let mut canvas = CanvasView::new();
        canvas.tool = Tool::Text;
        canvas.interaction = Interaction::PotentialCreate(CreateGesture {
            object_type: ObjectType::Text,
            pointer_start_screen: point(100.0, 120.0),
            pointer_start_world: point(100.0, 120.0),
            current_world: point(100.0, 120.0),
        });

        canvas.finish_interaction(point(100.0, 120.0));

        let text = canvas.session.runtime.objects().last().unwrap();
        assert_eq!(text.object_type, ObjectType::Text);
        assert_eq!(text.text_content.as_deref(), Some("Type something"));
        assert_eq!(text.position, point(100.0, 120.0));
        assert_eq!(canvas.selection.ids(), &[text.id]);
        assert_eq!(canvas.session.history.undo_len(), 1);
    }

    #[test]
    fn created_object_can_be_selected_moved_and_resized() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(100.0, 100.0),
            size(120.0, 80.0),
            None,
        );
        let mut selection = Selection::default();
        selection.click(Some(object.id), false);
        let snapshot = ObjectSnapshot {
            id: object.id,
            geometry: object.geometry(),
        };

        apply_move(&mut document, &[snapshot], point(25.0, 15.0));
        let moved = document.geometry(object.id).unwrap();
        let resized = resized_geometry(moved, ResizeHandle::BottomRight, point(20.0, 10.0), false);
        document.set_geometry(object.id, resized);

        assert!(selection.contains(object.id));
        assert_eq!(
            document.geometry(object.id).unwrap().position,
            point(125.0, 115.0)
        );
        assert_eq!(
            document.geometry(object.id).unwrap().size,
            size(140.0, 90.0)
        );
    }

    #[test]
    fn document_hit_test_returns_hit_and_miss() {
        let document = Document::default();

        assert_eq!(
            document.hit_test(point(100.0, 100.0)),
            Some(ObjectId::LANDING)
        );
        assert_eq!(document.hit_test(point(440.0, 100.0)), None);
    }

    #[test]
    fn overlapping_objects_resolve_to_the_topmost_document_object() {
        let document = Document {
            objects: vec![
                frame(
                    ObjectId::LANDING,
                    node_id("spool-node-test-back"),
                    "Back",
                    0.0,
                    0.0,
                    100.0,
                    100.0,
                ),
                frame(
                    ObjectId::EDITOR,
                    node_id("spool-node-test-front"),
                    "Front",
                    25.0,
                    25.0,
                    100.0,
                    100.0,
                ),
            ],
            next_id: 5,
            next_node_id: 1,
            next_names: [1; 4],
            layer_structure_revision: 0,
        };

        assert_eq!(document.hit_test(point(50.0, 50.0)), Some(ObjectId::EDITOR));
    }

    #[test]
    fn screen_to_world_hit_test_works_at_non_default_zoom() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        camera.set_zoom_at_center(0.5);
        let document = Document::default();
        let world_point = point(100.0, 100.0);

        assert_eq!(
            document.hit_test(camera.screen_to_world(camera.world_to_screen(world_point))),
            Some(ObjectId::LANDING)
        );
    }

    #[test]
    fn hit_test_remains_correct_after_camera_pan() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        let document = Document::default();
        let world_point = point(100.0, 100.0);
        let start_offset = camera.offset;
        camera.pan_from(start_offset, point(40.0, 40.0), point(190.0, 95.0));

        assert_eq!(
            document.hit_test(camera.screen_to_world(camera.world_to_screen(world_point))),
            Some(ObjectId::LANDING)
        );
    }

    #[test]
    fn empty_click_clears_selection() {
        let mut selection = Selection::default();
        selection.click(Some(ObjectId::LANDING), false);
        selection.click(None, false);

        assert!(selection.is_empty());
    }

    #[test]
    fn shift_click_toggles_object_selection() {
        let mut selection = Selection::default();
        selection.click(Some(ObjectId::LANDING), true);
        selection.click(Some(ObjectId::EDITOR), true);
        assert_eq!(selection.ids(), &[ObjectId::LANDING, ObjectId::EDITOR]);

        selection.click(Some(ObjectId::LANDING), true);

        assert_eq!(selection.ids(), &[ObjectId::EDITOR]);
    }

    fn test_geometry(x: f32, y: f32, width: f32, height: f32) -> ObjectGeometry {
        ObjectGeometry {
            position: point(x, y),
            size: size(width, height),
        }
    }

    #[test]
    fn moving_one_object_changes_its_position_by_world_delta() {
        let mut document = Document::default();
        let object = document.object(ObjectId::LANDING).unwrap();
        let snapshot = ObjectSnapshot {
            id: object.id,
            geometry: object.geometry(),
        };
        let start = snapshot.geometry.position;

        apply_move(&mut document, &[snapshot], point(40.0, -12.0));

        assert_eq!(
            document.object(ObjectId::LANDING).unwrap().position,
            point(start.x + 40.0, start.y - 12.0)
        );
    }

    #[test]
    fn moving_multiple_objects_preserves_their_relative_positions() {
        let mut document = Document::default();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let snapshots: Vec<_> = ids
            .iter()
            .map(|id| {
                let object = document.object(*id).unwrap();
                ObjectSnapshot {
                    id: *id,
                    geometry: object.geometry(),
                }
            })
            .collect();
        let initial_delta = point(
            snapshots[1].geometry.position.x - snapshots[0].geometry.position.x,
            snapshots[1].geometry.position.y - snapshots[0].geometry.position.y,
        );

        apply_move(&mut document, &snapshots, point(40.0, 20.0));

        let landing = document.object(ObjectId::LANDING).unwrap();
        let editor = document.object(ObjectId::EDITOR).unwrap();
        assert_eq!(
            point(
                editor.position.x - landing.position.x,
                editor.position.y - landing.position.y,
            ),
            initial_delta
        );
    }

    #[test]
    fn pointer_movement_becomes_world_movement_at_non_default_zoom() {
        let mut camera = Camera::default();
        camera.resize(size(960.0, 720.0));
        camera.set_zoom_at_center(0.5);
        let pointer_start_screen = point(300.0, 220.0);
        let pointer_start_world = camera.screen_to_world(pointer_start_screen);
        let pointer_end_screen = point(350.0, 250.0);

        assert_eq!(
            movement_delta(camera, pointer_start_world, pointer_end_screen),
            point(100.0, 60.0)
        );
    }

    #[test]
    fn resize_screen_delta_scales_by_camera_zoom() {
        for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
            let mut camera = Camera::default();
            camera.resize(size(960.0, 720.0));
            camera.set_zoom_at_center(zoom);
            let mut document = Document::default();
            let object = document.object(ObjectId::LANDING).unwrap();
            let gesture = ResizeGesture {
                pointer_start_screen: point(300.0, 220.0),
                pointer_start_world: camera.screen_to_world(point(300.0, 220.0)),
                object: ObjectSnapshot {
                    id: object.id,
                    geometry: object.geometry(),
                },
                handle: ResizeHandle::Right,
            };
            let start_width = gesture.object.geometry.size.width;

            apply_resize(&mut document, &gesture, camera, point(350.0, 220.0), false);

            assert_eq!(
                document.object(ObjectId::LANDING).unwrap().size.width,
                start_width + 50.0 / zoom
            );
        }
    }

    #[test]
    fn drag_threshold_is_screen_space() {
        assert!(!drag_threshold_crossed(point(0.0, 0.0), point(3.0, 0.0)));
        assert!(drag_threshold_crossed(point(0.0, 0.0), point(4.0, 0.0)));
    }

    #[test]
    fn right_resize_changes_width_and_keeps_left_edge_fixed() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Right, point(25.0, 0.0), false);

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(125.0, 80.0));
    }

    #[test]
    fn left_resize_changes_position_and_width() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Left, point(20.0, 0.0), false);

        assert_eq!(resized.position, point(30.0, 20.0));
        assert_eq!(resized.size, size(80.0, 80.0));
    }

    #[test]
    fn bottom_resize_changes_height_and_keeps_top_edge_fixed() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Bottom, point(0.0, 18.0), false);

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(100.0, 98.0));
    }

    #[test]
    fn top_resize_changes_position_and_height() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Top, point(0.0, 20.0), false);

        assert_eq!(resized.position, point(10.0, 40.0));
        assert_eq!(resized.size, size(100.0, 60.0));
    }

    #[test]
    fn corner_resize_changes_both_dimensions() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::BottomRight, point(20.0, 15.0), false);

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(120.0, 95.0));
    }

    #[test]
    fn resize_enforces_minimum_size_without_flipping() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let right = resized_geometry(start, ResizeHandle::Right, point(-200.0, 0.0), false);
        let left = resized_geometry(start, ResizeHandle::Left, point(200.0, 0.0), false);
        let top = resized_geometry(start, ResizeHandle::Top, point(0.0, 200.0), false);

        assert_eq!(right.size.width, MIN_OBJECT_SIZE);
        assert_eq!(left.position.x, 90.0);
        assert_eq!(left.size.width, MIN_OBJECT_SIZE);
        assert_eq!(top.position.y, 80.0);
        assert_eq!(top.size.height, MIN_OBJECT_SIZE);
    }

    #[test]
    fn cancel_restores_original_geometry() {
        let mut document = Document::default();
        let object = document.object(ObjectId::LANDING).unwrap();
        let snapshot = ObjectSnapshot {
            id: object.id,
            geometry: object.geometry(),
        };
        let original = snapshot.geometry;
        let gesture = MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        };
        apply_move(&mut document, &gesture.objects, point(75.0, 30.0));
        let interaction = Interaction::Moving(gesture);

        interaction.restore(&mut document);

        assert_eq!(
            document.object(ObjectId::LANDING).unwrap().geometry(),
            original
        );
    }

    fn record_position(
        history: &mut SemanticHistory,
        document: &mut Document,
        id: ObjectId,
        x: f32,
    ) {
        let before = document.geometry(id).unwrap();
        let after = Geometry {
            position: point(x, before.position.y),
            size: before.size,
        };
        document.set_geometry(id, after);
        history.record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
            GeometryChange { id, before, after },
        ])));
    }

    /// Start a move gesture the way a pointer-down on a selected object does.
    fn begin_live_move(canvas: &mut CanvasView, ids: &[ObjectId]) {
        begin_live_move_with_snap(canvas, ids, false);
    }

    /// The same gesture, but with the pointer already `suspend_snap`.
    ///
    /// Modifier state is a property of the press, not of the movement, so a
    /// test that wants `⌘`-drag has to start the gesture with it held rather
    /// than flip it mid-flight.
    fn begin_live_move_with_snap(canvas: &mut CanvasView, ids: &[ObjectId], suspend_snap: bool) {
        let start = point(0.0, 0.0);
        canvas.interaction = Interaction::PotentialMove(MoveGesture {
            pointer_start_screen: start,
            pointer_start_world: start,
            objects: snapshots(&canvas.session.runtime, ids),
            selected_ids: ids.to_vec(),
            click_selection: ClickSelection::SelectOnly(ids[0]),
            suspend_snap,
        });
    }

    /// Drive a gesture to `end` and commit it, exactly as pointer-up does.
    ///
    /// Returns whether the gesture was considered live, which is false for a
    /// click that never moved.
    fn drag_to(canvas: &mut CanvasView, end: Point<f32>) -> bool {
        let live = canvas.update_interaction(end);
        canvas.finish_interaction(end);
        live
    }

    fn positions(canvas: &CanvasView, ids: &[ObjectId]) -> Vec<(f32, f32)> {
        ids.iter()
            .map(|id| {
                let g = canvas.session.runtime.geometry(*id).expect("object");
                (g.position.x, g.position.y)
            })
            .collect()
    }

    #[test]
    fn live_move_commits_exactly_one_entry_and_undo_redo_round_trips() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let before = canvas.session.runtime.geometry(id).unwrap();
        assert_eq!(canvas.session.history.undo_len(), 0);

        begin_live_move(&mut canvas, &[id]);
        drag_to(&mut canvas, point(37.0, 23.0));

        let moved = canvas.session.runtime.geometry(id).unwrap();
        assert_eq!(
            moved.position,
            point(before.position.x + 37.0, before.position.y + 23.0)
        );
        assert_eq!(
            canvas.session.history.undo_len(),
            1,
            "one committed gesture is one entry, not one per pointer event"
        );

        assert!(canvas.undo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), before);
        assert!(canvas.redo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), moved);
    }

    #[test]
    fn a_drag_near_a_neighbour_snap_into_line_and_say_so() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let before = canvas.session.runtime.geometry(id).unwrap();

        begin_live_move(&mut canvas, &[id]);
        // Editor's left edge is at 454; this drag puts Landing's left edge at
        // 456, two world pixels away at 100%.
        canvas.update_interaction(point(456.0, 0.0));

        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap().position.x,
            454.0,
            "held in line with Editor's left edge"
        );
        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap().position.y,
            before.position.y,
            "a purely horizontal drag must not also move vertically"
        );
        assert_eq!(canvas.snap_guides().len(), 1, "the snap explains itself");
        assert_eq!(canvas.snap_guides()[0].at, 454.0);

        canvas.finish_interaction(point(456.0, 0.0));
    }

    #[test]
    fn the_command_modifier_places_an_object_freely_and_draws_no_guide() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        begin_live_move_with_snap(&mut canvas, &[id], true);
        drag_to(&mut canvas, point(456.0, 0.0));

        let moved = canvas.session.runtime.geometry(id).unwrap();
        assert_eq!(
            moved.position.x, 456.0,
            "exactly where the pointer asked, not where the magnet wanted"
        );
        assert!(canvas.snap_guides().is_empty());
    }

    #[test]
    fn shift_constrains_a_move_to_the_axis_the_pointer_chose() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let start = canvas.session.runtime.geometry(id).unwrap().position;

        // Suspended snapping: these tests are about the axis constraint, and a
        // neighbouring edge nearby would answer a different question.
        begin_live_move_with_snap(&mut canvas, &[id], true);
        // Mostly horizontal with `⇧` held: only the x survives.
        canvas.update_interaction_with(point(60.0, 12.0), true);
        let horizontal = canvas.session.runtime.geometry(id).unwrap().position;
        assert_eq!(horizontal, point(start.x + 60.0, start.y));

        // Mostly vertical with `⇧` held: only the y survives.
        canvas.update_interaction_with(point(30.0, 60.0), true);
        let vertical = canvas.session.runtime.geometry(id).unwrap().position;
        assert_eq!(vertical, point(start.x, start.y + 60.0));
    }

    #[test]
    fn releasing_shift_mid_drag_lets_the_object_move_on_both_axes_again() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let start = canvas.session.runtime.geometry(id).unwrap().position;

        begin_live_move_with_snap(&mut canvas, &[id], true);
        canvas.update_interaction_with(point(40.0, 5.0), true);
        canvas.update_interaction_with(point(60.0, 12.0), false);
        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap().position,
            point(start.x + 60.0, start.y + 12.0),
            "the constraint is sampled per movement, not latched at press time"
        );
    }

    #[test]
    fn shift_preserves_the_aspect_ratio_of_a_corner_resize() {
        let start = ObjectGeometry {
            position: point(0.0, 0.0),
            size: size(200.0, 100.0),
        };
        // 40 across a 200-wide box is a 1.2 scale, so the height becomes 120
        // rather than the 110 an unconstrained drag would have produced.
        let resized = resized_geometry(start, ResizeHandle::BottomRight, point(40.0, 10.0), true);
        assert!(
            (resized.size.width - 240.0).abs() < 0.01,
            "{:?}",
            resized.size
        );
        assert!(
            (resized.size.height - 120.0).abs() < 0.01,
            "{:?}",
            resized.size
        );

        let free = resized_geometry(start, ResizeHandle::BottomRight, point(40.0, 10.0), false);
        assert_eq!(free.size, size(240.0, 110.0));
    }

    #[test]
    fn shift_scales_a_resize_from_the_dominant_axis() {
        let start = ObjectGeometry {
            position: point(0.0, 0.0),
            size: size(200.0, 100.0),
        };
        // A tall drag on a short box: the height drives the scale.
        let resized = resized_geometry(start, ResizeHandle::BottomRight, point(10.0, 100.0), true);
        assert_eq!(resized.size.height, 200.0, "doubled from 100 to 200");
        assert_eq!(resized.size.width, 400.0, "and the width follows");
    }

    #[test]
    fn snap_guides_disappear_when_the_gesture_ends() {
        let mut canvas = CanvasView::new();
        begin_live_move(&mut canvas, &[ObjectId::LANDING]);
        canvas.update_interaction(point(456.0, 0.0));
        assert_eq!(canvas.snap_guides().len(), 1, "held during the drag");
        canvas.finish_interaction(point(456.0, 0.0));
        assert!(
            canvas.snap_guides().is_empty(),
            "a guide that outlives its gesture is a line drawn on the document"
        );
    }

    #[test]
    fn a_nudge_is_never_pulled_back_by_an_alignment() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        // Park the object two pixels from Editor's left edge, which is well
        // inside a snap width.
        canvas.session.runtime.set_position(id, point(452.0, 24.0));
        canvas.selection.click(Some(id), false);
        let before = canvas.session.runtime.geometry(id).unwrap();

        assert!(canvas.nudge_selection(1.0, 0.0));

        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap().position.x,
            before.position.x + 1.0,
            "an arrow key states a position; it does not ask where it should be"
        );
    }

    #[test]
    fn hovering_tracks_the_pointer_and_stops_when_a_gesture_starts() {
        let mut canvas = CanvasView::new();
        // Landing occupies world (0, 24) to (430, 310) at 100%.
        assert!(canvas.update_hover(point(10.0, 30.0)));
        assert_eq!(canvas.hovered(), Some(ObjectId::LANDING));
        assert!(
            !canvas.update_hover(point(11.0, 31.0)),
            "same object, no repaint"
        );
        assert!(canvas.update_hover(point(900.0, 700.0)));
        assert_eq!(canvas.hovered(), None, "empty canvas is not hoverable");

        assert!(canvas.update_hover(point(10.0, 30.0)));
        assert_eq!(canvas.hovered(), Some(ObjectId::LANDING));

        begin_live_move(&mut canvas, &[ObjectId::LANDING]);
        assert!(
            canvas.update_hover(point(10.0, 30.0)),
            "dragging replaces hover rather than layering it"
        );
        assert_eq!(canvas.hovered(), None);
    }

    #[test]
    fn only_the_command_modifier_suspends_snapping() {
        // The corpus is unambiguous that it is the command key, and equally that
        // no other modifier does this: `⇧` constrains and `⌥` duplicates. A
        // blanket "any modifier" rule would quietly take snapping away from the
        // gesture `⇧` is already changing.
        let mods = |shift: bool, alt: bool, control: bool, platform: bool| gpui::Modifiers {
            shift,
            alt,
            control,
            platform,
            ..Default::default()
        };
        assert!(!suspends_snap(mods(false, false, false, false)));
        assert!(!suspends_snap(mods(true, false, false, false)));
        assert!(!suspends_snap(mods(false, true, false, false)));
        assert!(suspends_snap(mods(false, false, true, false)));
        assert!(suspends_snap(mods(false, false, false, true)));
        assert!(
            suspends_snap(mods(true, false, true, false)),
            "`⇧⌘` is still the command key, held harder"
        );
    }

    #[test]
    fn an_object_does_not_snap_to_its_own_child() {
        let mut canvas = CanvasView::new();
        // Make Editor a child of Landing in the persistent structure, and put the
        // child 100 to the right of the parent so there is a real line to snap
        // to.
        let landing = node_id("spool-node-landing");
        let editor = node_id("spool-node-editor");
        let node = |id: NodeId, parent: Option<NodeId>, name: &str| {
            crate::source_document::StructuralNode {
                id,
                name: name.to_owned(),
                kind: "frame".to_owned(),
                parent,
                children: Vec::new(),
                source: crate::source_document::SourceBinding {
                    file: "index.html".to_owned(),
                    selector: name.to_owned(),
                },
            }
        };
        canvas.session.document.structure.nodes = vec![
            node(landing.clone(), None, "Landing"),
            node(editor.clone(), Some(landing), "Editor"),
        ];
        let parent = canvas
            .session
            .runtime
            .object(ObjectId::LANDING)
            .unwrap()
            .spool_id
            .clone();
        let child = canvas
            .session
            .runtime
            .object(ObjectId::EDITOR)
            .unwrap()
            .spool_id
            .clone();
        canvas
            .session
            .runtime
            .set_position(ObjectId::EDITOR, point(100.0, 24.0));
        let child_x = canvas
            .session
            .runtime
            .geometry(ObjectId::EDITOR)
            .unwrap()
            .position
            .x;

        // The parent's left edge lands two pixels from the child's — inside the
        // threshold — but the child moves with the parent, so aligning to it
        // would be aligning to the user.
        let targets = canvas.snap_targets(&[ObjectId::LANDING]);
        assert!(
            !targets
                .iter()
                .any(|rect| rect.left() == child_x && rect.x == child_x),
            "a descendant is not a snap target for its ancestor: {targets:?}"
        );
        assert!(
            targets.iter().any(|rect| rect.left() == 106.0),
            "an unrelated sibling still is: {targets:?}"
        );
        let _ = (parent, child);
    }

    #[test]
    fn live_resize_commits_exactly_one_entry_and_undo_redo_round_trips() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let before = canvas.session.runtime.geometry(id).unwrap();

        let start = point(0.0, 0.0);
        canvas.interaction = Interaction::PotentialResize(ResizeGesture {
            pointer_start_screen: start,
            pointer_start_world: start,
            object: snapshots(&canvas.session.runtime, &[id])[0],
            handle: ResizeHandle::BottomRight,
        });
        drag_to(&mut canvas, point(30.0, 15.0));

        let resized = canvas.session.runtime.geometry(id).unwrap();
        assert_ne!(resized, before, "the live resize changed the object");
        assert_eq!(
            canvas.session.history.undo_len(),
            1,
            "one committed resize is one entry"
        );

        assert!(canvas.undo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), before);
        assert!(canvas.redo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), resized);
    }

    #[test]
    fn live_multi_object_move_is_one_entry_and_undo_restores_all() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR, ObjectId::FEATURES];
        let before = positions(&canvas, &ids);

        begin_live_move(&mut canvas, &ids);
        drag_to(&mut canvas, point(25.0, 12.0));

        let moved = positions(&canvas, &ids);
        for (id, (x, y)) in moved.iter().enumerate() {
            assert_eq!(*x, before[id].0 + 25.0, "object {id} moved horizontally");
            assert_eq!(*y, before[id].1 + 12.0, "object {id} moved vertically");
        }
        assert_eq!(
            canvas.session.history.undo_len(),
            1,
            "three objects dragged together is still one semantic edit"
        );

        assert!(canvas.undo_history());
        assert_eq!(
            positions(&canvas, &ids),
            before,
            "undo restored every object"
        );
        assert!(canvas.redo_history());
        assert_eq!(positions(&canvas, &ids), moved);
    }

    #[test]
    fn live_gesture_cancel_restores_geometry_and_records_nothing() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;
        let before = canvas.session.runtime.geometry(id).unwrap();

        begin_live_move(&mut canvas, &[id]);
        // Transient mutation happens exactly as it would during a real drag.
        assert!(canvas.update_interaction(point(80.0, 45.0)));
        assert_ne!(
            canvas.session.runtime.geometry(id).unwrap(),
            before,
            "the preview really moved the object"
        );

        assert!(canvas.cancel_interaction(), "the gesture was active");
        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap(),
            before,
            "cancel restored the exact pre-gesture geometry"
        );
        assert_eq!(
            canvas.session.history.undo_len(),
            0,
            "a cancelled gesture must not become a history entry"
        );
        assert!(!canvas.session.history.can_undo());
        assert!(!canvas.session.history.can_redo());
    }

    #[test]
    fn live_selection_camera_pan_and_zoom_never_enter_history() {
        let mut canvas = CanvasView::new();
        // Commit one real edit so the stack is non-empty and any stray entry
        // from runtime state would be visible.
        begin_live_move(&mut canvas, &[ObjectId::LANDING]);
        drag_to(&mut canvas, point(10.0, 10.0));
        let depth = canvas.session.history.undo_len();
        assert_eq!(depth, 1);

        // Selection changes.
        canvas.selection.click(Some(ObjectId::EDITOR), false);
        canvas.selection.click(Some(ObjectId::FEATURES), true);
        // Camera changes: pan offset and zoom.
        canvas.camera.offset = point(-140.0, 92.0);
        canvas.camera.set_zoom_at_center(2.5);
        canvas.camera.fit();
        canvas.pan = Some(PanGesture {
            button: MouseButton::Left,
            pointer_start: point(0.0, 0.0),
            offset_start: point(0.0, 0.0),
        });

        assert_eq!(
            canvas.session.history.undo_len(),
            depth,
            "selection, camera, pan and zoom are runtime state, not history"
        );
        assert_eq!(canvas.selection.ids().len(), 2);
    }

    #[test]
    fn live_canvas_edit_then_rename_undoes_in_commit_order() {
        use crate::operations::SemanticOperation;
        use crate::source_document::{LamineStructure, RenameNode, SourceBinding, StructuralNode};

        let mut canvas = CanvasView::new();
        canvas.session.document.structure = LamineStructure {
            nodes: vec![StructuralNode {
                id: NodeId::new("spool-order").unwrap(),
                name: "Ordered".into(),
                kind: "frame".into(),
                parent: None,
                children: vec![],
                source: SourceBinding {
                    file: "index.html".into(),
                    selector: "[data-spool-id=\"spool-order\"]".into(),
                },
            }],
        };
        let id = ObjectId::LANDING;
        let resting = canvas.session.runtime.geometry(id).unwrap();

        // Opposite order from the sibling test: canvas edit FIRST.
        begin_live_move(&mut canvas, &[id]);
        drag_to(&mut canvas, point(18.0, 9.0));
        let moved = canvas.session.runtime.geometry(id).unwrap();

        canvas
            .session
            .execute(SemanticOperation::Rename(RenameNode {
                id: NodeId::new("spool-order").unwrap(),
                before: "Ordered".into(),
                after: "Reordered".into(),
            }))
            .unwrap();
        assert_eq!(canvas.session.history.undo_len(), 2);

        // First undo pops the rename (committed last) and must leave the
        // canvas move completely untouched.
        assert!(canvas.undo_history());
        assert_eq!(canvas.session.document.structure.nodes[0].name, "Ordered");
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), moved);

        // Second undo pops the canvas move.
        assert!(canvas.undo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), resting);
        assert_eq!(canvas.session.history.undo_len(), 0);

        // Redo replays in the same order.
        assert!(canvas.redo_history());
        assert_eq!(canvas.session.runtime.geometry(id).unwrap(), moved);
        assert!(canvas.redo_history());
        assert_eq!(canvas.session.document.structure.nodes[0].name, "Reordered");
    }

    #[test]
    fn live_no_op_gesture_records_nothing_and_preserves_redo() {
        let mut canvas = CanvasView::new();
        let id = ObjectId::LANDING;

        begin_live_move(&mut canvas, &[id]);
        // Pointer goes down and up in the same place: a real click, not a drag.
        assert!(
            !drag_to(&mut canvas, point(0.0, 0.0)),
            "a pointer that never moved is a click, not a drag"
        );
        assert_eq!(
            canvas.session.history.undo_len(),
            0,
            "a gesture that ends where it started is not a committed edit"
        );

        // A real edit, undone, leaves a redo branch.
        begin_live_move(&mut canvas, &[id]);
        drag_to(&mut canvas, point(12.0, 0.0));
        let moved_x = canvas.session.runtime.geometry(id).unwrap().position.x;
        assert!(canvas.undo_history());
        assert!(canvas.session.history.can_redo());

        // A no-op afterwards must not destroy that redo branch.
        begin_live_move(&mut canvas, &[id]);
        drag_to(&mut canvas, point(0.0, 0.0));
        assert!(
            canvas.session.history.can_redo(),
            "a no-op must not clear the redo branch"
        );
        assert!(canvas.redo_history());
        assert_eq!(
            canvas.session.runtime.geometry(id).unwrap().position.x,
            moved_x,
            "redo restored the undone drag"
        );
    }

    /// Architectural invariant, proven through the live editor path.
    ///
    /// This drives a real [`CanvasView`] through the real gesture entry
    /// points (`update_interaction` / `finish_interaction` / `undo`) rather
    /// than calling [`EditSession`] directly, so it fails if the canvas ever
    /// stops routing committed edits through the semantic boundary.
    ///
    /// The claim under test: a canvas move and a metadata rename live in one
    /// stack and undo as one LIFO sequence. If the canvas kept a private
    /// history beside the session, the rename would be invisible here.
    #[test]
    fn live_canvas_and_metadata_share_one_history_stack() {
        use crate::operations::SemanticOperation;
        use crate::source_document::{LamineStructure, RenameNode, SourceBinding, StructuralNode};

        let mut canvas = CanvasView::new();

        // Give the session a real node so a metadata rename has a target.
        canvas.session.document.structure = LamineStructure {
            nodes: vec![StructuralNode {
                id: NodeId::new("spool-shared").unwrap(),
                name: "Shared".into(),
                kind: "frame".into(),
                parent: None,
                children: vec![],
                source: SourceBinding {
                    file: "index.html".into(),
                    selector: "[data-spool-id=\"spool-shared\"]".into(),
                },
            }],
        };
        assert_eq!(canvas.session.history.undo_len(), 0);

        // --- Live canvas gesture: pointer down, move, pointer up. ---
        let origin = point(0.0, 0.0);
        let pointer_end = point(60.0, 40.0);
        let resting = canvas
            .session
            .runtime
            .geometry(ObjectId::LANDING)
            .unwrap()
            .position;
        let expected = point(resting.x + 60.0, resting.y + 40.0);
        canvas.interaction = Interaction::PotentialMove(MoveGesture {
            pointer_start_screen: origin,
            pointer_start_world: origin,
            objects: snapshots(&canvas.session.runtime, &[ObjectId::LANDING]),
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        });
        assert!(canvas.update_interaction(pointer_end));
        canvas.finish_interaction(pointer_end);

        assert_eq!(
            canvas
                .session
                .runtime
                .geometry(ObjectId::LANDING)
                .unwrap()
                .position,
            expected,
            "the live gesture moved the object"
        );
        assert_eq!(
            canvas.session.history.undo_len(),
            1,
            "one entry for the move"
        );

        // --- A metadata rename on the same session, through the boundary. ---
        // `execute` is the single mutation entry point: it validates, records,
        // and applies. No separate apply call, or the rename would be applied
        // twice and the second attempt would correctly fail as stale.
        let rename = SemanticOperation::Rename(RenameNode {
            id: NodeId::new("spool-shared").unwrap(),
            before: "Shared".into(),
            after: "Renamed".into(),
        });
        assert!(canvas.session.execute(rename).unwrap());
        assert_eq!(canvas.session.document.structure.nodes[0].name, "Renamed");
        assert_eq!(
            canvas.session.history.undo_len(),
            2,
            "the rename and the move share one depth"
        );

        // --- Undo is strictly LIFO across both kinds. ---
        assert!(canvas.undo_history());
        assert_eq!(
            canvas.session.document.structure.nodes[0].name, "Shared",
            "the rename, committed second, undoes first"
        );
        assert_eq!(
            canvas
                .session
                .runtime
                .geometry(ObjectId::LANDING)
                .unwrap()
                .position,
            expected,
            "the move is untouched until the rename is undone"
        );

        assert!(canvas.undo_history());
        assert_eq!(
            canvas
                .session
                .runtime
                .geometry(ObjectId::LANDING)
                .unwrap()
                .position,
            resting,
            "then the live canvas move undoes"
        );
        assert_eq!(canvas.session.history.undo_len(), 0);

        // --- And redo replays both in the same order. ---
        assert!(canvas.redo_history());
        assert_eq!(
            canvas
                .session
                .runtime
                .geometry(ObjectId::LANDING)
                .unwrap()
                .position,
            expected
        );
        assert!(canvas.redo_history());
        assert_eq!(canvas.session.document.structure.nodes[0].name, "Renamed");
        assert_eq!(canvas.session.history.redo_len(), 0);
    }

    fn snapshots(document: &Document, ids: &[ObjectId]) -> Vec<ObjectSnapshot> {
        ids.iter()
            .map(|id| ObjectSnapshot {
                id: *id,
                geometry: document.geometry(*id).unwrap(),
            })
            .collect()
    }

    #[test]
    fn history_records_a_geometry_command() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();

        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn create_command_can_be_undone_and_redone_with_the_same_id() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        let created = document.create_object(
            ObjectType::Rectangle,
            point(40.0, 50.0),
            size(120.0, 80.0),
            None,
        );
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            document.placement(created.id).unwrap(),
        ])));
        assert_eq!(document.objects().len(), 5);

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.objects().len(), 4);
        assert!(document.object(created.id).is_none());
        let after_undo =
            document.create_object(ObjectType::Ellipse, point(0.0, 0.0), size(50.0, 50.0), None);
        assert_ne!(after_undo.id, created.id);

        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.object(created.id), Some(&created));
        assert_eq!(document.objects()[4].id, created.id);
        assert_eq!(document.objects().last().unwrap().id, after_undo.id);
    }

    #[test]
    fn create_move_undoes_geometry_before_creation() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(40.0, 50.0),
            size(120.0, 80.0),
            None,
        );
        let initial = object.geometry();
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            document.placement(object.id).unwrap(),
        ])));
        let moved = Geometry {
            position: point(90.0, 110.0),
            size: initial.size,
        };
        document.set_geometry(object.id, moved);
        history.record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
            GeometryChange {
                id: object.id,
                before: initial,
                after: moved,
            },
        ])));

        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.geometry(object.id), Some(initial));
        assert_eq!(document.objects().len(), 5);
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert!(document.object(object.id).is_none());

        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.geometry(object.id), Some(initial));
        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(document.geometry(object.id), Some(moved));
    }

    #[test]
    fn undo_restores_a_move_snapshot() {
        let mut document = Document::default();
        let original = document.geometry(ObjectId::LANDING).unwrap();
        let mut history = SemanticHistory::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());

        assert_eq!(document.geometry(ObjectId::LANDING), Some(original));
    }

    #[test]
    fn redo_reapplies_a_move_snapshot() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();

        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());

        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
    }

    #[test]
    fn multiple_commands_undo_in_reverse_order() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        record_position(&mut history, &mut document, ObjectId::LANDING, 200.0);

        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            0.0
        );
    }

    #[test]
    fn multiple_commands_redo_in_forward_order() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        record_position(&mut history, &mut document, ObjectId::LANDING, 200.0);
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();

        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
        history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            200.0
        );
    }

    #[test]
    fn multi_object_move_commits_as_one_command_and_undoes_together() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = snapshots(&canvas.session.runtime, &ids);
        let gesture = MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: before.clone(),
            selected_ids: ids.to_vec(),
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        };
        canvas.interaction = Interaction::Moving(gesture);
        canvas.finish_interaction(point(50.0, 25.0));

        assert_eq!(canvas.session.history.undo_len(), 1);
        match &match canvas.session.history.peek_undo() {
            Some(SemanticOperation::Runtime(command)) => &command.operation,
            _ => panic!("expected a recorded canvas command"),
        } {
            CommandOperation::Geometry(changes) => assert_eq!(changes.len(), 2),
            _ => panic!("expected geometry command"),
        }
        assert_eq!(
            canvas.session.runtime.geometry(ids[0]).unwrap().position,
            point(50.0, 49.0)
        );
        assert_eq!(
            canvas.session.runtime.geometry(ids[1]).unwrap().position,
            point(504.0, 49.0)
        );

        canvas.session.undo().unwrap();
        for snapshot in before {
            assert_eq!(
                canvas.session.runtime.geometry(snapshot.id),
                Some(snapshot.geometry)
            );
        }
    }

    #[test]
    fn redo_restores_every_object_in_a_multi_object_move() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = snapshots(&canvas.session.runtime, &ids);
        canvas.interaction = Interaction::Moving(MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: before,
            selected_ids: ids.to_vec(),
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        });
        canvas.finish_interaction(point(50.0, 25.0));
        let after: Vec<_> = ids
            .iter()
            .map(|id| canvas.session.runtime.geometry(*id).unwrap())
            .collect();
        canvas.session.undo().unwrap();

        assert!(canvas.session.redo().unwrap());

        for (id, geometry) in ids.into_iter().zip(after) {
            assert_eq!(canvas.session.runtime.geometry(id), Some(geometry));
        }
    }

    #[test]
    fn new_command_invalidates_redo_branch() {
        let mut document = Document::default();
        let mut history = SemanticHistory::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap();
        assert!(history.can_redo());

        record_position(&mut history, &mut document, ObjectId::LANDING, 250.0);

        assert!(!history.can_redo());
        assert_eq!(history.redo_len(), 0);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            250.0
        );
    }

    #[test]
    fn no_op_geometry_change_is_not_recorded() {
        let mut history = SemanticHistory::default();
        let geometry = test_geometry(10.0, 20.0, 100.0, 80.0);

        history.record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
            GeometryChange {
                id: ObjectId::LANDING,
                before: geometry,
                after: geometry,
            },
        ])));

        assert!(!history.can_undo());
    }

    #[test]
    fn resize_interaction_can_be_undone_and_redone() {
        let mut canvas = CanvasView::new();
        let snapshot = snapshots(&canvas.session.runtime, &[ObjectId::LANDING])[0];
        canvas.interaction = Interaction::Resizing(ResizeGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            object: snapshot,
            handle: ResizeHandle::Right,
        });
        canvas.finish_interaction(point(24.0, 0.0));
        let resized = canvas.session.runtime.geometry(ObjectId::LANDING).unwrap();

        assert_eq!(canvas.session.history.undo_len(), 1);
        assert!(canvas.session.undo().unwrap());
        assert_eq!(
            canvas.session.runtime.geometry(ObjectId::LANDING),
            Some(snapshot.geometry)
        );
        assert!(canvas.session.redo().unwrap());
        assert_eq!(
            canvas.session.runtime.geometry(ObjectId::LANDING),
            Some(resized)
        );
    }

    #[test]
    fn escape_cancellation_restores_geometry_without_history() {
        let mut document = Document::default();
        let snapshot = snapshots(&document, &[ObjectId::LANDING])[0];
        let gesture = MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        };
        apply_move(&mut document, &gesture.objects, point(80.0, 30.0));
        let interaction = Interaction::Moving(gesture);
        let history = SemanticHistory::default();

        interaction.restore(&mut document);

        assert_eq!(
            document.geometry(ObjectId::LANDING),
            Some(snapshot.geometry)
        );
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn click_or_return_to_start_does_not_create_history() {
        let mut canvas = CanvasView::new();
        let snapshot = snapshots(&canvas.session.runtime, &[ObjectId::LANDING])[0];
        canvas.interaction = Interaction::PotentialMove(MoveGesture {
            pointer_start_screen: point(10.0, 10.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        });
        canvas.finish_interaction(point(10.0, 10.0));
        assert!(!canvas.session.history.can_undo());

        canvas.interaction = Interaction::Moving(MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
            suspend_snap: false,
        });
        canvas.finish_interaction(point(0.0, 0.0));
        assert!(!canvas.session.history.can_undo());
    }

    #[test]
    fn selection_and_camera_changes_do_not_enter_history() {
        let mut canvas = CanvasView::new();
        canvas.selection.click(Some(ObjectId::LANDING), false);
        canvas.selection.click(Some(ObjectId::EDITOR), true);
        canvas.camera.zoom_at(2.0, point(100.0, 100.0));
        canvas
            .camera
            .pan_from(point(0.0, 0.0), point(0.0, 0.0), point(40.0, 20.0));
        canvas.camera.fit();

        assert!(!canvas.session.history.can_undo());
        assert!(!canvas.session.history.can_redo());
    }

    #[test]
    fn removing_and_restoring_object_preserves_its_original_index() {
        let mut document = Document::default();
        let first = document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(40.0, 30.0),
            None,
        );
        let second = document.create_object(
            ObjectType::Ellipse,
            point(60.0, 10.0),
            size(40.0, 30.0),
            None,
        );
        let expected = document.objects().to_vec();
        let deleted = document.remove_objects(&[first.id]);

        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].index, 4);
        assert_eq!(document.objects().last().unwrap().id, second.id);

        document.insert_objects(&deleted);

        assert_eq!(document.objects(), expected);
    }

    #[test]
    fn removing_multiple_objects_and_restoring_them_keeps_original_order() {
        let mut document = Document::default();
        let objects: Vec<_> = [ObjectType::Rectangle, ObjectType::Ellipse, ObjectType::Text]
            .into_iter()
            .enumerate()
            .map(|(index, object_type)| {
                document.create_object(
                    object_type,
                    point(index as f32 * 30.0, 20.0),
                    size(40.0, 30.0),
                    (object_type == ObjectType::Text).then(|| "remember me".to_string()),
                )
            })
            .collect();
        let expected = document.objects().to_vec();
        let deleted = document.remove_objects(&[objects[0].id, objects[2].id]);

        assert_eq!(
            deleted.iter().map(|item| item.index).collect::<Vec<_>>(),
            [4, 6]
        );
        document.insert_objects(&deleted);

        assert_eq!(document.objects(), expected);
    }

    #[test]
    fn duplicate_preserves_object_data_and_allocates_new_ids_and_names() {
        let mut document = Document::default();
        let original = document.create_object(
            ObjectType::Text,
            point(31.0, 47.0),
            size(180.0, 48.0),
            Some("Type something".to_string()),
        );

        let duplicates = document.duplicate_objects(&[original.id]);
        let duplicate = &duplicates[0].object;

        assert_ne!(duplicate.id, original.id);
        assert_ne!(duplicate.spool_id, original.spool_id);
        assert_eq!(duplicate.name, "Text 2");
        assert_eq!(duplicate.object_type, original.object_type);
        assert_eq!(duplicate.size, original.size);
        assert_eq!(duplicate.text_content, original.text_content);
        assert_eq!(duplicate.position, point(47.0, 63.0));
        assert_eq!(document.object(duplicate.id), Some(duplicate));
    }

    #[test]
    fn spool_node_identity_survives_history_and_is_unique_for_duplicates() {
        let mut document = Document::default();
        let original = document.create_object(
            ObjectType::Rectangle,
            point(10.0, 20.0),
            size(80.0, 50.0),
            None,
        );
        let original_node_id = original.spool_id.clone();
        let mut history = SemanticHistory::default();
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            document.placement(original.id).unwrap(),
        ])));

        let duplicate = document.duplicate_objects(&[original.id]).remove(0);
        assert_ne!(duplicate.object.spool_id, original_node_id);
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            duplicate.clone(),
        ])));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());

        assert_eq!(
            document.object(original.id).unwrap().spool_id,
            original_node_id
        );
        assert_eq!(
            document.object(duplicate.object.id).unwrap().spool_id,
            duplicate.object.spool_id
        );
        assert_ne!(
            document.object(original.id).unwrap().spool_id,
            document.object(duplicate.object.id).unwrap().spool_id
        );
    }

    #[test]
    fn duplicate_preserves_each_supported_object_type() {
        for object_type in [
            ObjectType::Frame,
            ObjectType::Rectangle,
            ObjectType::Ellipse,
            ObjectType::Text,
        ] {
            let mut document = Document::default();
            let original = document.create_object(
                object_type,
                point(20.0, 30.0),
                size(100.0, 80.0),
                (object_type == ObjectType::Text).then(|| "Type something".to_string()),
            );

            let duplicates = document.duplicate_objects(&[original.id]);
            let duplicate = &duplicates[0].object;

            assert_eq!(duplicate.object_type, object_type);
            assert_ne!(duplicate.id, original.id);
        }
    }

    #[test]
    fn duplicate_preserves_relative_order_and_is_topmost_for_hit_testing() {
        let mut document = Document::default();
        let lower = document.create_object(
            ObjectType::Rectangle,
            point(20.0, 20.0),
            size(100.0, 80.0),
            None,
        );
        let upper = document.create_object(
            ObjectType::Ellipse,
            point(20.0, 20.0),
            size(100.0, 80.0),
            None,
        );
        let duplicates = document.duplicate_objects(&[upper.id, lower.id]);
        let duplicate_ids: Vec<_> = duplicates
            .iter()
            .map(|placement| placement.object.id)
            .collect();

        assert_eq!(document.objects().len(), 8);
        assert_eq!(document.objects()[4].id, lower.id);
        assert_eq!(document.objects()[5].id, upper.id);
        assert_eq!(document.objects()[6].id, duplicate_ids[0]);
        assert_eq!(document.hit_test(point(40.0, 40.0)), Some(duplicate_ids[1]));
        assert_eq!(
            document.objects()[4..]
                .iter()
                .map(|object| object.id)
                .collect::<Vec<_>>(),
            [lower.id, upper.id, duplicate_ids[0], duplicate_ids[1]]
        );
    }

    #[test]
    fn duplicate_multiple_objects_preserves_document_order_and_offsets_each_by_sixteen() {
        let mut document = Document::default();
        let first = document.create_object(
            ObjectType::Rectangle,
            point(12.0, 14.0),
            size(40.0, 50.0),
            None,
        );
        let second = document.create_object(
            ObjectType::Ellipse,
            point(90.0, 100.0),
            size(60.0, 70.0),
            None,
        );
        let duplicates = document.duplicate_objects(&[second.id, first.id]);

        assert_eq!(duplicates[0].object.object_type, ObjectType::Rectangle);
        assert_eq!(duplicates[0].object.position, point(28.0, 30.0));
        assert_eq!(duplicates[1].object.object_type, ObjectType::Ellipse);
        assert_eq!(duplicates[1].object.position, point(106.0, 116.0));
    }

    #[test]
    fn deletion_is_one_history_command_and_undo_restores_all_original_data() {
        let mut document = Document::default();
        let first = document.create_object(
            ObjectType::Text,
            point(15.0, 25.0),
            size(180.0, 48.0),
            Some("Type something".to_string()),
        );
        let second = document.create_object(
            ObjectType::Ellipse,
            point(210.0, 125.0),
            size(64.0, 72.0),
            None,
        );
        let expected = document.objects().to_vec();
        let mut history = SemanticHistory::default();
        let deleted = document.remove_objects(&[first.id, second.id]);
        history.record(SemanticOperation::Runtime(DocumentCommand::delete(deleted)));

        assert_eq!(history.undo_len(), 1);
        assert_eq!(document.objects().len(), 4);
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.objects(), expected);
        assert_eq!(
            document.object(first.id).unwrap().text_content.as_deref(),
            Some("Type something")
        );
    }

    #[test]
    fn delete_undo_redo_restores_then_removes_same_object() {
        let mut document = Document::default();
        let created = document.create_object(
            ObjectType::Rectangle,
            point(20.0, 30.0),
            size(100.0, 50.0),
            None,
        );
        let mut history = SemanticHistory::default();
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            document.placement(created.id).unwrap(),
        ])));
        history.record(SemanticOperation::Runtime(DocumentCommand::delete(
            document.remove_objects(&[created.id]),
        )));

        assert!(document.object(created.id).is_none());
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.object(created.id), Some(&created));
        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(document.object(created.id).is_none());
    }

    #[test]
    fn duplicate_insert_undo_redo_keeps_duplicate_ids() {
        let mut document = Document::default();
        let original = document.create_object(
            ObjectType::Rectangle,
            point(30.0, 40.0),
            size(80.0, 60.0),
            None,
        );
        let mut history = SemanticHistory::default();
        let duplicate_objects = document.duplicate_objects(&[original.id]);
        let duplicate_id = duplicate_objects[0].object.id;
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(
            duplicate_objects,
        )));
        assert_eq!(history.undo_len(), 1);

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(document.object(original.id).is_some());
        assert!(document.object(duplicate_id).is_none());
        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.objects().last().unwrap().id, duplicate_id);
    }

    #[test]
    fn deletion_followed_by_creation_invalidates_redo_branch() {
        let mut document = Document::default();
        let target = document.create_object(
            ObjectType::Rectangle,
            point(15.0, 15.0),
            size(40.0, 40.0),
            None,
        );
        let mut history = SemanticHistory::default();
        history.record(SemanticOperation::Runtime(DocumentCommand::delete(
            document.remove_objects(&[target.id]),
        )));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(history.can_redo());

        let created = document.create_object(
            ObjectType::Ellipse,
            point(80.0, 80.0),
            size(50.0, 50.0),
            None,
        );
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(vec![
            document.placement(created.id).unwrap(),
        ])));

        assert!(!history.can_redo());
        assert!(document.object(target.id).is_some());
        assert!(document.object(created.id).is_some());
    }

    #[test]
    fn duplicate_then_move_undoes_movement_before_removing_duplicates() {
        let mut document = Document::default();
        let original = document.create_object(
            ObjectType::Rectangle,
            point(20.0, 30.0),
            size(80.0, 60.0),
            None,
        );
        let mut history = SemanticHistory::default();
        let duplicates = document.duplicate_objects(&[original.id]);
        let duplicate = duplicates[0].object.clone();
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(
            duplicates,
        )));
        let before_move = duplicate.geometry();
        let after_move = Geometry {
            position: point(before_move.position.x + 22.0, before_move.position.y + 9.0),
            size: before_move.size,
        };
        document.set_geometry(duplicate.id, after_move);
        history.record(SemanticOperation::Runtime(DocumentCommand::geometry(vec![
            GeometryChange {
                id: duplicate.id,
                before: before_move,
                after: after_move,
            },
        ])));

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.geometry(duplicate.id), Some(before_move));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(document.object(duplicate.id).is_none());
        assert!(document.object(original.id).is_some());
    }

    #[test]
    fn selection_delete_clears_ids_and_selection_reconciliation_drops_missing_objects() {
        let mut canvas = CanvasView::new();
        let created = canvas.session.runtime.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(60.0, 40.0),
            None,
        );
        canvas.selection.click(Some(created.id), false);
        assert!(canvas.delete_selected_objects());
        assert!(canvas.selection.is_empty());
        assert!(canvas.session.runtime.object(created.id).is_none());
        assert_eq!(canvas.session.history.undo_len(), 1);

        canvas.session.undo().unwrap();
        canvas.retain_existing_selection();
        assert!(canvas.selection.is_empty());
        canvas.session.redo().unwrap();
        canvas.retain_existing_selection();
        assert!(canvas.selection.is_empty());
    }

    #[test]
    fn duplication_selects_only_the_duplicates_and_is_one_command() {
        let mut canvas = CanvasView::new();
        let first = canvas.session.runtime.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(60.0, 40.0),
            None,
        );
        let second = canvas.session.runtime.create_object(
            ObjectType::Ellipse,
            point(100.0, 100.0),
            size(60.0, 40.0),
            None,
        );
        canvas.selection.replace(vec![second.id, first.id]);

        assert!(canvas.duplicate_selected_objects());

        let selected = canvas.selection.ids();
        assert_eq!(selected.len(), 2);
        assert!(!selected.contains(&first.id));
        assert!(!selected.contains(&second.id));
        assert_eq!(canvas.session.history.undo_len(), 1);
        assert_eq!(
            canvas
                .session
                .runtime
                .object(selected[0])
                .unwrap()
                .object_type,
            ObjectType::Rectangle
        );
        assert_eq!(
            canvas
                .session
                .runtime
                .object(selected[1])
                .unwrap()
                .object_type,
            ObjectType::Ellipse
        );
        assert!(selected
            .iter()
            .all(|id| canvas.session.runtime.object(*id).is_some()));
    }

    #[test]
    fn deleting_multiple_selected_objects_is_one_command_and_undo_restores_them() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = canvas.session.runtime.objects().to_vec();
        canvas.selection.replace(ids.to_vec());

        assert!(canvas.delete_selected_objects());

        assert_eq!(canvas.session.history.undo_len(), 1);
        assert!(canvas.selection.is_empty());
        assert!(canvas.session.runtime.object(ids[0]).is_none());
        assert!(canvas.session.runtime.object(ids[1]).is_none());
        assert!(canvas.session.undo().unwrap());
        assert_eq!(canvas.session.runtime.objects(), before);
    }

    #[test]
    fn duplicate_without_selection_and_delete_without_selection_are_no_ops() {
        let mut canvas = CanvasView::new();

        assert!(!canvas.delete_selected_objects());
        assert!(!canvas.duplicate_selected_objects());
        assert_eq!(canvas.session.runtime.objects().len(), 4);
        assert!(!canvas.session.history.can_undo());
    }

    #[test]
    fn object_types_receive_their_expected_default_styles() {
        let rectangle = default_style(ObjectType::Rectangle);
        let ellipse = default_style(ObjectType::Ellipse);
        let frame_style = default_style(ObjectType::Frame);
        let text = default_style(ObjectType::Text);

        assert_eq!(
            rectangle.fill.unwrap().color.to_rgb(),
            theme::SURFACE_RAISED
        );
        assert_eq!(rectangle.stroke.unwrap().color.to_rgb(), theme::BORDER);
        assert_eq!(rectangle.stroke.unwrap().width, 1.0);
        assert_eq!(ellipse, rectangle);
        assert_eq!(frame_style.fill.unwrap().color.to_rgb(), theme::PAPER);
        assert_eq!(frame_style.stroke.unwrap().color.to_rgb(), theme::BORDER);
        assert_eq!(frame_style.stroke.unwrap().width, 1.0);
        assert_eq!(
            text,
            ObjectStyle {
                fill: None,
                stroke: None,
                ..ObjectStyle::default()
            }
        );
    }

    fn apply_style_to_document(
        document: &mut Document,
        history: &mut SemanticHistory,
        ids: &[ObjectId],
        edit: StyleEdit,
    ) {
        let changes: Vec<_> = ids
            .iter()
            .filter_map(|id| {
                let before = document.appearance(*id)?;
                let after = edited_style(before, edit);
                (before != after).then_some(StyleChange {
                    id: *id,
                    before,
                    after,
                })
            })
            .collect();
        for change in &changes {
            document.set_appearance(change.id, change.after);
        }
        history.record(SemanticOperation::Runtime(DocumentCommand::style(changes)));
    }

    #[test]
    fn fill_and_stroke_properties_can_be_changed_or_disabled() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(80.0, 60.0),
            None,
        );
        let green = Color::from_rgb(theme::SAGE);
        let red = Color::from_rgb(0xc45d5d);
        let mut history = SemanticHistory::default();

        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(green)),
        );
        assert_eq!(
            document.object(object.id).unwrap().fill,
            Some(Fill { color: green })
        );
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(None),
        );
        assert_eq!(document.object(object.id).unwrap().fill, None);
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Stroke(Some(red)),
        );
        assert_eq!(
            document.object(object.id).unwrap().stroke.unwrap().color,
            red
        );
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::StrokeWidth(4.0),
        );
        assert_eq!(
            document.object(object.id).unwrap().stroke.unwrap().width,
            4.0
        );
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Stroke(None),
        );
        assert_eq!(document.object(object.id).unwrap().stroke, None);
    }

    #[test]
    fn style_command_undo_redo_and_no_op_filtering_work() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(80.0, 60.0),
            None,
        );
        let default = document.style(object.id).unwrap();
        let mut history = SemanticHistory::default();
        let blue = Color::from_rgb(0x6689c7);

        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(blue)),
        );
        let changed = document.style(object.id).unwrap();
        assert_eq!(history.undo_len(), 1);
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.style(object.id), Some(default));
        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.style(object.id), Some(changed));

        let commands = history.undo_len();
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(blue)),
        );
        assert_eq!(history.undo_len(), commands);
    }

    #[test]
    fn style_changes_are_one_command_for_multi_selection_and_preserve_selection() {
        let mut canvas = CanvasView::new();
        let rectangle = canvas.session.runtime.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(80.0, 60.0),
            None,
        );
        let text = canvas.session.runtime.create_object(
            ObjectType::Text,
            point(110.0, 10.0),
            size(80.0, 60.0),
            Some("Type something".to_string()),
        );
        canvas.selection.replace(vec![rectangle.id, text.id]);
        let selected = canvas.selection.ids().to_vec();
        let fill = Color::from_rgb(0x6689c7);

        assert!(canvas.apply_selected_style(StyleEdit::Fill(Some(fill))));

        assert_eq!(canvas.session.history.undo_len(), 1);
        assert_eq!(canvas.selection.ids(), selected);
        assert!(selected.iter().all(|id| {
            canvas.session.runtime.object(*id).unwrap().fill == Some(Fill { color: fill })
        }));
        assert!(matches!(match canvas.session.history.peek_undo() {
                Some(SemanticOperation::Runtime(command)) => &command.operation,
                _ => panic!("expected a recorded canvas command"),
            }, CommandOperation::Style(ref changes) if changes.len() == 2));

        let stroke = Color::from_rgb(0xc45d5d);
        assert!(canvas.apply_selected_style(StyleEdit::Stroke(Some(stroke))));
        assert!(selected.iter().all(|id| {
            canvas
                .session
                .runtime
                .object(*id)
                .unwrap()
                .stroke
                .map(|value| value.color)
                == Some(stroke)
        }));
        assert_eq!(canvas.session.history.undo_len(), 2);
    }

    #[test]
    fn style_change_then_move_undoes_in_command_order() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(20.0, 30.0),
            size(80.0, 60.0),
            None,
        );
        let mut history = SemanticHistory::default();
        let default_style = document.style(object.id).unwrap();
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(Color::from_rgb(0x6689c7))),
        );
        let styled = document.style(object.id).unwrap();
        record_position(&mut history, &mut document, object.id, 140.0);

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.object(object.id).unwrap().position.x, 20.0);
        assert_eq!(document.style(object.id), Some(styled));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.style(object.id), Some(default_style));
    }

    #[test]
    fn style_is_preserved_by_duplicate_delete_restore_and_insertion_history() {
        let mut document = Document::default();
        let original = document.create_object(
            ObjectType::Text,
            point(20.0, 30.0),
            size(180.0, 48.0),
            Some("styled text".to_string()),
        );
        let mut history = SemanticHistory::default();
        let fill = Color::from_rgb(0x9576b8);
        apply_style_to_document(
            &mut document,
            &mut history,
            &[original.id],
            StyleEdit::Fill(Some(fill)),
        );
        let duplicate = document.duplicate_objects(&[original.id]);
        let duplicate_object = duplicate[0].object.clone();
        assert_eq!(duplicate_object.fill, Some(Fill { color: fill }));
        assert_eq!(
            duplicate_object.text_content.as_deref(),
            Some("styled text")
        );
        history.record(SemanticOperation::Runtime(DocumentCommand::insert(
            duplicate,
        )));

        history.record(SemanticOperation::Runtime(DocumentCommand::delete(
            document.remove_objects(&[original.id]),
        )));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(
            document.object(original.id).unwrap().fill,
            Some(Fill { color: fill })
        );
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(document.object(duplicate_object.id).is_none());
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(
            document.object(original.id).unwrap().fill,
            default_style(ObjectType::Text).fill
        );
    }

    fn create_test_text(canvas: &mut CanvasView, text: &str) -> DesignObject {
        canvas.session.runtime.create_object(
            ObjectType::Text,
            point(45.0, 60.0),
            size(180.0, 48.0),
            Some(text.to_owned()),
        )
    }

    #[test]
    fn text_objects_have_default_content_and_document_text_api_mutates_only_text() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Text,
            point(0.0, 0.0),
            size(180.0, 48.0),
            Some("Type something".to_owned()),
        );
        let geometry = object.geometry();
        let style = document.style(object.id).unwrap();

        assert_eq!(document.text_content(object.id), Some("Type something"));
        assert!(document.set_text_content(object.id, "Hello world".to_owned()));
        assert_eq!(document.text_content(object.id), Some("Hello world"));
        assert_eq!(document.geometry(object.id), Some(geometry));
        assert_eq!(document.style(object.id), Some(style));
        assert!(!document.set_text_content(ObjectId::LANDING, "not text".to_owned()));
    }

    #[test]
    fn text_change_command_undoes_and_redoes_exact_content() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Text,
            point(0.0, 0.0),
            size(180.0, 48.0),
            Some("before".to_owned()),
        );
        let mut history = SemanticHistory::default();
        document.set_text_content(object.id, "after 🧵".to_owned());
        history.record(SemanticOperation::Runtime(DocumentCommand::text(vec![
            TextChange {
                id: object.id,
                before: "before".to_owned(),
                after: "after 🧵".to_owned(),
            },
        ])));

        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.text_content(object.id), Some("before"));
        assert!(history
            .redo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert_eq!(document.text_content(object.id), Some("after 🧵"));
    }

    #[test]
    fn unchanged_text_change_is_not_recorded_and_new_edit_clears_redo() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Text,
            point(0.0, 0.0),
            size(180.0, 48.0),
            Some("same".to_owned()),
        );
        let mut history = SemanticHistory::default();
        history.record(SemanticOperation::Runtime(DocumentCommand::text(vec![
            TextChange {
                id: object.id,
                before: "same".to_owned(),
                after: "same".to_owned(),
            },
        ])));
        assert!(!history.can_undo());

        document.set_text_content(object.id, "edited".to_owned());
        history.record(SemanticOperation::Runtime(DocumentCommand::text(vec![
            TextChange {
                id: object.id,
                before: "same".to_owned(),
                after: "edited".to_owned(),
            },
        ])));
        assert!(history
            .undo(&mut OperationTarget::Runtime(&mut document))
            .unwrap());
        assert!(history.can_redo());
        document.set_text_content(object.id, "new edit".to_owned());
        history.record(SemanticOperation::Runtime(DocumentCommand::text(vec![
            TextChange {
                id: object.id,
                before: "same".to_owned(),
                after: "new edit".to_owned(),
            },
        ])));
        assert!(!history.can_redo());
        assert_eq!(document.text_content(object.id), Some("new edit"));
    }

    #[test]
    fn text_content_survives_duplicate_and_delete_restore() {
        let mut document = Document::default();
        let object = document.create_object(
            ObjectType::Text,
            point(0.0, 0.0),
            size(180.0, 48.0),
            Some("keep 🧵".to_owned()),
        );
        let duplicate = document.duplicate_objects(&[object.id]).remove(0);
        assert_eq!(document.text_content(duplicate.object.id), Some("keep 🧵"));
        let removed = document.remove_objects(&[object.id]);
        document.insert_objects(&removed);
        assert_eq!(document.text_content(object.id), Some("keep 🧵"));
    }

    #[test]
    fn multiple_character_edits_commit_as_one_history_command_and_leave_selection() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "Type something");
        canvas.selection.click(Some(object.id), false);
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "Type something".to_owned(),
            editing_text: "Hello world".to_owned(),
            selected_range: 11..11,
            selection_reversed: false,
            marked_range: None,
            pointer_anchor: None,
        });

        assert!(canvas.commit_text_edit());
        assert_eq!(
            canvas.session.runtime.text_content(object.id),
            Some("Hello world")
        );
        assert_eq!(canvas.session.history.undo_len(), 1);
        assert_eq!(canvas.selection.ids(), &[object.id]);
        assert!(matches!(
            match canvas.session.history.peek_undo() {
                Some(SemanticOperation::Runtime(command)) => &command.operation,
                _ => panic!("expected a recorded canvas command"),
            },
            CommandOperation::Text(_)
        ));
    }

    #[test]
    fn escape_discards_text_buffer_without_mutation_or_history() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "Original");
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "Original".to_owned(),
            editing_text: "Cancelled edit".to_owned(),
            selected_range: 14..14,
            selection_reversed: false,
            marked_range: None,
            pointer_anchor: None,
        });

        assert!(canvas.discard_text_edit());
        assert!(!canvas.is_text_editing());
        assert_eq!(
            canvas.session.runtime.text_content(object.id),
            Some("Original")
        );
        assert!(!canvas.session.history.can_undo());
    }

    #[test]
    fn text_edits_do_not_change_geometry_or_style() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "before");
        let geometry = object.geometry();
        let style = canvas.session.runtime.style(object.id).unwrap();
        canvas.selection.click(Some(object.id), false);
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "before".to_owned(),
            editing_text: "after".to_owned(),
            selected_range: 5..5,
            selection_reversed: false,
            marked_range: None,
            pointer_anchor: None,
        });
        canvas.commit_text_edit();

        assert_eq!(canvas.session.runtime.geometry(object.id), Some(geometry));
        assert_eq!(canvas.session.runtime.style(object.id), Some(style));
    }

    #[test]
    fn text_hit_testing_places_caret_before_and_after_text() {
        let positions = [
            (0, 0.0),
            (1, 6.0),
            (2, 12.0),
            (3, 18.0),
            (4, 24.0),
            (5, 30.0),
        ];
        assert_eq!(
            nearest_boundary_from_positions("Hello", &positions, -20.0),
            0
        );
        assert_eq!(
            nearest_boundary_from_positions("Hello", &positions, 80.0),
            5
        );
    }

    #[test]
    fn text_hit_testing_uses_nearest_shaped_character_boundary() {
        let text = "Hello world";
        let positions = [
            (0, 0.0),
            (1, 7.0),
            (2, 12.0),
            (3, 18.0),
            (4, 24.0),
            (5, 29.0),
            (6, 33.0),
            (7, 40.0),
            (8, 46.0),
            (9, 52.0),
            (10, 58.0),
            (11, 64.0),
        ];
        assert_eq!(nearest_boundary_from_positions(text, &positions, 9.0), 1);
        assert_eq!(nearest_boundary_from_positions(text, &positions, 32.0), 6);
    }

    #[test]
    fn text_hit_testing_handles_empty_text_and_unicode_boundaries() {
        assert_eq!(nearest_boundary_from_positions("", &[(0, 0.0)], 10.0), 0);

        let text = "Café 👋";
        let positions = text
            .char_indices()
            .map(|(offset, _)| (offset, offset as f32))
            .chain(std::iter::once((text.len(), text.len() as f32)))
            .collect::<Vec<_>>();
        let nearest = nearest_boundary_from_positions(text, &positions, 5.8);
        assert!(text.is_char_boundary(nearest));
        assert_eq!(nearest, 6);
    }

    #[test]
    fn text_pointer_coordinates_respect_camera_zoom() {
        let object_position = point(120.0, 80.0);
        let local_world = point(32.0, 18.0);
        for zoom in [0.5, 1.0, 2.0] {
            let camera = Camera {
                offset: point(15.0, 10.0),
                zoom,
                viewport: size(800.0, 600.0),
                initialized: true,
                pending_fit: None,
            };
            let world = point(
                object_position.x + local_world.x,
                object_position.y + local_world.y,
            );
            let screen = camera.world_to_screen(world);
            assert_eq!(
                screen_to_object_local(camera, screen, object_position),
                point(local_world.x * zoom, local_world.y * zoom)
            );
        }
    }

    #[test]
    fn forward_and_reverse_pointer_drags_create_normalized_selection() {
        assert_eq!(
            selection_from_anchor_and_caret("Hello world", 2, 8),
            (2..8, false)
        );
        assert_eq!(
            selection_from_anchor_and_caret("Hello world", 8, 2),
            (2..8, true)
        );
    }

    #[test]
    fn shift_click_keeps_the_existing_selection_anchor() {
        let range = 3..9;
        let anchor = selection_anchor(&range, false);
        assert_eq!(
            selection_from_anchor_and_caret("Hello world", anchor, 10),
            (3..10, false)
        );

        let reverse_range = 3..9;
        let reverse_anchor = selection_anchor(&reverse_range, true);
        assert_eq!(
            selection_from_anchor_and_caret("Hello world", reverse_anchor, 1),
            (1..9, true)
        );
    }

    #[test]
    fn pointer_selection_does_not_change_document_or_history() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "Hello world");
        let geometry = object.geometry();
        let style = canvas.session.runtime.style(object.id).unwrap();
        let selection = selection_from_anchor_and_caret("Hello world", 1, 7);
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "Hello world".to_owned(),
            editing_text: "Hello world".to_owned(),
            selected_range: selection.0,
            selection_reversed: selection.1,
            marked_range: None,
            pointer_anchor: Some(1),
        });

        assert_eq!(
            canvas.session.runtime.text_content(object.id),
            Some("Hello world")
        );
        assert_eq!(canvas.session.runtime.geometry(object.id), Some(geometry));
        assert_eq!(canvas.session.runtime.style(object.id), Some(style));
        assert!(!canvas.session.history.can_undo());
    }

    #[test]
    fn text_edit_after_pointer_selection_commits_once_and_keeps_object_selected() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "Hello world");
        canvas.selection.click(Some(object.id), false);
        let (selected_range, selection_reversed) =
            selection_from_anchor_and_caret("Hello world", 0, 5);
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "Hello world".to_owned(),
            editing_text: "Hello".to_owned(),
            selected_range: 5..5,
            selection_reversed: false,
            marked_range: None,
            pointer_anchor: None,
        });
        let edit = canvas.text_edit.as_mut().unwrap();
        edit.selected_range = selected_range;
        edit.selection_reversed = selection_reversed;

        assert!(canvas.commit_text_edit());
        assert_eq!(
            canvas.session.runtime.text_content(object.id),
            Some("Hello")
        );
        assert_eq!(canvas.selection.ids(), &[object.id]);
        assert_eq!(canvas.session.history.undo_len(), 1);
    }

    #[test]
    fn escape_after_pointer_selection_discards_text_without_history() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "Original");
        let (selected_range, selection_reversed) = selection_from_anchor_and_caret("Changed", 1, 4);
        canvas.text_edit = Some(TextEditState {
            id: object.id,
            original_text: "Original".to_owned(),
            editing_text: "Changed".to_owned(),
            selected_range,
            selection_reversed,
            marked_range: None,
            pointer_anchor: None,
        });

        assert!(canvas.discard_text_edit());
        assert_eq!(
            canvas.session.runtime.text_content(object.id),
            Some("Original")
        );
        assert!(!canvas.session.history.can_undo());
    }

    #[test]
    fn utf16_ranges_map_to_valid_utf8_boundaries_for_ime_and_selection() {
        let text = "a🧵é";
        assert_eq!(utf16_range_to_utf8(text, 1..3), 1..5);
        assert_eq!(utf8_range_to_utf16(text, 1..5), 1..3);
        assert_eq!(utf16_range_to_utf8(text, 3..4), 5..7);
    }

    // Phase 14: conservative viewport-aware culling.

    fn culling_camera(zoom: f32, offset: Point<f32>) -> Camera {
        Camera {
            offset,
            zoom,
            viewport: size(400.0, 400.0),
            initialized: true,
            pending_fit: None,
        }
    }

    fn culling_object(id: u64, position: Point<f32>, object_size: Size<f32>) -> DesignObject {
        DesignObject {
            id: ObjectId(id),
            spool_id: node_id(format!("cull-probe-{id}")),
            name: "Cull probe".to_owned(),
            position,
            size: object_size,
            object_type: ObjectType::Rectangle,
            text_content: None,
            text_color: None,
            font_size: None,
            fill: default_style(ObjectType::Rectangle).fill,
            stroke: None,
            border_radius: default_style(ObjectType::Rectangle).border_radius,
            opacity: 1.0,
        }
    }

    #[test]
    fn culling_predicate_covers_inside_partial_edge_and_outside_cases() {
        let camera = culling_camera(1.0, point(0.0, 0.0));

        // Fully inside.
        assert!(camera.affects_viewport(point(100.0, 100.0), size(50.0, 50.0)));
        // Partially intersecting the right edge.
        assert!(camera.affects_viewport(point(370.0, 100.0), size(50.0, 50.0)));
        // Touching the viewport edge exactly: inclusive model.
        assert!(camera.affects_viewport(point(400.0, 100.0), size(50.0, 50.0)));
        assert!(camera.affects_viewport(point(-50.0, 100.0), size(50.0, 50.0)));
        // Fully outside the viewport but within the 64-unit padding.
        assert!(camera.affects_viewport(point(460.0, 100.0), size(50.0, 50.0)));
        assert!(camera.affects_viewport(point(100.0, 460.0), size(50.0, 50.0)));
        // Exactly at the padding boundary stays inclusive.
        assert!(camera.affects_viewport(point(464.0, 100.0), size(50.0, 50.0)));
        // Just beyond the padding.
        assert!(!camera.affects_viewport(point(465.0, 100.0), size(50.0, 50.0)));
        assert!(!camera.affects_viewport(point(100.0, 465.0), size(50.0, 50.0)));
        // Fully outside, far beyond the padding.
        assert!(!camera.affects_viewport(point(600.0, 600.0), size(50.0, 50.0)));
    }

    #[test]
    fn culling_predicate_handles_negative_world_coordinates() {
        let camera = culling_camera(1.0, point(-500.0, -500.0));
        // Maps to screen (10,10)-(60,60): inside.
        assert!(camera.affects_viewport(point(-490.0, -490.0), size(50.0, 50.0)));
        // Maps to screen (-60,-60): outside, but within padding of the edge.
        assert!(camera.affects_viewport(point(-560.0, -560.0), size(50.0, 50.0)));
        // Far outside the padded viewport in negative world space.
        assert!(!camera.affects_viewport(point(-1100.0, -490.0), size(50.0, 50.0)));
        // An object entirely in negative coordinates visible to a camera there.
        let deep = culling_camera(1.0, point(-5_000.0, -5_000.0));
        assert!(deep.affects_viewport(point(-4_990.0, -4_990.0), size(50.0, 50.0)));
        assert!(!deep.affects_viewport(point(-6_000.0, -4_990.0), size(50.0, 50.0)));
    }

    #[test]
    fn culling_predicate_keeps_large_spanning_objects() {
        let camera = culling_camera(1.0, point(0.0, 0.0));
        assert!(camera.affects_viewport(point(-10_000.0, -10_000.0), size(20_000.0, 20_000.0)));
        // Origin far outside the viewport, box still spans it in x.
        assert!(camera.affects_viewport(point(-1_000_000.0, 100.0), size(1_500_000.0, 50.0)));
        let zoomed_out = culling_camera(0.25, point(0.0, 0.0));
        assert!(zoomed_out.affects_viewport(point(-40_000.0, -40_000.0), size(80_000.0, 80_000.0)));
    }

    #[test]
    fn culling_predicate_tracks_zoom_and_pan() {
        let base = culling_camera(1.0, point(0.0, 0.0));
        let zoomed_out = culling_camera(0.5, point(0.0, 0.0));
        let zoomed_in = culling_camera(4.0, point(0.0, 0.0));
        let far = point(600.0, 100.0);
        let near = point(200.0, 100.0);

        // Same world object: visible when zoomed out, culled at 1x and 4x.
        assert!(zoomed_out.affects_viewport(far, size(50.0, 50.0)));
        assert!(!base.affects_viewport(far, size(50.0, 50.0)));
        assert!(!zoomed_in.affects_viewport(far, size(50.0, 50.0)));

        // Same world object: visible at 1x and 0.5x, culled when zoomed to 4x.
        assert!(base.affects_viewport(near, size(50.0, 50.0)));
        assert!(zoomed_out.affects_viewport(near, size(50.0, 50.0)));
        assert!(!zoomed_in.affects_viewport(near, size(50.0, 50.0)));

        // Panning brings a culled object into the padded viewport...
        let panned = culling_camera(1.0, point(300.0, 0.0));
        assert!(panned.affects_viewport(far, size(50.0, 50.0)));
        // ...and panning away culls it again.
        let away = culling_camera(1.0, point(-1_000.0, 0.0));
        assert!(base.affects_viewport(point(100.0, 100.0), size(50.0, 50.0)));
        assert!(!away.affects_viewport(point(100.0, 100.0), size(50.0, 50.0)));
    }

    #[test]
    fn culling_predicate_constructs_when_inputs_cannot_be_evaluated() {
        // Empty viewport before the first prepaint resize: construct, never cull.
        let uninitialized = Camera::default();
        assert_eq!(uninitialized.viewport, size(0.0, 0.0));
        assert!(uninitialized.affects_viewport(point(5_000.0, 5_000.0), size(50.0, 50.0)));

        let camera = culling_camera(1.0, point(0.0, 0.0));
        // Non-positive zoom.
        assert!(Camera {
            zoom: 0.0,
            ..camera
        }
        .affects_viewport(point(5_000.0, 5_000.0), size(50.0, 50.0)));
        // Non-finite camera offset.
        assert!(Camera {
            offset: point(f32::NAN, 0.0),
            ..camera
        }
        .affects_viewport(point(5_000.0, 5_000.0), size(50.0, 50.0)));
        // Non-finite geometry.
        assert!(camera.affects_viewport(point(f32::NAN, 100.0), size(50.0, 50.0)));
        assert!(camera.affects_viewport(point(5_000.0, 5_000.0), size(f32::INFINITY, 50.0),));
        // Negative size.
        assert!(camera.affects_viewport(point(5_000.0, 5_000.0), size(-50.0, -50.0),));
    }

    #[test]
    fn culling_includes_every_object_the_diagnostic_model_sees() {
        for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
            for offset in [point(0.0, 0.0), point(-1_000.0, 500.0)] {
                let camera = culling_camera(zoom, offset);
                for position in [-300.0_f32, -150.0, 0.0, 150.0, 350.0, 399.0, 450.0, 700.0] {
                    for object_size in [size(10.0, 10.0), size(80.0, 80.0), size(1_000.0, 60.0)] {
                        if diagnostics::intersects(
                            point(position, position),
                            object_size,
                            offset,
                            camera.viewport,
                            zoom,
                        ) {
                            assert!(
                                camera.affects_viewport(point(position, position), object_size),
                                "culling must retain every object the Phase 13 diagnostic \
                                 visibility model sees: position {position} size {object_size:?} \
                                 zoom {zoom} offset {offset:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn culling_exempts_only_the_active_text_edit_object() {
        let camera = culling_camera(1.0, point(0.0, 0.0));
        let inside = culling_object(5, point(100.0, 100.0), size(50.0, 50.0));
        let outside = culling_object(6, point(5_000.0, 5_000.0), size(50.0, 50.0));

        // Geometry gate without editing.
        assert!(should_construct(camera, &inside, None));
        assert!(!should_construct(camera, &outside, None));
        // The actively edited object is constructed even far outside the viewport.
        assert!(should_construct(camera, &outside, Some(outside.id)));
        // A different object being edited does not exempt the offscreen one.
        assert!(!should_construct(camera, &outside, Some(inside.id)));
        // An inside object is constructed whether or not it is being edited.
        assert!(should_construct(camera, &inside, Some(inside.id)));

        // Selection must not gate base-element construction: selection outlines
        // and resize handles are constructed separately in `artboards`, so an
        // offscreen selected object stays culled without losing its chrome.
        let mut selection = Selection::default();
        selection.click(Some(outside.id), false);
        assert_eq!(selection.ids(), &[outside.id]);
        assert!(!should_construct(camera, &outside, None));
    }

    // ---------------------------------------------------------------------
    // Editing a source-backed project
    //
    // These cover the product loop the milestone is about: a loaded project is
    // an ordinary editable document, and what the editor changes is written
    // back to the authored source it came from.
    // ---------------------------------------------------------------------

    /// Copy a fixture into a temp directory so a save cannot touch the repo.
    fn project_scratch(fixture: &str) -> std::path::PathBuf {
        let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(fixture);
        let to = std::env::temp_dir().join(format!(
            "spool-canvas-{fixture}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&to);
        copy_tree(&from, &to).expect("copy fixture");
        to
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }

    fn project_view(root: &std::path::Path) -> CanvasView {
        let loaded = crate::project_open::open_project(root).expect("project opens");
        let mut view = CanvasView::new();
        view.load_project(loaded);
        view
    }

    fn object_with_node(view: &CanvasView, node: &str) -> DesignObject {
        view.document_objects()
            .iter()
            .find(|object| object.spool_id.as_str() == node)
            .unwrap_or_else(|| panic!("{node} is projected"))
            .clone()
    }

    #[test]
    fn a_created_object_never_reuses_a_loaded_identity() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let loaded_ids: Vec<String> = view
            .document_objects()
            .iter()
            .map(|object| object.spool_id.as_str().to_owned())
            .collect();
        assert_eq!(
            loaded_ids.len(),
            3,
            "the fixture has three projected objects"
        );

        // The counter behind `allocate_node_id` restarts at 1 when a project is
        // loaded, so a freshly created object's candidate identity collides with
        // an authored one unless allocation checks what is live.
        for index in 0..5 {
            let created = view.session.runtime.create_object(
                ObjectType::Rectangle,
                point(index as f32 * 10.0, 0.0),
                size(10.0, 10.0),
                None,
            );
            assert!(
                !loaded_ids.contains(&created.spool_id.as_str().to_owned()),
                "a new object must not take the identity of {:?}",
                created.spool_id.as_str()
            );
        }

        // And identities stay unique across the whole document, not just against
        // the loaded set.
        let all: Vec<&str> = view
            .document_objects()
            .iter()
            .map(|object| object.spool_id.as_str())
            .collect();
        let mut unique = all.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), all.len(), "every live identity is unique");
    }

    #[test]
    fn a_new_identity_is_never_one_that_is_already_live() {
        // The counter that feeds `allocate_node_id` is set from scratch when a
        // projected document is loaded, so its next value can name an identity
        // that is already in the scene. Allocation has to notice and move on;
        // without that check the editor would end up with two objects claiming
        // one identity, and every later lookup by identity would be ambiguous.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let first = view.session.runtime.create_object(
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
            None,
        );

        // Rewind the counter to the value it would hand out next time.
        view.session.runtime.next_node_id = 1;
        let second = view.session.runtime.create_object(
            ObjectType::Rectangle,
            point(10.0, 0.0),
            size(10.0, 10.0),
            None,
        );
        assert_ne!(
            first.spool_id, second.spool_id,
            "a live identity was handed out twice"
        );
        assert_eq!(
            first.spool_id,
            node_id(format!("spool-node-{:016x}", 1)),
            "the first object took the identity the counter offered"
        );
    }

    #[test]
    fn deleting_an_object_does_not_free_its_identity_for_reuse() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");

        let removed = view.session.runtime.remove_objects(&[cta.id]);
        view.commit(DocumentCommand::delete(removed));
        assert!(
            view.document_objects()
                .iter()
                .all(|object| object.spool_id != cta.spool_id),
            "the deleted object is gone from the runtime"
        );
        assert!(
            view.session
                .document
                .structure
                .nodes
                .iter()
                .any(|node| node.id == cta.spool_id),
            "and its identity is still live in the persistent document"
        );

        // A new object minted after the deletion must still not land on it: the
        // identity belongs to the document, not to the runtime object that used
        // to hold it.
        let created = view.session.runtime.create_object(
            ObjectType::Rectangle,
            point(0.0, 0.0),
            size(10.0, 10.0),
            None,
        );
        assert_ne!(created.spool_id, cta.spool_id);
    }

    #[test]
    fn duplicating_a_projected_object_gives_it_new_identities() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let originals: Vec<String> = view
            .document_objects()
            .iter()
            .map(|object| object.spool_id.as_str().to_owned())
            .collect();

        let duplicates = view.session.runtime.duplicate_objects(&[cta.id]);
        view.commit(DocumentCommand::insert(duplicates.clone()));
        assert_eq!(duplicates.len(), 1);

        let copy = &duplicates[0].object;
        assert_ne!(copy.id, cta.id, "a new runtime key");
        assert_ne!(copy.spool_id, cta.spool_id, "a new persistent identity");
        assert!(
            !originals.contains(&copy.spool_id.as_str().to_owned()),
            "the duplicate does not take an existing identity"
        );
        assert_eq!(
            copy.text_content, cta.text_content,
            "and it carries the same authored text"
        );

        // The original is untouched and both remain in the scene.
        let after: Vec<String> = view
            .document_objects()
            .iter()
            .map(|object| object.spool_id.as_str().to_owned())
            .collect();
        assert!(after.contains(&cta.spool_id.as_str().to_owned()));
        assert!(after.contains(&copy.spool_id.as_str().to_owned()));
    }

    #[test]
    fn a_moved_object_survives_undo_redo_and_then_the_save_loop() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let resting = cta.position;

        // A real gesture: select, drag, release. The move goes through the
        // canvas commit path, so it is one history entry.
        view.selection.replace(vec![cta.id]);
        let depth_before = view.session.history.undo_len();
        begin_live_move(&mut view, &[cta.id]);
        drag_to(&mut view, point(120.0, 40.0));
        assert_eq!(
            view.session.history.undo_len() - depth_before,
            1,
            "one drag is one history entry"
        );
        let committed = object_with_node(&view, "spool-cta-primary").position;
        assert_ne!(committed, resting, "the object actually moved");

        assert!(view.undo_history());
        assert_eq!(
            object_with_node(&view, "spool-cta-primary").position,
            resting,
            "undo restored the authored position"
        );
        assert!(view.redo_history());
        assert_eq!(
            object_with_node(&view, "spool-cta-primary").position,
            committed,
            "redo re-applied the move"
        );

        // Save, then reopen from disk through the ordinary loader.
        let outcome = view.save_project().expect("save succeeds");
        assert!(
            outcome.unsupported.is_empty(),
            "nothing about this edit was dropped: {:?}",
            outcome.unsupported
        );
        let reopened = project_view(&root);
        let after = object_with_node(&reopened, "spool-cta-primary");
        assert_eq!(
            after.position, committed,
            "the saved position came back from disk"
        );
        assert_eq!(
            after.size, cta.size,
            "an untouched size is not rewritten as a new one"
        );
        assert_eq!(
            reopened.persistent_document().structure.nodes.len(),
            3,
            "identity and hierarchy survive the loop"
        );
        assert_eq!(
            after.text_content, cta.text_content,
            "authored text survives the loop"
        );
    }

    #[test]
    fn moving_a_frame_and_its_children_survives_the_round_trip() {
        // The regression this covers: a child's absolute position is measured
        // from the element that contains it, so saving a moved frame and its
        // children has to write the children relative to where the frame ended
        // up. Writing world coordinates as `left` put every child twice as far
        // from the origin as it should be once the parent itself moved.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let ids: Vec<ObjectId> = view.document_objects().iter().map(|o| o.id).collect();
        let resting: Vec<(f32, f32)> = view
            .document_objects()
            .iter()
            .map(|o| (o.position.x, o.position.y))
            .collect();

        view.selection.replace(ids.clone());
        begin_live_move(&mut view, &ids);
        drag_to(&mut view, point(120.0, 90.0));
        let committed: Vec<(f32, f32)> = view
            .document_objects()
            .iter()
            .map(|o| (o.position.x, o.position.y))
            .collect();
        assert_ne!(committed, resting);

        let outcome = view.save_project().expect("save succeeds");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);

        let reopened = project_view(&root);
        let after: Vec<(f32, f32)> = reopened
            .document_objects()
            .iter()
            .map(|o| (o.position.x, o.position.y))
            .collect();
        for ((want_x, want_y), (got_x, got_y)) in committed.iter().zip(&after) {
            assert!(
                (want_x - got_x).abs() < 0.01 && (want_y - got_y).abs() < 0.01,
                "every object came back where it was left: want ({want_x}, {want_y}), got ({got_x}, {got_y})"
            );
        }
    }

    #[test]
    fn an_inspector_field_change_is_one_operation_and_undoes_exactly() {
        // The Inspector is a second way to ask the same question a canvas drag
        // asks, not a second store: same command, same history, same undo.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let before = view.object_geometry(cta.id).expect("geometry");

        assert!(view.set_object_geometry(
            cta.id,
            Geometry {
                position: point(before.position.x + 24.0, before.position.y),
                size: size(before.size.width, before.size.height + 8.0),
            },
        ));
        assert_eq!(view.session.history.undo_len(), 1, "one edit, one entry");

        assert!(view.undo_history());
        assert_eq!(
            view.object_geometry(cta.id),
            Some(before),
            "undo restores the exact geometry the field replaced"
        );
        assert!(view.redo_history());
        assert_eq!(
            view.object_geometry(cta.id).unwrap().position.x,
            before.position.x + 24.0
        );

        // An edit that changes nothing is not an edit, and in particular does
        // not destroy the redo that undo just made available.
        assert!(view.undo_history());
        assert_eq!(view.session.history.redo_len(), 1);
        let undone = view.object_geometry(cta.id).unwrap();
        assert!(!view.set_object_geometry(cta.id, undone));
        assert_eq!(view.session.history.redo_len(), 1);
        assert!(view.redo_history(), "the redo survived the no-op");
    }

    #[test]
    fn an_inspector_field_drag_records_one_operation_not_one_per_movement() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let before = view.object_geometry(cta.id).expect("geometry");

        // What the shell does across a drag: open, move, move again, commit.
        let scrub = view.begin_geometry_scrub(cta.id).expect("scrub opens");
        assert_eq!(
            view.session.history.undo_len(),
            0,
            "an open scrub has recorded nothing"
        );
        for x in [4.0, 9.0, 15.0] {
            view.scrub_geometry(
                &scrub,
                Geometry {
                    position: point(before.position.x + x, before.position.y),
                    size: before.size,
                },
            );
        }
        assert_eq!(
            view.session.history.undo_len(),
            0,
            "still nothing: the drag is not over"
        );
        assert!(view.commit_geometry_scrub(scrub));
        assert_eq!(view.session.history.undo_len(), 1);

        assert!(view.undo_history());
        assert_eq!(view.object_geometry(cta.id), Some(before));
    }

    #[test]
    fn an_inspector_drag_that_ends_where_it_started_records_nothing() {
        // The same rule a canvas drag already follows: a gesture that returns to
        // its origin is not a change, so it must not clear redo either.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let before = view.object_geometry(cta.id).expect("geometry");

        let scrub = view.begin_geometry_scrub(cta.id).expect("scrub opens");
        for x in [30.0, 12.0, 0.0] {
            view.scrub_geometry(
                &scrub,
                Geometry {
                    position: point(before.position.x + x, before.position.y),
                    size: before.size,
                },
            );
        }
        assert!(!view.commit_geometry_scrub(scrub));
        assert_eq!(view.object_geometry(cta.id), Some(before));
        assert_eq!(view.session.history.undo_len(), 0);
    }

    #[test]
    fn a_cancelled_inspector_drag_restores_the_geometry_and_records_nothing() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let before = view.object_geometry(cta.id).expect("geometry");

        let scrub = view.begin_geometry_scrub(cta.id).expect("scrub opens");
        view.scrub_geometry(
            &scrub,
            Geometry {
                position: point(before.position.x - 40.0, before.position.y - 12.0),
                size: before.size,
            },
        );
        view.cancel_geometry_scrub(scrub);

        assert_eq!(
            view.object_geometry(cta.id),
            Some(before),
            "a cancelled drag is not a move"
        );
        assert_eq!(view.session.history.undo_len(), 0);
    }

    #[test]
    fn the_canvas_and_the_inspector_report_the_same_geometry() {
        // Direct manipulation and the Inspector are two views of one fact: after
        // a canvas drag, the value the Inspector reads is the dragged value.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        let before = view.object_geometry(cta.id).expect("geometry");

        view.selection.replace(vec![cta.id]);
        begin_live_move(&mut view, &[cta.id]);
        drag_to(&mut view, point(50.0, 30.0));

        let dragged = view.object_geometry(cta.id).expect("geometry");
        let object = view
            .document_objects()
            .iter()
            .find(|object| object.id == cta.id)
            .expect("still there");
        assert_eq!(
            (object.position.x, object.position.y),
            (dragged.position.x, dragged.position.y),
            "the object the Inspector draws its fields from holds the dragged value"
        );
        assert_ne!(
            dragged.position, before.position,
            "and the drag really moved something"
        );
    }

    #[test]
    fn renaming_an_object_is_one_operation_that_survives_save() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");

        assert!(
            view.rename_object(cta.id, "Primary action".to_owned()),
            "a source-backed object renames"
        );
        assert_eq!(
            view.document_objects()
                .iter()
                .find(|object| object.id == cta.id)
                .map(|object| object.name.clone()),
            Some("Primary action".to_owned()),
            "the runtime mirrors the document, which is the authority"
        );
        assert_eq!(view.session.history.undo_len(), 1);

        let outcome = view.save_project().expect("save succeeds");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);
        let yaml = std::fs::read_to_string(root.join("lamine.yaml")).expect("read metadata");
        assert!(
            yaml.contains("Primary action"),
            "the name reaches the metadata file: {yaml}"
        );

        let reopened = project_view(&root);
        assert_eq!(
            object_with_node(&reopened, "spool-cta-primary").name,
            "Primary action"
        );

        assert!(view.undo_history());
        assert_eq!(
            view.document_objects()
                .iter()
                .find(|object| object.id == cta.id)
                .map(|object| object.name.clone()),
            Some("Primary CTA".to_owned()),
            "undo restores the authored name"
        );
    }

    #[test]
    fn a_created_object_cannot_be_renamed_because_it_has_no_node_yet() {
        // Reported, not invented: a created object has no metadata entry to
        // rename, and writing one would be a structural edit the user did not
        // ask for.
        let mut view = CanvasView::new();
        let created = view.session.runtime.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(20.0, 20.0),
            None,
        );
        assert!(!view.rename_object(created.id, "New".to_owned()));
        assert_eq!(view.session.history.undo_len(), 0);
    }

    #[test]
    fn one_session_edits_text_style_geometry_and_a_name_and_all_of_it_survives() {
        // The milestone in one test: one project, one history stack, four kinds
        // of semantic operation, one save, one reopen. Every dimension is
        // checked against what came back off disk rather than against what the
        // editor was showing, because "the canvas looked right" is not the claim
        // being made.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let headline = object_with_node(&view, "spool-text-headline");
        let cta = object_with_node(&view, "spool-cta-primary");
        let html_before = std::fs::read_to_string(root.join("index.html")).expect("read html");
        let css_before = std::fs::read_to_string(root.join("styles.css")).expect("read css");
        let start_depth = view.session.history.undo_len();

        // 1. Text.
        let edited = "Design in source, structure in Spool — saved";
        assert!(view.set_object_text(headline.id, edited.to_owned()));
        assert_eq!(
            view.session.history.undo_len() - start_depth,
            1,
            "a text edit is one operation"
        );

        // 2. Appearance, through the same edits the Inspector sends. The fill is
        // owned by `.cta`, so it lands in the stylesheet; the opacity is owned
        // by nobody, so it becomes a local declaration.
        view.selection.replace(vec![cta.id]);
        let depth_before_style = view.session.history.undo_len();
        for edit in [
            StyleEdit::Fill(Some(Color::from_rgb(0xc4_5d_5d))),
            StyleEdit::TextColor(Some(Color::from_rgb(0x16_16_1d))),
            StyleEdit::FontSize(24.0),
            StyleEdit::BorderRadius(16.0),
            StyleEdit::Opacity(0.5),
        ] {
            assert!(
                view.apply_selected_style(edit),
                "{edit:?} changed something"
            );
        }
        assert_eq!(
            view.session.history.undo_len() - depth_before_style,
            5,
            "five style edits are five operations, not one batch and not five per property"
        );

        // 3. Geometry, through the Inspector's own operation.
        let moved_to = match view.object_geometry(cta.id) {
            Some(geometry) => Geometry {
                position: point(geometry.position.x + 40.0, geometry.position.y + 24.0),
                size: size(geometry.size.width + 20.0, geometry.size.height + 8.0),
            },
            None => panic!("the CTA is projected"),
        };
        assert!(view.set_object_geometry(cta.id, moved_to));

        // 4. A name, which is persistent metadata rather than runtime state.
        assert!(view.rename_object(headline.id, "Page headline".to_owned()));
        let history_count = view.session.history.undo_len() - start_depth;
        assert_eq!(
            history_count, 8,
            "one entry per user action, across four kinds of operation"
        );

        // Undo the last two actions: the resize-then-move geometry edit and the
        // rename. The text and the style must be untouched, which is what proves
        // the stack is one stack and not four.
        assert!(view.undo_history(), "undo the rename");
        assert_eq!(
            view.document_objects()
                .iter()
                .find(|object| object.id == headline.id)
                .map(|object| object.name.clone()),
            Some("Headline".to_owned())
        );
        assert_eq!(
            view.object_geometry(cta.id),
            Some(moved_to),
            "geometry is a separate step"
        );
        assert!(view.undo_history(), "undo the geometry edit");
        assert!(view.redo_history(), "redo it");
        assert_eq!(view.object_geometry(cta.id), Some(moved_to));
        assert!(view.redo_history(), "redo the rename as well");
        assert_eq!(
            view.document_objects()
                .iter()
                .find(|object| object.id == headline.id)
                .map(|object| object.name.clone()),
            Some("Page headline".to_owned()),
            "two operations of different kinds share one stack in order"
        );

        let outcome = view.save_project().expect("save succeeds");
        assert!(outcome.unsupported.is_empty(), "{:?}", outcome.unsupported);

        // The authored files changed in exactly the places the edits name.
        let html_after = std::fs::read_to_string(root.join("index.html")).expect("read html");
        let css_after = std::fs::read_to_string(root.join("styles.css")).expect("read css");
        assert!(html_after.contains(edited), "the headline text was written");
        assert!(
            html_after.contains("opacity: 0.5"),
            "the unowned property became local"
        );
        assert!(
            html_after.contains(&format!(
                "width: {}px",
                crate::project_save::css_length(moved_to.size.width)
            )),
            "the measured width was written: {html_after}"
        );
        assert!(
            css_after.contains("#c45d5d"),
            "the fill was rewritten where `.cta` owns it"
        );
        assert_ne!(html_before, html_after);
        assert_ne!(css_before, css_after);
        // The stylesheet gained a value and lost nothing: every rule the author
        // wrote is still there, and no rule was added.
        assert_eq!(
            css_after.matches(".cta").count(),
            css_before.matches(".cta").count(),
            "no rule was added or removed"
        );
        assert!(
            html_after.contains("href=\"#start\""),
            "unrelated attributes survived"
        );

        // Reopen from disk and check every dimension against the disk.
        let reopened = project_view(&root);
        let headline_after = object_with_node(&reopened, "spool-text-headline");
        let cta_after = object_with_node(&reopened, "spool-cta-primary");
        assert_eq!(headline_after.text_content.as_deref(), Some(edited));
        assert_eq!(headline_after.name, "Page headline");
        assert_eq!(
            cta_after.fill.map(|fill| fill.color.to_rgb()),
            Some(0xc4_5d_5d)
        );
        assert_eq!(cta_after.font_size, Some(24.0));
        assert_eq!(cta_after.border_radius, 16.0);
        assert_eq!(cta_after.opacity, 0.5);
        assert_eq!(cta_after.text_color, Some(Color::from_rgb(0x16_16_1d)));
        assert!(
            (cta_after.size.width - moved_to.size.width).abs() < 0.01
                && (cta_after.size.height - moved_to.size.height).abs() < 0.01,
            "the size survived: {:?}",
            cta_after.size
        );
        assert!(
            (cta_after.position.x - moved_to.position.x).abs() < 0.01
                && (cta_after.position.y - moved_to.position.y).abs() < 0.01,
            "and so did the move: {:?}",
            cta_after.position
        );
    }

    #[test]
    fn an_undo_after_save_does_not_rewrite_a_file_that_still_matches() {
        // Save writes the disk; undo changes only the editor. Nothing here says
        // an undo must be persisted, and nothing here should quietly write the
        // undone state either.
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        view.selection.replace(vec![cta.id]);
        view.apply_selected_style(StyleEdit::Opacity(0.25));
        view.save_project().expect("save succeeds");
        let after_save = std::fs::read_to_string(root.join("index.html")).expect("read html");

        assert!(view.undo_history());
        let outcome = view.save_project().expect("save succeeds");
        assert!(
            !outcome
                .written
                .iter()
                .any(|path| path.ends_with("index.html")),
            "an undone edit that matches the file on disk writes nothing: {:?}",
            outcome.written
        );
        assert_eq!(
            std::fs::read_to_string(root.join("index.html")).expect("read html"),
            after_save
        );
    }

    #[test]
    fn saving_an_untouched_project_writes_nothing() {
        let root = project_scratch("landing");
        let before: Vec<(std::path::PathBuf, Vec<u8>)> = std::fs::read_dir(&root)
            .expect("read project")
            .map(|entry| entry.expect("entry"))
            .map(|entry| {
                let bytes = std::fs::read(entry.path()).expect("read file");
                (entry.path(), bytes)
            })
            .collect();

        let view = project_view(&root);
        let outcome = view.save_project().expect("save succeeds");

        assert!(
            outcome.written.is_empty(),
            "no file should be rewritten for a session that changed nothing: {:?}",
            outcome.written
        );
        for (path, bytes) in before {
            assert_eq!(
                std::fs::read(&path).expect("read file"),
                bytes,
                "{} is byte-for-byte unchanged",
                path.display()
            );
        }
    }

    #[test]
    fn a_session_created_object_is_reported_rather_than_silently_dropped() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let created = view.session.runtime.create_object(
            ObjectType::Rectangle,
            point(400.0, 400.0),
            size(60.0, 60.0),
            None,
        );
        view.commit(DocumentCommand::insert(vec![view
            .session
            .runtime
            .placement(created.id)
            .expect("new object is placed")]));

        let outcome = view.save_project().expect("save succeeds");
        let reported: Vec<&str> = outcome
            .unsupported
            .iter()
            .map(|edit| edit.node.as_str())
            .collect();
        assert_eq!(
            reported,
            vec![created.spool_id.as_str()],
            "the object with no authored element is named, not quietly skipped"
        );
        assert!(
            outcome.unsupported[0].reason.contains("authored element"),
            "and the reason says why: {}",
            outcome.unsupported[0].reason
        );
        assert!(
            !std::fs::read_to_string(root.join("index.html"))
                .expect("html readable")
                .contains("spool-node"),
            "nothing was invented in the source to stand in for it"
        );
    }

    #[test]
    fn a_style_change_rewrites_the_declaration_that_already_owns_it() {
        let root = project_scratch("landing");
        let mut view = project_view(&root);
        let cta = object_with_node(&view, "spool-cta-primary");
        assert_eq!(
            cta.fill.map(|fill| fill.color),
            Some(Color::from_rgb(0x3b5bfd)),
            "the CTA's colour comes from the authored stylesheet"
        );

        // The inspector writes a new fill through the ordinary style path.
        let red = Color::from_rgb(0xff0000);
        view.selection.replace(vec![cta.id]);
        assert!(
            view.apply_selected_style(StyleEdit::Fill(Some(red))),
            "the inspector path applies and commits the fill"
        );

        let outcome = view.save_project().expect("save succeeds");
        assert!(
            outcome.unsupported.is_empty(),
            "the fill had an authored owner: {:?}",
            outcome.unsupported
        );
        let css = std::fs::read_to_string(root.join("styles.css")).expect("css readable");
        assert!(
            css.contains("background: #ff0000"),
            "the owning declaration was rewritten in place: {css}"
        );
        assert_eq!(
            css.matches(":root").count(),
            1,
            "no rule was added to make room for the edit"
        );
        assert_eq!(
            css.matches("background").count(),
            1,
            "the edit replaced a value rather than adding a declaration"
        );

        // And it comes back as what was authored.
        let reopened = project_view(&root);
        assert_eq!(
            object_with_node(&reopened, "spool-cta-primary")
                .fill
                .map(|fill| fill.color),
            Some(red)
        );
    }

    #[test]
    fn what_the_renderer_would_draw_comes_from_source_not_from_editor_defaults() {
        // The strongest statement about appearance that does not need a screen:
        // these are the exact values the render path hands to GPUI for a loaded
        // project. If the editor defaults were still winning, the CTA would draw
        // in the theme's blue-on-paper instead of its authored white-on-accent.
        let root = project_scratch("landing");
        let view = project_view(&root);

        let cta = object_with_node(&view, "spool-cta-primary");
        assert_eq!(
            ink_of(&cta),
            rgb(0xffffff),
            "the CTA's label is authored white, not a contrast guess"
        );
        assert_eq!(
            cta.fill.map(|fill| fill.color),
            Some(Color::from_rgb(0x3b5bfd))
        );
        assert_eq!(
            text_size_of(&cta),
            16.0,
            "no authored font size, so the renderer's own default applies"
        );

        let headline = object_with_node(&view, "spool-text-headline");
        assert_eq!(
            ink_of(&headline),
            rgb(0x16161d),
            "the headline is the ink colour body authored"
        );
        assert_eq!(
            text_size_of(&headline),
            16.0,
            "and uses the inherited font size"
        );

        // A node the source gave no paint still falls back, and says so.
        let frame = object_with_node(&view, "spool-frame-root");
        assert!(frame.fill.is_some(), "the editor default still paints it");
        assert_eq!(ink_of(&frame), rgb(0x16161d), "from the inherited colour");
    }

    #[test]
    fn a_loaded_project_renders_its_authored_text_style_and_geometry() {
        let root = project_scratch("landing");
        let view = project_view(&root);

        let headline = object_with_node(&view, "spool-text-headline");
        assert_eq!(
            headline.text_content.as_deref(),
            Some("Design in source, structure in Spool"),
            "authored text reaches the runtime"
        );
        assert_eq!(
            headline.text_color,
            // `body { color: var(--ink) }` inherited by the headline.
            Some(Color::from_rgb(0x16161d)),
            "an inherited colour is still an authored colour"
        );

        let cta = object_with_node(&view, "spool-cta-primary");
        assert_eq!(cta.text_content.as_deref(), Some("Start designing"));
        assert_eq!(cta.text_color, Some(Color::from_rgb(0xffffff)));
        assert_eq!(
            cta.fill.map(|fill| fill.color),
            Some(Color::from_rgb(0x3b5bfd))
        );
        // Padding from `.cta` is part of the box, not a decoration.
        assert!(
            cta.size.height >= 40.0,
            "the CTA's box accounts for its padding, got {}",
            cta.size.height
        );

        // Children flow inside their parent's content box.
        let frame = object_with_node(&view, "spool-frame-root");
        assert_eq!(frame.size.width, crate::visual::DEFAULT_CONTENT_WIDTH);
        assert!(
            headline.position.y >= frame.position.y,
            "the headline is laid out below the frame's origin"
        );
        assert!(
            cta.position.y >= headline.position.y,
            "and the CTA follows the headline in document order"
        );
    }
}
