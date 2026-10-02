use gpui::{div, prelude::*, px, rgb, Context, Entity, Render, SharedString, Subscription, Window};

use crate::{canvas, layers::LayersView, project_open, theme};

const TOOLS: [(&str, &str, &str, canvas::Tool); 7] = [
    ("↖", "Select", "V", canvas::Tool::Select),
    ("▱", "Frame", "F", canvas::Tool::Frame),
    ("□", "Rectangle", "R", canvas::Tool::Rectangle),
    ("○", "Ellipse", "O", canvas::Tool::Ellipse),
    ("⌁", "Pen", "P", canvas::Tool::Pen),
    ("T", "Text", "T", canvas::Tool::Text),
    ("◌", "Comment", "C", canvas::Tool::Comment),
];

const PAGES: [&str; 4] = ["Landing", "App", "Components", "Explorations"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StyleProperty {
    Fill,
    Stroke,
    TextColor,
}

const STYLE_COLORS: [(&str, u32); 9] = [
    ("Ink", theme::INK),
    ("Paper", theme::PAPER),
    ("Gray", 0x777d84),
    ("Red", 0xc45d5d),
    ("Orange", 0xd2844d),
    ("Yellow", 0xd5b957),
    ("Green", theme::SAGE),
    ("Blue", 0x6689c7),
    ("Purple", 0x9576b8),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShortcutAction {
    Undo,
    Redo,
    Delete,
    Duplicate,
    Save,
    Tool(canvas::Tool),
}

fn shortcut_action(
    key: &str,
    platform: bool,
    control: bool,
    shift: bool,
) -> Option<ShortcutAction> {
    if (platform || control) && key == "z" {
        return Some(if shift {
            ShortcutAction::Redo
        } else {
            ShortcutAction::Undo
        });
    }
    if control && key == "y" {
        return Some(ShortcutAction::Redo);
    }
    if (platform || control) && key == "d" {
        return Some(ShortcutAction::Duplicate);
    }
    if (platform || control) && key == "s" {
        return Some(ShortcutAction::Save);
    }
    match key {
        "delete" | "backspace" => Some(ShortcutAction::Delete),
        "v" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Select)),
        "f" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Frame)),
        "r" if !platform && !control && !shift => {
            Some(ShortcutAction::Tool(canvas::Tool::Rectangle))
        }
        "o" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Ellipse)),
        "t" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Text)),
        "p" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Pen)),
        "c" if !platform && !control && !shift => Some(ShortcutAction::Tool(canvas::Tool::Comment)),
        _ => None,
    }
}

pub struct AppShell {
    canvas: Entity<canvas::CanvasView>,
    layers: Entity<LayersView>,
    selected_tool: canvas::Tool,
    selected_page: usize,
    inspector_tab: usize,
    collapsed_sections: [bool; 4],
    ai_open: bool,
    share_open: bool,
    export_open: bool,
    zoom_open: bool,
    color_picker: Option<StyleProperty>,
    /// Result of the last save, shown over the canvas.
    ///
    /// Save reports what it wrote and what it could not, so the message has to
    /// be visible: a save that silently dropped an edit would look identical to
    /// one that persisted everything.
    save_status: Option<(SharedString, bool)>,
    /// Repaint subscriptions that must outlive this function.
    ///
    /// A dropped `Subscription` detaches its observer, so the shell keeps them
    /// for its whole life: it reads the canvas on every render, and a canvas
    /// edit has to cause a shell render or the Inspector goes stale. Nothing
    /// reads this field; holding it is the point.
    #[allow(dead_code)]
    observations: Vec<Subscription>,
    ///
    /// The shell owns the widget; the canvas owns the document and the history
    /// boundary, so a drag here is one operation when it ends rather than one
    /// per pointer movement. `None` the rest of the time.
    geometry_scrub: Option<GeometryScrub>,
    /// The geometry field the arrow keys adjust, once one has been touched.
    geometry_target: Option<(canvas::ObjectId, GeometryField)>,
    /// The object being renamed and the text typed so far.
    ///
    /// A buffer, not a second name store: the name only becomes real when the
    /// rename is handed to the canvas as one semantic operation.
    rename: Option<(canvas::ObjectId, String)>,
}

/// One editable number in the Inspector's geometry fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryField {
    X,
    Y,
    Width,
    Height,
}

impl GeometryField {
    fn label(self) -> &'static str {
        match self {
            GeometryField::X => "X",
            GeometryField::Y => "Y",
            GeometryField::Width => "W",
            GeometryField::Height => "H",
        }
    }

    fn value_of(self, geometry: canvas::Geometry) -> f32 {
        match self {
            GeometryField::X => geometry.position.x,
            GeometryField::Y => geometry.position.y,
            GeometryField::Width => geometry.size.width,
            GeometryField::Height => geometry.size.height,
        }
    }

    /// The same geometry with this field replaced.
    ///
    /// A size cannot go below one pixel: a zero-sized box cannot be clicked
    /// again, so shrinking one to nothing would make it unrecoverable rather
    /// than small.
    fn with_value(self, geometry: canvas::Geometry, value: f32) -> canvas::Geometry {
        let value = match self {
            GeometryField::X | GeometryField::Y => value,
            GeometryField::Width | GeometryField::Height => value.max(1.0),
        };
        let mut next = geometry;
        match self {
            GeometryField::X => next.position.x = value,
            GeometryField::Y => next.position.y = value,
            GeometryField::Width => next.size.width = value,
            GeometryField::Height => next.size.height = value,
        }
        next
    }
}

/// A live geometry drag from an Inspector field.
struct GeometryScrub {
    field: GeometryField,
    /// The gesture the canvas opened for this object.
    gesture: canvas::GeometryScrub,
    /// Screen position the drag started at.
    start_x: f32,
    /// The field's value when the drag started.
    start_value: f32,
}

