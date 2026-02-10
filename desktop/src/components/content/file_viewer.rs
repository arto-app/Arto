use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::context_menu::ContextMenuData;
use super::context_menu_state::{open_context_menu, ContentContextMenuState};
use crate::markdown::render_to_html_with_toc;
use crate::state::{AppState, TabContent};
use crate::utils::file::is_markdown_file;
use crate::watcher::FILE_WATCHER;

/// Data structure for markdown link clicks from JavaScript
#[derive(Serialize, Deserialize)]
struct LinkClickData {
    path: String,
    button: u32,
    /// Current scroll position at the time of click (for history preservation)
    scroll_position: f64,
}

/// Mouse button constants
const LEFT_CLICK: u32 = 0;
const MIDDLE_CLICK: u32 = 1;

#[component]
pub fn FileViewer(file: PathBuf) -> Element {
    let state = use_context::<AppState>();
    let html = use_signal(String::new);
    let reload_trigger = use_signal(|| 0usize);

    // Get base directory for link resolution
    let base_dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Setup component hooks
    use_file_loader(file.clone(), html, reload_trigger, state);
    use_file_watcher(file.clone(), reload_trigger, state);
    use_line_jump_handler(state);
    use_link_click_handler(file.clone(), state);
    use_mermaid_window_handler();
    use_context_menu_handler(file.clone(), base_dir);

    rsx! {
        div {
            class: "markdown-viewer",
            article {
                class: "markdown-body",
                dangerous_inner_html: "{html}"
            }
            // Context menu is rendered at App level to avoid re-rendering content
        }
    }
}

/// Hook to load and render file content
fn use_file_loader(
    file: PathBuf,
    html: Signal<String>,
    reload_trigger: Signal<usize>,
    mut state: AppState,
) {
    use_effect(use_reactive!(|file, reload_trigger| {
        let mut html = html;
        let _ = reload_trigger();
        let file = file.clone();

        // Handle scroll position SYNCHRONOUSLY before spawning async task.
        // This ensures the onRenderComplete callback is registered before
        // MutationObserver triggers #executeBatchRender().
        handle_scroll_position(&mut state);

        spawn(async move {
            tracing::info!("Loading and rendering file: {:?}", &file);

            // Try to read as string (UTF-8 text file)
            match tokio::fs::read_to_string(file.as_path()).await {
                Ok(content) => {
                    // Check if file has markdown extension
                    if is_markdown_file(&file) {
                        // Render as markdown with TOC heading extraction
                        match render_to_html_with_toc(&content, &file) {
                            Ok((rendered, headings)) => {
                                html.set(rendered);
                                state.right_sidebar_headings.set(headings);
                                tracing::trace!("Rendered as Markdown: {:?}", &file);
                            }
                            Err(e) => {
                                // Markdown parsing failed, render as plain text
                                tracing::warn!(
                                    "Markdown parsing failed for {:?}, rendering as plain text: {}",
                                    &file,
                                    e
                                );
                                let escaped_content = html_escape::encode_text(&content);
                                let plain_html = format!(
                                    r#"<pre class="plain-text-viewer">{}</pre>"#,
                                    escaped_content
                                );
                                html.set(plain_html);
                                state.right_sidebar_headings.set(Vec::new());
                            }
                        }
                    } else {
                        // Non-markdown file, render as plain text directly
                        tracing::info!("Rendering non-markdown file as plain text: {:?}", &file);
                        let escaped_content = html_escape::encode_text(&content);
                        let plain_html = format!(
                            r#"<pre class="plain-text-viewer">{}</pre>"#,
                            escaped_content
                        );
                        html.set(plain_html);
                        state.right_sidebar_headings.set(Vec::new());
                    }

                    // Re-apply search highlighting after content changes.
                    // Skips when PendingScroll::Line is set to avoid duplicate execution:
                    // - Line jumps are handled by use_line_jump_handler
                    // - This prevents race condition and unnecessary re-highlighting
                    if should_reapply_search(&state) {
                        reapply_search_from_state().await;
                    }
                }
                Err(e) => {
                    // Failed to read as UTF-8 text (likely binary file)
                    tracing::error!("Failed to read file {:?} as text: {}", file, e);
                    let error_msg = format!("{:?}", e);

                    // Update tab content to FileError
                    let file_clone = file.clone();
                    state.update_current_tab(move |tab| {
                        tab.content = TabContent::FileError(file_clone, error_msg);
                    });
                    html.set(String::new());
                }
            }
        });
    }));
}

