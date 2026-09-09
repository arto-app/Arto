mod context_menu;
mod context_menu_state;
mod file_error_view;
mod file_viewer;
mod gutter;
mod inline_viewer;
mod no_file_view;
mod preferences_view;
mod search_handler;

use dioxus::prelude::*;

use crate::scroll_anchor::ScrollAnchor;
use crate::state::{AppState, DocumentContent};
use file_error_view::FileErrorView;
use file_viewer::FileViewer;
use gutter::ContentsGutter;
use inline_viewer::InlineViewer;
use no_file_view::NoFileView;

// Re-export for menu system
pub use preferences_view::{set_preferences_tab_to_about, PreferencesView};

// Re-export context menu types for App-level rendering
pub use context_menu::ContentContextMenu;
pub use context_menu_state::{close_context_menu, CONTENT_CONTEXT_MENU};

// Re-export search handler for App-level setup
pub use search_handler::use_search_handler;

#[component]
pub fn Content() -> Element {
    let state = use_context::<AppState>();
    let zoom_level = state.zoom_level;

    // Memoize the document's content so unrelated writes to the state do not
    // re-render Content and its children, which would disturb the scroll
    // position.
    let content = use_memo(move || state.document.read().content.clone());

    // Use CSS zoom property for vector-based scaling (not transform: scale)
    // This ensures fonts and images remain sharp at any zoom level.
    // Applied to a wrapper INSIDE the scroll container (.content) rather than
    // on .content itself, because zoom on a scroll container causes WebKit to
    // miscalculate scrollHeight, producing extra blank space at the bottom.
    let zoom_style = format!("zoom: {};", zoom_level());

    // Set up scroll position tracking via JavaScript
    use_scroll_anchor_tracker(state);

    let headings = state.headings;

    rsx! {
        div {
            class: "content-area",

        div {
            class: "content",

            // Apply zoom wrapper to all content (user content gets zoomed, system UI doesn't need it but wrapper is harmless)
            div {
                style: "{zoom_style}",

                match content() {
                    DocumentContent::File(file) => {
                        rsx! { FileViewer { file } }
                    },
                    DocumentContent::Inline(markdown) => {
                        rsx! { InlineViewer { markdown } }
                    },
                    DocumentContent::FileError(file, error) => {
                        let filename = file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown file")
                            .to_string();
                        rsx! { FileErrorView { filename, error_message: error } }
                    },
                    _ => rsx! { NoFileView {} },
                }
            }
        }

        // The contents live beside the document rather than in a panel of
        // their own: always there, 24px wide, and impossible to open by
        // accident because there is nothing to open.
        ContentsGutter { headings: headings() }
        }
    }
}

/// Hook to track scroll position via JavaScript and update state.
/// Uses a passive scroll listener that sends position updates to Rust.
fn use_scroll_anchor_tracker(mut state: AppState) {
    use_effect(move || {
        let mut eval = document::eval(indoc::indoc! {r#"
            // Set up scroll listener on .content element
            const content = document.querySelector('.content');
            if (content) {
                // Remove any existing listener to prevent duplicates
                if (window.__artoScrollHandler) {
                    content.removeEventListener('scroll', window.__artoScrollHandler);
                }

                // What travels is an anchor, not a pixel offset: the line at
                // the top of the view plus how far into that block it sits.
                // The document changes height after it appears — diagrams and
                // formulas are drawn as the reader reaches them — so a pixel
                // offset stops meaning the same place. See
                // `frontend/src/scroll-anchor.ts`.
                //
                // Coalesced to one frame: naming the anchor measures blocks,
                // and a scroll event arrives more often than the frames those
                // measurements are worth. The trailing frame still fires, so
                // where the reader stopped is always the last value sent.
                //
                // The listener outlives the wait for the renderer module,
                // which is imported asynchronously and installs `window.Arto`
                // only once it resolves. Until then there is no anchor to
                // name, and asking for one unguarded would throw on every
                // frame the reader scrolls before the app has finished
                // starting.
                const sendAnchor = () => {
                    const anchor = window.Arto?.scroll?.anchor?.();
                    if (anchor) {
                        dioxus.send(anchor);
                    }
                };

                let pendingAnchor = false;
                window.__artoScrollHandler = () => {
                    if (pendingAnchor) {
                        return;
                    }
                    pendingAnchor = true;
                    requestAnimationFrame(() => {
                        pendingAnchor = false;
                        sendAnchor();
                    });
                };

                content.addEventListener('scroll', window.__artoScrollHandler, { passive: true });

                // Send initial position
                sendAnchor();
            }
        "#});

        spawn(async move {
            while let Ok(scroll) = eval.recv::<ScrollAnchor>().await {
                state.current_scroll_anchor.set(scroll);
            }
        });
    });
}
