use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::context_menu::ContextMenuData;
use super::context_menu_state::{open_context_menu, ContentContextMenuState};
use crate::baselines::Change;
use crate::config::CONFIG;
use crate::document_link::{open_document_link, scroll_to_heading_js, LinkOpen};
use crate::highlights::page::{show_js, take_up, use_page_highlights};
use crate::lenses::RenderedSource;
use crate::markdown::render_to_html_with_toc;
use crate::scroll_anchor::ScrollAnchor;
use crate::state::{AppState, DocumentContent};
use crate::utils::file::is_markdown_file;
use crate::watcher::FILE_WATCHER;

/// Data structure for markdown link clicks from JavaScript
#[derive(Serialize, Deserialize)]
struct LinkClickData {
    path: String,
    button: u32,
    /// Where the reader was when they clicked, so that going back lands there
    scroll_anchor: ScrollAnchor,
}

/// Mouse button constants
const LEFT_CLICK: u32 = 0;
const MIDDLE_CLICK: u32 = 1;

/// Jump to the top of a document that has just been replaced.
///
/// It goes through the renderer rather than scrolling `.content` directly so
/// that a destination still being held from the previous document is given up
/// (see `frontend/src/scroll-destination.ts`); the raw scroll is the fallback
/// for the moment before the renderer module has finished loading.
const SCROLL_RESET_JS: &str = "if (window.Arto?.scroll?.reset) { window.Arto.scroll.reset(); } \
     else { document.querySelector('.content')?.scrollTo(0, 0); }";