/// Handle scroll position restoration when navigating to a file.
///
/// Only processes PendingScroll::Position (pixel-based scroll from history/tab switching).
/// PendingScroll::Line is handled by use_line_jump_handler and left untouched.
///
/// Position restoration uses two-phase approach:
/// 1. Immediately when DOM content changes (MutationObserver, before browser paint)
/// 2. After Mermaid/KaTeX rendering completes (adjusts for content height changes)
fn handle_scroll_position(state: &mut AppState) {
    use crate::state::PendingScroll;

    let pending_scroll = *state.pending_scroll.read();

    let scroll = match pending_scroll {
        Some(PendingScroll::Position(scroll)) => scroll,
        Some(PendingScroll::Line(_line)) => {
            // Line jumps handled by use_line_jump_handler - leave value untouched
            return;
        }
        None => return,
    };

    // Only clear if we're handling Position
    state.pending_scroll.set(None);

    // Fast path: scrolling to top doesn't need two-phase restoration
    if scroll <= 0.0 {
        let _ = document::eval("document.querySelector('.content')?.scrollTo(0, 0);");
        tracing::debug!("Reset scroll position to top (fast path)");
        return;
    }

    {
        // Two-phase scroll restoration for non-zero positions:
        // Phase 1: MutationObserver on .markdown-body fires synchronously after innerHTML
        //          update but before browser paint, preventing visible scroll flash.
        // Phase 2: onRenderComplete fires after Mermaid/KaTeX render, adjusting for any
        //          content height changes from dynamic rendering.
        let scroll_js = format!(
            r#"(() => {{
                const target = {};
                const container = document.querySelector('.markdown-body');
                let observer;
                if (container) {{
                    observer = new MutationObserver(() => {{
                        if (observer) {{
                            observer.disconnect();
                            observer = null;
                        }}
                        document.querySelector('.content')?.scrollTo(0, target);
                    }});
                    observer.observe(container, {{ childList: true }});
                    // Fallback: ensure the observer is disconnected even if no mutation occurs.
                    setTimeout(() => {{
                        if (observer) {{
                            observer.disconnect();
                            observer = null;
                        }}
                    }}, 5000);
                }}
                window.Arto.onRenderComplete(() => {{
                    if (observer) {{
                        observer.disconnect();
                        observer = null;
                    }}
                    document.querySelector('.content')?.scrollTo(0, target);
                }});
            }})();"#,
            scroll
        );
        let _ = document::eval(&scroll_js);
        tracing::debug!(scroll, "Scheduled two-phase scroll position restoration");
    }
}

/// Check if search should be reapplied after file content changes.
/// Returns false when PendingScroll::Line is set to avoid duplicate execution.
fn should_reapply_search(state: &AppState) -> bool {
    use crate::state::PendingScroll;
    // Line jumps are handled by use_line_jump_handler
    !matches!(*state.pending_scroll.read(), Some(PendingScroll::Line(_)))
}

