use dioxus::desktop::window;
use dioxus::prelude::*;

use crate::events::{SidebarSide, SET_SIDEBAR_ZOOM_IN_WINDOW};
use crate::state::AppState;

/// Setup listeners for cross-window file/directory open events (from sidebar context menu)
pub(super) fn setup_cross_window_open_listeners(mut state: AppState) {
    let current_window_id = window().id();

    // A saved preference has to reach the windows already open. The
    // configuration is not a signal, so this is what makes anything derived
    // from it — the layout thresholds, the margin trace — redraw.
    use_future(move || async move {
        let mut rx = crate::config::CONFIG_CHANGED_BROADCAST.subscribe();
        while rx.recv().await.is_ok() {
            let next = state.config_revision.read().wrapping_add(1);
            state.config_revision.set(next);
        }
    });

    // Listen for sidebar zoom changes made in the preferences window
    use_future(move || async move {
        let mut rx = SET_SIDEBAR_ZOOM_IN_WINDOW.subscribe();

        while let Ok((target_window_id, side, zoom)) = rx.recv().await {
            if target_window_id == current_window_id {
                match side {
                    SidebarSide::Left => state.sidebar.write().zoom_level = zoom,
                }
            }
        }
    });

    // Listen for sidebar zoom changes made in the preferences window
    use_future(move || async move {
        let mut rx = SET_SIDEBAR_ZOOM_IN_WINDOW.subscribe();

        while let Ok((target_window_id, side, zoom)) = rx.recv().await {
            if target_window_id == current_window_id {
                match side {
                    SidebarSide::Left => state.sidebar.write().zoom_level = zoom,
                }
            }
        }
    });
}