/// Open the project named by `SPOOL_PROJECT`, if one was requested.
///
/// This is the application's only project-opening path. `SPOOL_PROJECT` points
/// at a directory containing `lamine.yaml` and its authored source; without it
/// the editor opens the blank starter scene exactly as before.
///
/// A failure is reported deterministically on stderr and leaves the blank
/// document in place. It never silently produces an empty canvas that looks
/// like a project that happened to be empty, and it never falls back to a
/// partially loaded document.
fn open_requested_project(view: &mut canvas::CanvasView) {
    let Ok(root) = std::env::var("SPOOL_PROJECT") else {
        return;
    };
    match project_open::open_project(&root) {
        Ok(loaded) => {
            // Reported so a launch log shows what actually opened, rather than
            // leaving the operator to infer it from pixels.
            eprintln!(
                "spool_project_open ok root={} persistent_nodes={} runtime_objects={} unrendered={:?} unstyled={:?}",
                loaded.root.display(),
                loaded.document.structure.nodes.len(),
                loaded.runtime.objects().len(),
                loaded.unrendered,
                loaded.unstyled,
            );
            view.load_project(loaded);
        }
        Err(error) => {
            eprintln!("spool_project_open failed root={root}: {error}");
        }
    }
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let canvas = cx.new(canvas::CanvasView::new_with_context);
        canvas.update(cx, |view, _| open_requested_project(view));
        let layers = cx.new(|_| LayersView::new(canvas.downgrade()));
        // Held, not dropped: a GPUI subscription detaches its observer when it
        // goes away, and without it the Inspector would only repaint when the
        // shell itself changed — showing the geometry an object had before the
        // user dragged it.
        let observations = vec![cx.observe(&canvas, |_, _, cx| cx.notify())];
        Self {
            canvas,
            layers,
            observations,
            selected_tool: canvas::Tool::Select,
            selected_page: 0,
            inspector_tab: 0,
            collapsed_sections: [false; 4],
            ai_open: false,
            share_open: false,
            export_open: false,
            zoom_open: false,
            color_picker: None,
            save_status: None,
            geometry_scrub: None,
            geometry_target: None,
            rename: None,
        }
    }

    /// Run a canvas mutation and repaint both sides of the editor.
    ///
    /// The canvas and the Inspector are two views of one document, so a change
    /// made through either has to redraw both. Notifying the canvas alone leaves
    /// the fields showing a value the canvas has already moved past; notifying
    /// the shell alone leaves the object where it was on screen.
    fn edit_canvas<T>(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut canvas::CanvasView, &mut Context<canvas::CanvasView>) -> T,
    ) -> T {
        let result = self.canvas.update(cx, |canvas, cx| edit(canvas, cx));
        self.canvas.update(cx, |_, cx| cx.notify());
        result
    }

    /// Keys pressed while an Inspector rename is in progress.
    ///
    /// Returns `true` when the key belonged to the rename, so the editor's own
    /// shortcuts stay out of the way: typing `f` into a name must not also
    /// switch to the frame tool.
    fn rename_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some((id, buffer)) = self.rename.as_mut() else {
            return false;
        };
        let id = *id;
        let committed = match key {
            "enter" | "return" => Some(buffer.clone()),
            "escape" => Some(String::new()),
            "backspace" => {
                buffer.pop();
                None
            }
            // Modifier combinations are shortcuts, not characters.
            other if other.chars().count() == 1 => {
                buffer.push_str(other);
                None
            }
            _ => return true,
        };
        self.rename = None;
        if let Some(name) = committed {
            let name = name.trim().to_owned();
            if !name.is_empty() {
                self.edit_canvas(cx, |canvas, _| canvas.rename_object(id, name));
            }
        }
        cx.notify();
        true
    }

    /// Arrow keys adjusting the geometry field the user last touched.
    ///
    /// One key press is one semantic operation, the same as one field drag.
    fn geometry_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some((id, field)) = self.geometry_target else {
            return false;
        };
        let step = if shift { 10.0 } else { 1.0 };
        let (dx, dy) = match key {
            "left" => (-step, 0.0),
            "right" => (step, 0.0),
            "up" => (0.0, -step),
            "down" => (0.0, step),
            _ => return false,
        };
        let delta = match field {
            GeometryField::X | GeometryField::Width => gpui::point(dx, 0.0),
            GeometryField::Y | GeometryField::Height => gpui::point(0.0, dy),
        };
        self.edit_canvas(cx, |canvas, _| {
            let Some(current) = canvas.object_geometry(id) else {
                return false;
            };
            let next = match field {
                GeometryField::X | GeometryField::Y => canvas::Geometry {
                    position: gpui::point(
                        current.position.x + delta.x,
                        current.position.y + delta.y,
                    ),
                    size: current.size,
                },
                GeometryField::Width | GeometryField::Height => canvas::Geometry {
                    position: current.position,
                    size: gpui::size(
                        (current.size.width + delta.x).max(1.0),
                        (current.size.height + delta.y).max(1.0),
                    ),
                },
            };
            canvas.set_object_geometry(id, next)
        });
        true
    }

    /// Save the open project and report what actually happened.
    ///
    /// Goes through the canvas so there is exactly one save path, and says what
    /// could not be written rather than implying everything persisted.
    fn save_project(&mut self, cx: &mut Context<Self>) {
        let status = match self.canvas.update(cx, |canvas, _| canvas.save_project()) {
            Ok(outcome) if outcome.written.is_empty() && outcome.unsupported.is_empty() => {
                "No changes to save".to_owned()
            }
            Ok(outcome) if !outcome.unsupported.is_empty() => {
                let names: Vec<&str> = outcome
                    .unsupported
                    .iter()
                    .map(|edit| edit.node.as_str())
                    .collect();
                format!(
                    "Saved {} file(s); not saved: {}",
                    outcome.written.len(),
                    names.join(", ")
                )
            }
            Ok(outcome) => format!("Saved {} file(s)", outcome.written.len()),
            Err(error) => format!("Save failed: {error}"),
        };
        let failed = status.starts_with("Save failed");
        eprintln!("spool_save_command {status}");
        self.save_status = Some((status.into(), failed));
        cx.notify();
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let left = div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.0))
                    .rounded_md()
                    .bg(rgb(theme::ACCENT_WASH))
                    .text_color(rgb(theme::ACCENT))
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("S"),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .child("Spool"),
            )
            .child(div().w(px(1.0)).h(px(22.0)).bg(rgb(theme::BORDER)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT))
                            .child("Fieldnotes"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(format!("Website  /  {}", PAGES[self.selected_page])),
                    ),
            );

        let mut tools = div()
            .flex()
            .items_center()
            .gap(px(3.0))
            .p(px(4.0))
            .rounded_lg()
            .bg(rgb(theme::SURFACE))
            .border_1()
            .border_color(rgb(theme::BORDER));
        for (index, (icon, label, _shortcut, tool)) in TOOLS.iter().enumerate() {
            let selected = self.selected_tool == *tool;
            let background = if selected {
                theme::SURFACE_HOVER
            } else {
                theme::SURFACE
            };
            let foreground = if selected {
                theme::TEXT
            } else {
                theme::TEXT_SECONDARY
            };
            let tool = *tool;
            tools = tools.child(
                div()
                    .id(SharedString::from(format!("top-tool-{index}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(9.0))
                    .py(px(7.0))
                    .rounded_md()
                    .bg(rgb(background))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .active(|style| style.opacity(0.78))
                    .text_sm()
                    .text_color(rgb(foreground))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_tool = tool;
                        this.canvas.update(cx, |canvas, _| canvas.set_tool(tool));
                        cx.notify();
                    }))
                    .child(*icon)
                    .child(div().text_xs().child(*label)),
            );
        }

        let mut share = div().relative().child(
            div()
                .id("share-button")
                .flex()
                .items_center()
                .gap_2()
                .px(px(12.0))
                .py(px(8.0))
                .rounded_md()
                .bg(rgb(theme::SURFACE_RAISED))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                .active(|style| style.opacity(0.78))
                .text_sm()
                .text_color(rgb(theme::TEXT))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.share_open = !this.share_open;
                    this.export_open = false;
                    cx.notify();
                }))
                .child("↗")
                .child("Share"),
        );
        if self.share_open {
            share = share.child(share_popover());
        }

        let mut export = div().relative().child(
            div()
                .id("export-button")
                .flex()
                .items_center()
                .gap_2()
                .px(px(12.0))
                .py(px(8.0))
                .rounded_md()
                .bg(rgb(theme::SURFACE_RAISED))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                .active(|style| style.opacity(0.78))
                .text_sm()
                .text_color(rgb(theme::TEXT))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.export_open = !this.export_open;
                    this.share_open = false;
                    cx.notify();
                }))
                .child("⇧")
                .child("Export"),
        );
        if self.export_open {
            export = export.child(export_popover());
        }

        let agent = div()
            .id("agent-toggle")
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.0))
            .py(px(8.0))
            .rounded_md()
            .bg(rgb(theme::ACCENT_WASH))
            .hover(|style| style.bg(rgb(0x343954)))
            .active(|style| style.opacity(0.78))
            .text_sm()
            .text_color(rgb(theme::TEXT))
            .on_click(cx.listener(|this, _, _, cx| {
                this.ai_open = !this.ai_open;
                cx.notify();
            }))
            .child("✦")
            .child(if self.ai_open {
                "Hide agent"
            } else {
                "Ask Spool"
            });

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(62.0))
            .px(px(14.0))
            .bg(rgb(theme::WINDOW))
            .border_b_1()
            .border_color(rgb(theme::BORDER_SOFT))
            .child(left)
            .child(tools)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.zoom_control(cx))
                    .child(share)
                    .child(export)
                    .child(agent),
            )
    }

    fn zoom_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = format!("{}%", self.canvas.read(cx).zoom_percent());
        let mut control = div().relative().child(
            div()
                .id("zoom-control")
                .flex()
                .items_center()
                .gap_2()
                .px(px(10.0))
                .py(px(8.0))
                .rounded_md()
                .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                .text_xs()
                .text_color(rgb(theme::TEXT_SECONDARY))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.zoom_open = !this.zoom_open;
                    this.share_open = false;
                    this.export_open = false;
                    cx.notify();
                }))
                .child(label)
                .child("⌄"),
        );
        if self.zoom_open {
            control = control.child(self.zoom_menu(cx));
        }
        control
    }

    fn zoom_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(38.0))
            .right(px(0.0))
            .flex()
            .flex_col()
            .gap_1()
            .w(px(116.0))
            .p(px(6.0))
            .rounded_lg()
            .bg(rgb(theme::SURFACE_RAISED))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .shadow_lg()
            .child(self.zoom_option("25%", Some(25), cx))
            .child(self.zoom_option("50%", Some(50), cx))
            .child(self.zoom_option("100%", Some(100), cx))
            .child(self.zoom_option("200%", Some(200), cx))
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgb(theme::BORDER_SOFT)))
            .child(self.zoom_option("Fit", None, cx))
    }

    fn zoom_option(
        &self,
        label: &'static str,
        zoom_percent: Option<u32>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("zoom-option-{label}")))
            .px(px(9.0))
            .py(px(8.0))
            .rounded_md()
            .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
            .text_xs()
            .text_color(rgb(theme::TEXT_SECONDARY))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.canvas.update(cx, |canvas, _| {
                    if let Some(zoom_percent) = zoom_percent {
                        canvas.set_zoom_percent(zoom_percent);
                    } else {
                        canvas.fit_canvas();
                    }
                });
                this.zoom_open = false;
                cx.notify();
            }))
            .child(label)
    }

    fn left_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut pages = div().flex().flex_col().gap_1().px(px(10.0)).pb(px(14.0));
        for (index, page) in PAGES.iter().enumerate() {
            let selected = self.selected_page == index;
            let background = if selected {
                theme::SURFACE_HOVER
            } else {
                theme::SURFACE
            };
            let foreground = if selected {
                theme::TEXT
            } else {
                theme::TEXT_SECONDARY
            };
            pages = pages.child(
                div()
                    .id(SharedString::from(format!("page-{index}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(30.0))
                    .px(px(9.0))
                    .rounded_md()
                    .bg(rgb(background))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .text_sm()
                    .text_color(rgb(foreground))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_page = index;
                        cx.notify();
                    }))
                    .child(if selected { "◉" } else { "○" })
                    .child(*page),
            );
        }

        let tree = self
            .layers
            .clone()
            .cached(self.layers.read(cx).cached_style());

        div()
            .flex()
            .flex_col()
            .w(px(204.0))
            .flex_shrink_0()
            .bg(rgb(theme::SURFACE))
            .border_r_1()
            .border_color(rgb(theme::BORDER_SOFT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .pt(px(15.0))
                    .pb(px(9.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_SECONDARY))
                            .child("PAGES"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("+"),
                    ),
            )
            .child(pages)
            .child(div().h(px(1.0)).bg(rgb(theme::BORDER_SOFT)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .pt(px(14.0))
                    .pb(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_SECONDARY))
                            .child("LAYERS"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("+"),
                    ),
            )
            .child(tree)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .justify_end()
                    .p(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px(px(9.0))
                            .py(px(8.0))
                            .rounded_md()
                            .bg(rgb(theme::WINDOW))
                            .border_1()
                            .border_color(rgb(theme::BORDER_SOFT))
                            .child(
                                div()
                                    .size(px(7.0))
                                    .rounded_full()
                                    .bg(rgb(theme::TEXT_MUTED)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::TEXT_SECONDARY))
                                    .child("Prototype workspace"),
                            ),
                    ),
            )
    }

    fn tool_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut palette = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .w(px(58.0))
            .p(px(5.0))
            .rounded_lg()
            .bg(rgb(theme::SURFACE))
            .border_1()
            .border_color(rgb(theme::BORDER));
        for (index, (icon, label, _shortcut, tool)) in TOOLS.iter().enumerate() {
            let selected = self.selected_tool == *tool;
            let background = if selected {
                theme::ACCENT_WASH
            } else {
                theme::SURFACE
            };
            let foreground = if selected {
                theme::ACCENT
            } else {
                theme::TEXT_SECONDARY
            };
            let tool = *tool;
            palette = palette.child(
                div()
                    .id(SharedString::from(format!("palette-tool-{index}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(2.0))
                    .w(px(46.0))
                    .h(px(43.0))
                    .rounded_md()
                    .bg(rgb(background))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .active(|style| style.opacity(0.78))
                    .text_color(rgb(foreground))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_tool = tool;
                        this.canvas.update(cx, |canvas, _| canvas.set_tool(tool));
                        cx.notify();
                    }))
                    .child(div().text_sm().child(*icon))
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(*label),
                    ),
            );
        }
        palette
    }

    fn canvas_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .relative()
            .overflow_hidden()
            .bg(rgb(theme::CANVAS))
            .child(self.canvas.clone());

        if let Some((message, failed)) = &self.save_status {
            content = content.child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .bottom(px(16.0))
                    .rounded_md()
                    .px(px(10.0))
                    .py(px(6.0))
                    .bg(rgb(0x1b1e24))
                    .text_color(rgb(if *failed { 0xff8a8a } else { 0xd8dbe2 }))
                    .text_xs()
                    .child(message.clone()),
            );
        }

        if self.ai_open {
            content = content.child(self.agent_panel(cx));
        }

        div()
            .flex()
            .flex_1()
            .gap_3()
            .p(px(12.0))
            .overflow_hidden()
            .bg(rgb(theme::CANVAS))
            .child(self.tool_palette(cx))
            .child(content)
    }

    fn agent_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .right(px(22.0))
            .bottom(px(20.0))
            .flex()
            .flex_col()
            .gap_3()
            .w(px(340.0))
            .p(px(16.0))
            .rounded_lg()
            .bg(rgb(0x1b1e24))
            .border_1()
            .border_color(rgb(0x343944))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(27.0))
                                    .rounded_full()
                                    .bg(rgb(theme::ACCENT_WASH))
                                    .text_color(rgb(theme::ACCENT))
                                    .child("✦"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(rgb(theme::TEXT))
                                            .child("Spool Agent"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(theme::TEXT_MUTED))
                                            .child("Local model · not connected"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("close-agent")
                            .px(px(5.0))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .hover(|style| style.text_color(rgb(theme::TEXT)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ai_open = false;
                                cx.notify();
                            }))
                            .child("×"),
                    ),
            )
            .child(
                div()
                    .text_size(px(17.0))
                    .line_height(px(22.0))
                    .text_color(rgb(theme::TEXT))
                    .child("What should we make better?"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(agent_suggestion("✳", "Improve the selected section"))
                    .child(agent_suggestion("↗", "Create a responsive version"))
                    .child(agent_suggestion("◇", "Explore a visual direction")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(11.0))
                    .py(px(9.0))
                    .rounded_md()
                    .bg(rgb(theme::WINDOW))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .text_sm()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Ask about this design…")
                    .child(
                        div()
                            .size(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(rgb(theme::ACCENT_WASH))
                            .text_color(rgb(theme::ACCENT))
                            .child("↑"),
                    ),
            )
    }

    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = div()
            .flex()
            .items_center()
            .gap_1()
            .p(px(5.0))
            .bg(rgb(theme::WINDOW));
        for (index, label) in ["Design", "Prototype", "AI"].iter().enumerate() {
            let selected = self.inspector_tab == index;
            let background = if selected {
                theme::SURFACE_RAISED
            } else {
                theme::WINDOW
            };
            let foreground = if selected {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            };
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("inspector-tab-{index}")))
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .py(px(8.0))
                    .rounded_md()
                    .bg(rgb(background))
                    .hover(|style| style.bg(rgb(theme::SURFACE_RAISED)))
                    .text_xs()
                    .text_color(rgb(foreground))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.inspector_tab = index;
                        cx.notify();
                    }))
                    .child(*label),
            );
        }

        let mut body = div().flex().flex_col().flex_1();
        if self.inspector_tab == 1 {
            body = body.child(prototype_inspector());
        } else if self.inspector_tab == 2 {
            body = body.child(ai_inspector());
        } else {
            body = body.child(self.design_inspector(cx));
        }

        div()
            .flex()
            .flex_col()
            .w(px(264.0))
            .flex_shrink_0()
            .bg(rgb(theme::SURFACE))
            .border_l_1()
            .border_color(rgb(theme::BORDER_SOFT))
            .child(tabs)
            .child(body)
    }

    fn appearance_fields(
        &self,
        selected: &[canvas::DesignObject],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let fill = common_fill(selected);
        let stroke = common_stroke(selected);
        let fill_enabled = selected.iter().all(|object| object.fill.is_some());
        let stroke_enabled = selected.iter().all(|object| object.stroke.is_some());
        let fill_mixed = fill.is_none();
        let stroke_mixed = stroke.is_none();
        let fill_label = match fill {
            Some(Some(color)) => color_hex(color),
            Some(None) => "None".to_string(),
            None => "Mixed".to_string(),
        };
        let stroke_label = match stroke {
            Some(Some(color)) => color_hex(color),
            Some(None) => "None".to_string(),
            None => "Mixed".to_string(),
        };
        let first_fill = selected
            .iter()
            .find_map(|object| object.fill.map(|fill| fill.color));
        let first_stroke = selected
            .iter()
            .find_map(|object| object.stroke.map(|stroke| stroke.color));
        let fill_toggle_color = fill
            .flatten()
            .or(first_fill)
            .unwrap_or(canvas::Color::from_rgb(theme::SURFACE_RAISED));
        let stroke_toggle_color = stroke
            .flatten()
            .or(first_stroke)
            .unwrap_or(canvas::Color::from_rgb(theme::BORDER));

        let mut fields = div().flex().flex_col().gap_3().px(px(14.0)).py(px(12.0));

        let fill_target = if fill_enabled && !fill_mixed {
            canvas::StyleEdit::Fill(None)
        } else {
            canvas::StyleEdit::Fill(Some(fill_toggle_color))
        };
        fields = fields.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .child("Fill"),
                )
                .child(
                    div()
                        .id("fill-state-toggle")
                        .px(px(7.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(theme::WINDOW))
                        .border_1()
                        .border_color(rgb(theme::BORDER_SOFT))
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.canvas.update(cx, |canvas, cx| {
                                canvas.set_selected_style(fill_target, cx);
                            });
                        }))
                        .child(if fill_mixed {
                            "Mixed"
                        } else if fill_enabled {
                            "On"
                        } else {
                            "Off"
                        }),
                )
                .child(
                    div()
                        .id("fill-color-trigger")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px(px(6.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(theme::WINDOW))
                        .border_1()
                        .border_color(rgb(theme::BORDER_SOFT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.color_picker = if this.color_picker == Some(StyleProperty::Fill) {
                                None
                            } else {
                                Some(StyleProperty::Fill)
                            };
                            cx.notify();
                        }))
                        .child(
                            div().size(px(12.0)).rounded_sm().bg(rgb(fill
                                .flatten()
                                .map_or(theme::SURFACE_RAISED, canvas::Color::to_rgb))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SECONDARY))
                                .child(fill_label),
                        ),
                ),
        );
        if self.color_picker == Some(StyleProperty::Fill) {
            fields = fields.child(self.style_palette(StyleProperty::Fill, cx));
        }

        let stroke_target = if stroke_enabled && !stroke_mixed {
            canvas::StyleEdit::Stroke(None)
        } else {
            canvas::StyleEdit::Stroke(Some(stroke_toggle_color))
        };
        fields = fields.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .child("Stroke"),
                )
                .child(
                    div()
                        .id("stroke-state-toggle")
                        .px(px(7.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(theme::WINDOW))
                        .border_1()
                        .border_color(rgb(theme::BORDER_SOFT))
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.canvas.update(cx, |canvas, cx| {
                                canvas.set_selected_style(stroke_target, cx);
                            });
                        }))
                        .child(if stroke_mixed {
                            "Mixed"
                        } else if stroke_enabled {
                            "On"
                        } else {
                            "Off"
                        }),
                )
                .child(
                    div()
                        .id("stroke-color-trigger")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px(px(6.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(theme::WINDOW))
                        .border_1()
                        .border_color(rgb(theme::BORDER_SOFT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.color_picker = if this.color_picker == Some(StyleProperty::Stroke)
                            {
                                None
                            } else {
                                Some(StyleProperty::Stroke)
                            };
                            cx.notify();
                        }))
                        .child(
                            div().size(px(12.0)).rounded_sm().bg(rgb(stroke
                                .flatten()
                                .map_or(theme::BORDER, canvas::Color::to_rgb))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SECONDARY))
                                .child(stroke_label),
                        ),
                ),
        );
        if self.color_picker == Some(StyleProperty::Stroke) {
            fields = fields.child(self.style_palette(StyleProperty::Stroke, cx));
        }

        let width_state = common_stroke_width(selected);
        let width_label = match width_state {
            Some(Some(width)) => format_width(width),
            Some(None) => "—".to_string(),
            None => "Mixed".to_string(),
        };
        let mut width_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child("Width"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(width_label),
            );
        for width in [1.0_f32, 2.0, 4.0, 8.0] {
            width_row = width_row.child(
                div()
                    .id(SharedString::from(format!("stroke-width-{width}")))
                    .px(px(7.0))
                    .py(px(5.0))
                    .rounded_sm()
                    .bg(rgb(theme::WINDOW))
                    .border_1()
                    .border_color(rgb(theme::BORDER_SOFT))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.canvas.update(cx, |canvas, cx| {
                            canvas.set_selected_style(canvas::StyleEdit::StrokeWidth(width), cx);
                        });
                    }))
                    .child(format_width(width)),
            );
        }
        fields
            .child(width_row)
            .child(self.style_value_fields(selected, cx))
    }

    /// The object's name, or the rename buffer while one is open.
    ///
    /// Clicking the name opens a rename; the typed text is a buffer in the shell
    /// and becomes a real name only when Enter hands it to the canvas as one
    /// semantic operation. Escape and an empty name abandon it, leaving the
    /// document untouched.
    fn object_name(
        &self,
        object: &canvas::DesignObject,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let renaming = self
            .rename
            .as_ref()
            .filter(|(id, _)| *id == object.id)
            .map(|(_, buffer)| buffer.clone());
        let name = div()
            .text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(if renaming.is_some() {
                theme::ACCENT
            } else {
                theme::TEXT
            }))
            .child(match &renaming {
                Some(buffer) => format!("{buffer}|"),
                None => object.name.clone(),
            });
        let id = object.id;
        let stateful = renaming.is_some();
        div()
            .id(SharedString::from(format!("rename-{}", object.id.0)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if stateful {
                    return;
                }
                this.rename = Some((id, String::new()));
                cx.notify();
            }))
            .child(name)
    }

    /// The second half of the Appearance section: the properties this milestone
    /// can express in CSS.
    ///
    /// Four controls, each mapped to one `StyleEdit` and one supported CSS
    /// property. Nothing here invents a value the style model cannot write back,
    /// because a control that cannot be saved is worse than a missing one.
    fn style_value_fields(
        &self,
        selected: &[canvas::DesignObject],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text_color = common_text_color(selected);
        let font_size = common_font_size(selected);
        let radius = common_number(selected, |object| object.border_radius);
        let opacity = common_number(selected, |object| object.opacity);

        let mut fields = div().flex().flex_col().gap_3().px(px(14.0)).py(px(12.0));

        // Text colour: the same palette as fill and stroke, because it is the
        // same model — one colour, one semantic operation, one CSS property.
        let text_label = match text_color {
            Some(Some(color)) => color_hex(color),
            Some(None) => "None".to_owned(),
            None => "Mixed".to_owned(),
        };
        let swatch = text_color
            .flatten()
            .map_or(theme::TEXT, canvas::Color::to_rgb);
        fields = fields.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .child("Text colour"),
                )
                .child(
                    div()
                        .id("text-color-trigger")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px(px(6.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .bg(rgb(theme::WINDOW))
                        .border_1()
                        .border_color(rgb(theme::BORDER_SOFT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.color_picker =
                                if this.color_picker == Some(StyleProperty::TextColor) {
                                    None
                                } else {
                                    Some(StyleProperty::TextColor)
                                };
                            cx.notify();
                        }))
                        .child(div().size(px(12.0)).rounded_sm().bg(rgb(swatch)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SECONDARY))
                                .child(text_label),
                        ),
                ),
        );
        if self.color_picker == Some(StyleProperty::TextColor) {
            fields = fields.child(self.style_palette(StyleProperty::TextColor, cx));
        }

        fields = fields
            .child(Self::value_presets(
                "Size",
                font_size.map(format_width),
                ["12", "16", "24", "32"],
                cx,
                |text| canvas::StyleEdit::FontSize(text.parse().unwrap_or(16.0)),
            ))
            .child(Self::value_presets(
                "Radius",
                radius.map(format_width),
                ["0", "4", "8", "16"],
                cx,
                |text| canvas::StyleEdit::BorderRadius(text.parse().unwrap_or(0.0)),
            ))
            .child(Self::value_presets(
                "Opacity",
                opacity.map(|value| format!("{:.0}%", value * 100.0)),
                ["25", "50", "75", "100"],
                cx,
                |text| canvas::StyleEdit::Opacity(text.parse::<f32>().unwrap_or(1.0) / 100.0),
            ));
        fields
    }

    fn style_palette(&self, property: StyleProperty, cx: &mut Context<Self>) -> impl IntoElement {
        let mut palette = div()
            .flex()
            .flex_col()
            .gap_1()
            .p(px(7.0))
            .rounded_md()
            .bg(rgb(theme::WINDOW))
            .border_1()
            .border_color(rgb(theme::BORDER));
        for chunk in STYLE_COLORS.chunks(3) {
            let mut row = div().flex().gap_1();
            for (name, value) in chunk {
                let name = *name;
                let value = *value;
                let edit = match property {
                    StyleProperty::Fill => {
                        canvas::StyleEdit::Fill(Some(canvas::Color::from_rgb(value)))
                    }
                    StyleProperty::Stroke => {
                        canvas::StyleEdit::Stroke(Some(canvas::Color::from_rgb(value)))
                    }
                    StyleProperty::TextColor => {
                        canvas::StyleEdit::TextColor(Some(canvas::Color::from_rgb(value)))
                    }
                };
                row = row.child(
                    div()
                        .id(SharedString::from(format!("style-color-{name}")))
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap_1()
                        .px(px(4.0))
                        .py(px(5.0))
                        .rounded_sm()
                        .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.canvas.update(cx, |canvas, cx| {
                                canvas.set_selected_style(edit, cx);
                            });
                            this.color_picker = None;
                            cx.notify();
                        }))
                        .child(div().size(px(10.0)).rounded_sm().bg(rgb(value)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SECONDARY))
                                .child(name),
                        ),
                );
            }
            palette = palette.child(row);
        }
        palette
    }

    fn design_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        crate::diagnostics::count("inspector_build", 1);
        let projection_start = crate::diagnostics::start();
        let selected = self.canvas.read(cx).selected_objects();
        crate::diagnostics::record("inspector_projection", projection_start);
        crate::diagnostics::count("inspector_objects_copied", selected.len() as u64);
        let content = match selected.as_slice() {
            [] => div()
                .flex()
                .flex_col()
                .gap_2()
                .p(px(16.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT))
                        .child("No selection"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child("Select an object on the canvas or in Layers."),
                )
                .into_any_element(),
            [_, _, ..] => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p(px(16.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(theme::TEXT))
                                .child(format!("{} objects selected", selected.len())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child("Style changes apply to the entire selection."),
                        ),
                )
                .child(self.inspector_section(
                    "Position and size",
                    0,
                    Self::position_fields_mixed(&selected),
                    cx,
                ))
                .child(self.inspector_section(
                    "Appearance",
                    2,
                    self.appearance_fields(&selected, cx),
                    cx,
                ))
                .into_any_element(),
            [object] => {
                let kind = object.object_type.label();
                let mut inspector = div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(14.0))
                            .py(px(13.0))
                            .border_b_1()
                            .border_color(rgb(theme::BORDER_SOFT))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(theme::TEXT_MUTED))
                                            .child("▱"),
                                    )
                                    .child(self.object_name(object, cx)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(kind),
                            ),
                    )
                    .child(self.inspector_section(
                        "Position and size",
                        0,
                        self.position_fields(object, cx),
                        cx,
                    ))
                    .child(self.inspector_section("Layout", 1, layout_fields(), cx))
                    .child(self.inspector_section(
                        "Appearance",
                        2,
                        self.appearance_fields(&selected, cx),
                        cx,
                    ));
                if let Some(text) = object.text_content.as_deref() {
                    inspector = inspector.child(text_content_section(text));
                }
                inspector
                    .child(simple_section("Effects", "Add effect"))
                    .child(simple_section("Typography", "2 text styles"))
                    .child(simple_section("Export", "Add export setting"))
                    .into_any_element()
            }
        };

        div()
            .flex()
            .flex_col()
            .id("inspector-scroll")
            .flex_1()
            .overflow_y_scroll()
            .child(content)
    }

    fn inspector_section(
        &self,
        title: &'static str,
        index: usize,
        fields: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let collapsed = self.collapsed_sections[index];
        let mut section = div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(rgb(theme::BORDER_SOFT))
            .child(
                div()
                    .id(SharedString::from(format!("section-{index}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .py(px(12.0))
                    .hover(|style| style.bg(rgb(theme::SURFACE_RAISED)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.collapsed_sections[index] = !this.collapsed_sections[index];
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(if collapsed { "＋" } else { "−" }),
                    ),
            );
        if !collapsed {
            section = section.child(fields);
        }
        section
    }

    /// The four geometry fields, for a single selected object.
    ///
    /// Each one is draggable: dragging changes the runtime geometry live, and the
    /// whole drag becomes one semantic operation when it ends. The values are read
    /// from the canvas on every render, so a canvas drag and a field drag are the
    /// same fact shown in two places rather than two stores.
    fn position_fields(
        &self,
        object: &canvas::DesignObject,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px(px(14.0))
            .pb(px(13.0))
            .child(self.geometry_field_pair(object, GeometryField::X, GeometryField::Y, cx))
            .child(self.geometry_field_pair(
                object,
                GeometryField::Width,
                GeometryField::Height,
                cx,
            ))
    }

    /// The same four fields for a multi-selection.
    ///
    /// Read-only on purpose: a mixed selection has no single value to drag, and
    /// inventing one would mean keeping a second copy of the geometry to average.
    /// Showing `Mixed` is the honest answer until a multi-object move is wired to
    /// these fields.
    fn position_fields_mixed(selected: &[canvas::DesignObject]) -> impl IntoElement {
        let common = |field: GeometryField| {
            let first = selected.first().map(|object| canvas::Geometry {
                position: object.position,
                size: object.size,
            });
            let first = first?;
            selected
                .iter()
                .all(|object| {
                    field.value_of(canvas::Geometry {
                        position: object.position,
                        size: object.size,
                    }) == field.value_of(first)
                })
                .then(|| format_geometry(field.value_of(first)))
        };
        let label = |field: GeometryField| common(field).unwrap_or_else(|| "Mixed".to_owned());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px(px(14.0))
            .pb(px(13.0))
            .child(Self::geometry_field_pair_static(
                "X",
                label(GeometryField::X),
                "Y",
                label(GeometryField::Y),
            ))
            .child(Self::geometry_field_pair_static(
                "W",
                label(GeometryField::Width),
                "H",
                label(GeometryField::Height),
            ))
    }

    fn geometry_field_pair_static(
        left_name: &'static str,
        left_value: String,
        right_name: &'static str,
        right_value: String,
    ) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .child(Self::geometry_field_static(left_name, left_value))
            .child(Self::geometry_field_static(right_name, right_value))
    }

    fn geometry_field_static(name: &'static str, value: String) -> impl IntoElement {
        Self::geometry_field_body(name, value, false)
    }

    /// One draggable geometry field.
    ///
    /// The baseline for the drag is read from the runtime at the moment the button
    /// goes down, so a field can never disagree with what the canvas is showing.
    fn geometry_field(
        &self,
        object: &canvas::DesignObject,
        field: GeometryField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let geometry = canvas::Geometry {
            position: object.position,
            size: object.size,
        };
        let value = format_geometry(field.value_of(geometry));
        let id = object.id;
        let active = self.geometry_target == Some((id, field));
        Self::geometry_field_body(field.label(), value, active)
            .id(SharedString::from(format!(
                "geometry-{}-{}",
                object.id.0,
                field.label()
            )))
            .cursor_default()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.begin_geometry_scrub(id, field, event.position.x.into(), cx)
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                    this.move_geometry_scrub(event.position.x.into(), field, cx)
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_geometry_scrub(cx)),
            )
    }

    fn geometry_field_pair(
        &self,
        object: &canvas::DesignObject,
        left: GeometryField,
        right: GeometryField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .child(self.geometry_field(object, left, cx))
            .child(self.geometry_field(object, right, cx))
    }

    /// Start dragging a geometry field.
    ///
    /// The value has already moved by the time this runs if the pointer moved into
    /// the field, so the baseline is the runtime geometry rather than a value read
    /// from the widget.
    fn begin_geometry_scrub(
        &mut self,
        id: canvas::ObjectId,
        field: GeometryField,
        screen_x: f32,
        cx: &mut Context<Self>,
    ) {
        self.end_geometry_scrub(cx);
        let gesture = self
            .canvas
            .update(cx, |canvas, _| canvas.begin_geometry_scrub(id));
        let Some(gesture) = gesture else {
            return;
        };
        self.geometry_target = Some((id, field));
        self.geometry_scrub = Some(GeometryScrub {
            field,
            gesture,
            start_x: screen_x,
            start_value: field.value_of(gesture.before),
        });
        cx.notify();
    }

    /// Move the object the scrub is dragging, without recording anything yet.
    fn move_geometry_scrub(&mut self, screen_x: f32, field: GeometryField, cx: &mut Context<Self>) {
        let Some(scrub) = self.geometry_scrub.as_ref() else {
            return;
        };
        if scrub.field != field {
            return;
        }
        let value = (scrub.start_value + (screen_x - scrub.start_x)).round();
        let next = field.with_value(scrub.gesture.before, value);
        let gesture = scrub.gesture;
        self.edit_canvas(cx, |canvas, _| canvas.scrub_geometry(&gesture, next));
        cx.notify();
    }

    /// Finish a scrub: one semantic operation, or none at all if it landed where it
    /// started.
    fn end_geometry_scrub(&mut self, cx: &mut Context<Self>) {
        let Some(scrub) = self.geometry_scrub.take() else {
            return;
        };
        self.edit_canvas(cx, |canvas, _| canvas.commit_geometry_scrub(scrub.gesture));
        cx.notify();
    }

    /// The shared look of a geometry field.
    fn geometry_field_body(name: &'static str, value: String, active: bool) -> gpui::Div {
        let border = if active {
            theme::ACCENT
        } else {
            theme::BORDER_SOFT
        };
        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_between()
            .gap_2()
            .px(px(8.0))
            .py(px(8.0))
            .rounded_md()
            .bg(rgb(theme::WINDOW))
            .border_1()
            .border_color(rgb(border))
            .cursor_default()
            .text_xs()
            .child(div().text_color(rgb(theme::TEXT_MUTED)).child(name))
            .child(div().text_color(rgb(theme::TEXT_SECONDARY)).child(value))
    }

    /// A row of preset buttons that each set one style property.
    ///
    /// Presets rather than free text: each one is a value the style model can
    /// definitely express, so every button is a control whose edit saves. One click
    /// is one semantic operation, the same as a canvas gesture.
    fn value_presets(
        label: &'static str,
        current: Option<String>,
        presets: [&'static str; 4],
        cx: &mut Context<Self>,
        parse: fn(&'static str) -> canvas::StyleEdit,
    ) -> impl IntoElement {
        let current = current.unwrap_or_else(|| "Mixed".to_owned());
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child(label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(current),
            );
        for preset in presets {
            row = row.child(
                div()
                    .id(SharedString::from(format!("{label}-{preset}")))
                    .px(px(7.0))
                    .py(px(5.0))
                    .rounded_sm()
                    .bg(rgb(theme::WINDOW))
                    .border_1()
                    .border_color(rgb(theme::BORDER_SOFT))
                    .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let edit = parse(preset);
                        this.canvas
                            .update(cx, |canvas, cx| canvas.set_selected_style(edit, cx));
                    }))
                    .child(preset),
            );
        }
        row
    }

    fn render_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .overflow_hidden()
            .child(self.left_sidebar(cx))
            .child(self.canvas_area(cx))
            .child(self.inspector(cx))
    }
}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::diagnostics::count("shell_render", 1);
        let render_start = crate::diagnostics::start();
        self.layers.update(cx, |layers, cx| {
            // Structure changes and selection changes both notify the Layers
            // view so its subtree rebuilds. Selection changes are first
            // reduced to an O(changed rows) presentation diff (counted as
            // `row_presentation_updates`); applying the diff requires the
            // subtree rebuild described in the Phase 15 report.
            let outcome = layers.synchronize(self.canvas.read(cx));
            if outcome.structure_changed || !outcome.presentation.is_empty() {
                cx.notify();
            }
        });
        let element = div()
            .id("spool-shell")
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::WINDOW))
            .text_color(rgb(theme::TEXT))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.canvas.read(cx).workload_controls_input() {
                        return;
                    }
                    if this.canvas.read(cx).is_text_editing() {
                        this.canvas
                            .update(cx, |canvas, cx| canvas.commit_text_edit_session(cx));
                    }
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if this.canvas.read(cx).workload_controls_input() {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let modifiers = event.keystroke.modifiers;
                // A rename owns the keyboard while it is open: letters go into
                // the name, not into tool shortcuts.
                if this.rename.is_some()
                    && !modifiers.control
                    && !modifiers.platform
                    && this.rename_key(key, cx)
                {
                    return;
                }
                if this.geometry_key(key, modifiers.shift, cx) {
                    return;
                }
                // Escape abandons an in-progress Inspector drag, exactly as it
                // abandons a canvas one: the object goes back where it started
                // and nothing is recorded.
                if key == "escape" {
                    if let Some(scrub) = this.geometry_scrub.take() {
                        this.edit_canvas(cx, |canvas, _| {
                            canvas.cancel_geometry_scrub(scrub.gesture)
                        });
                        cx.notify();
                        return;
                    }
                }
                if this.canvas.read(cx).is_text_editing() {
                    match shortcut_action(
                        key,
                        modifiers.platform,
                        modifiers.control,
                        modifiers.shift,
                    ) {
                        Some(ShortcutAction::Undo) => {
                            this.canvas.update(cx, |canvas, cx| canvas.undo(cx));
                        }
                        Some(ShortcutAction::Redo) => {
                            this.canvas.update(cx, |canvas, cx| canvas.redo(cx));
                        }
                        Some(ShortcutAction::Duplicate) => {
                            this.canvas
                                .update(cx, |canvas, cx| canvas.duplicate_selection(cx));
                        }
                        Some(ShortcutAction::Delete)
                        | Some(ShortcutAction::Tool(_))
                        | Some(ShortcutAction::Save)
                        | None => {
                            if key == "escape" {
                                this.canvas
                                    .update(cx, |canvas, cx| canvas.cancel_text_edit(cx));
                            }
                        }
                    }
                    return;
                }
                match shortcut_action(key, modifiers.platform, modifiers.control, modifiers.shift) {
                    Some(ShortcutAction::Undo) => {
                        this.canvas.update(cx, |canvas, cx| canvas.undo(cx));
                        return;
                    }
                    Some(ShortcutAction::Save) => {
                        this.save_project(cx);
                        return;
                    }
                    Some(ShortcutAction::Redo) => {
                        this.canvas.update(cx, |canvas, cx| canvas.redo(cx));
                        return;
                    }
                    Some(ShortcutAction::Delete) => {
                        this.canvas
                            .update(cx, |canvas, cx| canvas.delete_selection(cx));
                        return;
                    }
                    Some(ShortcutAction::Duplicate) => {
                        this.canvas
                            .update(cx, |canvas, cx| canvas.duplicate_selection(cx));
                        return;
                    }
                    Some(ShortcutAction::Tool(tool)) => {
                        this.selected_tool = tool;
                        this.canvas.update(cx, |canvas, _| canvas.set_tool(tool));
                        cx.notify();
                        return;
                    }
                    None => {}
                }
                let tool = match key {
                    "escape" => {
                        let had_overlay =
                            this.ai_open || this.share_open || this.export_open || this.zoom_open;
                        let had_selection = !this.canvas.read(cx).selection().is_empty();
                        let cancelled_manipulation = this
                            .canvas
                            .update(cx, |canvas, cx| canvas.cancel_manipulation(cx));
                        this.ai_open = false;
                        this.share_open = false;
                        this.export_open = false;
                        this.zoom_open = false;
                        if !cancelled_manipulation {
                            this.canvas
                                .update(cx, |canvas, cx| canvas.clear_selection(cx));
                        }
                        if had_overlay || had_selection || cancelled_manipulation {
                            cx.notify();
                        }
                        None
                    }
                    "1" if event.keystroke.modifiers.shift => {
                        this.canvas.update(cx, |canvas, _| canvas.fit_canvas());
                        cx.notify();
                        None
                    }
                    "space" | " " => {
                        this.canvas
                            .update(cx, |canvas, _| canvas.set_space_held(true));
                        None
                    }
                    _ => None,
                };
                if let Some(tool) = tool {
                    this.selected_tool = tool;
                    this.canvas.update(cx, |canvas, _| canvas.set_tool(tool));
                    cx.notify();
                }
            }))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "space" | " ") {
                    this.canvas
                        .update(cx, |canvas, _| canvas.set_space_held(false));
                }
            }))
            .child(self.top_bar(cx))
            .child(self.render_body(cx));
        crate::diagnostics::record("shell_render_build", render_start);
        element
    }
}