/// Re-apply search highlighting from AppState.
/// Reads search results and selected index, applies highlights for current file.
async fn reapply_search_from_state() {
    let state = use_context::<AppState>();
    let query = state.directory_search_query.read().clone();
    let results = state.directory_search_results.read().clone();
    let selected_index = *state.selected_search_result_index.read();

    // Get current file
    let current_file = {
        let tabs = state.tabs.read();
        let active = *state.active_tab.read();
        tabs.get(active)
            .and_then(|t| t.file().map(|p| p.to_path_buf()))
    };

    let Some(file) = current_file else {
        // No file loaded, clear search highlights (but keep pinned)
        let _ = document::eval(
            r#"
            (() => {
                const container = document.querySelector('.markdown-body');
                if (container) {
                    const observer = new MutationObserver(() => {
                        observer.disconnect();
                        requestAnimationFrame(() => window.Arto.search.reapply());
                    });
                    observer.observe(container, { childList: true, subtree: true });
                    setTimeout(() => { observer.disconnect(); window.Arto.search.reapply(); }, 100);
                }
            })();
        "#,
        )
        .await;
        return;
    };

    // Filter for current file
    let file_results: Vec<_> = results.iter().filter(|r| r.file_path == file).collect();

    if file_results.is_empty() || query.is_none() {
        // No matches for this file, clear search highlights (but keep pinned)
        let _ = document::eval(
            r#"
            (() => {
                const container = document.querySelector('.markdown-body');
                if (container) {
                    const observer = new MutationObserver(() => {
                        observer.disconnect();
                        requestAnimationFrame(() => window.Arto.search.reapply());
                    });
                    observer.observe(container, { childList: true, subtree: true });
                    setTimeout(() => { observer.disconnect(); window.Arto.search.reapply(); }, 100);
                }
            })();
        "#,
        )
        .await;
        return;
    }

    // Build JS
    let json_query = serde_json::to_string(&query.unwrap_or_default()).unwrap_or_default();
    let matches_json: Vec<serde_json::Value> = file_results
        .iter()
        .map(|r| {
            serde_json::json!({
                "lineNumber": r.line_number,
                "contentIndices": r.content_match_indices,
            })
        })
        .collect();
    let json_matches = serde_json::to_string(&matches_json).unwrap_or_else(|_| "[]".to_string());

    let set_pending_js = selected_index
        .map(|idx| format!("window.Arto.search.setNavigateOnApply({});", idx))
        .unwrap_or_default();

    let apply_js = format!(
        r#"(() => {{
            let called = false;
            const doApply = () => {{
                if (called) return;
                called = true;
                {set_pending}
                window.Arto.search.applyHighlights({query}, {matches});
            }};
            const container = document.querySelector('.markdown-body');
            if (!container) {{
                const bodyObserver = new MutationObserver(() => {{
                    if (document.querySelector('.markdown-body')) {{
                        bodyObserver.disconnect();
                        requestAnimationFrame(doApply);
                    }}
                }});
                bodyObserver.observe(document.body, {{ childList: true, subtree: true }});
                setTimeout(() => {{ bodyObserver.disconnect(); doApply(); }}, 100);
                return;
            }}
            const observer = new MutationObserver(() => {{
                observer.disconnect();
                requestAnimationFrame(doApply);
            }});
            observer.observe(container, {{ childList: true, subtree: true }});
            setTimeout(() => {{ observer.disconnect(); doApply(); }}, 100);
        }})()"#,
        set_pending = set_pending_js,
        query = json_query,
        matches = json_matches
    );

    let _ = document::eval(&apply_js).await;
}

/// Hook to watch file for changes and trigger reload
fn use_file_watcher(file: PathBuf, reload_trigger: Signal<usize>, mut state: AppState) {
    use_effect(use_reactive!(|file| {
        let mut reload_trigger = reload_trigger;
        let file = file.clone();

        spawn(async move {
            let file_path = file.clone();
            let mut watcher = match FILE_WATCHER.watch(file_path.clone()).await {
                Ok(watcher) => watcher,
                Err(e) => {
                    tracing::error!(
                        "Failed to register file watcher for {:?}: {:?}",
                        file_path,
                        e
                    );
                    return;
                }
            };

            while watcher.recv().await.is_some() {
                tracing::info!("File change detected, reloading: {:?}", file_path);
                // Save current scroll position before reloading so it can be restored
                // after the content re-renders (reuses the back/forward restoration mechanism)
                let scroll = *state.current_scroll_position.read();
                state
                    .pending_scroll
                    .set(Some(crate::state::PendingScroll::Position(scroll)));
                reload_trigger.set(reload_trigger() + 1);
            }

            if let Err(e) = FILE_WATCHER.unwatch(file_path.clone()).await {
                tracing::error!(
                    "Failed to unregister file watcher for {:?}: {:?}",
                    file_path,
                    e
                );
            }
        });
    }));
}

