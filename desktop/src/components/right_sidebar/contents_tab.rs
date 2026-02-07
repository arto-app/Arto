use dioxus::document;
use dioxus::prelude::*;

use crate::markdown::HeadingInfo;
use crate::state::AppState;

#[component]
pub fn ContentsTab(headings: Vec<HeadingInfo>) -> Element {
    rsx! {
        div {
            class: "right-sidebar-contents",

            if headings.is_empty() {
                div {
                    class: "right-sidebar-contents-empty",
                    "No headings found"
                }
            } else {
                ul {
                    class: "right-sidebar-contents-list",
                    for (idx, heading) in headings.iter().enumerate() {
                        HeadingItem { heading: heading.clone(), index: idx }
                    }
                }
            }
        }
    }
}

#[component]
fn HeadingItem(heading: HeadingInfo, index: usize) -> Element {
    let state = use_context::<AppState>();
    let id = heading.id.clone();
    let level = heading.level;

    let is_keyboard_focused = *state.toc_cursor.read() == Some(index);

    rsx! {
        li {
            class: "right-sidebar-contents-item",
            class: if is_keyboard_focused { "keyboard-focused" },
            "data-level": "{level}",

            button {
                class: "right-sidebar-contents-item-button",
                onclick: move |_| {
                    let id = id.clone();
                    spawn(async move {
                        let js = format!(
                            r#"
                            (() => {{
                                const el = document.getElementById('{}');
                                if (el) {{
                                    el.scrollIntoView({{ behavior: 'smooth', block: 'start' }});
                                }}
                            }})();
                            "#,
                            id
                        );
                        let _ = document::eval(&js).await;
                    });
                },
                "{heading.text}"
            }
        }
    }
}
