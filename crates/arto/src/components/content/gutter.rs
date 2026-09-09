use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::components::pinned_marks::PinnedMarks;
use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;
use crate::pinned_search::{PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};

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
///
/// The marks a search pinned are listed here too, above the headings. Their
/// colour is already on these ticks; listing them anywhere else would put the
/// mark and the way to change it in two places, and the way to change it in a
/// bar that closes.
///
#[component]
pub fn ContentsGutter(headings: Vec<HeadingInfo>) -> Element {
    let mut pinned = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    use_future(move || async move {
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            pinned.set(PINNED_SEARCHES.read().pinned_searches.clone());
        }
    });

    let marks = pinned.read().clone();
    // Nothing to rest on, and nothing to open: with neither a heading nor a
    // mark there is no map to draw.
    if headings.is_empty() && marks.is_empty() {
        return rsx! {};
    }

    rsx! {
        Ruler { headings: headings.clone() }

        // A sibling of the ruler, and after it: resting in the column is what
        // brings this out, and that is said in CSS as
        // `.contents-gutter:hover ~ .contents-toc`.
        Names { headings, marks }
    }
}

/// The document's shape, as ticks down its edge.
#[component]
fn Ruler(headings: Vec<HeadingInfo>) -> Element {
    rsx! {
        div {
            class: "contents-gutter",
            "aria-hidden": "true",

            // With no headings there are no ticks, and a column with nothing
            // in it cannot be rested on. The marks put one thing in it.
            if headings.is_empty() {
                span {
                    class: "contents-gutter-marks",
                    Icon { name: IconName::Pin, size: 12 }
                }
            }

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
    }
}

/// What the ticks stand for: the marks a search pinned, then the headings.
#[component]
fn Names(headings: Vec<HeadingInfo>, marks: Vec<PinnedSearch>) -> Element {
    let nothing_pinned = marks.is_empty();

    let go_to = move |id: String| {
        spawn(async move {
            let _ = document::eval(&scroll_to_heading_js(&id)).await;
        });
    };

    rsx! {
        nav {
            class: "contents-toc",
            "aria-label": "Contents",

            PinnedMarks { pinned_searches: marks }

            if !headings.is_empty() {
                div { class: "contents-toc-label", "Contents" }
            }

            if headings.is_empty() && nothing_pinned {
                div { class: "contents-toc-empty", "No headings" }
            }

            for heading in headings.iter().cloned() {
                {
                    let id = heading.id.clone();
                    rsx! {
                        button {
                            key: "{heading.id}",
                            class: "contents-toc-row",
                            "data-level": "{heading.level}",
                            "data-heading": "{heading.id}",
                            onclick: move |_| go_to(id.clone()),
                            span { class: "contents-toc-name", "{heading.text}" }
                            // One dot per mark under this heading, filled in
                            // by `reading-position.ts`: which marks those are
                            // is a question about the rendered page, which
                            // only the page can answer.
                            span { class: "contents-toc-hits" }
                        }
                    }
                }
            }
        }
    }
}
