use dioxus::document;
use dioxus::prelude::*;

use crate::state::AppState;

use super::shortcut_overlay::{
    close_shortcut_overlay, handle_shortcut_overlay_close_key, handle_shortcut_overlay_scroll_key,
    is_shortcut_overlay_visible, toggle_shortcut_overlay, ShortcutOverlayVisibility,
};

/// Key event data received from JS keyboard interceptor via dioxus.send().
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KeyEventData {
    pub(super) key: String,
    pub(super) modifiers: u32,
    pub(super) repeat: bool,
    #[serde(default)]
    pub(super) search_focused: bool,
}

fn should_skip_keybinding(data: &KeyEventData) -> bool {
    // In search input, plain Escape is handled by SearchBar (blur input).
    // Skip keybinding processing so it does not trigger search.clear.
    data.search_focused && data.modifiers == 0 && data.key == "Escape"
}

/// Set up the keybinding engine with JS keyboard interceptor bridge.
///
/// Creates the engine from current config, then establishes a JS → Rust bridge:
/// JS keyboard interceptor captures keydown events → sends via dioxus.send() →
/// Rust recv loop processes through engine → dispatches matched actions.
///
/// The engine is wrapped in `Signal<RefCell<>>` so that a separate config-change
/// listener can rebuild it without interrupting the keyboard event loop.
pub(super) fn setup_keybinding_engine(
    mut state: AppState,
    shortcut_overlay_visibility: Signal<ShortcutOverlayVisibility>,
) {
    use crate::config::{CONFIG, CONFIG_CHANGED_BROADCAST};
    use crate::keybindings::dispatcher::dispatch_action;
    use crate::keybindings::{Action, KeyContext, KeyMatchResult, KeybindingEngine};
    use crate::shortcut::KeyChord;
    use std::cell::RefCell;

    // use_signal must be called at component render level (not inside use_hook)
    let initial_config = CONFIG.read().keybindings.clone();
    let engine = use_signal(|| RefCell::new(KeybindingEngine::new(&initial_config)));

    // spawn/spawn_forever wrapped in use_hook to run only once
    use_hook(move || {
        // Keyboard event processing loop
        spawn(async move {
            // Wait for JS keyboard API to be ready, then register callback.
            // Retries up to 200 times (10s) before giving up.
            // Windows WebView2 may take longer to parse large JS bundles.
            let mut eval = document::eval(
                r#"
            (async () => {
                let retries = 0;
                while (!window.Arto?.keyboard?.onKeydown && retries++ < 200) {
                    await new Promise(r => setTimeout(r, 50));
                }
                if (!window.Arto?.keyboard?.onKeydown) {
                    console.error("Keyboard interceptor API not available after timeout");
                    return;
                }
                window.Arto.keyboard.onKeydown((data) => {
                    dioxus.send(data);
                });
            })();
            "#,
            );

            // If JS initialization fails (timeout), recv returns Err immediately and
            // the loop never starts. Log a warning so the issue is diagnosable.
            let mut received_any = false;
            while let Ok(data) = eval.recv::<KeyEventData>().await {
                if !received_any {
                    received_any = true;
                }
                let chord = KeyChord::from_js_event(&data.key, data.modifiers);
                if chord.is_modifier_only() {
                    continue;
                }
                if should_skip_keybinding(&data) {
                    continue;
                }
                let overlay_visible = is_shortcut_overlay_visible(shortcut_overlay_visibility);
                if overlay_visible
                    && handle_shortcut_overlay_close_key(&data, shortcut_overlay_visibility)
                {
                    engine.read().borrow_mut().reset();
                    continue;
                }
                if overlay_visible && handle_shortcut_overlay_scroll_key(&data) {
                    continue;
                }

                let context = if data.search_focused {
                    KeyContext::Search
                } else {
                    state.focused_panel.read().key_context()
                };
                let result = engine
                    .read()
                    .borrow_mut()
                    .process_key(&chord, data.repeat, context);

                match result {
                    KeyMatchResult::Matched(action) => {
                        if overlay_visible {
                            engine.read().borrow_mut().reset();
                            if action == Action::Cancel {
                                close_shortcut_overlay(shortcut_overlay_visibility);
                            } else if action == Action::HelpShowKeyboardShortcuts {
                                toggle_shortcut_overlay(shortcut_overlay_visibility);
                            }
                            continue;
                        }
                        if action == Action::Cancel {
                            // Cancel chain: reset engine state + return focus to content + close overlays + close search + clear content cursor
                            engine.read().borrow_mut().reset();
                            state.focused_panel.set(crate::state::FocusedPanel::Content);
                            state.left_hover_active.set(false);
                            state.right_hover_active.set(false);
                            if *state.search_open.read() {
                                state.toggle_search();
                            }
                            crate::keybindings::dispatcher::content_cursor_eval("clearCursor");
                            close_shortcut_overlay(shortcut_overlay_visibility);
                        } else if action == Action::HelpShowKeyboardShortcuts {
                            engine.read().borrow_mut().reset();
                            toggle_shortcut_overlay(shortcut_overlay_visibility);
                        } else {
                            dispatch_action(&action, state);
                        }
                    }
                    KeyMatchResult::Pending | KeyMatchResult::NoMatch => {
                        if overlay_visible {
                            engine.read().borrow_mut().reset();
                        }
                    }
                }
            }
            if !received_any {
                tracing::warn!("Keybinding engine: JS keyboard interceptor failed to initialize");
            }
        });

        // Config change listener: rebuild engine when keybindings are saved
        dioxus_core::spawn_forever(async move {
            let mut rx = CONFIG_CHANGED_BROADCAST.subscribe();
            while rx.recv().await.is_ok() {
                let new_config = CONFIG.read().keybindings.clone();
                *engine.read().borrow_mut() = KeybindingEngine::new(&new_config);
                tracing::debug!("Keybinding engine rebuilt after config change");
            }
        });
    }); // use_hook
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_plain_escape_when_search_input_is_focused() {
        let data = KeyEventData {
            key: "Escape".to_string(),
            modifiers: 0,
            repeat: false,
            search_focused: true,
        };

        assert!(should_skip_keybinding(&data));
    }

    #[test]
    fn does_not_skip_non_escape_in_search_input() {
        let data = KeyEventData {
            key: "Enter".to_string(),
            modifiers: 0,
            repeat: false,
            search_focused: true,
        };

        assert!(!should_skip_keybinding(&data));
    }

    #[test]
    fn does_not_skip_escape_when_search_input_is_not_focused() {
        let data = KeyEventData {
            key: "Escape".to_string(),
            modifiers: 0,
            repeat: false,
            search_focused: false,
        };

        assert!(!should_skip_keybinding(&data));
    }

    #[test]
    fn does_not_skip_modified_escape_in_search_input() {
        let data = KeyEventData {
            key: "Escape".to_string(),
            modifiers: 8,
            repeat: false,
            search_focused: true,
        };

        assert!(!should_skip_keybinding(&data));
    }
}