/// The directory relative links in `file` resolve against.
fn base_dir_of(file: &Path) -> PathBuf {
    file.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `file` is a `ReadSignal` so the hooks below re-run when the parent passes
/// a different path: reading it inside an effect subscribes the effect, which
/// is what `use_reactive!` used to emulate for a plain value.
#[component]
pub fn FileViewer(file: ReadSignal<PathBuf>) -> Element {
    let state = use_context::<AppState>();
    let html = use_signal(String::new);

    // Setup component hooks
    use_file_loader(file, html, state);
    use_file_watcher(file, state);
    use_changes_on_config(state);
    use_link_click_handler(file, state);
    use_mermaid_window_handler();
    use_math_window_handler();
    use_image_window_handler();
    use_clipboard_handlers();
    use_context_menu_handler(file);
    use_page_highlights(file, state);
    // A page that is gone has no source; the lens following it closes. It
    // has also been read: the window closed on it, or the welcome page took
    // its place.
    use_drop(move || {
        state.keep_read_version(crate::baselines::Moment::Left);
        let mut source = state.rendered_source;
        if let Ok(mut rendered) = source.try_write() {
            *rendered = None;
        };
        let mut profile = state.reading_profile;
        if let Ok(mut profile) = profile.try_write() {
            *profile = None;
        };
        // Nor has it highlights, which the contents would otherwise go on
        // listing.
        let mut highlights = state.highlights;
        if let Ok(mut kept) = highlights.try_write() {
            kept.clear();
        };
    });

    // The render the page shows, which lenses check the blocks they are
    // offered against: right after a re-render the page may briefly still
    // hold the previous one.
    let generation = state
        .rendered_source
        .read()
        .as_ref()
        .map(|rendered| rendered.generation.to_string());

    // A saved preference changes only this attribute: the article's inner
    // HTML is not rendered again, so the text is reset in place and the
    // reader stays where they were.
    let typography = use_memo(move || {
        let _ = state.config_revision.read();
        CONFIG.read().typography.css_declarations()
    });

    rsx! {
        div {
            class: "markdown-viewer",
            class: if *state.content_full_width.read() { "full-width" },
            style: "{typography}",
            article {
                class: "markdown-body",
                "data-render-generation": generation,
                dangerous_inner_html: "{html}"
            }
            // Where the marks of what changed since last read are drawn,
            // beside the page rather than in it: a table or a code block
            // clips anything drawn outside it. Rendered here, not by the page,
            // so it is not a stranger among the elements this component owns;
            // `frontend/src/changes.ts` fills it.
            div { "data-arto-change-marks": "true", "aria-hidden": "true" }
            // Context menu is rendered at App level to avoid re-rendering content
        }
    }
}

/// Hook to load and render file content
fn use_file_loader(file: ReadSignal<PathBuf>, html: Signal<String>, mut state: AppState) {
    use_effect(move || {
        let file = file();
        let mut html = html;
        // Reading reload_trigger subscribes this effect to it, so a manual
        // reload or a file-watcher event re-runs the load as well.
        let _ = state.reload_trigger.read();

        // Handle scroll position SYNCHRONOUSLY before spawning async task.
        // This ensures the onRenderComplete callback is registered before
        // MutationObserver triggers #executeBatchRender().
        handle_scroll_anchor(&mut state);

        spawn(async move {
            tracing::info!("Loading and rendering file: {:?}", &file);

            // Try to read as string (UTF-8 text file)
            match tokio::fs::read_to_string(file.as_path()).await {
                Ok(content) => {
                    // Check if file has markdown extension
                    if is_markdown_file(&file) {
                        // Render as markdown with TOC heading extraction
                        match render_to_html_with_toc(&content, &file) {
                            Ok(rendered) => {
                                html.set(rendered.html);
                                state.headings.set(rendered.headings);
                                state.reading_profile.set(Some(Arc::new(rendered.reading)));
                                let source = RenderedSource::new(file.clone(), content);
                                let generation = source.generation;
                                let text = source.source.clone();
                                state.rendered_source.set(Some(source));
                                tracing::trace!("Rendered as Markdown: {:?}", &file);
                                show_changes(state, file.clone(), text, generation).await;
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
                                state.headings.set(Vec::new());
                                state.rendered_source.set(None);
                                state.reading_profile.set(None);
                                forget_changes(state);
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
                        state.headings.set(Vec::new());
                        state.rendered_source.set(None);
                        state.reading_profile.set(None);
                        forget_changes(state);
                    }

                    // The reader's highlights go on first: the search marks
                    // are drawn around them. Only a rendered document has
                    // any; plain text is one code block, which is not marked.
                    let generation = state
                        .rendered_source
                        .peek()
                        .as_ref()
                        .map(|rendered| rendered.generation);
                    let highlights = take_up(state, generation.and(Some(file.as_path())));
                    let show = show_js(&file, &highlights, generation);

                    // Re-apply search highlighting after content changes
                    // This preserves search state across document changes
                    reapply_search(&show).await;
                }
                Err(e) => {
                    // Failed to read as UTF-8 text (likely binary file)
                    tracing::error!("Failed to read file {:?} as text: {}", file, e);
                    let error_msg = format!("{:?}", e);

                    // Report the failure as the document's content
                    let file_clone = file.clone();
                    state.update_document(move |document| {
                        document.content = DocumentContent::FileError(file_clone, error_msg);
                    });
                    state.rendered_source.set(None);
                    state.reading_profile.set(None);
                    forget_changes(state);
                    html.set(String::new());
                }
            }
        });
    });
}

/// Mark the changes again when the configuration changes, so that turning
/// them off or on, or ignoring spacing, answers on the page already open
/// rather than on the next document.
fn use_changes_on_config(state: AppState) {
    use_effect(move || {
        let _ = state.config_revision.read();
        let Some(rendered) = state.rendered_source.peek().clone() else {
            return;
        };
        spawn(async move {
            show_changes(state, rendered.path, rendered.source, rendered.generation).await;
        });
    });
}

/// Mark on the page what changed in `file` since it was last read, for the
/// render `generation` drawn from `source`.
///
/// Reading the version last read and comparing it is file work and a diff,
/// so it runs off the UI thread; by the time it is done the reader may have
/// moved on, and a result for a render no longer on screen is dropped.
async fn show_changes(mut state: AppState, file: PathBuf, source: Arc<str>, generation: u64) {
    let reading = CONFIG.read().reading.clone();
    let comparison = if reading.show_changes {
        tokio::task::spawn_blocking(move || {
            // Turned off while this waited for a thread: keep nothing.
            if !CONFIG.read().reading.show_changes {
                return crate::baselines::Comparison::default();
            }
            crate::baselines::compare(&file, &source, reading.ignore_whitespace_changes)
        })
        .await
        .unwrap_or_default()
    } else {
        crate::baselines::Comparison::default()
    };
    let current = state
        .rendered_source
        .peek()
        .as_ref()
        .map(|rendered| rendered.generation);
    // Another document, or the configuration changed again: a later call
    // answers for what is on screen now.
    if current != Some(generation) || CONFIG.read().reading != reading {
        return;
    }
    let read_at = comparison.read_at.map(|read_at| read_at.timestamp_millis());
    let js = set_changes_js(&comparison.changes, generation, read_at);
    state.changes.set(comparison.changes);
    let _ = document::eval(&js);
}

/// Take the marks of what changed off the page, now showing something that is
/// not a Markdown render: the layer they are drawn in outlives the page's own
/// elements, and nothing else would clear it.
fn forget_changes(mut state: AppState) {
    state.changes.set(Vec::new());
    let _ = document::eval("window.Arto?.changes?.clear?.()");
}

/// Hand `changes` to the page for the render `generation`, with when the
/// version they are measured from was read (ms since the epoch) for the page
/// to say since when.
///
/// The renderer module is imported asynchronously, and the first document a
/// window opens can arrive before it has installed `window.Arto`; the call
/// waits for it rather than being lost.
fn set_changes_js(changes: &[Change], generation: u64, read_at: Option<i64>) -> String {
    let changes = serde_json::to_string(changes).unwrap_or_else(|_| "[]".to_string());
    let read_at = read_at.map_or_else(|| "null".to_string(), |ms| ms.to_string());
    format!(
        r#"(async () => {{
            for (let i = 0; i < 500 && !window.Arto?.changes; i++) {{
                await new Promise((resolve) => setTimeout(resolve, 10));
            }}
            window.Arto?.changes?.set({changes}, {generation}, {read_at});
        }})();"#
    )
}

