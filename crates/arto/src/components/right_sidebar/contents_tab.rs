use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;

#[component]
pub fn ContentsTab(headings: Vec<HeadingInfo>, cursor_index: Option<usize>) -> Element {
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
                    for (index, heading) in headings.iter().enumerate() {
                        HeadingItem {
                            heading: heading.clone(),
                            is_keyboard_focused: cursor_index == Some(index),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn HeadingItem(heading: HeadingInfo, is_keyboard_focused: bool) -> Element {
    let id = heading.id.clone();
    let level = heading.level;

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
                        let _ = document::eval(&scroll_to_heading_js(&id)).await;
                    });
                },
                "{heading.text}"
            }
        }
    }
}
