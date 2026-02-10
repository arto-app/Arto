use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::finder::{build_highlight_segments, DirectorySearchMatch, FinderMode};
use crate::state::AppState;

/// Unified results list for both Finder modes.
///
/// Results are passed via props from the parent FuzzyFinder component (local
/// signal), NOT read from AppState. This keeps the Finder modal: results only
/// commit to AppState on Enter (confirm).
///
/// Tracks mouse activity to avoid spurious `:hover` highlights when the finder
/// opens with the cursor already over a card. Hover is only enabled after the
/// user physically moves the mouse; keyboard navigation and finder open/close
/// disable it again. Mouse coordinate comparison guards against scroll-triggered
/// `mousemove` events that fire without actual mouse movement.
#[component]
pub fn FinderResultsList(
    results: Vec<DirectorySearchMatch>,
    mode: FinderMode,
    on_select: EventHandler<usize>,
) -> Element {
    let state = use_context::<AppState>();
    let selected_index = *state.finder_selected_index.read();
    let is_open = *state.finder_open.read();

    let mut mouse_active = use_signal(|| false);
    let mut last_mouse_pos = use_signal(|| (0.0f64, 0.0f64));

    // Reset hover state when finder opens/closes or keyboard navigation happens.
    // Both `is_open` and `selected_index` trigger a reset so that:
    //  - Re-opening the finder doesn't carry over stale mouse_active = true
    //  - Arrow/Ctrl+N/P navigation disables hover immediately
    use_effect(use_reactive!(|is_open, selected_index| {
        let _ = (is_open, selected_index);
        mouse_active.set(false);
    }));

    let results_class = if mouse_active() {
        "finder-results finder-results--mouse-active"
    } else {
        "finder-results"
    };

    // Only enable hover when the mouse physically moves (coordinates change).
    // This prevents scroll_to() from triggering mousemove and re-enabling hover.
    let on_mousemove = move |evt: Event<MouseData>| {
        let coords = evt.client_coordinates();
        let pos = (coords.x, coords.y);
        let last = *last_mouse_pos.read();
        if (pos.0 - last.0).abs() > 0.5 || (pos.1 - last.1).abs() > 0.5 {
            last_mouse_pos.set(pos);
            mouse_active.set(true);
        }
    };

    if results.is_empty() {
        return rsx! {};
    }

    let count_label = match mode {
        FinderMode::File => format!("{} matches in current file", results.len()),
        FinderMode::Directory => format!("{} results across files", results.len()),
    };

    rsx! {
        div {
            class: results_class,
            onmousemove: on_mousemove,

            div {
                class: "finder-results-count",
                "{count_label}"
            }

            for (i, result) in results.iter().enumerate() {
                ResultCard {
                    key: "{i}",
                    result: result.clone(),
                    is_selected: i == selected_index,
                    show_file_info: mode == FinderMode::Directory,
                    index: i,
                    on_select: move |index: usize| on_select.call(index),
                }
            }
        }
    }
}

// ── Unified result card ─────────────────────────

/// A card showing a single fuzzy match result with context lines.
///
/// Directory mode: metadata header (file name + directory) + code block.
/// File mode: code block only (no metadata header).
#[component]
fn ResultCard(
    result: DirectorySearchMatch,
    is_selected: bool,
    show_file_info: bool,
    index: usize,
    on_select: EventHandler<usize>,
) -> Element {
    let state = use_context::<AppState>();

    // Auto-scroll selected item into view using block:'nearest' to avoid
    // jumping the list so the selected card sits at the top every time.
    use_effect(use_reactive!(|is_selected| {
        if is_selected {
            let js = format!(
                "document.getElementById('finder-card-{}')?.scrollIntoView({{ behavior: 'smooth', block: 'nearest' }})",
                index
            );
            spawn(async move {
                let _ = document::eval(&js).await;
            });
        }
    }));

    // Build highlighted line text
    let highlighted_segments = build_highlight_segments(&result.line_text, &result.match_indices);

    let card_class = if is_selected {
        "finder-card selected"
    } else {
        "finder-card"
    };

    // File metadata (only for directory mode)
    let file_name = if show_file_info {
        result
            .file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    let display_dir = if show_file_info {
        let sidebar = state.sidebar.read();
        if let Some(root) = &sidebar.root_directory {
            result
                .file_path
                .parent()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    if s.is_empty() {
                        ".".to_string()
                    } else {
                        s
                    }
                })
                .unwrap_or_else(|| ".".to_string())
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Compute the first line number for gutter width alignment
    let first_line = result
        .line_number
        .saturating_sub(result.context_before.len());
    let last_line = result.line_number + result.context_after.len();
    let gutter_width = format!("{}ch", last_line.to_string().len());

    rsx! {
        div {
            id: "finder-card-{index}",
            class: card_class,
            onclick: move |_| on_select.call(index),

            // Metadata header (directory mode only)
            if show_file_info {
                div {
                    class: "finder-card-meta",
                    Icon { name: IconName::File, size: 12 }
                    span { class: "finder-card-filename", "{file_name}" }
                    if !display_dir.is_empty() {
                        span { class: "finder-card-dir", "{display_dir}" }
                    }
                }
            }

            // Code block with context lines + match line
            div {
                class: "finder-card-code",

                // Context before
                for (i, line) in result.context_before.iter().enumerate() {
                    div {
                        key: "before-{i}",
                        class: "finder-card-code-line finder-card-code-line--context",
                        span {
                            class: "finder-card-code-gutter",
                            min_width: "{gutter_width}",
                            "{first_line + i}"
                        }
                        span { class: "finder-card-code-text", "{line}" }
                    }
                }

                // Match line (highlighted)
                div {
                    class: "finder-card-code-line finder-card-code-line--match",
                    span {
                        class: "finder-card-code-gutter",
                        min_width: "{gutter_width}",
                        "{result.line_number}"
                    }
                    span { class: "finder-card-code-text",
                        for (text, is_match) in highlighted_segments {
                            if is_match {
                                span {
                                    class: "finder-match-highlight",
                                    "{text}"
                                }
                            } else {
                                span { "{text}" }
                            }
                        }
                    }
                }

                // Context after
                for (i, line) in result.context_after.iter().enumerate() {
                    div {
                        key: "after-{i}",
                        class: "finder-card-code-line finder-card-code-line--context",
                        span {
                            class: "finder-card-code-gutter",
                            min_width: "{gutter_width}",
                            "{result.line_number + 1 + i}"
                        }
                        span { class: "finder-card-code-text", "{line}" }
                    }
                }
            }
        }
    }
}
