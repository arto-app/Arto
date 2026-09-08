use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;

/// The contents, as a column of ticks beside the document — and the names
/// those ticks stand for, for as long as the pointer is in the column.
///
/// A panel of headings was a second thing to open, close and mis-open; a ruler
/// down the edge of the text is always there and costs 24px. Depth is the
/// tick's length, so the shape of the document is legible without a word being
/// read, and the current tick is the dense one.
///
/// Resting in the column brings the names out over the document. It is the
/// rail's rule applied to the other edge: the strip is visible, it exists to
/// be aimed at, and what it opens floats rather than pushing the text aside,
/// so opening it by accident costs nothing. The ticks stay where they are
/// while it is out — they are what a pinned search will colour, and a list
/// that replaced them would take those marks away with it.
///
/// It sits inside the document's own area rather than at the window's edge,
/// which is why it cannot be opened by accident: there is nothing to open.
#[component]
pub fn ContentsGutter(headings: Vec<HeadingInfo>) -> Element {
    if headings.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "contents-gutter",
            "aria-hidden": "true",

            for heading in headings.iter().cloned() {
                {
                    let id = heading.id.clone();
                    rsx! {
                        button {
                            key: "{heading.id}",
                            class: "contents-gutter-tick",
                            "data-level": "{heading.level}",
                            // Which heading this tick stands for, so the page
                            // can mark the one the reader is inside.
                            "data-heading": "{heading.id}",
                            // The ruler repeats what the list beside it says
                            // in words, so it is the list that answers to the
                            // keyboard and to a screen reader.
                            tabindex: "-1",
                            onclick: move |_| {
                                let id = id.clone();
                                spawn(async move {
                                    let _ = document::eval(&scroll_to_heading_js(&id)).await;
                                });
                            },
                        }
                    }
                }
            }
        }

        nav {
            class: "contents-toc",
            "aria-label": "Contents",

            for heading in headings.iter().cloned() {
                {
                    let id = heading.id.clone();
                    rsx! {
                        button {
                            key: "{heading.id}",
                            class: "contents-toc-row",
                            "data-level": "{heading.level}",
                            "data-heading": "{heading.id}",
                            onclick: move |_| {
                                let id = id.clone();
                                spawn(async move {
                                    let _ = document::eval(&scroll_to_heading_js(&id)).await;
                                });
                            },
                            span { class: "contents-toc-name", "{heading.text}" }
                        }
                    }
                }
            }
        }
    }
}
