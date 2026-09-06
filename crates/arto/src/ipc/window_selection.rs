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
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowListOptionOnScreenOnly,
        kCGWindowNumber,
    };

    let mut result = std::collections::HashMap::new();

    let Some(info) = copy_window_info(kCGWindowListOptionOnScreenOnly, kCGNullWindowID) else {
        return result;
    };

    let wn_key = unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) };
    let bounds_key = unsafe { CFString::wrap_under_get_rule(kCGWindowBounds) };

    for i in 0..info.len() {
        let dict_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(info.as_concrete_TypeRef(), i)
        };
        if dict_ptr.is_null() {
            continue;
        }

        // Get window number
        let wn_cfnum = unsafe {
            cf_dict_get_value(
                dict_ptr as core_foundation::dictionary::CFDictionaryRef,
                &wn_key,
            )
        };
        let Some(wn_cfnum) = wn_cfnum else {
            continue;
        };
        let wn_cfnum = unsafe {
            CFNumber::wrap_under_get_rule(wn_cfnum as core_foundation::number::CFNumberRef)
        };
        let Some(window_number) = wn_cfnum.to_i64() else {
            continue;
        };

        // Get window bounds (CGRect stored as CFDictionary with X, Y, Width, Height)
        let bounds_ptr = unsafe {
            cf_dict_get_value(
                dict_ptr as core_foundation::dictionary::CFDictionaryRef,
                &bounds_key,
            )
        };
        let Some(bounds_ptr) = bounds_ptr else {
            continue;
        };
        let bounds_dict = bounds_ptr as core_foundation::dictionary::CFDictionaryRef;

        let get_f64 = |key: &str| -> f64 {
            let cf_key = CFString::new(key);
            let val = unsafe { cf_dict_get_value(bounds_dict, &cf_key) };
            val.and_then(|v| {
                let num = unsafe {
                    CFNumber::wrap_under_get_rule(v as core_foundation::number::CFNumberRef)
                };
                num.to_f64()
            })
            .unwrap_or(0.0)
        };

        result.insert(
            window_number,
            WindowBounds {
                x: get_f64("X") as i32,
                y: get_f64("Y") as i32,
                width: get_f64("Width") as u32,
                height: get_f64("Height") as u32,
            },
        );
    }

    result
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
