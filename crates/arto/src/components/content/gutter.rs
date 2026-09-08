use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;

/// The contents, as a column of ticks beside the document.
///
/// A panel of headings was a second thing to open, close and mis-open; a
/// ruler down the edge of the text is always there and costs 24px. Depth is
/// the tick's length, so the shape of the document is legible without a word
/// being read, and hovering a tick names its heading.
///
/// It sits inside the document's own area rather than at the window's edge,
/// which is why it cannot be opened by accident: there is nothing to open.
///
/// Pinned searches are meant to colour the ticks holding their hits, which
/// needs the renderer to report which heading each match falls under; until it
/// does, the pinned chips keep carrying those colours.
#[component]
pub fn ContentsGutter(headings: Vec<HeadingInfo>) -> Element {
    if headings.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "contents-gutter",
            "aria-label": "Contents",

            for heading in headings {
                {
                    let id = heading.id.clone();
                    rsx! {
                        button {
                            class: "contents-gutter-tick",
                            "data-level": "{heading.level}",
                            title: "{heading.text}",
                            "aria-label": "{heading.text}",
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
    }
}
