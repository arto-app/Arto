mod input;
mod mode_switch;
mod results;

use dioxus::document;
use dioxus::prelude::*;
use dioxus_core::use_drop;

use crate::components::pinned_chips::PinnedChipsRow;
use crate::finder::nucleo_worker::{NucleoCommand, NucleoResults};
use crate::finder::FinderMode;
use crate::pinned_search::{PinnedScope, PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};
use crate::state::AppState;

use input::FuzzyFinderInput;
use mode_switch::FinderModeSwitch;
use results::FinderResultsList;

/// JavaScript to clear the search input field only (no DOM highlight changes).
/// Used during modal editing — highlights are only applied on confirm (Enter).
const JS_CLEAR_INPUT: &str = r#"
    const input = document.querySelector('.search-input');
    if (input) {
        input.value = '';
        input.focus();
    }
"#;

/// JavaScript to focus the search input and install keydown interceptors that
/// prevent default browser behavior (e.g. macOS Emacs-style Ctrl+N/P scrolling).
///
/// Two handlers are installed:
/// 1. On the `input` — suppresses the input's native key handling in WebView.
/// 2. On the `.fuzzy-finder` overlay (capture phase) — suppresses the scrollable
///    `.finder-results` container from reacting to navigation keys.
///
/// Both are needed: the input handler alone doesn't prevent the overflow container
/// from scrolling, and the overlay handler alone doesn't suppress native input
/// behavior on macOS.
const JS_FOCUS: &str = r#"
    const NAV_HANDLER = (e) => {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp' ||
            e.key === 'Escape' || e.key === 'Enter') {
            e.preventDefault();
        }
        if (e.ctrlKey && (e.key === 'n' || e.key === 'p')) {
            e.preventDefault();
        }
        if (e.metaKey && e.key === 'm') {
            e.preventDefault();
        }
    };

    const input = document.querySelector('.search-input');
    if (input) {
        input.focus();
        if (!input.__finderKeyHandler) {
            input.__finderKeyHandler = NAV_HANDLER;
            input.addEventListener('keydown', NAV_HANDLER, { passive: false });
        }
    }

    const overlay = document.querySelector('.fuzzy-finder');
    if (overlay && !overlay.__finderKeyHandler) {
        overlay.__finderKeyHandler = NAV_HANDLER;
        overlay.addEventListener('keydown', NAV_HANDLER, { capture: true, passive: false });
    }
"#;

/// Build JSON for pinned searches to sync to JavaScript.
/// Includes all searches (enabled and disabled) so disabled ones still show matches in sidebar.
fn build_pinned_json(searches: &[PinnedSearch]) -> String {
    serde_json::to_string(searches).unwrap_or_else(|_| "[]".to_string())
}

/// Wait for JavaScript search API to be ready.
async fn wait_for_js_ready() {
    use std::time::Duration;

    // Poll until window.Arto.search.setPinned is available
    for _ in 0..50 {
        // 50 * 50ms = 2.5s max wait
        let mut eval =
            document::eval("dioxus.send(typeof window.Arto?.search?.setPinned === 'function')");
        if let Ok(true) = eval.recv::<bool>().await {
            return;
        }
        // Small delay before retrying
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tracing::warn!("Timeout waiting for JavaScript search API to be ready");
}

/// Sync pinned searches to JavaScript for highlighting (async version).
async fn sync_pinned_to_js_async(searches: &[PinnedSearch]) {
    let json = build_pinned_json(searches);
    let js = format!("window.Arto.search.setPinned({});", json);
    let _ = document::eval(&js).await;
}

/// Extract current file path and root directory from AppState.
fn get_current_context(
    state: &AppState,
) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    let current_file = {
        let tabs = state.tabs.read();
        let active = *state.active_tab.read();
        tabs.get(active)
            .and_then(|t| t.file().map(|p| p.to_path_buf()))
    };
    let current_dir = state.sidebar.read().root_directory.clone();
    (current_file, current_dir)
}

/// Filter pinned searches to only those in scope for the current context.
fn filter_in_scope_pinned(searches: &[PinnedSearch], state: &AppState) -> Vec<PinnedSearch> {
    let (current_file, current_dir) = get_current_context(state);
    searches
        .iter()
        .filter(|p| p.is_in_scope(current_file.as_deref(), current_dir.as_deref()))
        .cloned()
        .collect()
}

