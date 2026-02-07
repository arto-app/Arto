use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::components::pinned_chips::PinnedChipsRow;
use crate::pinned_search::{
    add_pinned_search, PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED,
};
use crate::state::AppState;

/// JavaScript to trigger search from input value
const JS_FIND: &str = r#"
    const input = document.querySelector('.search-input');
    if (input) window.Arto.search.find(input.value);
"#;

/// JavaScript to clear search input and highlights
const JS_CLEAR: &str = r#"
    const input = document.querySelector('.search-input');
    if (input) {
        input.value = '';
        input.focus();
    }
    window.Arto.search.clear();
"#;

/// Navigate to next or previous match
fn navigate(direction: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.search.navigate('{direction}')");
        let _ = document::eval(&js).await;
    });
}

/// Build JSON for pinned searches to sync to JavaScript.
/// Includes all searches (enabled and disabled) so disabled ones still show matches in sidebar.
fn build_pinned_json(searches: &[PinnedSearch]) -> String {
    let json_entries: Vec<String> = searches
        .iter()
        .map(|p| {
            format!(
                r#"{{"id":"{}","pattern":"{}","color":"{}","caseSensitive":{},"disabled":{}}}"#,
                p.id,
                p.pattern.replace('\\', "\\\\").replace('"', "\\\""),
                p.color.to_js_name(),
                p.case_sensitive,
                p.disabled
            )
        })
        .collect();

    format!("[{}]", json_entries.join(","))
}

/// Wait for JavaScript search API to be ready.
async fn wait_for_js_ready() {
    use std::time::Duration;

    // Poll until window.Arto.search.setPinned is available
    for _ in 0..50 {
        // 50 * 50ms = 2.5s max wait
        let mut eval =
            document::eval("dioxus.send(typeof window.Arto?.search?.setPinned === 'function')");
        if let Ok(true) = eval.recv::<bool>().await {
            return;
        }
        // Small delay before retrying
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tracing::warn!("Timeout waiting for JavaScript search API to be ready");
}

/// Sync pinned searches to JavaScript for highlighting (async version).
async fn sync_pinned_to_js_async(searches: &[PinnedSearch]) {
    let json = build_pinned_json(searches);
    let js = format!("window.Arto.search.setPinned({});", json);
    let _ = document::eval(&js).await;
}

#[component]
pub fn SearchBar() -> Element {
    let mut state = use_context::<AppState>();
    let is_open = *state.search_open.read();
    let match_count = *state.search_match_count.read();
    let current_index = *state.search_current_index.read();
    let initial_text = state.search_initial_text.read().clone();
    let mut has_input = use_signal(|| false);

    // Local signal for pinned searches (updated via broadcast)
    let mut pinned_searches = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    // Subscribe to pinned search changes and sync to JavaScript
    use_future(move || async move {
        // Wait for JavaScript search API to be ready before initial sync
        wait_for_js_ready().await;

        // Initial sync to JavaScript on component mount
        let searches = pinned_searches.read().clone();
        sync_pinned_to_js_async(&searches).await;

        // Listen for changes
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            let searches = PINNED_SEARCHES.read().pinned_searches.clone();
            pinned_searches.set(searches.clone());

            // Update JavaScript with new pinned searches
            sync_pinned_to_js_async(&searches).await;
        }
    });

    // Handle initial text when search bar opens
    use_effect(use_reactive!(|is_open, initial_text| {
        if is_open {
            if let Some(ref text) = initial_text {
                if !text.is_empty() {
                    has_input.set(true);
                    // Use JSON encoding to safely escape the string for JavaScript
                    let json_encoded = serde_json::to_string(text).unwrap_or_default();
                    let js = format!(
                        r#"
                        const input = document.querySelector('.search-input');
                        if (input) {{
                            input.value = {};
                            input.focus();
                            input.select();
                            window.Arto.search.find(input.value);
                        }}
                        "#,
                        json_encoded
                    );
                    spawn(async move {
                        let _ = document::eval(&js).await;
                    });
                }
                // Clear the initial text after using it
                state.search_initial_text.set(None);
            } else {
                // No initial text — just focus the input
                spawn(async move {
                    let _ = document::eval(
                        "const input = document.querySelector('.search-input'); if (input) input.focus();",
                    ).await;
                });
            }
        }
    }));

    rsx! {
        div {
            class: if is_open { "search-bar search-bar--open" } else { "search-bar" },

            // Main search row
            div {
                class: "search-bar-main",

                Icon { name: IconName::Search, size: 16 }

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
                            has_input.set(!evt.value().is_empty());
                            spawn(async move {
                                let _ = document::eval(JS_FIND).await;
                            });
                        },
                        onkeydown: move |evt| {
                            match evt.key() {
                                Key::Enter => {
                                    let direction = if evt.modifiers().shift() { "prev" } else { "next" };
                                    navigate(direction);
                                    evt.stop_propagation();
                                }
                                Key::Escape => {
                                    state.toggle_search();
                                    // Prevent the keyboard interceptor from also handling Escape
                                    evt.stop_propagation();
                                }
                                _ => {}
                            }
                        },
                    }

                    // Clear button (only shown when there's input)
                    if has_input() {
                        button {
                            class: "search-clear-button",
                            title: "Clear",
                            onclick: move |_| {
                                has_input.set(false);
                                state.update_search_results(0, 0);
                                spawn(async move {
                                    let _ = document::eval(JS_CLEAR).await;
                                });
                            },
                            Icon { name: IconName::Close, size: 14 }
                        }
                    }
                }

                // Pin button - adds current search to pinned searches
                button {
                    class: "search-pin-button",
                    disabled: !has_input(),
                    title: "Pin this search",
                    onclick: move |_| {
                        // Get the current search value from the input, then clear
                        let mut has_input = has_input;
                        let mut state = state;
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
                                    add_pinned_search(result.value);
                                    // Clear search input after pinning
                                    has_input.set(false);
                                    state.update_search_results(0, 0);
                                    let _ = document::eval(JS_CLEAR).await;
                                }
                            }
                        });
                    },
                    Icon { name: IconName::Pin, size: 16 }
                }

                button {
                    class: "search-nav-button",
                    disabled: match_count == 0,
                    title: "Previous match (Shift+Enter)",
                    onclick: move |_| navigate("prev"),
                    Icon { name: IconName::ChevronUp, size: 16 }
                }

                button {
                    class: "search-nav-button",
                    disabled: match_count == 0,
                    title: "Next match (Enter)",
                    onclick: move |_| navigate("next"),
                    Icon { name: IconName::ChevronDown, size: 16 }
                }

                span {
                    class: "search-match-count",
                    "{current_index}/{match_count}"
                }

                button {
                    class: "search-close-button",
                    title: "Close (Escape)",
                    onclick: move |_| state.toggle_search(),
                    Icon { name: IconName::Close, size: 16 }
                }
            }

            // Pinned chips row (only visible when pinned searches exist)
            PinnedChipsRow {
                pinned_searches: pinned_searches.read().clone(),
            }
        }
    }
}