/// Hook to handle line jumps from Fuzzy Finder search results.
/// Monitors pending_scroll Signal and triggers search re-application when Line is set.
/// This enables both same-file and different-file jumps.
fn use_line_jump_handler(mut state: AppState) {
    use_effect(move || {
        use crate::state::PendingScroll;

        // Check if pending_scroll is set to Line variant
        // Calling the Signal makes this effect reactive to its changes
        let pending = (state.pending_scroll)();
        if let Some(PendingScroll::Line(_line)) = pending {
            // Re-apply search from state to update fuzzyMatches and navigate to selected index
            // This is necessary for same-file jumps where file Signal doesn't change
            spawn(async move {
                reapply_search_from_state().await;
            });

            // Clear pending_scroll after consuming
            state.pending_scroll.set(None);
        }
    });
}

/// Hook to setup JavaScript handler for markdown link clicks
fn use_link_click_handler(file: PathBuf, state: AppState) {
    use_effect(use_reactive!(|file| {
        let file = file.clone();
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleMarkdownLinkClick = (path, button) => {
                const scrollPosition = document.querySelector('.content')?.scrollTop || 0;
                dioxus.send({ path, button, scroll_position: scrollPosition });
            };
        "#});

        let base_dir = file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut state_clone = state;

        spawn(async move {
            while let Ok(click_data) = eval_provider.recv::<LinkClickData>().await {
                handle_link_click(click_data, &base_dir, &mut state_clone);
            }
        });
    }));
}

/// Handle a markdown link click event
fn handle_link_click(click_data: LinkClickData, base_dir: &Path, state: &mut AppState) {
    let LinkClickData {
        path,
        button,
        scroll_position,
    } = click_data;

    tracing::info!("Markdown link clicked: {} (button: {})", path, button);

    // Resolve and normalize the path
    let target_path = base_dir.join(&path);
    let Ok(canonical_path) = target_path.canonicalize() else {
        tracing::error!("Failed to resolve path: {:?}", target_path);
        return;
    };

    tracing::info!("Opening file: {:?}", canonical_path);

    match button {
        MIDDLE_CLICK => {
            // Open in new tab (always create a new tab for middle-click)
            state.add_file_tab(canonical_path, true);
        }
        LEFT_CLICK => {
            // Save current scroll position to history before navigating
            state.save_current_scroll_position(scroll_position);
            // Navigate in current tab (in-tab navigation, no existing tab check)
            state.navigate_to_file(canonical_path);
        }
        _ => {
            tracing::debug!("Ignoring click with button: {}", button);
        }
    }
}

/// Hook to setup Mermaid window open handler
fn use_mermaid_window_handler() {
    use_effect(|| {
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleMermaidWindowOpen = (source) => {
                dioxus.send({ type: "open_mermaid_window", source: source });
            };
        "#});

        spawn(async move {
            while let Ok(data) = eval_provider.recv::<serde_json::Value>().await {
                if let Some(msg_type) = data.get("type").and_then(|v| v.as_str()) {
                    if msg_type == "open_mermaid_window" {
                        if let Some(source) = data.get("source").and_then(|v| v.as_str()) {
                            let state = use_context::<AppState>();
                            let theme = *state.current_theme.read();
                            tracing::info!("Opening mermaid window for diagram");
                            crate::window::open_or_focus_mermaid_window(source.to_string(), theme);
                        }
                    }
                }
            }
        });
    });
}

/// Hook to setup context menu handler for right-clicks on content
///
/// Uses global state to avoid re-rendering FileViewer when menu state changes.
/// This preserves text selection in the content.
fn use_context_menu_handler(file: PathBuf, base_dir: PathBuf) {
    use_effect(use_reactive!(|file, base_dir| {
        let file = file.clone();
        let base_dir = base_dir.clone();

        // Setup JS context menu handler using the exported function
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            // Setup context menu handler
            window.Arto.setupContextMenu((data) => {
                dioxus.send(data);
            });
        "#});

        spawn(async move {
            while let Ok(data) = eval_provider.recv::<ContextMenuData>().await {
                tracing::debug!(?data, "Context menu triggered");
                // Write to global state (doesn't subscribe FileViewer)
                open_context_menu(ContentContextMenuState {
                    data,
                    current_file: Some(file.clone()),
                    base_dir: base_dir.clone(),
                });
            }
        });
    }));
}
