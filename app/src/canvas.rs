use gpui::{
    div, fill, point, prelude::*, px as gpui_px, relative, rgb, rgba, size, App, Bounds,
    ClipboardItem, Context, DispatchPhase, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, GlobalElementId, HitboxBehavior, HitboxId, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, PinchEvent, Pixels,
    Point, Render, ScrollWheelEvent, SharedString, Size, Style, TextRun, UTF16Selection, Window,
};
use std::{cell::Cell, ops::Range, rc::Rc};

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

use crate::theme;

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
const WORLD_BOUNDS: Size<f32> = size(764.0, 688.0);
const WORLD_CENTER: Point<f32> = point(382.0, 344.0);
const LABEL_HEIGHT: f32 = 24.0;
const MIN_OBJECT_SIZE: f32 = 20.0;
const DRAG_THRESHOLD: f32 = 4.0;
const RESIZE_HANDLE_SIZE: f32 = 8.0;
const RESIZE_HANDLE_HIT_RADIUS: f32 = 7.0;

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
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset: point(0.0, 0.0),
            zoom: 1.0,
            viewport: size(0.0, 0.0),
            initialized: false,
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

    fn set_zoom_at_center(&mut self, zoom: f32) {
        self.zoom_at(
            zoom / self.zoom,
            point(self.viewport.width / 2.0, self.viewport.height / 2.0),
        );
    }

    fn fit(&mut self) {
        let available = size(
            (self.viewport.width - 96.0).max(1.0),
            (self.viewport.height - 96.0).max(1.0),
        );
        self.zoom = (available.width / WORLD_BOUNDS.width)
            .min(available.height / WORLD_BOUNDS.height)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = point(
            WORLD_CENTER.x - self.viewport.width / (2.0 * self.zoom),
            WORLD_CENTER.y - self.viewport.height / (2.0 * self.zoom),
        );
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(u64);

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleEdit {
    Fill(Option<Color>),
    Stroke(Option<Color>),
    StrokeWidth(f32),
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct GeometryChange {
    pub id: ObjectId,
    pub before: Geometry,
    pub after: Geometry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleChange {
    pub id: ObjectId,
    pub before: ObjectStyle,
    pub after: ObjectStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChange {
    pub id: ObjectId,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ObjectPlacement {
    object: DesignObject,
    index: usize,
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

    fn style(changes: Vec<StyleChange>) -> Self {
        Self {
            operation: CommandOperation::Style(changes),
        }
    }

    fn text(changes: Vec<TextChange>) -> Self {
        Self {
            operation: CommandOperation::Text(changes),
        }
    }

    fn insert(objects: Vec<ObjectPlacement>) -> Self {
        Self {
            operation: CommandOperation::Insert(objects),
        }
    }

    fn delete(objects: Vec<ObjectPlacement>) -> Self {
        Self {
            operation: CommandOperation::Delete(objects),
        }
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<DocumentCommand>,
    redo: Vec<DocumentCommand>,
}

#[derive(Clone, Debug)]
pub struct Document {
    objects: Vec<DesignObject>,
    next_id: u64,
    next_names: [u64; 4],
}

impl Default for Document {
    fn default() -> Self {
        Self {
            objects: vec![
                frame(ObjectId::LANDING, "Landing", 0.0, 24.0, 430.0, 286.0),
                frame(ObjectId::EDITOR, "Editor", 454.0, 24.0, 310.0, 252.0),
                frame(ObjectId::FEATURES, "Features", 106.0, 366.0, 394.0, 280.0),
                frame(ObjectId::MOBILE, "Mobile", 524.0, 366.0, 192.0, 322.0),
            ],
            next_id: 5,
            next_names: [1; 4],
        }
    }
}

impl Document {
    pub fn objects(&self) -> &[DesignObject] {
        &self.objects
    }

    pub fn object(&self, id: ObjectId) -> Option<&DesignObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn text_content(&self, id: ObjectId) -> Option<&str> {
        self.object(id)?.text_content.as_deref()
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

    fn create_object(
        &mut self,
        object_type: ObjectType,
        position: Point<f32>,
        object_size: Size<f32>,
        text_content: Option<String>,
    ) -> DesignObject {
        let object = DesignObject {
            id: self.allocate_id(),
            name: self.allocate_name(object_type),
            position,
            size: size(
                object_size.width.max(MIN_OBJECT_SIZE),
                object_size.height.max(MIN_OBJECT_SIZE),
            ),
            object_type,
            text_content,
            fill: default_style(object_type).fill,
            stroke: default_style(object_type).stroke,
        };
        self.insert_object(object.clone(), self.objects.len());
        object
    }

    fn insert_object(&mut self, object: DesignObject, index: usize) -> bool {
        if self.object(object.id).is_some() {
            return false;
        }
        self.objects.insert(index.min(self.objects.len()), object);
        true
    }

    fn insert_objects(&mut self, placements: &[ObjectPlacement]) {
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

    fn remove_objects(&mut self, ids: &[ObjectId]) -> Vec<ObjectPlacement> {
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
        removed
    }

    fn duplicate_objects(&mut self, ids: &[ObjectId]) -> Vec<ObjectPlacement> {
        let originals: Vec<_> = self
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .cloned()
            .collect();
        let mut duplicates = Vec::with_capacity(originals.len());
        for mut object in originals {
            object.id = self.allocate_id();
            object.name = self.allocate_name(object.object_type);
            object.position = point(object.position.x + 16.0, object.position.y + 16.0);
            let index = self.objects.len();
            self.insert_object(object.clone(), index);
            duplicates.push(ObjectPlacement { object, index });
        }
        duplicates
    }

    fn style(&self, id: ObjectId) -> Option<ObjectStyle> {
        self.object(id).map(|object| ObjectStyle {
            fill: object.fill,
            stroke: object.stroke,
        })
    }

    fn set_style(&mut self, id: ObjectId, style: ObjectStyle) -> bool {
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return false;
        };
        object.fill = style.fill;
        object.stroke = style.stroke;
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

impl History {
    pub fn record(&mut self, mut command: DocumentCommand) {
        match &mut command.operation {
            CommandOperation::Geometry(changes) => {
                changes.retain(|change| change.before != change.after);
                if changes.is_empty() {
                    return;
                }
            }
            CommandOperation::Style(changes) => {
                changes.retain(|change| change.before != change.after);
                if changes.is_empty() {
                    return;
                }
            }
            CommandOperation::Text(changes) => {
                changes.retain(|change| change.before != change.after);
                if changes.is_empty() {
                    return;
                }
            }
            CommandOperation::Insert(objects) | CommandOperation::Delete(objects)
                if objects.is_empty() =>
            {
                return;
            }
            CommandOperation::Insert(_) | CommandOperation::Delete(_) => {}
        }
        self.undo.push(command);
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, document: &mut Document) -> bool {
        let Some(command) = self.undo.pop() else {
            return false;
        };
        match &command.operation {
            CommandOperation::Geometry(changes) => {
                for change in changes {
                    document.set_geometry(change.id, change.before);
                }
            }
            CommandOperation::Style(changes) => {
                for change in changes {
                    document.set_style(change.id, change.before);
                }
            }
            CommandOperation::Text(changes) => {
                for change in changes {
                    document.set_text_content(change.id, change.before.clone());
                }
            }
            CommandOperation::Insert(objects) => {
                let ids: Vec<_> = objects
                    .iter()
                    .map(|placement| placement.object.id)
                    .collect();
                document.remove_objects(&ids);
            }
            CommandOperation::Delete(objects) => document.insert_objects(objects),
        }
        self.redo.push(command);
        true
    }

    pub fn redo(&mut self, document: &mut Document) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        match &command.operation {
            CommandOperation::Geometry(changes) => {
                for change in changes {
                    document.set_geometry(change.id, change.after);
                }
            }
            CommandOperation::Style(changes) => {
                for change in changes {
                    document.set_style(change.id, change.after);
                }
            }
            CommandOperation::Text(changes) => {
                for change in changes {
                    document.set_text_content(change.id, change.after.clone());
                }
            }
            CommandOperation::Insert(objects) => document.insert_objects(objects),
            CommandOperation::Delete(objects) => {
                let ids: Vec<_> = objects
                    .iter()
                    .map(|placement| placement.object.id)
                    .collect();
                document.remove_objects(&ids);
            }
        }
        self.undo.push(command);
        true
    }
}

fn frame(id: ObjectId, name: &str, x: f32, y: f32, width: f32, height: f32) -> DesignObject {
    DesignObject {
        id,
        name: name.to_string(),
        position: point(x, y),
        size: size(width, height),
        object_type: ObjectType::Frame,
        text_content: None,
        fill: default_style(ObjectType::Frame).fill,
        stroke: default_style(ObjectType::Frame).stroke,
    }
}

fn edited_style(mut style: ObjectStyle, edit: StyleEdit) -> ObjectStyle {
    match edit {
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
    style
}

fn default_style(object_type: ObjectType) -> ObjectStyle {
    match object_type {
        ObjectType::Frame => ObjectStyle {
            fill: Some(Fill {
                color: Color::from_rgb(theme::PAPER),
            }),
            stroke: Some(Stroke {
                color: Color::from_rgb(theme::BORDER),
                width: 1.0,
            }),
        },
        ObjectType::Rectangle | ObjectType::Ellipse => ObjectStyle {
            fill: Some(Fill {
                color: Color::from_rgb(theme::SURFACE_RAISED),
            }),
            stroke: Some(Stroke {
                color: Color::from_rgb(theme::BORDER),
                width: 1.0,
            }),
        },
        ObjectType::Text => ObjectStyle {
            fill: None,
            stroke: None,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorldRect {
    min: Point<f32>,
    max: Point<f32>,
}

impl WorldRect {
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
struct ObjectSnapshot {
    id: ObjectId,
    geometry: ObjectGeometry,
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
) -> ObjectGeometry {
    let left = start.position.x;
    let top = start.position.y;
    let right = left + start.size.width;
    let bottom = top + start.size.height;

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
) {
    let delta = movement_delta(camera, gesture.pointer_start_world, screen);
    let geometry = resized_geometry(gesture.object.geometry, gesture.handle, delta);
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

pub struct CanvasView {
    camera: Camera,
    document: Document,
    history: History,
    selection: Selection,
    pan: Option<PanGesture>,
    interaction: Interaction,
    marquee: Option<MarqueeGesture>,
    tool: Tool,
    space_held: bool,
    hitbox: Rc<Cell<Option<CanvasHitbox>>>,
    focus_handle: Option<FocusHandle>,
    text_edit: Option<TextEditState>,
}

impl CanvasView {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            document: Document::default(),
            history: History::default(),
            selection: Selection::default(),
            pan: None,
            interaction: Interaction::None,
            marquee: None,
            tool: Tool::Select,
            space_held: false,
            hitbox: Rc::new(Cell::new(None)),
            focus_handle: None,
            text_edit: None,
        }
    }

    pub fn new_with_context(cx: &mut Context<Self>) -> Self {
        let mut view = Self::new();
        view.focus_handle = Some(cx.focus_handle());
        view
    }

    pub fn is_text_editing(&self) -> bool {
        self.text_edit.is_some()
    }

    fn begin_text_edit(&mut self, id: ObjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.document.text_content(id).map(str::to_owned) else {
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
        cx.notify();
    }

    pub fn cancel_text_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let cancelled = self.discard_text_edit();
        if cancelled {
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
            .document
            .set_text_content(edit.id, edit.editing_text.clone())
        {
            return false;
        }
        self.history.record(DocumentCommand::text(vec![TextChange {
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

    pub fn document_objects(&self) -> &[DesignObject] {
        self.document.objects()
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.commit_text_edit();
        self.interaction.restore(&mut self.document);
        self.tool = tool;
        self.interaction = Interaction::None;
        self.marquee = None;
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.document);
            self.interaction = Interaction::None;
        }
        let changed = self.history.can_undo() && self.history.undo(&mut self.document);
        if changed {
            self.retain_existing_selection();
            cx.notify();
        }
        changed
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.document);
            self.interaction = Interaction::None;
        }
        let changed = self.history.can_redo() && self.history.redo(&mut self.document);
        if changed {
            self.retain_existing_selection();
            cx.notify();
        }
        changed
    }

    pub fn selected_objects(&self) -> Vec<DesignObject> {
        self.selection
            .ids()
            .iter()
            .filter_map(|id| self.document.object(*id).cloned())
            .collect()
    }

    pub fn set_selected_style(&mut self, edit: StyleEdit, cx: &mut Context<Self>) -> bool {
        self.commit_text_edit();
        let had_interaction = self.interaction.is_active();
        let changed = self.apply_selected_style(edit);
        if changed || had_interaction {
            cx.notify();
        }
        changed
    }

    fn apply_selected_style(&mut self, edit: StyleEdit) -> bool {
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.document);
            self.interaction = Interaction::None;
        }
        let changes: Vec<_> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| {
                let before = self.document.style(*id)?;
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
            self.document.set_style(change.id, change.after);
        }
        self.history.record(DocumentCommand::style(changes));
        true
    }

    pub fn select_object(&mut self, id: ObjectId, additive: bool, cx: &mut Context<Self>) {
        self.commit_text_edit();
        self.selection.click(Some(id), additive);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.commit_text_edit();
        let had_marquee = self.marquee.take().is_some();
        if !self.selection.is_empty() {
            self.selection.click(None, false);
            cx.notify();
        } else if had_marquee {
            cx.notify();
        }
    }

    pub fn zoom_percent(&self) -> u32 {
        (self.camera.zoom * 100.0).round() as u32
    }

    pub fn set_zoom_percent(&mut self, zoom_percent: u32) {
        self.camera.set_zoom_at_center(zoom_percent as f32 / 100.0);
    }

    pub fn fit_canvas(&mut self) {
        self.camera.fit();
    }

    pub fn set_space_held(&mut self, held: bool) {
        self.space_held = held;
    }

    fn begin_pan(&mut self, button: MouseButton, event: &MouseDownEvent, window: &mut Window) {
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
        let object = self.document.object(id)?;
        let local = screen_to_object_local(self.camera, screen, object.position);
        text_offset_at_local_point(&edit.editing_text, local, self.camera.zoom, window)
    }

    fn begin_left_interaction(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_pan(MouseButton::Left, event, window);
        if self.pan.is_some() {
            return;
        }

        let screen = self.cursor_in_viewport(event.position);
        let world = self.camera.screen_to_world(screen);
        self.marquee = None;

        if let Some(editing_id) = self.text_edit.as_ref().map(|edit| edit.id) {
            let inside_editing_object = self.document.object(editing_id).is_some_and(|object| {
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
                cx.notify();
                return;
            }
            self.commit_text_edit();
        }

        if self.tool == Tool::Text {
            if let Some(id) = self.document.hit_test(world) {
                if self
                    .document
                    .object(id)
                    .is_some_and(|object| object.object_type == ObjectType::Text)
                {
                    self.begin_text_edit(id, window, cx);
                    self.begin_text_pointer_selection(screen, event.modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
            }
        }
        if self.tool == Tool::Select && event.click_count >= 2 {
            if let Some(id) = self.document.hit_test(world) {
                if self
                    .document
                    .object(id)
                    .is_some_and(|object| object.object_type == ObjectType::Text)
                {
                    self.begin_text_edit(id, window, cx);
                    self.begin_text_pointer_selection(screen, event.modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
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
            if let Some(object) = self.document.object(id) {
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

        if let Some(id) = self.document.hit_test(world) {
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
                    self.document
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
        cx.notify();
    }

    fn capture_pointer(&self, window: &mut Window) {
        if let Some(hitbox) = self.hitbox.get() {
            window.capture_pointer(hitbox.id);
        }
    }

    fn hit_test_resize_handle(&self, screen: Point<f32>) -> Option<ResizeHandle> {
        if self.selection.ids().len() != 1 {
            return None;
        }
        let object = self.document.object(self.selection.ids()[0])?;
        ResizeHandle::ALL.into_iter().find(|handle| {
            let handle_position = handle.screen_position(self.camera, object);
            (screen.x - handle_position.x).abs() <= RESIZE_HANDLE_HIT_RADIUS
                && (screen.y - handle_position.y).abs() <= RESIZE_HANDLE_HIT_RADIUS
        })
    }

    fn update_interaction(&mut self, screen: Point<f32>) -> bool {
        let interaction = std::mem::replace(&mut self.interaction, Interaction::None);
        match interaction {
            Interaction::None => false,
            Interaction::PotentialMove(gesture) => {
                if !drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    self.interaction = Interaction::PotentialMove(gesture);
                    return false;
                }
                self.selection.replace(gesture.selected_ids.clone());
                apply_move(
                    &mut self.document,
                    &gesture.objects,
                    movement_delta(self.camera, gesture.pointer_start_world, screen),
                );
                self.interaction = Interaction::Moving(gesture);
                true
            }
            Interaction::Moving(gesture) => {
                apply_move(
                    &mut self.document,
                    &gesture.objects,
                    movement_delta(self.camera, gesture.pointer_start_world, screen),
                );
                self.interaction = Interaction::Moving(gesture);
                true
            }
            Interaction::PotentialResize(gesture) => {
                if !drag_threshold_crossed(gesture.pointer_start_screen, screen) {
                    self.interaction = Interaction::PotentialResize(gesture);
                    return false;
                }
                apply_resize(&mut self.document, &gesture, self.camera, screen);
                self.interaction = Interaction::Resizing(gesture);
                true
            }
            Interaction::Resizing(gesture) => {
                apply_resize(&mut self.document, &gesture, self.camera, screen);
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

    fn finish_interaction(&mut self, screen: Point<f32>) {
        let interaction = std::mem::replace(&mut self.interaction, Interaction::None);
        match interaction {
            Interaction::PotentialMove(gesture) => match gesture.click_selection {
                ClickSelection::SelectOnly(id) => self.selection.click(Some(id), false),
                ClickSelection::Toggle(id) => self.selection.click(Some(id), true),
            },
            Interaction::Moving(gesture) => {
                apply_move(
                    &mut self.document,
                    &gesture.objects,
                    movement_delta(self.camera, gesture.pointer_start_world, screen),
                );
                let command = geometry_command(&self.document, &gesture.objects);
                self.history.record(command);
            }
            Interaction::PotentialResize(_) => {}
            Interaction::Resizing(gesture) => {
                apply_resize(&mut self.document, &gesture, self.camera, screen);
                let command =
                    geometry_command(&self.document, std::slice::from_ref(&gesture.object));
                self.history.record(command);
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
    }

    fn commit_creation(
        &mut self,
        object_type: ObjectType,
        position: Point<f32>,
        object_size: Size<f32>,
    ) {
        let text_content = (object_type == ObjectType::Text).then(|| "Type something".to_string());
        let object = self
            .document
            .create_object(object_type, position, object_size, text_content);
        let placement = self.document.placement(object.id).unwrap();
        self.selection.click(Some(object.id), false);
        self.history
            .record(DocumentCommand::insert(vec![placement]));
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had_interaction = self.interaction.is_active();
        let changed = self.delete_selected_objects();
        if changed || had_interaction {
            cx.notify();
        }
        changed
    }

    fn delete_selected_objects(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.document);
            self.interaction = Interaction::None;
        }
        let ids = self.selection.ids().to_vec();
        let deleted = self.document.remove_objects(&ids);
        if deleted.is_empty() {
            self.retain_existing_selection();
            return false;
        }
        self.history.record(DocumentCommand::delete(deleted));
        self.retain_existing_selection();
        true
    }

    pub fn duplicate_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had_interaction = self.interaction.is_active();
        let changed = self.duplicate_selected_objects();
        if changed || had_interaction {
            cx.notify();
        }
        changed
    }

    fn duplicate_selected_objects(&mut self) -> bool {
        self.commit_text_edit();
        if self.interaction.is_active() {
            self.interaction.restore(&mut self.document);
            self.interaction = Interaction::None;
        }
        let ids = self.selection.ids().to_vec();
        let duplicates = self.document.duplicate_objects(&ids);
        if duplicates.is_empty() {
            self.retain_existing_selection();
            return false;
        }
        let duplicate_ids = duplicates
            .iter()
            .map(|placement| placement.object.id)
            .collect();
        self.selection.replace(duplicate_ids);
        self.history.record(DocumentCommand::insert(duplicates));
        true
    }

    fn retain_existing_selection(&mut self) {
        let existing = self
            .selection
            .ids()
            .iter()
            .copied()
            .filter(|id| self.document.object(*id).is_some())
            .collect();
        self.selection.replace(existing);
    }

    fn cancel_interaction(&mut self) -> bool {
        if !self.interaction.is_active() {
            return false;
        }
        self.interaction.restore(&mut self.document);
        self.interaction = Interaction::None;
        true
    }

    pub fn cancel_manipulation(&mut self, cx: &mut Context<Self>) -> bool {
        let cancelled = self.cancel_interaction();
        if cancelled {
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
            .document
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
        let entity = cx.entity();
        let entity_for_prepaint = entity.clone();
        let entity_for_paint = entity.clone();
        let hitbox_slot = self.hitbox.clone();
        let current_camera = self.camera;
        let document = self.document.clone();
        let selection = self.selection.clone();
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
                if !event.modifiers.control && !event.modifiers.platform {
                    return;
                }
                let delta = f32::from(event.delta.pixel_delta(gpui_px(24.0)).y);
                let factor = (delta * 0.002).exp();
                let cursor = this.cursor_in_viewport(event.position);
                this.camera.zoom_at(factor, cursor);
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                let cursor = this.cursor_in_viewport(event.position);
                this.camera.zoom_at(1.0 + event.delta, cursor);
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
                                cx.notify();
                            }
                        });
                        hitbox
                    },
                    move |bounds, hitbox, window, cx| {
                        let view_state = entity_for_paint.read(cx);
                        if view_state.pan.is_some()
                            || view_state.interaction.is_active()
                            || view_state.marquee.is_some()
                            || view_state
                                .text_edit
                                .as_ref()
                                .is_some_and(|edit| edit.pointer_anchor.is_some())
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
                                if let Some(pan) = this.pan {
                                    this.camera.pan_from(
                                        pan.offset_start,
                                        pan.pointer_start,
                                        point(
                                            f32::from(event.position.x),
                                            f32::from(event.position.y),
                                        ),
                                    );
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
                                    if this.update_interaction(screen) {
                                        cx.notify();
                                    }
                                } else if this.marquee.is_some() {
                                    let screen = this.cursor_in_viewport(event.position);
                                    let world = this.camera.screen_to_world(screen);
                                    if let Some(marquee) = this.marquee.as_mut() {
                                        marquee.current = world;
                                    }
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
                                    cx.notify();
                                } else if this.pan.is_some_and(|pan| pan.button == event.button) {
                                    this.pan = None;
                                    cx.notify();
                                } else if event.button == MouseButton::Left
                                    && this.interaction.is_active()
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.finish_interaction(screen);
                                    if this.tool == Tool::Text {
                                        if let Some(id) = this.selection.ids().last().copied() {
                                            if this.document.object(id).is_some_and(|object| {
                                                object.object_type == ObjectType::Text
                                            }) {
                                                this.begin_text_edit(id, window, cx);
                                            }
                                        }
                                    }
                                    cx.notify();
                                } else if event.button == MouseButton::Left
                                    && this.marquee.is_some()
                                {
                                    let screen = this.cursor_in_viewport(event.position);
                                    this.finish_marquee(screen);
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
                TextInputRenderContext {
                    edit: text_edit,
                    focus_handle: focus_handle.clone(),
                    entity: input_entity,
                },
            ));
        if let Some(focus_handle) = focus_handle.as_ref() {
            viewport = viewport.track_focus(focus_handle);
        }
        viewport
    }
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

fn artboards(
    camera: Camera,
    document: Document,
    selection: Selection,
    marquee: Option<MarqueeGesture>,
    preview: Option<CreationPreview>,
    text_input: TextInputRenderContext,
) -> impl IntoElement {
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

    for object in document.objects() {
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
            let editing_this_object = text_edit.as_ref().is_some_and(|edit| edit.id == object.id);
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
    if object.object_type != ObjectType::Text {
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
                .text_size(px!(14.0, zoom))
                .text_color(rgb(theme::TEXT))
                .child(object.text_content.clone().unwrap_or_default());
        }
        ObjectType::Text => {}
    }
    body
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

        assert_eq!(canvas.document.objects().len(), 4);
        assert!(canvas.history.undo.is_empty());
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
            assert_eq!(canvas.document.objects().len(), 4);
            canvas.finish_interaction(end_screen);

            let created = canvas.document.objects().last().unwrap();
            assert_eq!(created.object_type, object_type);
            assert_eq!(created.size, size(60.0, 40.0));
            assert_eq!(canvas.selection.ids(), &[created.id]);
            assert_eq!(canvas.history.undo.len(), 1);
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

        assert_eq!(canvas.document.objects().len(), 4);
        assert!(canvas.history.undo.is_empty());
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

        let text = canvas.document.objects().last().unwrap();
        assert_eq!(text.object_type, ObjectType::Text);
        assert_eq!(text.text_content.as_deref(), Some("Type something"));
        assert_eq!(text.position, point(100.0, 120.0));
        assert_eq!(canvas.selection.ids(), &[text.id]);
        assert_eq!(canvas.history.undo.len(), 1);
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
        let resized = resized_geometry(moved, ResizeHandle::BottomRight, point(20.0, 10.0));
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
                frame(ObjectId::LANDING, "Back", 0.0, 0.0, 100.0, 100.0),
                frame(ObjectId::EDITOR, "Front", 25.0, 25.0, 100.0, 100.0),
            ],
            next_id: 5,
            next_names: [1; 4],
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

            apply_resize(&mut document, &gesture, camera, point(350.0, 220.0));

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

        let resized = resized_geometry(start, ResizeHandle::Right, point(25.0, 0.0));

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(125.0, 80.0));
    }

    #[test]
    fn left_resize_changes_position_and_width() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Left, point(20.0, 0.0));

        assert_eq!(resized.position, point(30.0, 20.0));
        assert_eq!(resized.size, size(80.0, 80.0));
    }

    #[test]
    fn bottom_resize_changes_height_and_keeps_top_edge_fixed() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Bottom, point(0.0, 18.0));

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(100.0, 98.0));
    }

    #[test]
    fn top_resize_changes_position_and_height() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::Top, point(0.0, 20.0));

        assert_eq!(resized.position, point(10.0, 40.0));
        assert_eq!(resized.size, size(100.0, 60.0));
    }

    #[test]
    fn corner_resize_changes_both_dimensions() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let resized = resized_geometry(start, ResizeHandle::BottomRight, point(20.0, 15.0));

        assert_eq!(resized.position, start.position);
        assert_eq!(resized.size, size(120.0, 95.0));
    }

    #[test]
    fn resize_enforces_minimum_size_without_flipping() {
        let start = test_geometry(10.0, 20.0, 100.0, 80.0);

        let right = resized_geometry(start, ResizeHandle::Right, point(-200.0, 0.0));
        let left = resized_geometry(start, ResizeHandle::Left, point(200.0, 0.0));
        let top = resized_geometry(start, ResizeHandle::Top, point(0.0, 200.0));

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
        };
        apply_move(&mut document, &gesture.objects, point(75.0, 30.0));
        let interaction = Interaction::Moving(gesture);

        interaction.restore(&mut document);

        assert_eq!(
            document.object(ObjectId::LANDING).unwrap().geometry(),
            original
        );
    }

    fn record_position(history: &mut History, document: &mut Document, id: ObjectId, x: f32) {
        let before = document.geometry(id).unwrap();
        let after = Geometry {
            position: point(x, before.position.y),
            size: before.size,
        };
        document.set_geometry(id, after);
        history.record(DocumentCommand::geometry(vec![GeometryChange {
            id,
            before,
            after,
        }]));
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
        let mut history = History::default();

        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo.len(), 1);
    }

    #[test]
    fn create_command_can_be_undone_and_redone_with_the_same_id() {
        let mut document = Document::default();
        let mut history = History::default();
        let created = document.create_object(
            ObjectType::Rectangle,
            point(40.0, 50.0),
            size(120.0, 80.0),
            None,
        );
        history.record(DocumentCommand::insert(vec![document
            .placement(created.id)
            .unwrap()]));
        assert_eq!(document.objects().len(), 5);

        assert!(history.undo(&mut document));
        assert_eq!(document.objects().len(), 4);
        assert!(document.object(created.id).is_none());
        let after_undo =
            document.create_object(ObjectType::Ellipse, point(0.0, 0.0), size(50.0, 50.0), None);
        assert_ne!(after_undo.id, created.id);

        assert!(history.redo(&mut document));
        assert_eq!(document.object(created.id), Some(&created));
        assert_eq!(document.objects()[4].id, created.id);
        assert_eq!(document.objects().last().unwrap().id, after_undo.id);
    }

    #[test]
    fn create_move_undoes_geometry_before_creation() {
        let mut document = Document::default();
        let mut history = History::default();
        let object = document.create_object(
            ObjectType::Rectangle,
            point(40.0, 50.0),
            size(120.0, 80.0),
            None,
        );
        let initial = object.geometry();
        history.record(DocumentCommand::insert(vec![document
            .placement(object.id)
            .unwrap()]));
        let moved = Geometry {
            position: point(90.0, 110.0),
            size: initial.size,
        };
        document.set_geometry(object.id, moved);
        history.record(DocumentCommand::geometry(vec![GeometryChange {
            id: object.id,
            before: initial,
            after: moved,
        }]));

        history.undo(&mut document);
        assert_eq!(document.geometry(object.id), Some(initial));
        assert_eq!(document.objects().len(), 5);
        history.undo(&mut document);
        assert!(document.object(object.id).is_none());

        history.redo(&mut document);
        assert_eq!(document.geometry(object.id), Some(initial));
        history.redo(&mut document);
        assert_eq!(document.geometry(object.id), Some(moved));
    }

    #[test]
    fn undo_restores_a_move_snapshot() {
        let mut document = Document::default();
        let original = document.geometry(ObjectId::LANDING).unwrap();
        let mut history = History::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);

        assert!(history.undo(&mut document));

        assert_eq!(document.geometry(ObjectId::LANDING), Some(original));
    }

    #[test]
    fn redo_reapplies_a_move_snapshot() {
        let mut document = Document::default();
        let mut history = History::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        history.undo(&mut document);

        assert!(history.redo(&mut document));

        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
    }

    #[test]
    fn multiple_commands_undo_in_reverse_order() {
        let mut document = Document::default();
        let mut history = History::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        record_position(&mut history, &mut document, ObjectId::LANDING, 200.0);

        history.undo(&mut document);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
        history.undo(&mut document);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            0.0
        );
    }

    #[test]
    fn multiple_commands_redo_in_forward_order() {
        let mut document = Document::default();
        let mut history = History::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        record_position(&mut history, &mut document, ObjectId::LANDING, 200.0);
        history.undo(&mut document);
        history.undo(&mut document);

        history.redo(&mut document);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            100.0
        );
        history.redo(&mut document);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            200.0
        );
    }

    #[test]
    fn multi_object_move_commits_as_one_command_and_undoes_together() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = snapshots(&canvas.document, &ids);
        let gesture = MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: before.clone(),
            selected_ids: ids.to_vec(),
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
        };
        canvas.interaction = Interaction::Moving(gesture);
        canvas.finish_interaction(point(50.0, 25.0));

        assert_eq!(canvas.history.undo.len(), 1);
        match &canvas.history.undo[0].operation {
            CommandOperation::Geometry(changes) => assert_eq!(changes.len(), 2),
            _ => panic!("expected geometry command"),
        }
        assert_eq!(
            canvas.document.geometry(ids[0]).unwrap().position,
            point(50.0, 49.0)
        );
        assert_eq!(
            canvas.document.geometry(ids[1]).unwrap().position,
            point(504.0, 49.0)
        );

        canvas.history.undo(&mut canvas.document);
        for snapshot in before {
            assert_eq!(
                canvas.document.geometry(snapshot.id),
                Some(snapshot.geometry)
            );
        }
    }

    #[test]
    fn redo_restores_every_object_in_a_multi_object_move() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = snapshots(&canvas.document, &ids);
        canvas.interaction = Interaction::Moving(MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: before,
            selected_ids: ids.to_vec(),
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
        });
        canvas.finish_interaction(point(50.0, 25.0));
        let after: Vec<_> = ids
            .iter()
            .map(|id| canvas.document.geometry(*id).unwrap())
            .collect();
        canvas.history.undo(&mut canvas.document);

        assert!(canvas.history.redo(&mut canvas.document));

        for (id, geometry) in ids.into_iter().zip(after) {
            assert_eq!(canvas.document.geometry(id), Some(geometry));
        }
    }

    #[test]
    fn new_command_invalidates_redo_branch() {
        let mut document = Document::default();
        let mut history = History::default();
        record_position(&mut history, &mut document, ObjectId::LANDING, 100.0);
        history.undo(&mut document);
        assert!(history.can_redo());

        record_position(&mut history, &mut document, ObjectId::LANDING, 250.0);

        assert!(!history.can_redo());
        assert_eq!(history.redo.len(), 0);
        assert_eq!(
            document.geometry(ObjectId::LANDING).unwrap().position.x,
            250.0
        );
    }

    #[test]
    fn no_op_geometry_change_is_not_recorded() {
        let mut history = History::default();
        let geometry = test_geometry(10.0, 20.0, 100.0, 80.0);

        history.record(DocumentCommand::geometry(vec![GeometryChange {
            id: ObjectId::LANDING,
            before: geometry,
            after: geometry,
        }]));

        assert!(!history.can_undo());
    }

    #[test]
    fn resize_interaction_can_be_undone_and_redone() {
        let mut canvas = CanvasView::new();
        let snapshot = snapshots(&canvas.document, &[ObjectId::LANDING])[0];
        canvas.interaction = Interaction::Resizing(ResizeGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            object: snapshot,
            handle: ResizeHandle::Right,
        });
        canvas.finish_interaction(point(24.0, 0.0));
        let resized = canvas.document.geometry(ObjectId::LANDING).unwrap();

        assert_eq!(canvas.history.undo.len(), 1);
        assert!(canvas.history.undo(&mut canvas.document));
        assert_eq!(
            canvas.document.geometry(ObjectId::LANDING),
            Some(snapshot.geometry)
        );
        assert!(canvas.history.redo(&mut canvas.document));
        assert_eq!(canvas.document.geometry(ObjectId::LANDING), Some(resized));
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
        };
        apply_move(&mut document, &gesture.objects, point(80.0, 30.0));
        let interaction = Interaction::Moving(gesture);
        let history = History::default();

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
        let snapshot = snapshots(&canvas.document, &[ObjectId::LANDING])[0];
        canvas.interaction = Interaction::PotentialMove(MoveGesture {
            pointer_start_screen: point(10.0, 10.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
        });
        canvas.finish_interaction(point(10.0, 10.0));
        assert!(!canvas.history.can_undo());

        canvas.interaction = Interaction::Moving(MoveGesture {
            pointer_start_screen: point(0.0, 0.0),
            pointer_start_world: point(0.0, 0.0),
            objects: vec![snapshot],
            selected_ids: vec![ObjectId::LANDING],
            click_selection: ClickSelection::SelectOnly(ObjectId::LANDING),
        });
        canvas.finish_interaction(point(0.0, 0.0));
        assert!(!canvas.history.can_undo());
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

        assert!(!canvas.history.can_undo());
        assert!(!canvas.history.can_redo());
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
        assert_eq!(duplicate.name, "Text 2");
        assert_eq!(duplicate.object_type, original.object_type);
        assert_eq!(duplicate.size, original.size);
        assert_eq!(duplicate.text_content, original.text_content);
        assert_eq!(duplicate.position, point(47.0, 63.0));
        assert_eq!(document.object(duplicate.id), Some(duplicate));
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
        let mut history = History::default();
        let deleted = document.remove_objects(&[first.id, second.id]);
        history.record(DocumentCommand::delete(deleted));

        assert_eq!(history.undo.len(), 1);
        assert_eq!(document.objects().len(), 4);
        assert!(history.undo(&mut document));
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
        let mut history = History::default();
        history.record(DocumentCommand::insert(vec![document
            .placement(created.id)
            .unwrap()]));
        history.record(DocumentCommand::delete(
            document.remove_objects(&[created.id]),
        ));

        assert!(document.object(created.id).is_none());
        assert!(history.undo(&mut document));
        assert_eq!(document.object(created.id), Some(&created));
        assert!(history.redo(&mut document));
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
        let mut history = History::default();
        let duplicate_objects = document.duplicate_objects(&[original.id]);
        let duplicate_id = duplicate_objects[0].object.id;
        history.record(DocumentCommand::insert(duplicate_objects));
        assert_eq!(history.undo.len(), 1);

        assert!(history.undo(&mut document));
        assert!(document.object(original.id).is_some());
        assert!(document.object(duplicate_id).is_none());
        assert!(history.redo(&mut document));
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
        let mut history = History::default();
        history.record(DocumentCommand::delete(
            document.remove_objects(&[target.id]),
        ));
        assert!(history.undo(&mut document));
        assert!(history.can_redo());

        let created = document.create_object(
            ObjectType::Ellipse,
            point(80.0, 80.0),
            size(50.0, 50.0),
            None,
        );
        history.record(DocumentCommand::insert(vec![document
            .placement(created.id)
            .unwrap()]));

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
        let mut history = History::default();
        let duplicates = document.duplicate_objects(&[original.id]);
        let duplicate = duplicates[0].object.clone();
        history.record(DocumentCommand::insert(duplicates));
        let before_move = duplicate.geometry();
        let after_move = Geometry {
            position: point(before_move.position.x + 22.0, before_move.position.y + 9.0),
            size: before_move.size,
        };
        document.set_geometry(duplicate.id, after_move);
        history.record(DocumentCommand::geometry(vec![GeometryChange {
            id: duplicate.id,
            before: before_move,
            after: after_move,
        }]));

        assert!(history.undo(&mut document));
        assert_eq!(document.geometry(duplicate.id), Some(before_move));
        assert!(history.undo(&mut document));
        assert!(document.object(duplicate.id).is_none());
        assert!(document.object(original.id).is_some());
    }

    #[test]
    fn selection_delete_clears_ids_and_selection_reconciliation_drops_missing_objects() {
        let mut canvas = CanvasView::new();
        let created = canvas.document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(60.0, 40.0),
            None,
        );
        canvas.selection.click(Some(created.id), false);
        assert!(canvas.delete_selected_objects());
        assert!(canvas.selection.is_empty());
        assert!(canvas.document.object(created.id).is_none());
        assert_eq!(canvas.history.undo.len(), 1);

        canvas.history.undo(&mut canvas.document);
        canvas.retain_existing_selection();
        assert!(canvas.selection.is_empty());
        canvas.history.redo(&mut canvas.document);
        canvas.retain_existing_selection();
        assert!(canvas.selection.is_empty());
    }

    #[test]
    fn duplication_selects_only_the_duplicates_and_is_one_command() {
        let mut canvas = CanvasView::new();
        let first = canvas.document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(60.0, 40.0),
            None,
        );
        let second = canvas.document.create_object(
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
        assert_eq!(canvas.history.undo.len(), 1);
        assert_eq!(
            canvas.document.object(selected[0]).unwrap().object_type,
            ObjectType::Rectangle
        );
        assert_eq!(
            canvas.document.object(selected[1]).unwrap().object_type,
            ObjectType::Ellipse
        );
        assert!(selected
            .iter()
            .all(|id| canvas.document.object(*id).is_some()));
    }

    #[test]
    fn deleting_multiple_selected_objects_is_one_command_and_undo_restores_them() {
        let mut canvas = CanvasView::new();
        let ids = [ObjectId::LANDING, ObjectId::EDITOR];
        let before = canvas.document.objects().to_vec();
        canvas.selection.replace(ids.to_vec());

        assert!(canvas.delete_selected_objects());

        assert_eq!(canvas.history.undo.len(), 1);
        assert!(canvas.selection.is_empty());
        assert!(canvas.document.object(ids[0]).is_none());
        assert!(canvas.document.object(ids[1]).is_none());
        assert!(canvas.history.undo(&mut canvas.document));
        assert_eq!(canvas.document.objects(), before);
    }

    #[test]
    fn duplicate_without_selection_and_delete_without_selection_are_no_ops() {
        let mut canvas = CanvasView::new();

        assert!(!canvas.delete_selected_objects());
        assert!(!canvas.duplicate_selected_objects());
        assert_eq!(canvas.document.objects().len(), 4);
        assert!(!canvas.history.can_undo());
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
                stroke: None
            }
        );
    }

    fn apply_style_to_document(
        document: &mut Document,
        history: &mut History,
        ids: &[ObjectId],
        edit: StyleEdit,
    ) {
        let changes: Vec<_> = ids
            .iter()
            .filter_map(|id| {
                let before = document.style(*id)?;
                let after = edited_style(before, edit);
                (before != after).then_some(StyleChange {
                    id: *id,
                    before,
                    after,
                })
            })
            .collect();
        for change in &changes {
            document.set_style(change.id, change.after);
        }
        history.record(DocumentCommand::style(changes));
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
        let mut history = History::default();

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
        let mut history = History::default();
        let blue = Color::from_rgb(0x6689c7);

        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(blue)),
        );
        let changed = document.style(object.id).unwrap();
        assert_eq!(history.undo.len(), 1);
        assert!(history.undo(&mut document));
        assert_eq!(document.style(object.id), Some(default));
        assert!(history.redo(&mut document));
        assert_eq!(document.style(object.id), Some(changed));

        let commands = history.undo.len();
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(blue)),
        );
        assert_eq!(history.undo.len(), commands);
    }

    #[test]
    fn style_changes_are_one_command_for_multi_selection_and_preserve_selection() {
        let mut canvas = CanvasView::new();
        let rectangle = canvas.document.create_object(
            ObjectType::Rectangle,
            point(10.0, 10.0),
            size(80.0, 60.0),
            None,
        );
        let text = canvas.document.create_object(
            ObjectType::Text,
            point(110.0, 10.0),
            size(80.0, 60.0),
            Some("Type something".to_string()),
        );
        canvas.selection.replace(vec![rectangle.id, text.id]);
        let selected = canvas.selection.ids().to_vec();
        let fill = Color::from_rgb(0x6689c7);

        assert!(canvas.apply_selected_style(StyleEdit::Fill(Some(fill))));

        assert_eq!(canvas.history.undo.len(), 1);
        assert_eq!(canvas.selection.ids(), selected);
        assert!(selected
            .iter()
            .all(|id| { canvas.document.object(*id).unwrap().fill == Some(Fill { color: fill }) }));
        assert!(
            matches!(canvas.history.undo[0].operation, CommandOperation::Style(ref changes) if changes.len() == 2)
        );

        let stroke = Color::from_rgb(0xc45d5d);
        assert!(canvas.apply_selected_style(StyleEdit::Stroke(Some(stroke))));
        assert!(selected.iter().all(|id| {
            canvas
                .document
                .object(*id)
                .unwrap()
                .stroke
                .map(|value| value.color)
                == Some(stroke)
        }));
        assert_eq!(canvas.history.undo.len(), 2);
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
        let mut history = History::default();
        let default_style = document.style(object.id).unwrap();
        apply_style_to_document(
            &mut document,
            &mut history,
            &[object.id],
            StyleEdit::Fill(Some(Color::from_rgb(0x6689c7))),
        );
        let styled = document.style(object.id).unwrap();
        record_position(&mut history, &mut document, object.id, 140.0);

        assert!(history.undo(&mut document));
        assert_eq!(document.object(object.id).unwrap().position.x, 20.0);
        assert_eq!(document.style(object.id), Some(styled));
        assert!(history.undo(&mut document));
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
        let mut history = History::default();
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
        history.record(DocumentCommand::insert(duplicate));

        history.record(DocumentCommand::delete(
            document.remove_objects(&[original.id]),
        ));
        assert!(history.undo(&mut document));
        assert_eq!(
            document.object(original.id).unwrap().fill,
            Some(Fill { color: fill })
        );
        assert!(history.undo(&mut document));
        assert!(document.object(duplicate_object.id).is_none());
        assert!(history.undo(&mut document));
        assert_eq!(
            document.object(original.id).unwrap().fill,
            default_style(ObjectType::Text).fill
        );
    }

    fn create_test_text(canvas: &mut CanvasView, text: &str) -> DesignObject {
        canvas.document.create_object(
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
        let mut history = History::default();
        document.set_text_content(object.id, "after 🧵".to_owned());
        history.record(DocumentCommand::text(vec![TextChange {
            id: object.id,
            before: "before".to_owned(),
            after: "after 🧵".to_owned(),
        }]));

        assert!(history.undo(&mut document));
        assert_eq!(document.text_content(object.id), Some("before"));
        assert!(history.redo(&mut document));
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
        let mut history = History::default();
        history.record(DocumentCommand::text(vec![TextChange {
            id: object.id,
            before: "same".to_owned(),
            after: "same".to_owned(),
        }]));
        assert!(!history.can_undo());

        document.set_text_content(object.id, "edited".to_owned());
        history.record(DocumentCommand::text(vec![TextChange {
            id: object.id,
            before: "same".to_owned(),
            after: "edited".to_owned(),
        }]));
        assert!(history.undo(&mut document));
        assert!(history.can_redo());
        document.set_text_content(object.id, "new edit".to_owned());
        history.record(DocumentCommand::text(vec![TextChange {
            id: object.id,
            before: "same".to_owned(),
            after: "new edit".to_owned(),
        }]));
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
        assert_eq!(canvas.document.text_content(object.id), Some("Hello world"));
        assert_eq!(canvas.history.undo.len(), 1);
        assert_eq!(canvas.selection.ids(), &[object.id]);
        assert!(matches!(
            canvas.history.undo[0].operation,
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
        assert_eq!(canvas.document.text_content(object.id), Some("Original"));
        assert!(!canvas.history.can_undo());
    }

    #[test]
    fn text_edits_do_not_change_geometry_or_style() {
        let mut canvas = CanvasView::new();
        let object = create_test_text(&mut canvas, "before");
        let geometry = object.geometry();
        let style = canvas.document.style(object.id).unwrap();
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

        assert_eq!(canvas.document.geometry(object.id), Some(geometry));
        assert_eq!(canvas.document.style(object.id), Some(style));
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
        let style = canvas.document.style(object.id).unwrap();
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

        assert_eq!(canvas.document.text_content(object.id), Some("Hello world"));
        assert_eq!(canvas.document.geometry(object.id), Some(geometry));
        assert_eq!(canvas.document.style(object.id), Some(style));
        assert!(!canvas.history.can_undo());
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
        assert_eq!(canvas.document.text_content(object.id), Some("Hello"));
        assert_eq!(canvas.selection.ids(), &[object.id]);
        assert_eq!(canvas.history.undo.len(), 1);
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
        assert_eq!(canvas.document.text_content(object.id), Some("Original"));
        assert!(!canvas.history.can_undo());
    }

    #[test]
    fn utf16_ranges_map_to_valid_utf8_boundaries_for_ime_and_selection() {
        let text = "a🧵é";
        assert_eq!(utf16_range_to_utf8(text, 1..3), 1..5);
        assert_eq!(utf8_range_to_utf16(text, 1..5), 1..3);
        assert_eq!(utf16_range_to_utf8(text, 3..4), 5..7);
    }
}
