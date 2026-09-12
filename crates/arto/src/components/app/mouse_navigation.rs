use dioxus::document;
use dioxus::prelude::*;

use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::Action;
use crate::state::AppState;

/// Maximum readiness-poll attempts for the JS mouse API before giving up.
///
/// The listener ships inside the renderer bundle, which a cold WebView2 start
/// can spend several seconds parsing; each attempt sleeps 50 ms, so 200 ≈ 10 s
/// and 50 ≈ 2.5 s. Mirrors the keyboard interceptor's bound, for the same
/// reason.
const JS_MOUSE_READY_MAX_RETRIES: u32 = if cfg!(target_os = "windows") { 200 } else { 50 };

/// Which way the reader asked to go, as `frontend/src/mouse-navigation.ts`
/// spells it.
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum NavigationDirection {
    Back,
    Forward,
}

/// Bridge the mouse's back / forward side buttons to the history actions.
///
/// The buttons reach the page rather than the window: they arrive as a
/// `mousedown` inside the WebView, so the listener that reads them is in JS
/// and this is the half that turns a direction into the action the menu and
/// the keyboard already dispatch.
pub(super) fn setup_mouse_navigation(state: AppState) {
    use_hook(move || {
        spawn(async move {
            let mut eval = document::eval(&format!(
                r#"
            (async () => {{
                let retries = 0;
                while (!window.Arto?.mouse?.onNavigate && retries++ < {max}) {{
                    await new Promise(r => setTimeout(r, 50));
                }}
                if (!window.Arto?.mouse?.onNavigate) {{
                    console.error("Mouse navigation API not available after timeout");
                    return;
                }}
                window.Arto.mouse.onNavigate((direction) => {{
                    dioxus.send(direction);
                }});
            }})();
            "#,
                max = JS_MOUSE_READY_MAX_RETRIES,
            ));

            while let Ok(direction) = eval.recv::<NavigationDirection>().await {
                let action = match direction {
                    NavigationDirection::Back => Action::HistoryBack,
                    NavigationDirection::Forward => Action::HistoryForward,
                };
                dispatch_action(&action, state);
            }
        });
    });
}