fn agent_suggestion(icon: &'static str, label: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px(px(10.0))
        .py(px(8.0))
        .rounded_md()
        .bg(rgb(theme::SURFACE_RAISED))
        .border_1()
        .border_color(rgb(theme::BORDER_SOFT))
        .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
        .text_xs()
        .text_color(rgb(theme::TEXT_SECONDARY))
        .child(div().text_color(rgb(theme::ACCENT)).child(icon))
        .child(label)
}

fn share_popover() -> impl IntoElement {
    div()
        .absolute()
        .top(px(40.0))
        .right(px(0.0))
        .flex()
        .flex_col()
        .gap_2()
        .w(px(240.0))
        .p(px(14.0))
        .rounded_lg()
        .bg(rgb(theme::SURFACE_RAISED))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .shadow_lg()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT))
                .child("Share this project"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child("Invites and shared links will be available here."),
        )
        .child(
            div()
                .mt(px(4.0))
                .px(px(10.0))
                .py(px(8.0))
                .rounded_md()
                .bg(rgb(theme::ACCENT_WASH))
                .text_xs()
                .text_color(rgb(theme::ACCENT))
                .child("Sharing not connected"),
        )
}

fn export_popover() -> impl IntoElement {
    div()
        .absolute()
        .top(px(40.0))
        .right(px(0.0))
        .flex()
        .flex_col()
        .gap_1()
        .w(px(205.0))
        .p(px(7.0))
        .rounded_lg()
        .bg(rgb(theme::SURFACE_RAISED))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .shadow_lg()
        .child(popover_row("Export selection…", "⇧ E"))
        .child(popover_row("Export page…", ""))
        .child(div().h(px(1.0)).my(px(4.0)).bg(rgb(theme::BORDER_SOFT)))
        .child(popover_row("Copy as SVG", ""))
        .child(popover_row("Copy properties", ""))
}