/// Fuzzy Finder overlay — Raycast-style command palette.
///
/// Architecture: a persistent worker thread owns two `Nucleo<LineCandidate>` instances
/// (one for File mode, one for Directory mode). Communication uses channels:
/// - Commands (Dioxus → worker): SetFile, SetDirectory, Reparse, Stop
/// - Results (worker → Dioxus): NucleoResults { file_results, dir_results }
///
/// The FuzzyFinder is always mounted (CSS controls visibility), so the worker
/// thread lives for the entire window lifetime. Closing the finder just hides
/// the UI; all state (candidates, results, selected index) is preserved.
#[component]
pub fn FuzzyFinder() -> Element {
    let mut state = use_context::<AppState>();
    let is_open = *state.finder_open.read();
    let mode = *state.finder_mode.read();
    let selected_index = *state.finder_selected_index.read();
    let initial_text = state.search_initial_text.read().clone();
    let mut has_input = use_signal(|| false);
    let mut search_query = use_signal(String::new);
    // Generation counter for debouncing Reparse commands during fast typing.
    // Each keystroke increments the counter; only the last keystroke's delayed
    // task actually sends the command (if the counter hasn't changed).
    let mut debounce_gen = use_signal(|| 0u64);

    // Latest results from both Nucleo instances (persisted across open/close)
    let mut latest_results = use_signal(NucleoResults::default);

    // Create channels + spawn worker thread (window lifetime via use_hook).
    // Only the Sender is returned (it's Clone); the Receiver is consumed by
    // spawn inside the hook to avoid the Clone requirement.
    let cmd_tx = use_hook(move || {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (result_tx, mut result_rx) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(move || {
            crate::finder::nucleo_worker::nucleo_worker(cmd_rx, result_tx);
        });
        // Receive results in a Dioxus async task.
        // Uses spawn (not spawn_forever) to keep the task in FuzzyFinder's scope,
        // which is a descendant of App where the signals live. spawn_forever would
        // run at the root scope, causing signal ownership warnings.
        spawn(async move {
            while let Some(results) = result_rx.recv().await {
                // Drain all queued results, keeping only the latest.
                // The worker may send multiple intermediate snapshots as Nucleo's
                // parallel scoring progresses. Only the final snapshot matters.
                let mut latest = results;
                while let Ok(newer) = result_rx.try_recv() {
                    latest = newer;
                }
                // Only update the local signal — no AppState writes.
                // Results commit to AppState on Enter (confirm).
                latest_results.set(latest);
            }
        });
        cmd_tx
    });

    // Clone cmd_tx for each closure that needs it (Sender is Clone but not Copy).
    // use_drop gets the original (last consumer).
    let cmd_tx_file = cmd_tx.clone();
    let cmd_tx_dir = cmd_tx.clone();
    let cmd_tx_initial = cmd_tx.clone();
    let cmd_tx_input = cmd_tx.clone();
    let cmd_tx_clear = cmd_tx.clone();

    // Send SetFile when the active file changes
    use_effect(move || {
        let file = {
            let tabs = state.tabs.read();
            let active = *state.active_tab.read();
            tabs.get(active)
                .and_then(|t| t.file().map(|p| p.to_path_buf()))
        };
        if let Some(file) = file {
            let _ = cmd_tx_file.send(NucleoCommand::SetFile { file });
        }
    });

    // Send SetDirectory when the root directory changes
    use_effect(move || {
        let dir = state.sidebar.read().root_directory.clone();
        if let Some(dir) = dir {
            let _ = cmd_tx_dir.send(NucleoCommand::SetDirectory { dir });
        }
    });

    // Stop worker thread on window close
    use_drop(move || {
        let _ = cmd_tx.send(NucleoCommand::Stop);
    });

    // Local signal for pinned searches (updated via broadcast)
    let mut pinned_searches = use_signal(|| PINNED_SEARCHES.read().pinned_searches.clone());

    // Subscribe to pinned search changes and sync to JavaScript (scope-filtered)
    use_future(move || async move {
        // Wait for JavaScript search API to be ready before initial sync
        wait_for_js_ready().await;

        // Initial sync: only send pinned searches that are in scope
        let all = pinned_searches.read().clone();
        let active = filter_in_scope_pinned(&all, &state);
        sync_pinned_to_js_async(&active).await;

        // Listen for changes
        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            let all = PINNED_SEARCHES.read().pinned_searches.clone();
            pinned_searches.set(all.clone());

            // Filter by scope before sending to JavaScript
            let active = filter_in_scope_pinned(&all, &state);
            sync_pinned_to_js_async(&active).await;
        }
    });

    // Re-sync pinned searches to JS when the active tab changes.
    // Different tabs may show different files, so the in-scope pinned set changes.
    let active_tab_for_pinned = *state.active_tab.read();
    use_effect(use_reactive!(|active_tab_for_pinned| {
        let _ = active_tab_for_pinned; // reactive dependency trigger
        let all = PINNED_SEARCHES.read().pinned_searches.clone();
        let active = filter_in_scope_pinned(&all, &state);
        spawn(async move {
            sync_pinned_to_js_async(&active).await;
        });
    }));

    // Focus input when finder opens, blur when it closes
    use_effect(use_reactive!(|is_open| {
        if is_open {
            spawn(async move {
                // Small delay to ensure DOM is ready
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                let _ = document::eval(JS_FOCUS).await;
            });
        } else {
            spawn(async move {
                let _ = document::eval("document.querySelector('.search-input')?.blur()").await;
            });
        }
    }));

    // Handle initial text when finder opens
    use_effect(use_reactive!(|is_open, initial_text| {
        if is_open {
            if let Some(ref text) = initial_text {
                if !text.is_empty() {
                    has_input.set(true);
                    search_query.set(text.clone());
                    // Set the input value and select text for editing
                    let json_encoded = serde_json::to_string(text).unwrap_or_default();
                    let js = format!(
                        r#"
                        const input = document.querySelector('.search-input');
                        if (input) {{
                            input.value = {};
                            input.focus();
                            input.select();
                        }}
                        "#,
                        json_encoded
                    );
                    spawn(async move {
                        let _ = document::eval(&js).await;
                    });
                    // Send Reparse to worker (both Nucleo instances will be updated)
                    let _ = cmd_tx_initial.send(NucleoCommand::Reparse {
                        query: text.clone(),
                    });
                }
                // Clear the initial text after using it
                state.search_initial_text.set(None);
            }
        }
    }));

    // Compute the current mode's result slice from the local signal.
    // This avoids reading AppState and keeps the Finder fully modal.
    let current_results = {
        let lr = latest_results.read();
        match mode {
            FinderMode::File => lr.file_results.clone(),
            FinderMode::Directory => lr.dir_results.clone(),
        }
    };
    let results_count = current_results.len();

    // Confirm the selected result: commit to AppState and open file.
    // File rendering will automatically apply highlights and jump to the selected index.
    let mut confirm_and_close = move |index: usize| {
        let query = search_query.read().clone();

        // If query is empty, just clear and close (don't navigate)
        if query.is_empty() {
            state.update_directory_search_results(None, Vec::new());
            state.close_finder();
            return;
        }

        // Read results from local signal (NOT AppState)
        let results = {
            let lr = latest_results.read();
            match *state.finder_mode.read() {
                FinderMode::File => lr.file_results.clone(),
                FinderMode::Directory => lr.dir_results.clone(),
            }
        };

        let Some(result) = results.get(index) else {
            // No matching results: just close without navigation
            state.close_finder();
            return;
        };

        let file_path = result.file_path.clone();
        let line_number = result.line_number;

        // Compute the selected index within same-file matches
        let selected_index = results[..=index]
            .iter()
            .filter(|r| r.file_path == file_path)
            .count()
            .saturating_sub(1);

        // Commit results to AppState
        state.update_directory_search_results(Some(query), results);
        state.set_selected_search_result_index(Some(selected_index));
        state.close_finder();

        // Open file with line number - works for both same file and different file
        state.open_file(
            file_path,
            crate::state::OpenFileOptions::with_line(line_number),
        );
    };

    // Keyboard handler at the overlay level
    let on_keydown = move |evt: Event<KeyboardData>| {
        match evt.key() {
            Key::Escape => state.close_finder(),
            Key::Enter => {
                let selected = *state.finder_selected_index.read();
                confirm_and_close(selected);
            }
            // Cmd+M: toggle finder mode (only if target mode has valid context)
            Key::Character(ref c) if c == "m" && evt.modifiers().meta() => {
                evt.prevent_default();
                state.switch_finder_mode();
            }
            Key::ArrowDown => {
                evt.prevent_default();
                if results_count > 0 {
                    let current = *state.finder_selected_index.read();
                    state
                        .finder_selected_index
                        .set((current + 1).min(results_count - 1));
                }
            }
            Key::Character(ref c) if c == "n" && evt.modifiers().ctrl() => {
                evt.prevent_default();
                if results_count > 0 {
                    let current = *state.finder_selected_index.read();
                    state
                        .finder_selected_index
                        .set((current + 1).min(results_count - 1));
                }
            }
            Key::ArrowUp => {
                evt.prevent_default();
                let current = *state.finder_selected_index.read();
                state.finder_selected_index.set(current.saturating_sub(1));
            }
            Key::Character(ref c) if c == "p" && evt.modifiers().ctrl() => {
                evt.prevent_default();
                let current = *state.finder_selected_index.read();
                state.finder_selected_index.set(current.saturating_sub(1));
            }
            _ => {}
        }
    };

    rsx! {
        // Backdrop: clicking outside the Finder panel closes it (same as Escape)
        if is_open {
            div {
                class: "fuzzy-finder-backdrop",
                onclick: move |_| state.close_finder(),
            }
        }

        div {
            class: if is_open { "fuzzy-finder fuzzy-finder--open" } else { "fuzzy-finder" },
            onkeydown: on_keydown,

            // Search input row
            FuzzyFinderInput {
                mode,
                has_input,
                result_count: results_count,
                selected_index: selected_index + 1,
                on_input: move |value: String| {
                    has_input.set(!value.is_empty());
                    search_query.set(value.clone());
                    state.finder_selected_index.set(0);
                    if !value.is_empty() {
                        // Debounce: only send Reparse after input stabilizes.
                        // With debounce, fast typing batches into a single Reparse.
                        let gen = debounce_gen() + 1;
                        debounce_gen.set(gen);
                        let tx = cmd_tx_input.clone();
                        spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            if debounce_gen() == gen {
                                let _ = tx.send(NucleoCommand::Reparse { query: value });
                            }
                        });
                    }
                },
                on_clear: move |_| {
                    has_input.set(false);
                    search_query.set(String::new());
                    // Send empty Reparse to reset both Nucleo patterns
                    let _ = cmd_tx_clear.send(NucleoCommand::Reparse {
                        query: String::new(),
                    });
                    // Only clear the input field — no AppState or DOM highlight changes
                    spawn(async move {
                        let _ = document::eval(JS_CLEAR_INPUT).await;
                    });
                },
                on_pin: move |query: String| {
                    let mode = *state.finder_mode.read();
                    let scope = match mode {
                        FinderMode::File => {
                            let file_path = {
                                let tabs = state.tabs.read();
                                let active = *state.active_tab.read();
                                tabs.get(active)
                                    .and_then(|t| t.file().map(|p| p.to_path_buf()))
                            };
                            match file_path {
                                Some(path) => PinnedScope::File(path),
                                None => return, // Can't pin without a file
                            }
                        }
                        FinderMode::Directory => {
                            let root = state.sidebar.read().root_directory.clone();
                            match root {
                                Some(path) => PinnedScope::Directory(path),
                                None => return, // Can't pin without a directory
                            }
                        }
                    };
                    crate::pinned_search::add_pinned_search(query, scope);
                    has_input.set(false);
                    spawn(async move {
                        let _ = document::eval(JS_CLEAR_INPUT).await;
                    });
                },
            }

            // Pinned chips row (only visible when pinned searches exist)
            PinnedChipsRow {
                pinned_searches: pinned_searches.read().clone(),
            }

            // Results list — fed from local signal, not AppState
            FinderResultsList {
                results: current_results,
                mode,
                on_select: move |index: usize| {
                    state.finder_selected_index.set(index);
                    confirm_and_close(index);
                },
            }

            // Footer with mode indicator and shortcut hint
            FinderModeSwitch {}
        }
    }
}
