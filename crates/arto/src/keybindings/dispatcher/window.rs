//! The window itself: opening another, putting the document down, closing,
//! and the windows that open from what the cursor is on.

use dioxus::document;

use super::*;

pub(super) fn new_document(state: &mut AppState) {
    state.update_document(|document| *document = crate::state::Document::default());
}

pub(super) fn close_window() {
    dioxus::desktop::window().close();
}

pub(super) fn reload(state: &mut AppState) {
    let current = *state.reload_trigger.read();
    state.reload_trigger.set(current + 1);
}

pub(super) fn open_about(state: &mut AppState) {
    crate::components::content::set_preferences_tab_to_about();
    state.open_preferences();
}

/// Open a copy of this window beside it.
///
/// A duplicate is for reading the same thing two ways — the document, the
/// place in it, and the folders that were open to reach it — so it carries
/// all three, and the window's own look with them. Its own scroll position
/// is saved into the history first, which is what the copy then restores.
pub(super) fn duplicate_window(state: &mut AppState) {
    let anchor = *state.current_scroll_anchor.read();
    state.save_current_scroll_anchor(anchor);

    let document = state.document();
    let (temps, sidebar_pinned, sidebar_width, sidebar_show_all_files, sidebar_zoom_level) = {
        let sidebar = state.sidebar.read();
        (
            sidebar.roots.temps().to_vec(),
            sidebar.pinned,
            sidebar.width,
            sidebar.show_all_files,
            sidebar.zoom_level,
        )
    };

    let params = crate::window::CreateMainWindowConfigParams {
        directory: None,
        temps,
        theme: *state.current_theme.read(),
        content_full_width: *state.content_full_width.read(),
        sidebar_pinned,
        sidebar_width,
        sidebar_show_all_files,
        sidebar_zoom_level,
        zoom_level: *state.zoom_level.read(),
        ..crate::window::CreateMainWindowConfigParams::default()
    };
    crate::window::create_main_window_sync(&dioxus::desktop::window(), document, params);
}

pub(super) fn open_content_viewer_from_cursor(state: &AppState) {
    let theme = *state.current_theme.read();
    spawn_detached(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum ViewerTarget {
            Image { src: String, alt: Option<String> },
            Math { source: String },
            Mermaid { source: String },
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!(el instanceof HTMLElement)) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() || el.getAttribute('src') || '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({
                        kind: 'image',
                        src,
                        alt: el.getAttribute('alt'),
                    });
                    return;
                }

                if (
                    el.classList.contains('preprocessed-math-display') ||
                    el.classList.contains('preprocessed-math')
                ) {
                    const source = el.dataset.originalContent || '';
                    if (!source) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'math', source });
                    return;
                }

                if (el.classList.contains('preprocessed-mermaid')) {
                    const source = el.dataset.originalContent || '';
                    if (!source) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'mermaid', source });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })();
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<ViewerTarget>().await else {
            return;
        };

        match target {
            ViewerTarget::Image { src, alt } => {
                crate::window::open_or_focus_image_window(src, alt, theme);
            }
            ViewerTarget::Math { source } => {
                crate::window::open_or_focus_math_window(source, theme);
            }
            ViewerTarget::Mermaid { source } => {
                crate::window::open_or_focus_mermaid_window(source, theme);
            }
            ViewerTarget::None => {}
        }
    });
}
