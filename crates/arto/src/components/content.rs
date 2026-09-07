mod context_menu;
mod context_menu_state;
mod file_error_view;
mod file_viewer;
mod inline_viewer;
mod no_file_view;
mod preferences_view;
mod search_handler;

use dioxus::prelude::*;

use crate::scroll_anchor::ScrollAnchor;
use crate::state::{AppState, TabContent};
use file_error_view::FileErrorView;
use file_viewer::FileViewer;
use inline_viewer::InlineViewer;
use no_file_view::NoFileView;
use preferences_view::PreferencesView;

// Re-export for menu system
pub use preferences_view::set_preferences_tab_to_about;

// Re-export context menu types for App-level rendering
pub use context_menu::ContentContextMenu;
pub use context_menu_state::{close_context_menu, CONTENT_CONTEXT_MENU};

// Re-export search handler for App-level setup
pub use search_handler::use_search_handler;

#[component]
pub fn Content() -> Element {
    let state = use_context::<AppState>();
    let zoom_level = state.zoom_level;

    // Memoize the current tab's content to prevent re-rendering when non-active tabs change.
    // Without this, any write to state.tabs (even for other tabs) would trigger a re-render
    // of Content and its children, potentially disrupting scroll position.
    let content = use_memo(move || state.current_tab().map(|tab| tab.content));

    // Use CSS zoom property for vector-based scaling (not transform: scale)
    // This ensures fonts and images remain sharp at any zoom level.
    // Applied to a wrapper INSIDE the scroll container (.content) rather than
    // on .content itself, because zoom on a scroll container causes WebKit to
    // miscalculate scrollHeight, producing extra blank space at the bottom.
    let zoom_style = format!("zoom: {};", zoom_level());

    // Set up scroll position tracking via JavaScript
    use_scroll_anchor_tracker(state);

    rsx! {
        div {
            class: "content",

            // Apply zoom wrapper to all content (user content gets zoomed, system UI doesn't need it but wrapper is harmless)
            div {
                style: "{zoom_style}",

                match content() {
                    Some(TabContent::File(file)) => {
                        rsx! { FileViewer { file } }
                    },
                    Some(TabContent::Inline(markdown)) => {
                        rsx! { InlineViewer { markdown } }
                    },
                    Some(TabContent::FileError(file, error)) => {
                        let filename = file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown file")
                            .to_string();
                        rsx! { FileErrorView { filename, error_message: error } }
                    },
                    Some(TabContent::Preferences) => {
                        rsx! { PreferencesView {} }
                    },
                    _ => rsx! { NoFileView {} },
                }
            }
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
                window.__artoScrollHandler = () => {
                    dioxus.send(window.Arto.scroll.anchor());
                };

                // Send scroll position on every scroll event
                // We send immediately to minimize latency for back/forward navigation
                content.addEventListener('scroll', window.__artoScrollHandler, { passive: true });

                // Send initial position
                dioxus.send(window.Arto.scroll.anchor());
            }
        "#});

        spawn(async move {
            while let Ok(scroll) = eval.recv::<ScrollAnchor>().await {
                state.current_scroll_anchor.set(scroll);
            }
        });
    });
}
