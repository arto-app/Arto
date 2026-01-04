use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::{window, Config, DesktopService, WeakDesktopContext, WindowBuilder};
use dioxus::prelude::*;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::assets::MAIN_STYLE;
use crate::components::app::{App, AppProps};
use crate::config::{WindowPositionOffset, CONFIG};
use crate::state::{Tab, LAST_FOCUSED_STATE};
use crate::theme::Theme;
use crate::utils::screen::get_current_display_bounds;

use super::child;
use super::index::build_custom_index;
use super::metrics::capture_window_metrics;
use super::settings;
use super::types::WindowMetrics;

const MAX_POSITION_SHIFT_ATTEMPTS: usize = 20;

/// Create base window config from parameters
/// This config can be further customized with .with_menu(), .with_custom_event_handler(), etc.
pub fn create_main_window_config(params: &CreateMainWindowConfigParams) -> Config {
    Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Arto")
                .with_position(params.position)
                .with_inner_size(params.size),
        )
        // Add main style in config. Otherwise the style takes time to load and
        // the window appears unstyled for a brief moment.
        .with_custom_head(indoc::formatdoc! {r#"<link rel="stylesheet" href="{MAIN_STYLE}">"#})
        // Use a custom index to set the initial theme correctly
        .with_custom_index(build_custom_index(params.theme))
}

/// Parameters for creating a new main window
pub struct CreateMainWindowConfigParams {
    pub directory: Option<PathBuf>, // Auto-detect from tab/file if None
    pub theme: Theme,               // The enum: Auto/Light/Dark
    pub sidebar_open: bool,
    pub sidebar_width: f64,
    pub sidebar_show_all_files: bool,
    pub size: LogicalSize<u32>,
    pub position: LogicalPosition<i32>,
    /// Skip position shifting for overlap avoidance.
    /// Used for preview windows during drag where exact cursor-relative position is required.
    pub skip_position_shift: bool,
}

impl CreateMainWindowConfigParams {
    /// Get default params from preferences
    /// Note: directory may be None (user hasn't set default_directory)
    pub fn from_preferences(is_first_window: bool) -> Self {
        let directory_pref = settings::get_directory_preference(is_first_window);
        let theme_pref = settings::get_theme_preference(is_first_window);
        let sidebar_pref = settings::get_sidebar_preference(is_first_window);
        let size_pref = settings::get_window_size_preference(is_first_window);
        let position_pref = settings::get_window_position_preference(is_first_window);

        Self {
            directory: directory_pref.directory,
            theme: theme_pref.theme,
            sidebar_open: sidebar_pref.open,
            sidebar_width: sidebar_pref.width,
            sidebar_show_all_files: sidebar_pref.show_all_files,
            size: size_pref.size,
            position: position_pref.position,
            skip_position_shift: false,
        }
    }
}

impl Default for CreateMainWindowConfigParams {
    fn default() -> Self {
        let is_first_window = !has_any_main_windows();
        Self::from_preferences(is_first_window)
    }
}

thread_local! {
    static MAIN_WINDOWS: RefCell<Vec<WeakDesktopContext>> = const { RefCell::new(Vec::new()) };
    static LAST_FOCUSED_WINDOW: RefCell<Option<WindowId>> = const { RefCell::new(None) };
}

/// List all active (upgraded) main window contexts
fn list_main_windows() -> Vec<Rc<DesktopService>> {
    MAIN_WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter_map(|w| w.upgrade())
            .collect()
    })
}

/// List all visible main window handles
///
/// Returns window handles for all visible main windows.
/// Callers can access window properties (id, title, position, size) via the handle.
pub fn list_visible_main_windows() -> Vec<Rc<DesktopService>> {
    list_main_windows()
        .into_iter()
        .filter(|ctx| ctx.window.is_visible())
        .collect()
}

pub fn register_main_window(handle: WeakDesktopContext) {
    MAIN_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|w| w.upgrade().is_some());
        if !windows.iter().any(|w| w.ptr_eq(&handle)) {
            windows.push(handle);
        }
    });
}

/// Checks if there are any visible main windows.
///
/// Note: With WindowCloseBehaviour::WindowHides, closed windows remain in memory
/// with valid weak references but are not visible. We must check visibility to
/// avoid sending events (e.g., FILE_OPEN_BROADCAST) to hidden windows, which would
/// be invisible to users.
pub fn has_any_main_windows() -> bool {
    !list_visible_main_windows().is_empty()
}

pub fn focus_last_focused_main_window() -> bool {
    if let Some(window_id) = get_last_focused_window() {
        // Resolve to parent window if the last focused was a child window
        let main_window_id = child::resolve_to_parent_window(window_id);

        list_main_windows()
            .into_iter()
            .find(|ctx| ctx.window.id() == main_window_id)
            .map(|ctx| {
                ctx.window.set_visible(true);
                ctx.window.set_focus();
                true
            })
            .unwrap_or(false)
    } else {
        false
    }
}

