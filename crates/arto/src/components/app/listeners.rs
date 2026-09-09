use dioxus::desktop::window;
use dioxus::prelude::*;

use crate::events::{
    OPEN_DIRECTORY_IN_WINDOW, OPEN_FILE_IN_WINDOW, SET_RIGHT_SIDEBAR_ZOOM_IN_WINDOW,
    SET_SIDEBAR_ZOOM_IN_WINDOW,
};
use crate::state::AppState;

/// Setup listeners for cross-window file/directory open events (from sidebar context menu)
pub(super) fn setup_cross_window_open_listeners(mut state: AppState) {
    let current_window_id = window().id();

    // Listen for "Open in Window" file events
    use_future(move || async move {
        let mut rx = OPEN_FILE_IN_WINDOW.subscribe();

        while let Ok((target_window_id, path)) = rx.recv().await {
            // Only handle if this window is the target
            if target_window_id == current_window_id {
                tracing::info!(?path, "Opening file from cross-window request");
                state.open_file(path);
            }
        }
    });

    // Listen for "Open in Window" directory events
    use_future(move || async move {
        let mut rx = OPEN_DIRECTORY_IN_WINDOW.subscribe();

        while let Ok((target_window_id, path)) = rx.recv().await {
            // Only handle if this window is the target
            if target_window_id == current_window_id {
                tracing::info!(?path, "Opening directory from cross-window request");
                state.add_root(&path);
                // Pin the sidebar so users can see the directory tree
                state.sidebar.write().pinned = true;
            }
        }
    });
}

/// Subscribe this window to what the preferences window changes about it.
///
/// The "Current Settings" sliders act on the window that opened preferences.
/// That window is no longer the one drawing them — preferences has a window of
/// its own and no `AppState` — so the value arrives as an event addressed to
/// this window.
pub(super) fn setup_preferences_listeners(mut state: AppState) {
    let current_window_id = window().id();

    use_future(move || async move {
        let mut rx = SET_SIDEBAR_ZOOM_IN_WINDOW.subscribe();
        while let Ok((target_window_id, zoom)) = rx.recv().await {
            if target_window_id == current_window_id {
                state.sidebar.write().zoom_level = zoom;
            }
        }
    });

    use_future(move || async move {
        let mut rx = SET_RIGHT_SIDEBAR_ZOOM_IN_WINDOW.subscribe();
        while let Ok((target_window_id, zoom)) = rx.recv().await {
            if target_window_id == current_window_id {
                state.right_sidebar.write().zoom_level = zoom;
            }
        }
    });
}
