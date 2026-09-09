//! Find, in the header's own row.
//!
//! What is typed is a temporary mark on the page: it highlights while the
//! field is open and is gone when it closes. Return keeps it — the mark
//! becomes one of the document's pinned searches, which live in the contents
//! beside the page ([`crate::components::content::gutter`]) because that is
//! where their colour already shows.
//!
//! So the field is one row, and one row fits where the breadcrumb sits. The
//! window gains nothing while a search is on: the document is neither covered
//! nor moved.

use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::state::AppState;

/// Search for what the field holds.
const JS_FIND: &str = r#"
    const input = document.querySelector('.search-input');
    if (input) window.Arto.search.find(input.value);
"#;

/// Empty the field and take its highlights off the page.
const JS_CLEAR: &str = r#"
    const input = document.querySelector('.search-input');
    if (input) {
        input.value = '';
        input.focus();
    }
    window.Arto.search.clear();
"#;

const JS_FOCUS: &str = "document.querySelector('.search-input')?.focus()";

/// Blur the field and hand the keyboard back to the document.
///
/// Without it the keybindings stop answering: keydown inside an input does not
/// reach the interceptor.
const JS_BLUR_TO_BODY: &str =
    "document.querySelector('.search-input')?.blur(); document.body.focus();";

/// The field, drawn only while the search is on.
#[component]
pub fn HeaderFind() -> Element {
    let mut state = use_context::<AppState>();
    let match_count = *state.search_match_count.read();
    let current_index = *state.search_current_index.read();

    // Opening is what mounts this, so this is where the focus goes — and
    // `search_focus_request` is bumped on every request, so pressing the
    // shortcut again with the field already open refocuses it.
    use_effect(move || {
        let _ = state.search_focus_request.read();
        let initial_text = state.search_initial_text.read().clone();
        let js = match initial_text.as_deref().filter(|text| !text.is_empty()) {
            Some(text) => format!(
                r#"
                const input = document.querySelector('.search-input');
                if (input) {{
                    input.value = {};
                    input.focus();
                    input.select();
                    window.Arto.search.find(input.value);
                }}
                "#,
                serde_json::to_string(text).unwrap_or_default()
            ),
            None => JS_FOCUS.to_string(),
        };
        if initial_text.is_some() {
            state.search_initial_text.set(None);
        }
        spawn(async move {
            let _ = document::eval(&js).await;
        });
    });

    // Closing takes the highlights with it: what was typed was a temporary
    // mark, and a mark nobody kept has no business outliving its field.
    use_drop(move || {
        spawn(async move {
            let _ = document::eval(JS_CLEAR).await;
            let _ = document::eval(JS_BLUR_TO_BODY).await;
        });
    });

    rsx! {
        div {
            class: "header-find",

            div {
                class: "header-find-field",

                Icon { name: IconName::Search }

                // Uncontrolled, so an input method's composition is left alone.
                input {
                    r#type: "text",
                    class: "search-input",
                    placeholder: "Find in page",
                    autocorrect: "off",
                    autocapitalize: "off",
                    spellcheck: "false",
                    oninput: move |_| {
                        spawn(async move {
                            let _ = document::eval(JS_FIND).await;
                        });
                    },
                    // Tab would otherwise take the focus out of the field
                    // before the binding it is bound to could run. Nothing
                    // else here is the browser's to act on.
                    onkeydown: move |evt| {
                        if evt.key() == Key::Tab {
                            evt.prevent_default();
                        }
                    },
                }

                // What is true of what was typed, inside the field that holds
                // it: how many there are, and what Return would do with them.
                if match_count > 0 {
                    span { class: "header-find-count", "{current_index}/{match_count}" }
                    kbd { class: "header-find-hint", "⏎" }
                }
            }

            // Walking the matches is Tab's, so there is nothing left for a
            // pair of chevrons to do that the hand on the keyboard is not
            // already doing.
            button {
                class: "nav-button",
                title: "Close",
                onclick: move |_| state.search_open.set(false),
                Icon { name: IconName::Close }
            }
        }
    }
}
