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

    // Get base directory for link resolution
    let base_dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Setup component hooks
    use_file_loader(file.clone(), html, state);
    use_file_watcher(file.clone(), state);
    use_link_click_handler(file.clone(), state);
    use_mermaid_window_handler();
    use_math_window_handler();
    use_image_window_handler();
    use_clipboard_handlers();
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
fn use_file_loader(file: PathBuf, html: Signal<String>, mut state: AppState) {
    use_effect(use_reactive!(|file| {
        let mut html = html;
        // Subscribe to reload_trigger via Dioxus auto-subscription so this
        // effect re-runs when the counter changes (manual reload or file watcher).
        // NOTE: We intentionally read() here instead of adding reload_trigger to the
        // use_reactive!(|...|) argument list, because use_reactive! compares Signal
        // by pointer identity — the same Signal object is always "equal" to itself,
        // so value changes would never be detected.
        let _ = state.reload_trigger.read();
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

                    // Re-apply search highlighting after content changes
                    // This preserves search state across tab switches
                    reapply_search().await;
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

/// Handle scroll position when navigating to a file.
///
/// If pending_scroll_position is set (from back/forward navigation or tab switch),
/// restore that position in two phases:
/// 1. Immediately when DOM content changes (MutationObserver, before browser paint)
/// 2. After Mermaid/KaTeX rendering completes (adjusts for content height changes)
///
/// Otherwise, reset to top immediately (for new navigation like clicking a link).
fn handle_scroll_position(state: &mut AppState) {
    let pending_scroll = state.pending_scroll_position.take();

    if let Some(scroll) = pending_scroll {
        // Fast path: scrolling to top doesn't need two-phase restoration
        if scroll <= 0.0 {
            let _ = document::eval("document.querySelector('.content')?.scrollTo(0, 0);");
            tracing::debug!("Reset scroll position to top (fast path)");
            return;
        }

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
                window.Arto.render.onComplete(() => {{
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
    } else {
        // Reset to top immediately for new navigation
        let _ = document::eval("document.querySelector('.content')?.scrollTo(0, 0);");
        tracing::debug!("Reset scroll position to top");
    }
}

/// Re-apply search highlighting after DOM changes.
/// This is called after content rendering to preserve search state across tab switches.
async fn reapply_search() {
    // Use MutationObserver to detect when DOM is actually updated, then reapply.
    // This is more robust than RAF-based timing which is not guaranteed.
    //
    // Flow:
    // 1. html.set() marks signal dirty (Rust side)
    // 2. This function runs and sets up MutationObserver
    // 3. Dioxus updates DOM (innerHTML changes)
    // 4. MutationObserver fires → reapply() is called
    // 5. Fallback timeout ensures reapply even if no mutation detected
    let _ = document::eval(indoc::indoc! {r#"
        (() => {
            let called = false;
            const doReapply = () => {
                if (called) return;
                called = true;
                window.Arto.search.reapply();
            };

            const container = document.querySelector('.markdown-body');
            if (!container) {
                // Container doesn't exist yet - Dioxus may still be building the DOM.
                // Wait for it to appear using MutationObserver on document.body.
                const bodyObserver = new MutationObserver(() => {
                    if (document.querySelector('.markdown-body')) {
                        bodyObserver.disconnect();
                        // Container appeared, wait one frame for content to render
                        requestAnimationFrame(doReapply);
                    }
                });
                bodyObserver.observe(document.body, { childList: true, subtree: true });

                // Fallback timeout in case container never appears
                setTimeout(() => {
                    bodyObserver.disconnect();
                    doReapply();
                }, 100);
                return;
            }

            const observer = new MutationObserver(() => {
                observer.disconnect();
                // Wait one frame after mutation to ensure rendering is complete
                requestAnimationFrame(doReapply);
            });

            // Note: childList + subtree is sufficient for innerHTML changes.
            // characterData is not needed since innerHTML replacement triggers childList mutations.
            observer.observe(container, {
                childList: true,
                subtree: true
            });

            // Fallback: if no mutation within 100ms, reapply anyway
            // This handles edge cases like navigating to the same file
            setTimeout(() => {
                observer.disconnect();
                doReapply();
            }, 100);
        })();
    "#})
    .await;
}

/// Hook to watch file for changes and trigger reload
fn use_file_watcher(file: PathBuf, mut state: AppState) {
    use_effect(use_reactive!(|file| {
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
                state.reload_current_tab();
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

/// Hook to setup Math window open handler
fn use_math_window_handler() {
    use_effect(|| {
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleMathWindowOpen = (source) => {
                dioxus.send({ type: "open_math_window", source: source });
            };
        "#});

        spawn(async move {
            while let Ok(data) = eval_provider.recv::<serde_json::Value>().await {
                if let Some(msg_type) = data.get("type").and_then(|v| v.as_str()) {
                    if msg_type == "open_math_window" {
                        if let Some(source) = data.get("source").and_then(|v| v.as_str()) {
                            let state = use_context::<AppState>();
                            let theme = *state.current_theme.read();
                            tracing::info!("Opening math window for LaTeX");
                            crate::window::open_or_focus_math_window(source.to_string(), theme);
                        }
                    }
                }
            }
        });
    });
}

/// Hook to setup Image window open handler
fn use_image_window_handler() {
    use_effect(|| {
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleImageWindowOpen = (src, alt) => {
                dioxus.send({ type: "open_image_window", src: src, alt: alt });
            };
        "#});

        spawn(async move {
            while let Ok(data) = eval_provider.recv::<serde_json::Value>().await {
                if let Some(msg_type) = data.get("type").and_then(|v| v.as_str()) {
                    if msg_type == "open_image_window" {
                        if let Some(src) = data.get("src").and_then(|v| v.as_str()) {
                            let alt = data
                                .get("alt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let state = use_context::<AppState>();
                            let theme = *state.current_theme.read();
                            tracing::info!("Opening image window");
                            crate::window::open_or_focus_image_window(src.to_string(), alt, theme);
                        }
                    }
                }
            }
        });
    });
}

/// Hook to register Rust clipboard handlers accessible from JavaScript.
///
/// Registers `window.rustCopyText(text)` and `window.rustCopyImage(dataUrl)` functions
/// that bridge JS clipboard requests to Rust's native clipboard utilities.
fn use_clipboard_handlers() {
    use_effect(|| {
        // Text copy handler
        spawn(async {
            let mut eval = document::eval(indoc::indoc! {r#"
                window.rustCopyText = (text) => {
                    dioxus.send({ type: "text", data: text });
                };
            "#});

            while let Ok(msg) = eval.recv::<serde_json::Value>().await {
                if let Some(text) = msg.get("data").and_then(|v| v.as_str()) {
                    let text = text.to_string();
                    std::thread::spawn(move || {
                        crate::utils::clipboard::copy_text(&text);
                    });
                }
            }
        });

        // Image copy handler
        spawn(async {
            let mut eval = document::eval(indoc::indoc! {r#"
                window.rustCopyImage = (dataUrl) => {
                    dioxus.send({ type: "image", data: dataUrl });
                };
            "#});

            while let Ok(msg) = eval.recv::<serde_json::Value>().await {
                if let Some(data_url) = msg.get("data").and_then(|v| v.as_str()) {
                    let data_url = data_url.to_string();
                    std::thread::spawn(move || {
                        crate::utils::clipboard::copy_image_from_data_url(&data_url);
                    });
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

        // Setup JS context menu handler using the exported function.
        // Wait for window.Arto to be available because the main JS module
        // is loaded asynchronously (async import in App) and may not have
        // resolved yet when this effect first fires on initial render.
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            (async () => {
                while (!window.Arto?.contextMenu) {
                    await new Promise(r => setTimeout(r, 50));
                }
                window.Arto.contextMenu.setup((data) => {
                    dioxus.send(data);
                });
            })();
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
