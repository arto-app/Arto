pub use dioxus_sdk_window::theme::Theme as DioxusTheme;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl From<&str> for Theme {
    fn from(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::Auto,
        }
    }
}

pub fn resolve_theme(theme: Theme) -> DioxusTheme {
    match theme {
        // NOTE:
        // We cannot use dioxus_sdk_window::theme::get_theme here because
        // it requires a Dioxus runtime and cannot be called from outside
        // of Dioxus context (this runs while building a window's index
        // page). That's why we use the dark_light crate instead.
        Theme::Auto => match detect_system_mode() {
            Some(dark_light::Mode::Light) => DioxusTheme::Light,
            Some(dark_light::Mode::Dark) => DioxusTheme::Dark,
            Some(dark_light::Mode::Unspecified) | None => DioxusTheme::Light,
        },
        Theme::Light => DioxusTheme::Light,
        Theme::Dark => DioxusTheme::Dark,
    }
}

/// How long to wait for the system appearance probe before assuming Light.
const SYSTEM_MODE_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

/// Whether a probe thread is still waiting on the OS. At most one exists at
/// a time, so an unresponsive portal costs one stuck thread, not one per
/// window.
static SYSTEM_MODE_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Ask the OS whether it is in light or dark mode, with a bounded wait.
///
/// On Linux `dark_light::detect` performs a synchronous D-Bus round-trip to
/// the xdg-desktop-portal with no deadline of its own. Where the portal is
/// missing or unresponsive (minimal window managers, containers, headless
/// CI) that call would block, and this runs on the main thread every time a
/// window is created. Probe on a helper thread and give up after
/// [`SYSTEM_MODE_PROBE_TIMEOUT`]; a probe that errors, panics or never
/// answers all resolve to `None`. The helper thread is left to finish (or
/// stay stuck) on its own rather than holding up window creation, and while
/// it is outstanding no second probe is started.
fn detect_system_mode() -> Option<dark_light::Mode> {
    use std::sync::atomic::Ordering;

    if SYSTEM_MODE_PROBE_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::debug!("System appearance probe still pending; assuming Light");
        return None;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // catch_unwind so the in-flight flag is released even if the probe
        // panics; a failed send only means the caller already gave up waiting.
        let mode = std::panic::catch_unwind(|| dark_light::detect().ok())
            .ok()
            .flatten();
        SYSTEM_MODE_PROBE_IN_FLIGHT.store(false, Ordering::Release);
        let _ = tx.send(mode);
    });
    // `recv_timeout` also returns an error when the thread panicked, because
    // dropping the sender disconnects the channel.
    rx.recv_timeout(SYSTEM_MODE_PROBE_TIMEOUT).ok().flatten()
}
