use gpui::{
    div, point, prelude::*, px, rgb, Context, Entity, Modifiers, Render, SharedString,
    Subscription, Window,
};

use crate::{canvas, commands, inspector, layers::LayersView, project_open, theme};

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

/// The editor shell.
///
/// The shell arranges the three surfaces and owns the editor-wide keyboard: which
/// keys are shortcuts, and the order Escape unwinds them in. Everything about
/// showing and changing a selection belongs to the Inspector, which holds its own
/// interaction state for exactly that reason — there is no second copy of a
/// number, a scrub or a layer name anywhere in here.
pub struct AppShell {
    canvas: Entity<canvas::CanvasView>,
    layers: Entity<LayersView>,
    inspector: Entity<inspector::Inspector>,
    selected_tool: canvas::Tool,
    selected_page: usize,
    ai_open: bool,
    share_open: bool,
    export_open: bool,
    zoom_open: bool,
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
    /// edit has to cause a shell render or the surrounding chrome goes stale.
    /// Nothing reads this field; holding it is the point.
    #[allow(dead_code)]
    observations: Vec<Subscription>,
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let canvas = cx.new(canvas::CanvasView::new_with_context);
        canvas.update(cx, |view, _| open_requested_project(view));
        let layers = cx.new(|_| LayersView::new(canvas.downgrade()));
        let inspector = cx.new(|cx| inspector::Inspector::new(canvas.clone(), cx));
        // Held, not dropped: a GPUI subscription detaches its observer when it
        // goes away, and without it the chrome around the canvas would only
        // repaint when the shell itself changed.
        let observations = vec![cx.observe(&canvas, |_, _, cx| cx.notify())];
        Self {
            canvas,
            layers,
            inspector,
            observations,
            selected_tool: canvas::Tool::Select,
            selected_page: 0,
            ai_open: false,
            share_open: false,
            export_open: false,
            zoom_open: false,
            save_status: None,
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
    /// Escape, as one ladder walked from the top.
    ///
    /// What Escape cancels must never depend on which listener happened to see
    /// the key first, so this is the only place that decides and it decides by
    /// reading [`commands::EscapeState`]: the ladder and its order live there,
    /// where they can be tested without a window, and this function is the only
    /// place that acts on the rung it returns.
    ///
    /// The order is the finding, not a preference. Figma, tldraw and Affinity
    /// all treat Escape as "up one level" — a selection scope, a state chart, a
    /// tool mode — and Canva treats it as cancel:
    ///
    /// 1. an open panel, because a modal is above everything
    /// 2. the Inspector, which walks its own rungs in the same order: a text
    ///    buffer before a gesture, because what the user typed into is above
    ///    what they are dragging
    /// 3. a canvas gesture
    /// 4. an open text session
    /// 5. the selection
    /// 6. a non-Select tool, which returns to Select
    ///
    /// Returns whether anything was cancelled, so the caller knows the key was
    /// consumed.
    /// Release the pan modifier, whatever else happens.
    ///
    /// The space bar is the one piece of state in the editor that a key-down
    /// sets and a key-up clears, and it is only correct while both halves arrive.
    /// Escape is the way to lose the second half deliberately: the user is
    /// asking to back out of whatever they started, and "you are still holding
    /// the pan modifier" is not it.
    ///
    /// Cheap and unconditional: the worst a redundant call costs is one
    /// comparison, and being wrong here is a canvas that silently stops
    /// responding to drags.
    fn release_pan_modifier(&mut self, cx: &mut Context<Self>) {
        self.canvas
            .update(cx, |canvas, _| canvas.set_space_held(false));
    }

    fn escape_topmost(&mut self, cx: &mut Context<Self>) -> bool {
        // Any rung at all, so the pan modifier is released on the way past.
        self.release_pan_modifier(cx);
        // Whatever the Inspector is part-way through — a number being typed, a
        // name being typed, or a drag in flight — is read as one rung. The
        // panel walks its own rungs internally, in the same order, so the two
        // cannot disagree about which of its two states is on top.
        let state = commands::EscapeState {
            panel: self.ai_open || self.share_open || self.export_open || self.zoom_open,
            inspector: self
                .inspector
                .update(cx, |inspector, _| inspector.is_busy()),
            gesture: self.canvas.read(cx).is_manipulating(),
            text_editing: self.canvas.read(cx).is_text_editing(),
            selection: !self.canvas.read(cx).selection().is_empty(),
            tool: self.selected_tool != canvas::Tool::Select,
        };
        match state.next() {
            Some(commands::Rung::Panel) => {
                self.ai_open = false;
                self.share_open = false;
                self.export_open = false;
                self.zoom_open = false;
                cx.notify();
                true
            }
            Some(commands::Rung::Inspector) => self
                .inspector
                .update(cx, |inspector, cx| inspector.cancel_in_flight(cx)),
            Some(commands::Rung::Gesture) => self
                .canvas
                .update(cx, |canvas, cx| canvas.cancel_manipulation(cx)),
            Some(commands::Rung::TextEditing) => self
                .canvas
                .update(cx, |canvas, cx| canvas.cancel_text_edit(cx)),
            Some(commands::Rung::Selection) => {
                self.canvas
                    .update(cx, |canvas, cx| canvas.clear_selection(cx));
                true
            }
            Some(commands::Rung::Tool) => {
                self.selected_tool = canvas::Tool::Select;
                self.canvas
                    .update(cx, |canvas, _| canvas.set_tool(canvas::Tool::Select));
                cx.notify();
                true
            }
            None => false,
        }
    }

    /// Arrow keys: nudge a selection, otherwise pan the canvas.
    ///
    /// Figma's rule, recorded in `navigation.md`: with nothing selected the
    /// arrow keys pan, and with a selection they nudge it by 1px, or 10px with
    /// `⇧`. One press is one semantic operation, so it is one undo step — which
    /// is what makes holding an arrow key cheap to undo.
    ///
    /// Which of the two a key means is a fact about the document, not about the
    /// key, so it is read here rather than in
    /// [`commands::resolve`](commands::resolve): the resolver can only say "an
    /// arrow key was pressed".
    fn arrow_key(
        &mut self,
        direction: commands::Direction,
        coarse: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let (dx, dy) = match direction {
            commands::Direction::Left => (-1.0, 0.0),
            commands::Direction::Right => (1.0, 0.0),
            commands::Direction::Up => (0.0, -1.0),
            commands::Direction::Down => (0.0, 1.0),
        };
        let step = if coarse { 10.0 } else { 1.0 };
        let moved = self.edit_canvas(cx, |canvas, _| canvas.nudge_selection(dx * step, dy * step));
        if moved {
            return true;
        }
        // Nothing selected: the same keys move the view instead. A bigger jump,
        // because panning is navigation rather than an edit and does not need
        // the fine granularity a nudge does.
        let pan_step = if coarse { 400.0 } else { 40.0 };
        self.canvas.update(cx, |canvas, _| {
            canvas.pan_by(point(dx * pan_step, dy * pan_step))
        });
        cx.notify();
        true
    }

    /// The one place a keystroke the whole editor answers to is handled.
    ///
    /// Four steps, always in this order, and the order is the whole point: what
    /// a key means is decided by which surface is above, never by which listener
    /// happened to run first.
    ///
    /// 1. A performance workload owns the input outright while it is running.
    /// 2. The Inspector gets first refusal. A number or a name being typed is
    ///    not an editor shortcut, so letters go into the field rather than into
    ///    the tool shortcuts. It declines Escape, which is the ladder's.
    /// 3. Escape walks [`commands::EscapeState`] from the top. It is not in the
    ///    command table, because "leave the most recent thing you entered" is
    ///    not one of the table's verbs.
    /// 4. Everything else is one row of [`commands::resolve`].
    fn handle_key(&mut self, key: &str, modifiers: Modifiers, cx: &mut Context<Self>) {
        if self.canvas.read(cx).workload_controls_input() {
            return;
        }
        if self.inspector.update(cx, |inspector, cx| {
            inspector.handle_key(key, modifiers.shift, cx)
        }) {
            return;
        }
        if key == "escape" {
            self.escape_topmost(cx);
            return;
        }
        // The pan modifier is held rather than pressed, so it is not a command
        // and has no place in the table: it is answered on the way down and
        // released on the way up. Space still scrolls the page when there is
        // nothing to pan, so it is answered only here and nowhere else.
        if matches!(key, "space" | " ") {
            self.canvas
                .update(cx, |canvas, _| canvas.set_space_held(true));
            return;
        }
        let scope = if self.canvas.read(cx).is_text_editing() {
            commands::Scope::TextEditing
        } else {
            commands::Scope::Editor
        };
        if let Some(command) = commands::resolve(key, modifiers, scope) {
            self.run_command(command, cx);
        }
    }

    /// Carry out one editor-wide command.
    ///
    /// Every history-affecting arm below goes through the canvas, which commits
    /// to the one `EditSession` history the pointer uses. There is no second
    /// route from the keyboard to the document, and no arm that edits state
    /// directly: a command that mutated here would be invisible to undo.
    fn run_command(&mut self, command: commands::Command, cx: &mut Context<Self>) {
        match command {
            commands::Command::Undo => {
                self.canvas.update(cx, |canvas, cx| canvas.undo(cx));
            }
            commands::Command::Redo => {
                self.canvas.update(cx, |canvas, cx| canvas.redo(cx));
            }
            commands::Command::Save => self.save_project(cx),
            commands::Command::Delete => {
                self.canvas
                    .update(cx, |canvas, cx| canvas.delete_selection(cx));
            }
            commands::Command::Duplicate => {
                self.canvas
                    .update(cx, |canvas, cx| canvas.duplicate_selection(cx));
            }
            commands::Command::SelectAll => {
                self.canvas.update(cx, |canvas, cx| canvas.select_all(cx));
            }
            commands::Command::Rename => self.rename_selection(cx),
            commands::Command::Tool(tool) => {
                self.selected_tool = tool;
                self.canvas.update(cx, |canvas, _| canvas.set_tool(tool));
                cx.notify();
            }
            commands::Command::Nudge { direction, coarse } => {
                self.arrow_key(direction, coarse, cx);
            }
            commands::Command::Traverse(step) => {
                self.canvas
                    .update(cx, |canvas, cx| canvas.traverse(step, cx));
            }
            commands::Command::ZoomIn => {
                self.canvas.update(cx, |canvas, _| canvas.zoom_in());
            }
            commands::Command::ZoomOut => {
                self.canvas.update(cx, |canvas, _| canvas.zoom_out());
            }
            commands::Command::ZoomToFit => {
                self.canvas.update(cx, |canvas, _| canvas.fit_canvas());
            }
            commands::Command::ZoomToSelection => {
                self.canvas
                    .update(cx, |canvas, _| canvas.zoom_to_selection());
            }
            commands::Command::ZoomToActualSize => {
                self.canvas
                    .update(cx, |canvas, _| canvas.zoom_to_actual_size());
            }
        }
        // A camera command moves no document state, so the chrome around the
        // canvas has to be told even though the canvas already notified.
        if command.is_camera() {
            cx.notify();
        }
    }

    /// Open a rename on the selection — `Enter` or `F2`.
    ///
    /// The Inspector owns the one rename model, whichever surface asked for it:
    /// the Layers panel states a rename request and this states the same kind of
    /// request, so a name typed from the canvas is the same buffer, the same
    /// commit, and the same Escape behaviour as one typed from the panel.
    ///
    /// Only a single selection. Figma renames the layer it has selected and
    /// does nothing otherwise, and renaming forty layers into one string would
    /// not be a rename.
    fn rename_selection(&mut self, cx: &mut Context<Self>) {
        let ids = self.canvas.read(cx).selection().ids().to_vec();
        let [id] = ids.as_slice() else {
            return;
        };
        self.inspector.update(cx, |inspector, cx| {
            inspector.open_rename(*id, cx);
            cx.notify();
        });
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

    fn render_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .overflow_hidden()
            .child(self.left_sidebar(cx))
            .child(self.canvas_area(cx))
            // The Inspector is its own view: it reads the canvas, holds its own
            // interaction state, and repaints itself when the document changes.
            // Mounting it is the whole integration — the shell has no say in
            // what a property row looks like or what a drag means.
            .child(self.inspector.clone())
    }
}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::diagnostics::count("shell_render", 1);
        let render_start = crate::diagnostics::start();
        // The Layers panel owns no editing state: it says which row the user
        // double-clicked, and the surface that owns the editing model opens it.
        // One rename model means one history boundary no matter which surface
        // started it.
        let requested = self
            .layers
            .update(cx, |layers, _| layers.take_rename_request());
        self.inspector.update(cx, |inspector, cx| {
            if let Some(id) = requested {
                inspector.open_rename(id, cx);
            }
        });
        let renaming_row = self.inspector.read(cx).renaming();
        self.layers.update(cx, |layers, cx| {
            // Structure changes and selection changes both notify the Layers
            // view so its subtree rebuilds. Selection changes are first
            // reduced to an O(changed rows) presentation diff (counted as
            // `row_presentation_updates`); applying the diff requires the
            // subtree rebuild described in the Phase 15 report.
            layers.set_renaming(renaming_row);
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
                this.handle_key(event.keystroke.key.as_str(), event.keystroke.modifiers, cx);
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

pub(crate) fn agent_suggestion(icon: &'static str, label: &'static str) -> impl IntoElement {
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
