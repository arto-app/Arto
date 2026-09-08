//! The title bar, on the one platform where the header can be it.
//!
//! Every window frame here belongs to the OS: the border, the shadow, the
//! resize edges, the window buttons. macOS is the only platform that will
//! hand over the *drawing* of the title bar without also handing over the
//! frame, so it is the only one where the header and the title bar are the
//! same strip. Windows and Linux keep their own title bar above the header,
//! and nothing in this module applies to them.

use dioxus::desktop::WindowBuilder;

/// Where the traffic lights sit, measured from the window's top-left.
///
/// The header is 50px tall (10px of padding either side of a 30px target), so
/// this puts the buttons on the same centre line as the glyphs beside them
/// rather than at the top of a title bar that is no longer drawn.
#[cfg(target_os = "macos")]
pub const TRAFFIC_LIGHT_INSET: (f64, f64) = (20.0, 18.0);

/// Let the header stand in for the title bar, where the platform allows it.
///
/// This is a drawing change, not a frame change: `titlebar_transparent` plus
/// `fullsize_content_view` extends the content view under the title bar and
/// stops AppKit painting over it, while the window itself keeps its standard
/// mask. AppKit still owns the traffic lights, the drag, the double-click
/// behaviour, resizing and the green button's full screen; the title text is
/// hidden because the breadcrumb already says what the window is showing.
pub fn apply_titlebar(builder: WindowBuilder) -> WindowBuilder {
    #[cfg(target_os = "macos")]
    let builder = {
        use dioxus::desktop::tao::dpi::LogicalPosition;
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

        builder
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
            .with_traffic_light_inset(LogicalPosition::new(
                TRAFFIC_LIGHT_INSET.0,
                TRAFFIC_LIGHT_INSET.1,
            ))
    };

    builder
}

/// Tell the page whether the header currently has traffic lights in it.
///
/// The header keeps its left end clear so the buttons have somewhere to sit —
/// but in full screen AppKit takes them away, and the gap would then be a hole
/// with nothing in it. tao reports no event for entering full screen, so the
/// resize that comes with it is the signal, and this is called from there.
///
/// A no-op off macOS, where the title bar was never ours to stand in for.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub fn sync_traffic_light_clearance(window: &dioxus::desktop::tao::window::Window) {
    #[cfg(target_os = "macos")]
    {
        use dioxus::desktop::tao::platform::macos::WindowExtMacOS;

        let clear = window.fullscreen().is_none() && !window.simple_fullscreen();
        let _ = dioxus::document::eval(&format!(
            "document.documentElement.classList.toggle('has-traffic-lights', {clear})"
        ));
    }
}