/// Focus a specific window by its ID
/// Returns true if the window was found and focused
///
/// Also updates `LAST_FOCUSED_WINDOW` so that `get_last_focused_window()`
/// returns the correct value for intersection priority.
pub fn focus_window(window_id: WindowId) -> bool {
    list_main_windows()
        .into_iter()
        .find(|ctx| ctx.window.id() == window_id)
        .map(|ctx| {
            ctx.window.set_focus();
            update_last_focused_window(window_id);
            true
        })
        .unwrap_or(false)
}

pub fn close_all_main_windows() {
    let windows = list_main_windows();
    windows.iter().for_each(|w| w.close());
    MAIN_WINDOWS.with(|w| w.borrow_mut().clear());
}

/// Core function: Create new main window with a tab
/// Returns the window handle (Rc<DesktopService>) for further operations
///
/// Directory resolution priority:
/// 1. params.directory (from config or user)
/// 2. tab.file().parent() (auto-detect from tab content)
/// 3. dirs::home_dir() (fallback)
/// 4. "/" (final fallback - always succeeds)
pub(crate) async fn create_new_main_window(
    tab: Tab,
    mut params: CreateMainWindowConfigParams,
) -> Rc<DesktopService> {
    // Resolve directory: params → tab parent → home dir → root (guaranteed to succeed)
    let directory = params
        .directory
        .take()
        .or_else(|| tab.file().and_then(|p| p.parent().map(|p| p.to_path_buf())))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    // Apply position shift based on existing windows (unless skip_position_shift is set)
    let shifted_position = if params.skip_position_shift {
        tracing::debug!(
            resolved_position=?params.position,
            "Position shift skipped (skip_position_shift=true)"
        );
        params.position
    } else {
        let position_offset = CONFIG.read().window_position.position_offset;
        let (screen_origin, screen_size) = get_current_display_bounds()
            .unwrap_or_else(|| (LogicalPosition::new(0, 0), LogicalSize::new(1000, 800)));
        let occupied = list_main_window_positions();
        let result = shift_position_if_needed(
            params.position,
            params.size,
            position_offset,
            screen_origin,
            screen_size,
            &occupied,
        );
        tracing::debug!(
            screen_size=?screen_size,
            position_offset=?position_offset,
            resolved_position=?params.position,
            shifted_position=?result,
            "Shifted position is calculated"
        );
        result
    };

    // Create VirtualDom with the provided tab and params
    let dom = VirtualDom::new_with_props(
        App,
        AppProps {
            tab,
            directory,
            theme: params.theme,
            sidebar_open: params.sidebar_open,
            sidebar_width: params.sidebar_width,
            sidebar_show_all_files: params.sidebar_show_all_files,
        },
    );

    // Override position with shifted position
    let params_with_shift = CreateMainWindowConfigParams {
        position: shifted_position,
        ..params
    };

    let config = create_main_window_config(&params_with_shift).with_menu(None); // To avoid child window taking over the main window's menu

    let pending = window().new_window(dom, config);
    let handle = pending.await;
    register_main_window(Rc::downgrade(&handle));

    handle
}

/// Convenience: Create window with file
pub async fn create_new_main_window_with_file(
    file: impl Into<PathBuf>,
    params: CreateMainWindowConfigParams,
) -> Rc<DesktopService> {
    let file = file.into();
    create_new_main_window(Tab::new(file), params).await
}

/// Convenience: Create window with empty tab
pub async fn create_new_main_window_with_empty(
    params: CreateMainWindowConfigParams,
) -> Rc<DesktopService> {
    create_new_main_window(Tab::default(), params).await
}

pub fn update_last_focused_window(window_id: WindowId) {
    LAST_FOCUSED_WINDOW.with(|last| *last.borrow_mut() = Some(window_id));
    if let Some(metrics) = find_window_metrics(window_id) {
        let mut last_focused = LAST_FOCUSED_STATE.write();
        last_focused.window_position = metrics.position;
        last_focused.window_size = metrics.size;
    }
}

pub(crate) fn get_last_focused_window() -> Option<WindowId> {
    LAST_FOCUSED_WINDOW.with(|last| *last.borrow())
}

/// Clear the last focused window if it matches the given window ID.
/// Called when a window is closed to prevent stale references.
pub fn clear_last_focused_if_matches(window_id: WindowId) {
    LAST_FOCUSED_WINDOW.with(|last| {
        let mut last = last.borrow_mut();
        if *last == Some(window_id) {
            *last = None;
        }
    });
}

fn find_window_metrics(window_id: WindowId) -> Option<WindowMetrics> {
    list_main_windows()
        .into_iter()
        .find(|ctx| ctx.window.id() == window_id)
        .map(|ctx| capture_window_metrics(&ctx.window))
}