fn popover_row(label: &'static str, shortcut: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(9.0))
        .py(px(8.0))
        .rounded_md()
        .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
        .text_xs()
        .text_color(rgb(theme::TEXT_SECONDARY))
        .child(label)
        .child(div().text_color(rgb(theme::TEXT_MUTED)).child(shortcut))
}

fn field_pair(left: &'static str, right: &'static str) -> impl IntoElement {
    div().flex().gap_2().child(field(left)).child(field(right))
}

fn field(label: &'static str) -> impl IntoElement {
    let (name, value) = label.split_once("  ").unwrap_or((label, ""));
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_between()
        .gap_2()
        .px(px(8.0))
        .py(px(8.0))
        .rounded_md()
        .bg(rgb(theme::WINDOW))
        .border_1()
        .border_color(rgb(theme::BORDER_SOFT))
        .text_xs()
        .child(div().text_color(rgb(theme::TEXT_MUTED)).child(name))
        .child(div().text_color(rgb(theme::TEXT_SECONDARY)).child(value))
}

fn format_geometry(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn layout_fields() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px(px(14.0))
        .pb(px(13.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(
                    div()
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .child("Auto layout"),
                )
                .child(
                    div()
                        .w(px(29.0))
                        .h(px(16.0))
                        .flex()
                        .items_center()
                        .justify_end()
                        .px(px(2.0))
                        .rounded_full()
                        .bg(rgb(theme::ACCENT_WASH))
                        .child(div().size(px(12.0)).rounded_full().bg(rgb(theme::ACCENT))),
                ),
        )
        .child(field_pair("Gap  16", "Padding  24"))
        .child(
            div()
                .flex()
                .gap_2()
                .child(alignment_button("≡", true))
                .child(alignment_button("☷", false))
                .child(alignment_button("↕", false))
                .child(alignment_button("⊞", false)),
        )
}

fn alignment_button(icon: &'static str, selected: bool) -> impl IntoElement {
    let (background, foreground) = if selected {
        (theme::ACCENT_WASH, theme::ACCENT)
    } else {
        (theme::WINDOW, theme::TEXT_SECONDARY)
    };
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .h(px(31.0))
        .rounded_md()
        .bg(rgb(background))
        .border_1()
        .border_color(rgb(theme::BORDER_SOFT))
        .text_sm()
        .text_color(rgb(foreground))
        .child(icon)
}

fn common_fill(selected: &[canvas::DesignObject]) -> Option<Option<canvas::Color>> {
    let first = selected.first()?.fill.map(|fill| fill.color);
    selected
        .iter()
        .all(|object| object.fill.map(|fill| fill.color) == first)
        .then_some(first)
}

fn common_stroke(selected: &[canvas::DesignObject]) -> Option<Option<canvas::Color>> {
    let first = selected.first()?.stroke.map(|stroke| stroke.color);
    selected
        .iter()
        .all(|object| object.stroke.map(|stroke| stroke.color) == first)
        .then_some(first)
}

fn common_stroke_width(selected: &[canvas::DesignObject]) -> Option<Option<f32>> {
    let first = selected.first()?.stroke.map(|stroke| stroke.width);
    selected
        .iter()
        .all(|object| object.stroke.map(|stroke| stroke.width) == first)
        .then_some(first)
}

fn color_hex(color: canvas::Color) -> String {
    format!("#{:06X}", color.to_rgb())
}

/// A mixed selection shows `Mixed` rather than the first object's value, which
/// would be a number the user did not choose and could not act on.
/// The one value every selected object has for `read`, or `None` when they
/// differ.
///
/// A mixed selection shows `Mixed` rather than the first object's value, which
/// would be a number the user did not choose and could not act on.
fn common_number(
    selected: &[canvas::DesignObject],
    read: impl Fn(&canvas::DesignObject) -> f32,
) -> Option<f32> {
    let first = read(selected.first()?);
    selected
        .iter()
        .all(|object| read(object) == first)
        .then_some(first)
}

fn common_text_color(selected: &[canvas::DesignObject]) -> Option<Option<canvas::Color>> {
    let first = selected.first()?.text_color;
    selected
        .iter()
        .all(|object| object.text_color == first)
        .then_some(first)
}

fn common_font_size(selected: &[canvas::DesignObject]) -> Option<f32> {
    common_number(selected, |object| object.font_size.unwrap_or_default())
}

fn format_width(width: f32) -> String {
    if width.fract() == 0.0 {
        format!("{}", width as u32)
    } else {
        width.to_string()
    }
}

#[cfg(test)]
mod style_inspector_tests {
    use super::*;

    fn object(fill: Option<u32>, stroke: Option<(u32, f32)>) -> canvas::DesignObject {
        canvas::DesignObject {
            id: canvas::ObjectId::LANDING,
            spool_id: crate::source_document::NodeId::new("test-node").unwrap(),
            name: "Shape".to_string(),
            position: gpui::point(0.0, 0.0),
            size: gpui::size(20.0, 20.0),
            object_type: canvas::ObjectType::Rectangle,
            text_content: None,
            text_color: None,
            font_size: None,
            border_radius: 0.0,
            opacity: 1.0,
            fill: fill.map(|color| canvas::Fill {
                color: canvas::Color::from_rgb(color),
            }),
            stroke: stroke.map(|(color, width)| canvas::Stroke {
                color: canvas::Color::from_rgb(color),
                width,
            }),
        }
    }

    #[test]
    fn inspector_reports_mixed_fill_stroke_and_width_states() {
        let selected = [
            object(Some(theme::PAPER), Some((theme::INK, 1.0))),
            object(None, Some((theme::INK, 4.0))),
        ];

        assert_eq!(common_fill(&selected), None);
        assert_eq!(
            common_stroke(&selected),
            Some(Some(canvas::Color::from_rgb(theme::INK)))
        );
        assert_eq!(common_stroke_width(&selected), None);
    }

    #[test]
    fn inspector_reports_uniform_disabled_and_enabled_style_states() {
        let selected = [object(None, None), object(None, None)];
        assert_eq!(common_fill(&selected), Some(None));
        assert_eq!(common_stroke(&selected), Some(None));
        assert_eq!(common_stroke_width(&selected), Some(None));

        let selected = [
            object(Some(theme::PAPER), Some((theme::INK, 2.0))),
            object(Some(theme::PAPER), Some((theme::INK, 2.0))),
        ];
        assert_eq!(
            common_fill(&selected),
            Some(Some(canvas::Color::from_rgb(theme::PAPER)))
        );
        assert_eq!(
            common_stroke(&selected),
            Some(Some(canvas::Color::from_rgb(theme::INK)))
        );
        assert_eq!(common_stroke_width(&selected), Some(Some(2.0)));
    }
}

fn text_content_section(content: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px(px(14.0))
        .py(px(13.0))
        .border_b_1()
        .border_color(rgb(theme::BORDER_SOFT))
        .child(
            div()
                .text_xs()
                .text_color(rgb(theme::TEXT_MUTED))
                .child("Text content"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(theme::TEXT))
                .child(content.to_string()),
        )
}

fn simple_section(title: &'static str, value: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(14.0))
        .py(px(13.0))
        .border_b_1()
        .border_color(rgb(theme::BORDER_SOFT))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT))
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(theme::TEXT_MUTED))
                .child(value),
        )
}

