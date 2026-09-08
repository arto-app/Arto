use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;
use crate::state::AppState;

/// The contents as a list, for when the gutter is not there to be pointed at.
///
/// The gutter beside the document is the everyday way in, but it is one of the
/// things a narrow window folds away, and below that width there would
/// otherwise be no way to reach the headings at all. This is that way: asked
/// for by name, over the document, gone again on the next keystroke.
///
/// It is deliberately the same list the gutter draws — depth is the indent
/// here rather than the tick's length — so nothing has to be learned twice.
#[component]
pub fn ContentsOverlay(headings: Vec<HeadingInfo>) -> Element {
    let mut state = use_context::<AppState>();

    let mut close = move || state.contents_open.set(false);

    rsx! {
        div {
            class: "contents-overlay-backdrop",
            onclick: move |_| close(),

            div {
                class: "contents-overlay",
                onclick: move |evt| evt.stop_propagation(),

                if headings.is_empty() {
                    div { class: "contents-overlay-empty", "No headings" }
                }

                for heading in headings {
                    {
                        let id = heading.id.clone();
                        rsx! {
                            button {
                                key: "{heading.id}",
                                class: "contents-overlay-row",
                                "data-level": "{heading.level}",
                                onclick: move |_| {
                                    let id = id.clone();
                                    close();
                                    spawn(async move {
                                        let _ = document::eval(&scroll_to_heading_js(&id)).await;
                                    });
                                },
                                "{heading.text}"
                            }
                        }
                    }
                }
            }
        }
    }
}
