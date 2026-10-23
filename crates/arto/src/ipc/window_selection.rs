use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::window::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenTarget<T> {
    ExistingWindow(T),
    NewWindow,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WindowSelectionInput<T> {
    pub(super) window_id: T,
    pub(super) is_on_current_screen: bool,
}

pub(super) fn choose_open_target<T: Copy + Eq>(
    behavior: crate::config::FileOpenBehavior,
    visible_windows: &[WindowSelectionInput<T>],
    last_focused: Option<T>,
) -> OpenTarget<T> {
    match behavior {
        crate::config::FileOpenBehavior::NewWindow => OpenTarget::NewWindow,
        crate::config::FileOpenBehavior::LastFocused => match last_focused {
            Some(window_id)
                if visible_windows
                    .iter()
                    .any(|window| window.window_id == window_id) =>
            {
                OpenTarget::ExistingWindow(window_id)
            }
            _ => OpenTarget::NewWindow,
        },
        crate::config::FileOpenBehavior::CurrentScreen => {
            let current_screen_windows: Vec<T> = visible_windows
                .iter()
                .filter(|window| window.is_on_current_screen)
                .map(|window| window.window_id)
                .collect();

            if current_screen_windows.is_empty() {
                return OpenTarget::NewWindow;
            }

            if let Some(window_id) = last_focused {
                if current_screen_windows.contains(&window_id) {
                    return OpenTarget::ExistingWindow(window_id);
                }
            }

            OpenTarget::ExistingWindow(current_screen_windows[0])
        }
    }
}

pub(super) fn select_target_window() -> Option<WindowId> {
    let behavior = crate::config::CONFIG.read().file_open;
    select_target_window_with_behavior(behavior)
}

pub(super) fn select_target_window_with_behavior(
    behavior: crate::config::FileOpenBehavior,
) -> Option<WindowId> {
    let cursor_display_bounds = crate::utils::screen::get_current_display_bounds();
    let visible_windows: Vec<WindowSelectionInput<WindowId>> =
        collect_visible_window_selection_inputs(cursor_display_bounds);
    let last_focused = crate::window::main::get_last_focused_window();

    match choose_open_target(behavior, &visible_windows, last_focused) {
        OpenTarget::ExistingWindow(window_id) => Some(window_id),
        OpenTarget::NewWindow => None,
    }
}

fn collect_visible_window_selection_inputs(
    current_display_bounds: Option<(LogicalPosition<i32>, LogicalSize<u32>)>,
) -> Vec<WindowSelectionInput<WindowId>> {
    let on_screen_bounds = get_on_screen_window_bounds();

    crate::window::main::list_visible_main_windows()
        .into_iter()
        .map(|ctx| {
            let window_number = get_window_number(&ctx.window);
            let is_on_current_screen = match (window_number, current_display_bounds) {
                (Some(wn), Some(display)) => on_screen_bounds
                    .get(&wn)
                    .map(|bounds| is_window_on_display(display, *bounds))
                    .unwrap_or(false),
                _ => false,
            };
            WindowSelectionInput {
                window_id: ctx.window.id(),
                is_on_current_screen,
            }
        })
        .collect()
}

/// Window bounds as reported by the window server (actual compositor position).
#[derive(Debug, Clone, Copy)]
pub(super) struct WindowBounds {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Get the NSWindow.windowNumber for a Tao window.
#[cfg(target_os = "macos")]
fn get_window_number(window: &dioxus::desktop::tao::window::Window) -> Option<i64> {
    use dioxus::desktop::tao::platform::macos::WindowExtMacOS;
    use objc2_app_kit::NSWindow;

    let ns_window_ptr = window.ns_window() as *const NSWindow;
    if ns_window_ptr.is_null() {
        return None;
    }
    // SAFETY: ns_window_ptr is checked for null and ns_window() returns
    // a valid NSWindow pointer for the lifetime of the Window.
    let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
    Some(ns_window.windowNumber() as i64)
}

#[cfg(not(target_os = "macos"))]
fn get_window_number(_window: &dioxus::desktop::tao::window::Window) -> Option<i64> {
    None
}

/// Query the window server for actual on-screen window bounds.
///
/// Uses `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly)` to get the
/// real compositor-level bounds. This reflects positions set by external tools
/// (e.g. Aerospace tiling WM) that move windows via the Accessibility API,
/// which may not be reflected in Tao's `outer_position()`.
#[cfg(target_os = "macos")]
fn get_on_screen_window_bounds() -> std::collections::HashMap<i64, WindowBounds> {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::string::CFString;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowListOptionOnScreenOnly,
        kCGWindowNumber,
    };

    let Some(info) = copy_window_info(kCGWindowListOptionOnScreenOnly, kCGNullWindowID) else {
        return std::collections::HashMap::new();
    };

    let wn_key = unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) };
    let bounds_key = unsafe { CFString::wrap_under_get_rule(kCGWindowBounds) };

    (0..info.len())
        .filter_map(|i| {
            let dict = unsafe {
                core_foundation::array::CFArrayGetValueAtIndex(info.as_concrete_TypeRef(), i)
            } as CFDictionaryRef;
            if dict.is_null() {
                return None;
            }
            let window_number = unsafe { cf_dict_get_number(dict, &wn_key) }?.to_i64()?;
            let rect = unsafe { cf_dict_get_value(dict, &bounds_key) }? as CFDictionaryRef;
            Some((window_number, unsafe { window_bounds_from_rect(rect) }))
        })
        .collect()
}

