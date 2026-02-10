use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::finder::{build_highlight_segments, DirectorySearchMatch};
use crate::state::AppState;

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Directory search results section for the right sidebar.
///
/// Groups results by file path and displays them in collapsible sections.
/// Clicking a result opens the file and triggers in-file search.
#[component]
pub fn DirectoryResultsSection(query: String, results: Vec<DirectorySearchMatch>) -> Element {
    let mut expanded = use_signal(|| true);
    let chevron = if *expanded.read() {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    let total_count = results.len();

    // Group results by file path (BTreeMap for consistent ordering)
    let grouped = group_by_file(&results);

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
                span { class: "right-sidebar-search-count", " - {total_count} results" }
            }

            // File groups (collapsible)
            if *expanded.read() {
                if results.is_empty() {
                    div {
                        class: "right-sidebar-search-empty",
                        "No results found"
                    }
                } else {
                    for (file_path, file_results) in grouped {
                        DirectoryFileGroup {
                            key: "{file_path:?}",
                            file_path,
                            results: file_results,
                        }
                    }
                }
            }
        }
    }
}

/// A group of results from a single file.
#[component]
fn DirectoryFileGroup(file_path: PathBuf, results: Vec<DirectorySearchMatch>) -> Element {
    let state = use_context::<AppState>();
    let mut expanded = use_signal(|| true);
    let chevron = if *expanded.read() {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    // Extract file name and relative directory
    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let display_dir = {
        let sidebar = state.sidebar.read();
        if let Some(root) = &sidebar.root_directory {
            file_path
                .parent()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    };

    let count = results.len();

    rsx! {
        div {
            class: "right-sidebar-dir-file-group",

            // File header
            div {
                class: "right-sidebar-dir-file-header",
                onclick: move |_| expanded.toggle(),

                Icon { name: chevron, size: 12 }
                Icon { name: IconName::File, size: 12 }
                span { class: "right-sidebar-dir-file-name", "{file_name}" }
                if !display_dir.is_empty() {
                    span { class: "right-sidebar-dir-file-dir", "{display_dir}" }
                }
                span { class: "right-sidebar-dir-file-count", "{count}" }
            }

            // Line results
            if *expanded.read() {
                ul {
                    class: "right-sidebar-dir-file-list",
                    for (i, result) in results.iter().enumerate() {
                        DirectoryMatchItem {
                            key: "{result.line_number}",
                            result: result.clone(),
                            index_in_file: i,
                        }
                    }
                }
            }
        }
    }
}

/// A single line match within a file group.
#[component]
fn DirectoryMatchItem(result: DirectorySearchMatch, index_in_file: usize) -> Element {
    let mut state = use_context::<AppState>();
    let file_path = result.file_path.clone();
    let line_number = result.line_number;
    let line_text = result.line_text.clone();

    let highlighted = build_highlight_segments(&line_text, &result.match_indices);

    rsx! {
        li {
            class: "right-sidebar-dir-match-item",
            onclick: move |_| {
                let current_file = {
                    let tabs = state.tabs.read();
                    let active = *state.active_tab.read();
                    tabs.get(active)
                        .and_then(|t| t.file().map(|p| p.to_path_buf()))
                };
                let same_file = current_file.as_ref() == Some(&file_path);

                if same_file {
                    // Same file: navigate directly to the match
                    let js = format!(
                        "window.Arto.search.setNavigateOnApply({}); window.Arto.search.navigateTo({});",
                        index_in_file, index_in_file
                    );
                    spawn(async move {
                        let _ = document::eval(&js).await;
                    });
                } else {
                    // Different file: set pending navigate, then open file
                    let js = format!("window.Arto.search.setNavigateOnApply({});", index_in_file);
                    spawn(async move {
                        let _ = document::eval(&js).await;
                    });
                    state.open_file(file_path.clone(), Default::default());
                }
            },

            span { class: "right-sidebar-dir-match-line-num", "{line_number}" }
            span { class: "right-sidebar-dir-match-text",
                for (text, is_match) in highlighted {
                    if is_match {
                        span { class: "right-sidebar-dir-match-highlight", "{text}" }
                    } else {
                        span { "{text}" }
                    }
                }
            }
        }
    }
}

/// Group directory search results by file path.
fn group_by_file(results: &[DirectorySearchMatch]) -> BTreeMap<PathBuf, Vec<DirectorySearchMatch>> {
    let mut grouped: BTreeMap<PathBuf, Vec<DirectorySearchMatch>> = BTreeMap::new();
    for result in results {
        grouped
            .entry(result.file_path.clone())
            .or_default()
            .push(result.clone());
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_by_file() {
        let results = vec![
            DirectorySearchMatch {
                file_path: PathBuf::from("/a.md"),
                line_number: 1,
                line_text: "hello".to_string(),
                score: 100,
                match_indices: vec![0],
                content_match_indices: vec![0],
                context_before: Vec::new(),
                context_after: Vec::new(),
            },
            DirectorySearchMatch {
                file_path: PathBuf::from("/b.md"),
                line_number: 1,
                line_text: "world".to_string(),
                score: 90,
                match_indices: vec![0],
                content_match_indices: vec![0],
                context_before: Vec::new(),
                context_after: Vec::new(),
            },
            DirectorySearchMatch {
                file_path: PathBuf::from("/a.md"),
                line_number: 5,
                line_text: "hello again".to_string(),
                score: 80,
                match_indices: vec![0],
                content_match_indices: vec![0],
                context_before: Vec::new(),
                context_after: Vec::new(),
            },
        ];

        let grouped = group_by_file(&results);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[&PathBuf::from("/a.md")].len(), 2);
        assert_eq!(grouped[&PathBuf::from("/b.md")].len(), 1);
    }
}
