use dioxus::desktop::window;
use dioxus::prelude::*;

use crate::events::SET_SIDEBAR_ZOOM_IN_WINDOW;
use crate::state::AppState;

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
}