/// Read a `CGRect` the way the window list stores it: a dictionary of `X`,
/// `Y`, `Width` and `Height`, where a missing field reads as zero.
#[cfg(target_os = "macos")]
unsafe fn window_bounds_from_rect(
    rect: core_foundation::dictionary::CFDictionaryRef,
) -> WindowBounds {
    let field = |key: &str| -> f64 {
        let key = core_foundation::string::CFString::new(key);
        unsafe { cf_dict_get_number(rect, &key) }
            .and_then(|number| number.to_f64())
            .unwrap_or(0.0)
    };
    WindowBounds {
        x: field("X") as i32,
        y: field("Y") as i32,
        width: field("Width") as u32,
        height: field("Height") as u32,
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_dict_get_number(
    dict: core_foundation::dictionary::CFDictionaryRef,
    key: &core_foundation::string::CFString,
) -> Option<core_foundation::number::CFNumber> {
    use core_foundation::base::TCFType;

    let value = cf_dict_get_value(dict, key)?;
    Some(core_foundation::number::CFNumber::wrap_under_get_rule(
        value as core_foundation::number::CFNumberRef,
    ))
}

#[cfg(target_os = "macos")]
unsafe fn cf_dict_get_value(
    dict: core_foundation::dictionary::CFDictionaryRef,
    key: &core_foundation::string::CFString,
) -> Option<*const std::ffi::c_void> {
    use core_foundation::base::TCFType;

    let mut val: *const std::ffi::c_void = std::ptr::null();
    let found = core_foundation::dictionary::CFDictionaryGetValueIfPresent(
        dict,
        key.as_concrete_TypeRef() as *const _,
        &mut val,
    );
    if found != 0 && !val.is_null() {
        Some(val)
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn get_on_screen_window_bounds() -> std::collections::HashMap<i64, WindowBounds> {
    std::collections::HashMap::new()
}

/// Minimum overlap ratio (0.0–1.0) for a window to be considered "on" a display.
///
/// Aerospace hides windows by moving them to a corner with only ~1px inside the
/// display. A threshold of 10% rejects those hidden windows while still accepting
/// windows that legitimately span two monitors.
const MIN_OVERLAP_RATIO: f64 = 0.1;

/// Check if a window has significant overlap with a display.
///
/// Returns true when at least [`MIN_OVERLAP_RATIO`] of the window's area overlaps
/// the display. This correctly handles:
/// - Aerospace hiding windows in corners with ~1px overlap → rejected
/// - Windows spanning two monitors → accepted on whichever display they overlap
pub(super) fn is_window_on_display(
    display_bounds: (LogicalPosition<i32>, LogicalSize<u32>),
    bounds: WindowBounds,
) -> bool {
    let (display_origin, display_size) = display_bounds;
    let display_left = display_origin.x;
    let display_top = display_origin.y;
    let display_right = display_left + display_size.width as i32;
    let display_bottom = display_top + display_size.height as i32;

    let window_left = bounds.x;
    let window_top = bounds.y;
    let window_right = bounds.x + bounds.width as i32;
    let window_bottom = bounds.y + bounds.height as i32;

    let overlap_left = window_left.max(display_left);
    let overlap_top = window_top.max(display_top);
    let overlap_right = window_right.min(display_right);
    let overlap_bottom = window_bottom.min(display_bottom);

    if overlap_left >= overlap_right || overlap_top >= overlap_bottom {
        return false;
    }

    let overlap_area =
        (overlap_right - overlap_left) as f64 * (overlap_bottom - overlap_top) as f64;
    let window_area = bounds.width as f64 * bounds.height as f64;

    if window_area == 0.0 {
        return false;
    }

    overlap_area / window_area >= MIN_OVERLAP_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileOpenBehavior;

    #[test]
    fn test_choose_open_target_new_window_always_creates_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: true,
        }];

        let target = choose_open_target(FileOpenBehavior::NewWindow, &windows, Some(1));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_choose_open_target_last_focused_uses_visible_window() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 1_u8,
                is_on_current_screen: false,
            },
            WindowSelectionInput {
                window_id: 2_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::LastFocused, &windows, Some(2));
        assert_eq!(target, OpenTarget::ExistingWindow(2));
    }

    #[test]
    fn test_choose_open_target_last_focused_falls_back_to_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: true,
        }];

        let target = choose_open_target(FileOpenBehavior::LastFocused, &windows, Some(2));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_choose_open_target_current_screen_prefers_last_focused() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 1_u8,
                is_on_current_screen: true,
            },
            WindowSelectionInput {
                window_id: 2_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, Some(2));
        assert_eq!(target, OpenTarget::ExistingWindow(2));
    }

    #[test]
    fn test_choose_open_target_current_screen_uses_first_candidate_without_last_focus() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 4_u8,
                is_on_current_screen: true,
            },
            WindowSelectionInput {
                window_id: 5_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, None);
        assert_eq!(target, OpenTarget::ExistingWindow(4));
    }

    #[test]
    fn test_choose_open_target_current_screen_falls_back_to_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: false,
        }];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, Some(1));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_is_window_on_display_fully_inside() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        let bounds = WindowBounds {
            x: 300,
            y: 250,
            width: 400,
            height: 300,
        };
        assert!(is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_spanning_two_monitors() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Window straddles the right edge: 50% on left monitor
        let bounds = WindowBounds {
            x: 1720,
            y: 200,
            width: 400,
            height: 300,
        };
        // overlap = 200*300 = 60000, window = 400*300 = 120000 → 50% > 10%
        assert!(is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_minor_overlap_rejected() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Only 20px overlap on a 400px-wide window → 5% < 10%
        let bounds = WindowBounds {
            x: 1900,
            y: 200,
            width: 400,
            height: 300,
        };
        // overlap = 20*300 = 6000, window = 400*300 = 120000 → 5% < 10%
        assert!(!is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_completely_outside() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        let bounds = WindowBounds {
            x: 1921,
            y: 0,
            width: 400,
            height: 300,
        };
        assert!(!is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_hidden_in_corner() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Simulates Aerospace hideInCorner: window at bottom-right with 1px overlap
        // overlap = 1*1 = 1, window = 1265*2109 → ~0.00004% < 10%
        let bounds = WindowBounds {
            x: 5119,
            y: 2132,
            width: 1265,
            height: 2109,
        };
        assert!(!is_window_on_display(display, bounds));
    }
}
