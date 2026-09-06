use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::components::pinned_chips::PinnedChip;
use crate::pinned_search::{HighlightColor, PinnedSearch};
use crate::state::SearchMatch;

use super::utils::split_context;

/// Pinned search results section.
#[component]
pub fn PinnedResultsSection(pinned: PinnedSearch, matches: Vec<SearchMatch>) -> Element {
    let mut expanded = use_signal(|| true);
    let chevron = if *expanded.read() {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    let color = pinned.color;
    let count = matches.len();
    let pinned_id = pinned.id.clone();

    rsx! {
        div {
            class: "right-sidebar-pinned-results",

            // Header: chevron + icon + chip + count (clickable to toggle)
            div {
                class: "right-sidebar-pinned-results-header",
                onclick: move |_| expanded.toggle(),

                Icon { name: chevron, size: 14 }
                Icon { name: IconName::Pin, size: 14 }
                PinnedChip { pinned: pinned.clone() }
                span { class: "right-sidebar-pinned-results-count", "{count} matches" }
            }

            // Match list (collapsible)
            if *expanded.read() {
                if matches.is_empty() {
                    div {
                        class: "right-sidebar-pinned-results-empty",
                        "No matches"
                    }
                } else {
                    ul {
                        class: "right-sidebar-pinned-results-list",
                        for m in matches.iter() {
                            PinnedMatchItem {
                                match_info: m.clone(),
                                pinned_id: pinned_id.to_string(),
                                color,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PinnedMatchItem(match_info: SearchMatch, pinned_id: String, color: HighlightColor) -> Element {
    let index = match_info.index;
    let context = match_info.context.clone();
    let start = match_info.context_start;
    let end = match_info.context_end;

    // Split context into before, matched, and after parts
    let (before, matched, after) = split_context(&context, start, end);

    let class = format!("right-sidebar-pinned-item {}", color.css_class());

    rsx! {
        li {
            class: "{class}",
            onclick: {
                let pinned_id = pinned_id.clone();
                move |_| scroll_to_pinned_match(&pinned_id, index)
            },

            span { class: "right-sidebar-search-context", "{before}" }
            span { class: "right-sidebar-pinned-highlight {color.css_class()}", "{matched}" }
            span { class: "right-sidebar-search-context", "{after}" }
        }
    }
}

/// Navigate to a specific pinned match by index.
fn scroll_to_pinned_match(pinned_id: &str, index: usize) {
    let pinned_id = pinned_id.to_string();
    spawn(async move {
        let js = format!(
            "window.Arto.search.scrollToPinnedMatch('{}', {});",
            pinned_id, index
        );
        let _ = document::eval(&js).await;
    });
}
