//! Resolving the user's theme preference against the system appearance.
//!
//! The preference itself (`Theme`) is a configuration type and lives in
//! arto-config; this module turns `Auto` into light or dark, and reports the
//! system appearance to the components that follow it.

use dioxus::desktop::tao::event::{Event as TaoEvent, WindowEvent};
use dioxus::desktop::tao::window::Theme as TaoTheme;
use dioxus::desktop::{use_wry_event_handler, window};
use dioxus::prelude::*;

pub use crate::config::Theme;

/// A [`Theme`] with `Auto` already resolved: what actually gets rendered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResolvedTheme {
    #[default]
    Light,
    Dark,
}

impl ResolvedTheme {
    /// The name the frontend knows this theme by, in `data-theme` and in the
    /// detail of the `arto:theme-changed` event.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::fmt::Display for ResolvedTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn resolve_theme(theme: Theme) -> ResolvedTheme {
    match theme {
        // NOTE:
        // This also runs while building a window's index page, outside any
        // Dioxus or window context, so it cannot ask the window for its
        // appearance. That is why the dark_light crate is used here.
        Theme::Auto => match detect_system_mode() {
            Some(dark_light::Mode::Dark) => ResolvedTheme::Dark,
            Some(dark_light::Mode::Light | dark_light::Mode::Unspecified) | None => {
                ResolvedTheme::Light
            }
        },
        Theme::Light => ResolvedTheme::Light,
        Theme::Dark => ResolvedTheme::Dark,
    }
}

/// A signal of the system appearance, kept current while the window lives.
///
/// Components use this to render `Theme::Auto` without knowing how the
/// appearance is obtained. tao reports a change through `ThemeChanged` on
/// macOS and Windows; on Linux it never does, so there the signal keeps the
/// value it started with.
pub fn use_system_theme() -> ReadSignal<ResolvedTheme> {
    let mut theme = use_signal(current_system_theme);

    use_wry_event_handler(move |event, _| {
        if let TaoEvent::WindowEvent {
            event: WindowEvent::ThemeChanged(changed),
            window_id,
            ..
        } = event
        {
            // Every window registers this handler and tao delivers each
            // event to all of them, so only the addressed window reacts.
            if *window_id == window().id() {
                theme.set(from_tao(*changed));
            }
        }
    });

    use_hook(|| ReadSignal::new(theme))
}

/// The system appearance right now.
fn current_system_theme() -> ResolvedTheme {
    // tao knows the appearance on macOS and Windows, and it is the same
    // source the change events come from. Its Linux answer is guessed from
    // the GTK theme name and misses the portal setting, so ask the same
    // probe that window creation uses instead.
    #[cfg(target_os = "linux")]
    {
        resolve_theme(Theme::Auto)
    }
    #[cfg(not(target_os = "linux"))]
    {
        from_tao(window().theme())
    }
}

/// `tao::window::Theme` is `#[non_exhaustive]`; anything but dark renders light.
fn from_tao(theme: TaoTheme) -> ResolvedTheme {
    match theme {
        TaoTheme::Dark => ResolvedTheme::Dark,
        _ => ResolvedTheme::Light,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The frontend switches on these exact strings, in `data-theme` and in
    /// the `arto:theme-changed` detail.
    #[test]
    fn theme_names_match_the_frontend() {
        assert_eq!(ResolvedTheme::Light.as_str(), "light");
        assert_eq!(ResolvedTheme::Dark.as_str(), "dark");
    }

    #[test]
    fn explicit_preferences_do_not_consult_the_system() {
        assert_eq!(resolve_theme(Theme::Light), ResolvedTheme::Light);
        assert_eq!(resolve_theme(Theme::Dark), ResolvedTheme::Dark);
    }
}
