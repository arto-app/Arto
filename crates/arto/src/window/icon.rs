use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::WindowBuilder;
use std::cell::RefCell;
use std::collections::HashMap;

/// The application icon, embedded so window creation never touches the
/// filesystem (the bundled asset directory sits in a different place relative
/// to the binary on every platform, and a missing icon must not be able to fail
/// a window).
const ICON_PNG: &[u8] = include_bytes!("../../assets/Arto.png");

/// Edge length for the icon Windows draws in the title bar. Windows happily
/// scales an icon of any size into the 16px slot, but does it without
/// smoothing, so the source is pre-scaled here with a real filter instead.
#[cfg(target_os = "windows")]
const SMALL_ICON_SIZE: u32 = 32;

/// Edge length for every other slot: the taskbar and Alt+Tab switcher on
/// Windows, the window list and dock on Linux. These are drawn anywhere from
/// 32px to 256px depending on DPI and shell, so hand over the large size and
/// let the compositor scale down.
const LARGE_ICON_SIZE: u32 = 256;

thread_local! {
    /// Decoding costs a PNG parse, a rescale and a native icon handle, so keep
    /// each size for the lifetime of the process. `Icon` wraps a native handle
    /// and is not `Send`, hence a thread-local rather than a `OnceLock` — every
    /// window is created on the main thread anyway.
    static ICONS: RefCell<HashMap<u32, Option<Icon>>> = RefCell::new(HashMap::new());
}

/// Give a window the application icon.
///
/// Windows and Linux draw a generic placeholder unless the window carries an
/// icon of its own — the icon linked into the executable only covers Explorer,
/// not the running window. macOS has no per-window icons and ignores this; its
/// icon comes from the app bundle.
pub fn apply_app_icon(builder: WindowBuilder) -> WindowBuilder {
    #[cfg(target_os = "windows")]
    let builder = builder.with_window_icon(app_icon(SMALL_ICON_SIZE));
    #[cfg(not(target_os = "windows"))]
    let builder = builder.with_window_icon(app_icon(LARGE_ICON_SIZE));

    // On Windows the builder icon only fills the small (title bar) slot. The
    // taskbar and Alt+Tab switcher read the big one, a separate slot that tao
    // exposes only through the platform extension trait.
    #[cfg(target_os = "windows")]
    let builder = {
        use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
        builder.with_taskbar_icon(app_icon(LARGE_ICON_SIZE))
    };

    builder
}

/// The application icon rendered at `size` x `size`, or `None` if the embedded
/// PNG cannot be decoded — which leaves the window looking exactly as it did
/// before rather than failing the launch.
fn app_icon(size: u32) -> Option<Icon> {
    ICONS.with(|cache| {
        cache
            .borrow_mut()
            .entry(size)
            .or_insert_with(|| load_app_icon(size))
            .clone()
    })
}

fn load_app_icon(size: u32) -> Option<Icon> {
    let image = match image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png) {
        Ok(image) => image,
        Err(err) => {
            tracing::warn!("Failed to decode the application icon: {err}");
            return None;
        }
    };
    let rgba = image
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();

    match Icon::from_rgba(rgba.into_raw(), size, size) {
        Ok(icon) => Some(icon),
        Err(err) => {
            tracing::warn!("Failed to build the application icon: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_decodes_at_every_size() {
        #[cfg(target_os = "windows")]
        assert!(app_icon(SMALL_ICON_SIZE).is_some());
        assert!(app_icon(LARGE_ICON_SIZE).is_some());
    }
}