fn list_main_window_positions() -> Vec<LogicalPosition<i32>> {
    list_main_windows()
        .iter()
        .map(|ctx| {
            let metrics = capture_window_metrics(&ctx.window);
            LogicalPosition::new(metrics.position.x, metrics.position.y)
        })
        .collect()
}

fn shift_position_if_needed(
    base: LogicalPosition<i32>,
    window_size: LogicalSize<u32>,
    offset: WindowPositionOffset,
    screen_origin: LogicalPosition<i32>,
    screen_size: LogicalSize<u32>,
    occupied: &[LogicalPosition<i32>],
) -> LogicalPosition<i32> {
    if offset.x == 0 && offset.y == 0 {
        return base;
    }
    let min_x = screen_origin.x;
    let min_y = screen_origin.y;
    let max_x = (screen_origin.x + screen_size.width as i32 - window_size.width as i32).max(min_x);
    let max_y =
        (screen_origin.y + screen_size.height as i32 - window_size.height as i32).max(min_y);
    let mut position = LogicalPosition::new(base.x.clamp(min_x, max_x), base.y.clamp(min_y, max_y));
    let mut offset_x = offset.x;
    let mut offset_y = offset.y;
    for attempt in 0..MAX_POSITION_SHIFT_ATTEMPTS {
        // Heuristic: avoid identical/nearby top-left positions rather than full rect overlap.
        let x_half = offset_x.abs().max(1) / 2;
        let y_half = offset_y.abs().max(1) / 2;
        let x_min = position.x - x_half;
        let x_max = position.x + x_half;
        let y_min = position.y - y_half;
        let y_max = position.y + y_half;
        if !occupied.iter().any(|existing| {
            existing.x >= x_min && existing.x <= x_max && existing.y >= y_min && existing.y <= y_max
        }) {
            break;
        }
        let mut next_x = position.x + offset_x;
        let mut next_y = position.y + offset_y;
        if next_x < min_x || next_x > max_x {
            offset_x = -offset_x;
            next_x = position.x + offset_x;
        }
        if next_y < min_y || next_y > max_y {
            offset_y = -offset_y;
            next_y = position.y + offset_y;
        }
        position = LogicalPosition::new(next_x.clamp(min_x, max_x), next_y.clamp(min_y, max_y));

        // Log warning if we've reached the limit
        if attempt == MAX_POSITION_SHIFT_ATTEMPTS - 1 {
            tracing::warn!(
                "Window position shift reached maximum attempts ({}), windows may overlap",
                MAX_POSITION_SHIFT_ATTEMPTS
            );
        }
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};

    #[test]
    fn test_shift_position_if_needed_no_offset() {
        let base = LogicalPosition::new(10, 10);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(100, 100),
            WindowPositionOffset { x: 0, y: 0 },
            LogicalPosition::new(0, 0),
            LogicalSize::new(500, 500),
            &[],
        );
        assert_eq!(result, base);
    }

    #[test]
    fn test_shift_position_if_needed_shifts_when_occupied() {
        let base = LogicalPosition::new(0, 0);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(50, 50),
            WindowPositionOffset { x: 20, y: 20 },
            LogicalPosition::new(0, 0),
            LogicalSize::new(200, 200),
            &[base],
        );
        assert_eq!(result, LogicalPosition::new(20, 20));
    }

    #[test]
    fn test_shift_position_if_needed_bounces_on_bounds() {
        let base = LogicalPosition::new(50, 50);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(50, 50),
            WindowPositionOffset { x: 20, y: 20 },
            LogicalPosition::new(0, 0),
            LogicalSize::new(100, 100),
            &[base],
        );
        assert_eq!(result, LogicalPosition::new(30, 30));
    }

    #[test]
    fn test_shift_position_if_needed_with_oversized_window_width() {
        let base = LogicalPosition::new(10, 10);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(500, 50),
            WindowPositionOffset { x: 20, y: 20 },
            LogicalPosition::new(0, 0),
            LogicalSize::new(100, 100),
            &[base],
        );
        assert_eq!(result, LogicalPosition::new(0, 30));
    }

    #[test]
    fn test_shift_position_if_needed_with_oversized_window() {
        let base = LogicalPosition::new(10, 10);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(500, 500),
            WindowPositionOffset { x: 20, y: 20 },
            LogicalPosition::new(0, 0),
            LogicalSize::new(100, 100),
            &[base],
        );
        assert_eq!(result, LogicalPosition::new(0, 0));
    }

    #[test]
    fn test_shift_position_if_needed_with_negative_origin() {
        let base = LogicalPosition::new(-240, 20);
        let result = shift_position_if_needed(
            base,
            LogicalSize::new(100, 100),
            WindowPositionOffset { x: 20, y: 20 },
            LogicalPosition::new(-300, -200),
            LogicalSize::new(200, 200),
            &[base],
        );
        assert_eq!(result, LogicalPosition::new(-240, -100));
    }
}