fn prototype_inspector() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p(px(15.0))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT))
                .child("Prototype connections"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child("Link this frame to another screen to start a flow."),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(9.0))
                .rounded_md()
                .bg(rgb(theme::WINDOW))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .text_xs()
                .text_color(rgb(theme::TEXT_MUTED))
                .child("＋  Add interaction"),
        )
}

fn ai_inspector() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p(px(15.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(theme::ACCENT)).child("✦"))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT))
                        .child("Design with context"),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child("Spool can work with the selected Hero section and the styles around it."),
        )
        .child(agent_suggestion("↗", "Make this responsive"))
        .child(agent_suggestion("✳", "Suggest spacing improvements"))
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn delete_and_backspace_map_to_delete_command() {
        for key in ["delete", "backspace"] {
            assert_eq!(
                shortcut_action(key, false, false, false),
                Some(ShortcutAction::Delete)
            );
        }
    }

    #[test]
    fn duplicate_uses_platform_or_control_modifier() {
        assert_eq!(
            shortcut_action("d", true, false, false),
            Some(ShortcutAction::Duplicate)
        );
        assert_eq!(
            shortcut_action("d", false, true, false),
            Some(ShortcutAction::Duplicate)
        );
        assert_eq!(shortcut_action("d", false, false, false), None);
    }

    #[test]
    fn undo_redo_shortcuts_and_tool_keys_remain_available() {
        assert_eq!(
            shortcut_action("z", true, false, false),
            Some(ShortcutAction::Undo)
        );
        assert_eq!(
            shortcut_action("z", false, true, true),
            Some(ShortcutAction::Redo)
        );
        assert_eq!(
            shortcut_action("y", false, true, false),
            Some(ShortcutAction::Redo)
        );
        assert_eq!(
            shortcut_action("v", false, false, false),
            Some(ShortcutAction::Tool(canvas::Tool::Select))
        );
        assert_eq!(
            shortcut_action("f", false, false, false),
            Some(ShortcutAction::Tool(canvas::Tool::Frame))
        );
        assert_eq!(
            shortcut_action("r", false, false, false),
            Some(ShortcutAction::Tool(canvas::Tool::Rectangle))
        );
        assert_eq!(
            shortcut_action("o", false, false, false),
            Some(ShortcutAction::Tool(canvas::Tool::Ellipse))
        );
        assert_eq!(
            shortcut_action("t", false, false, false),
            Some(ShortcutAction::Tool(canvas::Tool::Text))
        );
        assert_eq!(shortcut_action("r", true, false, false), None);
    }

    #[test]
    fn save_is_reachable_from_the_keyboard_on_both_platform_conventions() {
        // The save half of the loop needs a real command, not only a developer
        // workflow. Both conventions map to it so the shortcut works wherever
        // the editor runs.
        assert_eq!(
            shortcut_action("s", true, false, false),
            Some(ShortcutAction::Save)
        );
        assert_eq!(
            shortcut_action("s", false, true, false),
            Some(ShortcutAction::Save)
        );
        // A bare "s" is still not a command: it must keep falling through to
        // text entry rather than silently saving.
        assert_eq!(shortcut_action("s", false, false, false), None);
    }
}
