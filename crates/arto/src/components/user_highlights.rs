//! The reader's highlights, listed in the contents below the pinned marks.
//!
//! A highlight's colour is already on the tick of the heading it is under,
//! so the list of them heads the contents like the pinned marks do, and
//! everything done to one — going to it, its colour, taking it away — is
//! done there. A row shows the start of the highlight's note under its words,
//! and going to a highlight opens its card, where the note is written. A highlight whose words the page cannot find any more stays
//! listed at the end, faint, so the reader can see it was lost and let it go.

use dioxus::prelude::*;
use std::collections::HashMap;

use crate::components::icon::{Icon, IconName};
use crate::highlights::{Highlight, HighlightColor, HighlightId, HighlightPlace};
use crate::markdown::HeadingInfo;
use crate::state::AppState;

/// How much of a highlight's words its row quotes.
const QUOTE_CHARS: usize = 40;

/// The start of `exact`, on one line, cut to [`QUOTE_CHARS`] characters.
fn quote(exact: &str) -> String {
    let words = exact.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = words.chars();
    let head: String = chars.by_ref().take(QUOTE_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}\u{2026}")
    } else {
        head
    }
}

/// The highlights in the order they are listed: those on the page in the
/// order they are read, then the lost ones.
fn in_reading_order<'a>(
    highlights: &'a [Highlight],
    places: &HashMap<HighlightId, HighlightPlace>,
) -> Vec<(&'a Highlight, bool)> {
    let lost = |highlight: &Highlight| places.get(&highlight.id) == Some(&HighlightPlace::Lost);
    let mut listed: Vec<_> = highlights
        .iter()
        .map(|highlight| (highlight, lost(highlight)))
        .collect();
    listed.sort_by_key(|(highlight, lost)| (*lost, highlight.anchor.start));
    listed
}

/// The heading text a highlight is under, as the page reported it.
fn heading_of(
    highlight: &Highlight,
    places: &HashMap<HighlightId, HighlightPlace>,
    headings: &[HeadingInfo],
) -> Option<String> {
    match places.get(&highlight.id) {
        Some(HighlightPlace::Found {
            heading: Some(id), ..
        }) => headings
            .iter()
            .find(|heading| &heading.id == id)
            .map(|heading| heading.text.clone()),
        _ => None,
    }
}

/// What a row says of itself under the pointer: the whole note when there
/// is one, since the row shows only its start.
fn row_title(lost: bool, note: Option<&str>) -> String {
    match (lost, note) {
        (true, _) => "Not found in the document".to_string(),
        (false, Some(note)) => note.to_string(),
        (false, None) => "Go to the highlight".to_string(),
    }
}

/// The highlights, above the headings.
#[component]
pub fn UserHighlights() -> Element {
    let state = use_context::<AppState>();
    let highlights = state.highlights.read().clone();
    if highlights.is_empty() {
        return rsx! {};
    }
    let places = state.highlight_places.read().clone();
    let headings = state.headings.read().clone();

    rsx! {
        div { class: "contents-toc-label", "Highlights" }

        for (highlight, lost) in in_reading_order(&highlights, &places) {
            UserHighlightRow {
                key: "{highlight.id}",
                id: highlight.id.clone(),
                color: highlight.color,
                quote: quote(&highlight.anchor.exact),
                note: highlight.note.clone(),
                heading: heading_of(highlight, &places, &headings),
                lost,
            }
        }
    }
}

