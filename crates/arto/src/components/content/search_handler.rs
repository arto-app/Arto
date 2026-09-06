use dioxus::document;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::pinned_search::PinnedSearchId;
use crate::state::AppState;

/// Data structure for a single search match from JavaScript
#[derive(Serialize, Deserialize)]
struct SearchMatchData {
    index: usize,
    text: String,
    context: String,
    #[serde(rename = "contextStart")]
    context_start: usize,
    #[serde(rename = "contextEnd")]
    context_end: usize,
}

/// Unified data structure for search results from JavaScript
#[derive(Serialize, Deserialize)]
struct SearchResultData {
    count: usize,
    current: usize,
    query: String,
    matches: Vec<SearchMatchData>,
    #[serde(rename = "pinnedMatches")]
    pinned_matches: HashMap<String, Vec<SearchMatchData>>,
}

/// Hook to setup search result handler.
///
/// This should be called at the App level (not FileViewer) because search
/// is a window-wide feature that works across all content types (files,
/// welcome page, preferences, etc.).
pub fn use_search_handler(mut state: AppState) {
    use_effect(move || {
        // Setup unified JS search handler that receives all data at once
        // Wait for window.Arto to be available (init() is async)
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            (async () => {
                // Wait for window.Arto to be initialized
                while (!window.Arto?.search?.setup) {
                    await new Promise(resolve => setTimeout(resolve, 10));
                }
                window.Arto.search.setup((data) => {
                    dioxus.send(data);
                });
            })();
        "#});

        spawn(async move {
            while let Ok(data) = eval_provider.recv::<SearchResultData>().await {
                // Convert search matches
                let matches = data
                    .matches
                    .into_iter()
                    .map(|m| crate::state::SearchMatch {
                        index: m.index,
                        text: m.text,
                        context: m.context,
                        context_start: m.context_start,
                        context_end: m.context_end,
                    })
                    .collect();

                let query = if data.query.is_empty() {
                    None
                } else {
                    Some(data.query)
                };

                state.update_search_results_full(query, data.count, data.current, matches);

                // Convert pinned matches
                let pinned_matches: HashMap<PinnedSearchId, Vec<crate::state::SearchMatch>> = data
                    .pinned_matches
                    .into_iter()
                    .map(|(id, matches)| {
                        let converted = matches
                            .into_iter()
                            .map(|m| crate::state::SearchMatch {
                                index: m.index,
                                text: m.text,
                                context: m.context,
                                context_start: m.context_start,
                                context_end: m.context_end,
                            })
                            .collect();
                        (PinnedSearchId::from(id), converted)
                    })
                    .collect();

                state.update_pinned_matches(pinned_matches);
            }
        });
    });
}
