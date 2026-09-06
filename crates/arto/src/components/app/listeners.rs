use dioxus::desktop::window;
use dioxus::prelude::*;

use crate::events::{OPEN_DIRECTORY_IN_WINDOW, OPEN_FILE_IN_WINDOW};
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
                state.set_root_directory(path.clone());
                // Pin the sidebar so users can see the directory tree
                state.sidebar.write().pinned = true;
            }
        }
    });
}