/// One highlight: its colour, its words, where it is, and what can be done.
#[component]
fn UserHighlightRow(
    id: HighlightId,
    color: HighlightColor,
    quote: String,
    note: Option<String>,
    heading: Option<String>,
    lost: bool,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut show_popover = use_signal(|| false);

    let go = {
        let id = id.clone();
        move |_| {
            if lost {
                show_popover.toggle();
                return;
            }
            // Picking a place is the end of a list held open, as for a
            // heading: the card is what the reader went there for.
            if *state.contents_open.peek() {
                state.close_contents();
            }
            crate::highlights::card::reveal(state, id.clone());
        }
    };

    let remove = {
        let id = id.clone();
        move |_| {
            if let Some(file) = state.current_file() {
                crate::highlights::remove_highlights(&file, std::slice::from_ref(&id));
            }
            show_popover.set(false);
        }
    };

    rsx! {
        div {
            class: "contents-toc-pin-slot",

            div {
                class: "contents-toc-row contents-toc-pin contents-toc-highlight",
                class: if lost { "lost" },
                title: row_title(lost, note.as_deref()),
                onclick: go,

                button {
                    class: "contents-toc-pin-dot {color.css_class()}",
                    title: "Colour, remove",
                    onclick: move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        show_popover.toggle();
                    },
                }

                span { class: "contents-toc-name", "{quote}" }

                if let Some(heading) = heading {
                    span { class: "contents-toc-highlight-heading", "{heading}" }
                }

                if let Some(note) = note.as_deref() {
                    span { class: "contents-toc-highlight-note", "{note}" }
                }
            }

            if *show_popover.read() {
                div {
                    class: "color-palette-backdrop",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        show_popover.set(false);
                    },
                }

                div {
                    class: "color-palette-popover",

                    if lost {
                        span { class: "color-palette-note", "Not found in the document" }
                    } else {
                        for choice in HighlightColor::ALL {
                            button {
                                key: "{choice.to_js_name()}",
                                class: if choice == color {
                                    format!("color-palette-swatch selected {}", choice.css_class())
                                } else {
                                    format!("color-palette-swatch {}", choice.css_class())
                                },
                                onclick: {
                                    let id = id.clone();
                                    move |_| {
                                        if let Some(file) = state.current_file() {
                                            crate::highlights::recolor(&file, &id, choice);
                                        }
                                        show_popover.set(false);
                                    }
                                },
                            }
                        }
                    }

                    div { class: "color-palette-separator" }

                    button {
                        class: "color-palette-action color-palette-remove",
                        title: "Remove",
                        onclick: remove,
                        Icon { name: IconName::Trash, size: 18 }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlights::TextAnchor;

    fn highlight(exact: &str, start: u32) -> Highlight {
        Highlight::new(
            TextAnchor {
                exact: exact.to_string(),
                prefix: String::new(),
                suffix: String::new(),
                start,
                line: 1,
            },
            HighlightColor::Green,
        )
    }

    #[test]
    fn a_quote_is_one_line_cut_by_characters() {
        assert_eq!(quote("short\n  text"), "short text");
        let long = "あ".repeat(QUOTE_CHARS + 5);
        assert_eq!(
            quote(&long),
            format!("{}\u{2026}", "あ".repeat(QUOTE_CHARS))
        );
        assert_eq!(quote(&"x".repeat(QUOTE_CHARS)), "x".repeat(QUOTE_CHARS));
    }

    #[test]
    fn a_row_with_a_note_holds_all_of_it_under_the_pointer() {
        assert_eq!(row_title(false, Some("first\nsecond")), "first\nsecond");
        assert_eq!(row_title(false, None), "Go to the highlight");
        assert_eq!(row_title(true, Some("note")), "Not found in the document");
    }

    #[test]
    fn highlights_are_listed_as_read_with_the_lost_last() {
        let late = highlight("late", 50);
        let early = highlight("early", 5);
        let lost = highlight("lost", 1);
        let unknown = highlight("unknown", 20);
        let places = HashMap::from([
            (late.id.clone(), HighlightPlace::Found { heading: None }),
            (early.id.clone(), HighlightPlace::Found { heading: None }),
            (lost.id.clone(), HighlightPlace::Lost),
        ]);
        let highlights = [late, lost, early, unknown];

        let listed: Vec<_> = in_reading_order(&highlights, &places)
            .into_iter()
            .map(|(highlight, lost)| (highlight.anchor.exact.as_str(), lost))
            .collect();

        assert_eq!(
            listed,
            [
                ("early", false),
                ("unknown", false),
                ("late", false),
                ("lost", true)
            ]
        );
    }

    #[test]
    fn a_row_names_the_heading_the_page_found_its_highlight_under() {
        let found = highlight("x", 0);
        let headings = vec![HeadingInfo {
            level: 2,
            text: "Usage".to_string(),
            id: "usage".to_string(),
        }];
        let places = HashMap::from([(
            found.id.clone(),
            HighlightPlace::Found {
                heading: Some("usage".to_string()),
            },
        )]);

        assert_eq!(
            heading_of(&found, &places, &headings).as_deref(),
            Some("Usage")
        );
        assert_eq!(heading_of(&found, &HashMap::new(), &headings), None);
    }
}
