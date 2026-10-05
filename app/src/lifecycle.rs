//! Application and window lifecycle.
//!
//! Spool installs no menu bar, so AppKit never sees the two key equivalents a
//! Mac user reaches for without thinking. GPUI supplies neither of them by
//! itself: `cmd-q` and `cmd-w` are ordinary keybindings, and until something
//! binds them they fall through to nothing. So `⌘Q` did not quit and `⌘W` did
//! not close the window.
//!
//! The native close button needed no work. GPUI's macOS backend implements
//! `windowShouldClose:`, which returns `YES` when no `on_window_should_close`
//! callback is registered, and the platform window's `on_close` calls
//! `Window::remove_window`. The red button was already wired; only the two key
//! equivalents were missing.

use gpui::{App, KeyBinding};

gpui::actions!(
    spool_app,
    [
        /// Quits the application.
        Quit,
        /// Closes the current window.
        ///
        /// This is not a quit, and must not become one. On macOS `QuitMode::Default`
        /// resolves to `QuitMode::Explicit`, so removing a window does not terminate
        /// the process; that is the platform's own choice and this action stays
        /// inside it.
        CloseWindow,
    ]
);

/// The lifecycle key equivalents, kept separate from the editor's bindings so
/// this table can be asserted on its own.
pub fn bindings() -> [KeyBinding; 2] {
    [
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
    ]
}

/// Bind the lifecycle keys and handle the actions they dispatch.
///
/// Both handlers register globally, and that is not a shortcut around a better
/// option. An action dispatched to an element only reaches listeners along the
/// dispatch path, and GPUI builds that path from the focused node — falling back
/// to the dispatch tree's synthetic root when nothing is focused, a path that
/// contains no editor element at all. App-level keys would then go dead whenever
/// focus happened to be nowhere, which is exactly the state the app is in between
/// launching and the first click. Zed binds `Quit` globally for the same reason.
///
/// `CloseWindow` defers the window update. The action is dispatched from inside
/// the window update it wants to end, and GPUI takes that window out of
/// `App::windows` for the duration of an update, so updating it from in here
/// finds no window and is silently dropped. Deferring runs once that update has
/// finished and put it back.
pub fn install(cx: &mut App) {
    cx.bind_keys(bindings());
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &CloseWindow, cx| {
        let Some(window) = cx.active_window() else {
            return;
        };
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keystroke(binding: &KeyBinding) -> String {
        binding.keystrokes()[0].unparse()
    }

    #[test]
    fn cmd_q_dispatches_quit() {
        let binding = bindings()
            .into_iter()
            .find(|b| b.action().name() == "spool_app::Quit");
        let binding = binding.expect("cmd-q is bound to Quit");
        assert_eq!(keystroke(&binding), "cmd-q");
    }

    #[test]
    fn cmd_w_dispatches_close_window() {
        let binding = bindings()
            .into_iter()
            .find(|b| b.action().name() == "spool_app::CloseWindow");
        let binding = binding.expect("cmd-w is bound to CloseWindow");
        assert_eq!(keystroke(&binding), "cmd-w");
    }

    /// Neither key is conditional. A `when` context would let the editor's own
    /// listeners claim the keystroke first, which is how `⌘W` would quietly turn
    /// into a canvas action.
    #[test]
    fn lifecycle_keys_are_always_active() {
        for binding in bindings() {
            assert!(
                binding.predicate().is_none(),
                "{} must not be gated on a key context",
                binding.action().name()
            );
        }
    }

    /// The lifecycle table must not grow keys the editor already owns, or a
    /// collision would silently shadow `cmd-a`/`cmd-c`/`cmd-v`/`cmd-x`.
    #[test]
    fn lifecycle_keys_do_not_collide_with_editor_keys() {
        for binding in bindings() {
            assert!(
                !matches!(
                    keystroke(&binding).as_str(),
                    "cmd-a" | "cmd-c" | "cmd-v" | "cmd-x"
                ),
                "{} collides with an editor shortcut",
                keystroke(&binding)
            );
        }
    }
}
