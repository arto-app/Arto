use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;

use crate::events::SET_SIDEBAR_ZOOM_IN_WINDOW;
use crate::pinned_search::{PinnedSearch, PINNED_SEARCHES, PINNED_SEARCHES_CHANGED};
use crate::state::AppState;

/// Subscribe this window to what the preferences window changes about it.
///
/// The "Current Settings" sliders act on the window that opened preferences.
/// That window is no longer the one drawing them — preferences has a window of
/// its own and no `AppState` — so the value arrives as an event addressed to
/// this window.
pub(super) fn setup_preferences_listeners(mut state: AppState) {
    let current_window_id = window().id();

    setup_pinned_highlights();

    use_future(move || async move {
        let mut rx = SET_SIDEBAR_ZOOM_IN_WINDOW.subscribe();
        while let Ok((target_window_id, zoom)) = rx.recv().await {
            if target_window_id == current_window_id {
                state.sidebar.write().zoom_level = zoom;
            }
        }
    });
}

/// What the page needs to draw a pinned mark, as JSON.
///
/// Disabled marks are sent too: the contents still count them, and turning one
/// back on has to be a change of one field rather than a search run again.
fn pinned_json(searches: &[PinnedSearch]) -> String {
    let entries: Vec<String> = searches
        .iter()
        .map(|pinned| {
            format!(
                r#"{{"id":"{}","pattern":"{}","color":"{}","caseSensitive":{},"disabled":{}}}"#,
                pinned.id,
                pinned.pattern.replace('\\', "\\\\").replace('"', "\\\""),
                pinned.color.to_js_name(),
                pinned.case_sensitive,
                pinned.disabled
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

/// Keep the page's standing highlights in step with the marks.
///
/// Here rather than beside the list that draws them: the marks outlive the
/// find field, and the contents that list them are not drawn at every width.
/// What paints them has to be mounted for as long as the window is.
fn setup_pinned_highlights() {
    use_future(move || async move {
        // The page's search API arrives with the renderer bundle, which on a
        // cold start can be a second or two behind this.
        for _ in 0..50 {
            let mut ready =
                document::eval("dioxus.send(typeof window.Arto?.search?.setPinned === 'function')");
            if let Ok(true) = ready.recv::<bool>().await {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let paint = |searches: Vec<PinnedSearch>| async move {
            let js = format!("window.Arto.search.setPinned({});", pinned_json(&searches));
            let _ = document::eval(&js).await;
        };

        let marks = PINNED_SEARCHES.read().pinned_searches.clone();
        paint(marks).await;

        let mut rx = PINNED_SEARCHES_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            let marks = PINNED_SEARCHES.read().pinned_searches.clone();
            paint(marks).await;
        }
    });
}
