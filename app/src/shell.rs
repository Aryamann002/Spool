use gpui::{div, prelude::*, px, rgb, Context, Entity, Render, SharedString, Window};

use crate::{canvas, layers::LayersView, theme};

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
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let canvas = cx.new(canvas::CanvasView::new_with_context);
        let layers = cx.new(|_| LayersView::new(canvas.downgrade()));
        Self {
            canvas,
            layers,
            selected_tool: canvas::Tool::Select,
            selected_page: 0,
            inspector_tab: 0,
            collapsed_sections: [false; 4],
            ai_open: false,
            share_open: false,
            export_open: false,
            zoom_open: false,
            color_picker: None,
        }
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
        fields.child(width_row)
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
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(rgb(theme::TEXT))
                                            .child(object.name.clone()),
                                    ),
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
                        position_fields(object),
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
                        Some(ShortcutAction::Delete) | Some(ShortcutAction::Tool(_)) | None => {
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

fn position_fields(object: &canvas::DesignObject) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px(px(14.0))
        .pb(px(13.0))
        .child(geometry_field_pair(
            "X",
            object.position.x,
            "Y",
            object.position.y,
        ))
        .child(geometry_field_pair(
            "W",
            object.size.width,
            "H",
            object.size.height,
        ))
}

fn geometry_field_pair(
    left_name: &'static str,
    left_value: f32,
    right_name: &'static str,
    right_value: f32,
) -> impl IntoElement {
    div()
        .flex()
        .gap_2()
        .child(geometry_field(left_name, left_value))
        .child(geometry_field(right_name, right_value))
}

fn geometry_field(name: &'static str, value: f32) -> impl IntoElement {
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
        .child(
            div()
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child(format_geometry(value)),
        )
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
            name: "Shape".to_string(),
            position: gpui::point(0.0, 0.0),
            size: gpui::size(20.0, 20.0),
            object_type: canvas::ObjectType::Rectangle,
            text_content: None,
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
}
