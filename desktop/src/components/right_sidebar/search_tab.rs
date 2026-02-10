mod directory_results;

use dioxus::prelude::*;

use crate::pinned_search::{PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};
use crate::state::AppState;

use directory_results::DirectoryResultsSection;

#[component]
pub fn SearchTab() -> Element {
    let state = use_context::<AppState>();

    // Read committed search results from AppState.
    // These are only updated when the user confirms a search (Enter in Finder).
    let dir_query = state.directory_search_query.read().clone();
    let dir_results = state.directory_search_results.read().clone();

    // Local signal for pinned searches (updated via broadcast)
    let mut pinned_searches = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    // Subscribe to pinned search changes (JS sync is handled by FuzzyFinder)
    use_future(move || async move {
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            let searches = PINNED_SEARCHES.read().pinned_searches.clone();
            pinned_searches.set(searches);
        }
    });

    let has_dir_search = dir_query.as_ref().is_some_and(|q| !q.is_empty());
    let has_pinned = !pinned_searches.read().is_empty();
    let has_any_content = has_dir_search || has_pinned;

    rsx! {
        div {
            class: "right-sidebar-search",

            // Search results (shown for both File and Directory modes)
            if let Some(q) = dir_query {
                if !q.is_empty() {
                    DirectoryResultsSection {
                        query: q,
                        results: dir_results,
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