/// Handle scroll position when navigating to a file.
///
/// If pending_scroll_anchor is set (from back/forward navigation),
/// restore that position in two phases:
/// 1. Immediately when DOM content changes (MutationObserver, before browser paint)
/// 2. After Mermaid/KaTeX rendering completes (adjusts for content height changes)
///
/// The value is an anchor rather than a pixel offset — a source line plus a
/// fraction of the block on that line, see `frontend/src/scroll-anchor.ts` —
/// so the two phases can disagree about how tall the document is and still
/// land on the same content.
///
/// A pending fragment (from a `file.md#heading` link) wins over both: the
/// heading is scrolled into view once the document is in the DOM, and again
/// after Mermaid/KaTeX rendering has settled the layout.
///
/// Otherwise, reset to top immediately (for new navigation like clicking a link).
fn handle_scroll_anchor(state: &mut AppState) {
    let pending_scroll = state.pending_scroll_anchor.take();
    let pending_fragment = state.pending_scroll_fragment.take();

    if let Some(fragment) = pending_fragment {
        let jump = scroll_to_heading_js(&fragment);
        let fragment_js = format!(
            r#"(() => {{
                const jump = () => {{ {jump} }};
                const container = document.querySelector('.markdown-body');
                let observer;
                if (container) {{
                    observer = new MutationObserver(() => {{
                        if (observer) {{
                            observer.disconnect();
                            observer = null;
                        }}
                        jump();
                    }});
                    observer.observe(container, {{ childList: true }});
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
                    jump();
                }});
            }})();"#
        );
        let _ = document::eval(&fragment_js);
        tracing::debug!(fragment, "Scheduled scroll to heading after render");
        return;
    }

    if let Some(scroll) = pending_scroll {
        // Fast path: scrolling to top doesn't need two-phase restoration
        if scroll.is_top() {
            let _ = document::eval(SCROLL_RESET_JS);
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
                        window.Arto.scroll.toAnchor(target);
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
                    window.Arto.scroll.toAnchor(target);
                }});
            }})();"#,
            serde_json::to_string(&scroll).unwrap_or_else(|_| "null".to_string())
        );
        let _ = document::eval(&scroll_js);
        tracing::debug!(?scroll, "Scheduled two-phase scroll position restoration");
    } else {
        // Reset to top immediately for new navigation
        let _ = document::eval(SCROLL_RESET_JS);
        tracing::debug!("Reset scroll position to top");
    }
}

