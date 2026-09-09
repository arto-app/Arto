use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::components::pinned_marks::PinnedMarks;
use crate::document_link::scroll_to_heading_js;
use crate::markdown::HeadingInfo;
use crate::pinned_search::{PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};
use crate::state::AppState;

/// The contents, as a column of ticks beside the document — and the names
/// those ticks stand for, for as long as the pointer is in the column.
///
/// A panel of headings was a second thing to open, close and mis-open; a ruler
/// down the edge of the text is always there, and what it costs the page is a
/// column of margin (`layout_budget::GUTTER_WIDTH`, plus the distance it
/// stands off the text). Depth is the tick's length, so the shape of the
/// document is legible without a word being read, and the current tick is the
/// thick one.
///
/// Resting in the column brings the names out over the document. It is the
/// rail's rule applied to the other edge: the strip is visible, it exists to
/// be aimed at, and what it opens floats rather than pushing the text aside,
/// so opening it by accident costs nothing. The ticks stay where they are
/// while it is out — they are what a pinned search will colour, and a list
/// that replaced them would take those marks away with it.
///
/// It stands in the page's own right margin, opposite the margin trace and
/// the same distance from the text, rather than at the window's edge — a map
/// of the document belongs beside the document. That is also why it cannot be
/// opened by accident: it is nowhere the pointer crosses on its way anywhere,
/// and there is nothing to open.
///
/// The marks a search pinned are listed here too, above the headings. Their
/// colour is already on these ticks; listing them anywhere else would put the
/// mark and the way to change it in two places, and the way to change it in a
/// bar that closes.
///
/// Asked for by name (`contents.toggle`), the same list is held open instead
/// of following the pointer, and the keyboard walks it. It is one list either
/// way: a second panel drawn somewhere else would be a second thing to learn
/// for the same headings — and at a width that has folded the ruler away, the
/// list asked for by name is the only way to them, so it is drawn without the
/// ruler beside it.
#[component]
pub fn ContentsGutter(
    headings: Vec<HeadingInfo>,
    /// Whether the ruler is drawn beside the document. A narrow window folds
    /// it away; the list can still be asked for.
    ruler: bool,
) -> Element {
    let state = use_context::<AppState>();
    let mut pinned = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    use_future(move || async move {
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            pinned.set(PINNED_SEARCHES.read().pinned_searches.clone());
        }
    });

    let marks = pinned.read().clone();
    let open = *state.contents_open.read();
    // Nothing to rest on, and nothing to open: with neither a heading nor a
    // mark there is no map to draw. Asked for by name it still answers, so
    // that the key does something rather than nothing.
    if headings.is_empty() && marks.is_empty() && !open {
        return rsx! {};
    }

    rsx! {
        if ruler {
            Ruler { headings: headings.clone() }
        }

        // A sibling of the ruler, and after it: resting in the column is what
        // brings this out, and that is said in CSS as
        // `.contents-gutter:hover ~ .contents-toc`.
        Names { headings, marks, open }
    }
}

/// The document's shape, as ticks down its edge.
#[component]
fn Ruler(headings: Vec<HeadingInfo>) -> Element {
    rsx! {
        div {
            class: "contents-gutter",
            "aria-hidden": "true",

            // The ticks, in a box of their own. The column is as tall as the
            // page so that they sit level with its middle, and this is what
            // the pointer can rest in: crossing the empty part of a column is
            // crossing a margin, not reaching for the contents.
            div {
                class: "contents-gutter-run",

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
}

/// What the ticks stand for: the marks a search pinned, then the headings.
#[component]
fn Names(headings: Vec<HeadingInfo>, marks: Vec<PinnedSearch>, open: bool) -> Element {
    let mut state = use_context::<AppState>();
    let cursor = *state.contents_cursor.read();
    let nothing_pinned = marks.is_empty();

    // Picking a place is the end of a list held open. One that came out under
    // the pointer goes when the pointer does, so it has nothing to close.
    let mut go_to = move |id: String| {
        if open {
            state.close_contents();
        }
        spawn(async move {
            let _ = document::eval(&scroll_to_heading_js(&id)).await;
        });
    };

    rsx! {
        nav {
            class: "contents-toc",
            class: if open { "open" },
            "aria-label": "Contents",

            PinnedMarks { pinned_searches: marks }

            if !headings.is_empty() {
                div { class: "contents-toc-label", "Contents" }
            }

            if headings.is_empty() && nothing_pinned {
                div { class: "contents-toc-empty", "No headings" }
            }

            for (at, heading) in headings.iter().cloned().enumerate() {
                {
                    let id = heading.id.clone();
                    rsx! {
                        button {
                            key: "{heading.id}",
                            class: "contents-toc-row",
                            // Where the keys are, as opposed to where the
                            // reader is: the row marked `data-current` is the
                            // one being read, and the two are read together.
                            class: if cursor == Some(at) { "keyboard-focused" },
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
