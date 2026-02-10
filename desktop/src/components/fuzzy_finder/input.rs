use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::finder::FinderMode;

/// Search input row for the Fuzzy Finder.
///
/// Handles text input, clear, and pin.
/// Uses uncontrolled input to preserve IME (SKK, Japanese input) state.
/// The leading icon reflects the current search mode (file or directory).
#[component]
pub fn FuzzyFinderInput(
    mode: FinderMode,
    has_input: Signal<bool>,
    result_count: usize,
    selected_index: usize,
    on_input: EventHandler<String>,
    on_clear: EventHandler<()>,
    on_pin: EventHandler<String>,
) -> Element {
    let search_icon = match mode {
        FinderMode::File => IconName::FileSearch,
        FinderMode::Directory => IconName::FolderSearch,
    };

    rsx! {
        div {
            class: "fuzzy-finder-input",

            Icon { name: search_icon, size: 18 }

            // Input wrapper for positioning clear button
            div {
                class: "search-input-wrapper",

                // Uncontrolled input to preserve IME (SKK, Japanese input) state
                input {
                    r#type: "text",
                    class: "search-input",
                    placeholder: "Search...",
                    autofocus: true,
                    autocorrect: "off",
                    autocapitalize: "off",
                    spellcheck: "false",
                    oninput: move |evt| {
                        on_input.call(evt.value());
                    },
                    // Note: preventDefault for navigation keys (ArrowUp/Down, Ctrl+N/P,
                    // Cmd+M, Escape, Enter) is handled by a JavaScript keydown listener
                    // installed via JS_FOCUS. This is necessary because Dioxus desktop's
                    // prevent_default() goes through IPC and arrives too late to prevent
                    // the browser's synchronous default behavior (e.g. macOS Emacs scrolling).
                }

                // Clear button (only shown when there's input)
                if has_input() {
                    button {
                        class: "search-clear-button",
                        title: "Clear",
                        onclick: move |_| on_clear.call(()),
                        Icon { name: IconName::Close, size: 14 }
                    }
                }
            }

            // Selection position in results list (e.g. "3/20")
            if has_input() && result_count > 0 {
                span {
                    class: "search-match-count",
                    "{selected_index}/{result_count}"
                }
            }

            // Pin button
            button {
                class: "search-pin-button",
                disabled: !has_input(),
                title: "Pin this search",
                onclick: move |_| {
                    // Get the current search value from the input
                    spawn(async move {
                        #[derive(serde::Deserialize)]
                        struct QueryValue {
                            value: String,
                        }
                        let mut eval = document::eval(r#"
                            const input = document.querySelector('.search-input');
                            dioxus.send({ value: input?.value || '' });
                        "#);
                        if let Ok(result) = eval.recv::<QueryValue>().await {
                            if !result.value.is_empty() {
                                on_pin.call(result.value);
                            }
                        }
                    });
                },
                Icon { name: IconName::Pin, size: 14 }
            }
        }
    }
}
