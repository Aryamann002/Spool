mod canvas;
mod diagnostics;
mod document_runtime_bridge;
mod layers;
pub mod project_bundle;
pub mod source_binding;
mod shell;
pub mod source_document;
mod theme;

use gpui::{
    px, size, App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
};
use gpui_platform::application;

fn main() {
    application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", canvas::Backspace, None),
            KeyBinding::new("delete", canvas::Delete, None),
            KeyBinding::new("left", canvas::Left, None),
            KeyBinding::new("right", canvas::Right, None),
            KeyBinding::new("shift-left", canvas::SelectLeft, None),
            KeyBinding::new("shift-right", canvas::SelectRight, None),
            KeyBinding::new("cmd-a", canvas::SelectAll, None),
            KeyBinding::new("ctrl-a", canvas::SelectAll, None),
            KeyBinding::new("home", canvas::Home, None),
            KeyBinding::new("end", canvas::End, None),
            KeyBinding::new("cmd-v", canvas::Paste, None),
            KeyBinding::new("ctrl-v", canvas::Paste, None),
            KeyBinding::new("cmd-c", canvas::Copy, None),
            KeyBinding::new("ctrl-c", canvas::Copy, None),
            KeyBinding::new("cmd-x", canvas::Cut, None),
            KeyBinding::new("ctrl-x", canvas::Cut, None),
        ]);
        let bounds = Bounds::centered(None, size(px(1480.0), px(960.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Spool".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(shell::AppShell::new),
        )
        .expect("failed to open Spool window");
        cx.activate(true);
    });
}
