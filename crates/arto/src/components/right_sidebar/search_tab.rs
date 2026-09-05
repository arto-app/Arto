mod pinned_results;
mod utils;

use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::pinned_search::{PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};
use crate::state::{AppState, SearchMatch};

use pinned_results::PinnedResultsSection;
use utils::split_context;

#[component]
pub fn SearchTab() -> Element {
    let state = use_context::<AppState>();
    let query = state.search_query.read().clone();
    let matches = state.search_matches.read().clone();
    let current = *state.search_current_index.read();
    let pinned_matches = state.pinned_matches.read().clone();

    // Local signal for pinned searches (updated via broadcast)
    let mut pinned_searches = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    // Subscribe to pinned search changes (JS sync is handled by SearchBar)
    use_future(move || async move {
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            let searches = PINNED_SEARCHES.read().pinned_searches.clone();
            pinned_searches.set(searches);
        }
    });

    // Get all pinned searches for results display (including disabled)
    let all_pinned: Vec<PinnedSearch> = pinned_searches.read().clone();

    let has_active_search = query.as_ref().map(|q| !q.is_empty()).unwrap_or(false);
    let has_pinned = !pinned_searches.read().is_empty();
    let has_any_content = has_active_search || has_pinned;

    rsx! {
        div {
            class: "right-sidebar-search",

            // Active search results
            if let Some(q) = query {
                if !q.is_empty() {
                    SearchResultsSection {
                        query: q,
                        matches,
                        current_index: current,
                    }
                }
            }

            // Pinned search results (for all pinned searches, including disabled)
            for pinned in all_pinned.iter() {
                if let Some(matches) = pinned_matches.get(&pinned.id) {
                    if !matches.is_empty() {
                        PinnedResultsSection {
                            key: "{pinned.id}",
                            pinned: pinned.clone(),
                            matches: matches.clone(),
                        }
                    }
                }
            }

            // Empty state placeholder
            if !has_any_content {
                SearchTabPlaceholder {}
            }
        }
    }
}

#[component]
fn SearchTabPlaceholder() -> Element {
    rsx! {
        div {
            class: "right-sidebar-search-placeholder",
            "Type in the search bar or add pinned searches"
        }
    }
}

/// Active search results section.
#[component]
fn SearchResultsSection(query: String, matches: Vec<SearchMatch>, current_index: usize) -> Element {
    let mut expanded = use_signal(|| true);
    let chevron = if *expanded.read() {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    rsx! {
        div {
            class: "right-sidebar-search-results",

            // Header (clickable to toggle)
            div {
                class: "right-sidebar-search-header",
                onclick: move |_| expanded.toggle(),

                Icon { name: chevron, size: 14 }
                Icon { name: IconName::Search, size: 14 }
                span { class: "right-sidebar-search-query", "\"{query}\"" }
                span { class: "right-sidebar-search-count", " - {matches.len()} matches" }
            }

            // Match list (collapsible)
            if *expanded.read() {
                if matches.is_empty() {
                    div {
                        class: "right-sidebar-search-empty",
                        "No matches found"
                    }
                } else {
                    ul {
                        class: "right-sidebar-search-list",
                        for m in matches.iter() {
                            SearchMatchItem {
                                match_info: m.clone(),
                                is_current: m.index + 1 == current_index,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SearchMatchItem(match_info: SearchMatch, is_current: bool) -> Element {
    let index = match_info.index;
    let context = match_info.context.clone();
    let start = match_info.context_start;
    let end = match_info.context_end;

    // Split context into before, matched, and after parts
    let (before, matched, after) = split_context(&context, start, end);

    let class = if is_current {
        "right-sidebar-search-item current"
    } else {
        "right-sidebar-search-item"
    };

    rsx! {
        li {
            class: "{class}",
            onclick: move |_| scroll_to_match(index),

            span { class: "right-sidebar-search-context", "{before}" }
            span { class: "right-sidebar-search-highlight", "{matched}" }
            span { class: "right-sidebar-search-context", "{after}" }
        }
    }
}

/// Navigate to a specific match by index.
fn scroll_to_match(index: usize) {
    spawn(async move {
        let js = format!("window.Arto.search.navigateTo({});", index);
        let _ = document::eval(&js).await;
    });
}
