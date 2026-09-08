//! Making the header behave like the title bar it stands in for on macOS.
//!
//! Everywhere else the OS still draws a title bar of its own above the
//! header, so there is nothing here to do and this hook does nothing.

#[cfg(target_os = "macos")]
use dioxus::desktop::window;
#[cfg(target_os = "macos")]
use dioxus::prelude::*;

/// Give the header the two gestures a title bar owes its window: drag it by
/// the empty space, and zoom it on a double click.
///
/// The empty space is worked out in the page rather than in Rust, because
/// only the DOM knows what is under the pointer — a click that landed on a
/// control, on the breadcrumb, or in a text selection is not a drag, and a
/// Rust-side handler on the header cannot tell those apart. The page decides
/// and says so; the window is moved from here, where the handle is.
pub fn use_titlebar_gestures() {
    #[cfg(target_os = "macos")]
    use_effect(move || {
        // The clearance the traffic lights need is only right while they are
        // there, so settle it as the window appears and again on every resize
        // (see `window::titlebar::sync_traffic_light_clearance`).
        crate::window::titlebar::sync_traffic_light_clearance(&window().window);

        spawn(async move {
            let mut eval = dioxus::document::eval(indoc::indoc! {r#"
                (() => {
                    // Anything a reader can act on, plus the text they might
                    // be selecting. Everything else in the header is title
                    // bar as far as the pointer is concerned.
                    const INTERACTIVE = 'button, a, input, select, textarea, [role="button"], .breadcrumb';

                    const isTitleBar = (event) => {
                        const header = document.querySelector('.header');
                        if (!header || !header.contains(event.target)) return false;
                        return !event.target.closest(INTERACTIVE);
                    };

                    document.addEventListener('mousedown', (event) => {
                        if (event.button !== 0) return;
                        if (event.detail > 1) return;
                        if (!isTitleBar(event)) return;
                        dioxus.send('drag');
                    });

                    document.addEventListener('dblclick', (event) => {
                        if (!isTitleBar(event)) return;
                        dioxus.send('zoom');
                    });
                })();
            "#});

            while let Ok(gesture) = eval.recv::<String>().await {
                let window = window();
                match gesture.as_str() {
                    "drag" => {
                        let _ = window.drag_window();
                    }
                    "zoom" => window.set_maximized(!window.is_maximized()),
                    _ => {}
                }
            }
        });
    });
}