/// Re-apply search highlighting after DOM changes, after running `before`
/// (the script that draws the reader's highlights).
/// This is called after content rendering to preserve search state across
/// document changes.
async fn reapply_search(before: &str) {
    // Use MutationObserver to detect when DOM is actually updated, then reapply.
    // This is more robust than RAF-based timing which is not guaranteed.
    //
    // Flow:
    // 1. html.set() marks signal dirty (Rust side)
    // 2. This function runs and sets up MutationObserver
    // 3. Dioxus updates DOM (innerHTML changes)
    // 4. MutationObserver fires → reapply() is called
    // 5. Fallback timeout ensures reapply even if no mutation detected
    let script = indoc::indoc! {r#"
        (() => {
            let called = false;
            const doReapply = () => {
                if (called) return;
                called = true;
                __DRAW_HIGHLIGHTS__
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
    "#}
    .replace("__DRAW_HIGHLIGHTS__", before);
    let _ = document::eval(&script).await;
}

/// Hook to watch file for changes and trigger reload
fn use_file_watcher(file: ReadSignal<PathBuf>, mut state: AppState) {
    use_effect(move || {
        let file = file();

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
                state.reload_document();
            }

            if let Err(e) = FILE_WATCHER.unwatch(file_path.clone()).await {
                tracing::error!(
                    "Failed to unregister file watcher for {:?}: {:?}",
                    file_path,
                    e
                );
            }
        });
    });
}

/// Hook to setup JavaScript handler for markdown link clicks
fn use_link_click_handler(file: ReadSignal<PathBuf>, state: AppState) {
    use_effect(move || {
        let file = file();
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleMarkdownLinkClick = (path, button) => {
                // Where to come back to, named by content rather than by
                // pixels; see `frontend/src/scroll-anchor.ts`.
                //
                // The document is clickable before the renderer module, which
                // is imported asynchronously, has installed `window.Arto`.
                // Asking it unguarded would throw inside the inline handler
                // that already called preventDefault, so the click would do
                // nothing at all; the top of the document is the right answer
                // that early anyway.
                const anchor = window.Arto?.scroll?.anchor?.() ?? { line: 0, fraction: 0 };
                dioxus.send({ path, button, scroll_anchor: anchor });
            };
        "#});

        let mut state_clone = state;

        spawn(async move {
            while let Ok(click_data) = eval_provider.recv::<LinkClickData>().await {
                handle_link_click(click_data, &file, &mut state_clone);
            }
        });
    });
}

/// Handle a markdown link click event
fn handle_link_click(click_data: LinkClickData, current_file: &Path, state: &mut AppState) {
    let LinkClickData {
        path,
        button,
        scroll_anchor,
    } = click_data;

    tracing::info!("Markdown link clicked: {} (button: {})", path, button);

    let how = match button {
        MIDDLE_CLICK => LinkOpen::NewWindow,
        LEFT_CLICK => LinkOpen::Here { scroll_anchor },
        _ => {
            tracing::debug!("Ignoring click with button: {}", button);
            return;
        }
    };
    open_document_link(state, current_file, &path, how);
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
fn use_context_menu_handler(file: ReadSignal<PathBuf>) {
    use_effect(move || {
        let file = file();
        let base_dir = base_dir_of(&file);

        // Setup JS context menu handler using the exported function
        // Wait for window.Arto to be available (init() is async)
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            (async () => {
                while (!window.Arto?.contextMenu?.setup) {
                    await new Promise(resolve => setTimeout(resolve, 10));
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
    });
}
