use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::{
    window, Config, DesktopService, WeakDesktopContext, WindowBuilder, WindowCloseBehaviour,
};
use dioxus::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::state::AppState;

use crate::assets::get_main_style_path;
use crate::components::app::{App, AppProps};
use crate::components::right_sidebar::RightSidebarTab;
use crate::config::{WindowPositionOffset, CONFIG};
use crate::state::Tab;
use crate::theme::Theme;
use crate::utils::screen::get_current_display_bounds;

use super::index::build_custom_index;
use super::metrics::capture_window_metrics;
use super::settings;

const MAX_POSITION_SHIFT_ATTEMPTS: usize = 20;

/// Create base window config from parameters
/// This config can be further customized with .with_menu(), .with_custom_event_handler(), etc.
pub fn create_main_window_config(params: &CreateMainWindowConfigParams) -> Config {
    #[cfg(windows)]
    let window_builder = WindowBuilder::new()
        .with_title("Arto")
        .with_position(params.position)
        .with_inner_size(params.size)
        .with_decorations(true)
        .with_transparent(false);

    #[cfg(not(windows))]
    let window_builder = WindowBuilder::new()
        .with_title("Arto")
        .with_position(params.position)
        .with_inner_size(params.size);

    Config::new()
        .with_window(window_builder)
        // Add main style in config. Otherwise the style takes time to load and
        // the window appears unstyled for a brief moment.
        .with_custom_head(indoc::formatdoc!(
            r#"<link rel="stylesheet" href="{}">"#,
            get_main_style_path()
        ))
        // Use a custom index to set the initial theme correctly
        .with_custom_index(build_custom_index(params.theme))
}

/// Parameters for creating a new main window
pub struct CreateMainWindowConfigParams {
    pub directory: Option<PathBuf>, // Auto-detect from tab/file if None
    pub theme: Theme,               // The enum: Auto/Light/Dark
    pub sidebar_pinned: bool,
    pub sidebar_width: f64,
    pub sidebar_show_all_files: bool,
    pub sidebar_zoom_level: f64,
    pub right_sidebar_pinned: bool,
    pub right_sidebar_width: f64,
    pub right_sidebar_tab: RightSidebarTab,
    pub right_sidebar_zoom_level: f64,
    pub zoom_level: f64,
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
        let right_sidebar_pref = settings::get_right_sidebar_preference(is_first_window);
        let zoom_pref = settings::get_zoom_preference(is_first_window);
        let size_pref = settings::get_window_size_preference(is_first_window);
        let position_pref = settings::get_window_position_preference(is_first_window);

        Self {
            directory: directory_pref.directory,
            theme: theme_pref.theme,
            sidebar_pinned: sidebar_pref.pinned,
            sidebar_width: sidebar_pref.width,
            sidebar_show_all_files: sidebar_pref.show_all_files,
            sidebar_zoom_level: sidebar_pref.zoom_level,
            right_sidebar_pinned: right_sidebar_pref.pinned,
            right_sidebar_width: right_sidebar_pref.width,
            right_sidebar_tab: right_sidebar_pref.tab,
            right_sidebar_zoom_level: right_sidebar_pref.zoom_level,
            zoom_level: zoom_pref.zoom_level,
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
    static WINDOW_STATES: RefCell<HashMap<WindowId, AppState>> = RefCell::new(HashMap::new());
}

/// List all active (upgraded) main window contexts
pub fn list_main_windows() -> Vec<Rc<DesktopService>> {
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

/// Show and focus the first hidden main window (typically the MainApp window)
///
/// Returns true if a hidden window was found and shown, false otherwise.
/// This is used when handling reopen events (e.g., dock clicks) to restore
/// hidden windows instead of creating new ones.
pub fn show_and_focus_hidden_window() -> bool {
    let all_windows = list_main_windows();
    let visible_window_ids: std::collections::HashSet<WindowId> = list_visible_main_windows()
        .iter()
        .map(|ctx| ctx.window.id())
        .collect();

    all_windows
        .into_iter()
        .find(|ctx| !visible_window_ids.contains(&ctx.window.id()))
        .map(|ctx| {
            ctx.window.set_visible(true);
            ctx.window.set_focus();
            update_last_focused_window(ctx.window.id());
            true
        })
        .unwrap_or(false)
}

pub fn close_all_main_windows() {
    let windows = list_main_windows();
    windows.iter().for_each(|w| w.close());
    // Do not clear MAIN_WINDOWS: the MainApp window is configured with
    // WindowCloseBehaviour::WindowHides, so close() will typically hide it
    // instead of destroying it, while other windows may be destroyed on close.
    // Dead entries are pruned naturally by register_main_window().
}

/// Shutdown all app windows and allow the event loop to exit.
///
/// This is intended for app-level termination (e.g. SIGINT/SIGTERM). It differs
/// from `close_all_main_windows()` by forcing all main windows to use
/// `WindowCloseBehaviour::WindowCloses` so the hidden MainApp window is actually
/// destroyed and the process can exit on last window close.
pub fn shutdown_all_windows() -> usize {
    super::child::close_all_child_windows();

    let windows = list_main_windows();
    windows.iter().for_each(|w| {
        w.set_close_behavior(WindowCloseBehaviour::WindowCloses);
        w.close();
    });
    windows.len()
}

// ============================================================================
// Shared helpers for window creation (used by both sync and async paths)
// ============================================================================

/// Resolve the directory for a new window.
///
/// Priority: params.directory → tab parent → home dir → root
fn resolve_directory(params_directory: Option<PathBuf>, tabs: &[Tab]) -> PathBuf {
    params_directory
        .or_else(|| {
            tabs.iter()
                .find_map(|tab| tab.file().and_then(|p| p.parent().map(|p| p.to_path_buf())))
        })
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Compute the shifted position for a new window, avoiding overlap with existing windows.
fn compute_shifted_position(params: &CreateMainWindowConfigParams) -> LogicalPosition<i32> {
    if params.skip_position_shift {
        tracing::debug!(
            resolved_position=?params.position,
            "Position shift skipped (skip_position_shift=true)"
        );
        return params.position;
    }

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
}

/// Build VirtualDom and Config for a new main window.
fn build_window_dom_and_config(
    tabs: Vec<Tab>,
    mut params: CreateMainWindowConfigParams,
) -> (VirtualDom, Config) {
    let directory = resolve_directory(params.directory.take(), &tabs);
    let shifted_position = compute_shifted_position(&params);

    let dom = VirtualDom::new_with_props(
        App,
        AppProps {
            tabs,
            directory,
            theme: params.theme,
            sidebar_pinned: params.sidebar_pinned,
            sidebar_width: params.sidebar_width,
            sidebar_show_all_files: params.sidebar_show_all_files,
            sidebar_zoom_level: params.sidebar_zoom_level,
            right_sidebar_pinned: params.right_sidebar_pinned,
            right_sidebar_width: params.right_sidebar_width,
            right_sidebar_tab: params.right_sidebar_tab,
            right_sidebar_zoom_level: params.right_sidebar_zoom_level,
            zoom_level: params.zoom_level,
        },
    );

    let params_with_shift = CreateMainWindowConfigParams {
        position: shifted_position,
        ..params
    };

    // with_menu(None) prevents child window from taking over the main window's menu
    let config = create_main_window_config(&params_with_shift).with_menu(None);

    (dom, config)
}

// ============================================================================
// Synchronous (fire-and-forget) window creation
// ============================================================================

/// Create a new main window synchronously (fire-and-forget).
///
/// The window is created by the Tao event loop on the next iteration.
/// The App component self-registers via `register_main_window()`.
///
/// IMPORTANT: Must be called on the main thread (event loop thread).
pub fn create_main_window_sync(
    desktop: &Rc<DesktopService>,
    tab: Tab,
    params: CreateMainWindowConfigParams,
) {
    create_main_window_sync_with_tabs(desktop, vec![tab], params);
}

/// Create a new main window synchronously with multiple initial tabs.
///
/// The window is created by the Tao event loop on the next iteration.
/// The App component self-registers via `register_main_window()`.
///
/// IMPORTANT: Must be called on the main thread (event loop thread).
pub fn create_main_window_sync_with_tabs(
    desktop: &Rc<DesktopService>,
    tabs: Vec<Tab>,
    params: CreateMainWindowConfigParams,
) {
    let (dom, config) = build_window_dom_and_config(tabs, params);

    // Fire-and-forget: PendingDesktopContext is dropped, but window still gets created.
    // new_window() synchronously pushes PendingWebview and sends NewWindow event.
    let _pending = desktop.new_window(dom, config);
}

/// Get any live main window's DesktopService.
///
/// Used to access `new_window()` from outside Dioxus component context
/// (e.g., from `with_custom_event_handler`). All DesktopService instances
/// share the same `SharedContext`, so any window works.
pub fn get_any_main_window() -> Option<Rc<DesktopService>> {
    MAIN_WINDOWS.with(|windows| windows.borrow().iter().find_map(|w| w.upgrade()))
}

// ============================================================================
// Async window creation (used by drag-and-drop etc.)
// ============================================================================

/// Create a new main window with a tab (async version).
/// Returns the window handle for further operations (e.g., drag preview).
pub(crate) async fn create_main_window(
    tab: Tab,
    params: CreateMainWindowConfigParams,
) -> Rc<DesktopService> {
    let (dom, config) = build_window_dom_and_config(vec![tab], params);

    let pending = window().new_window(dom, config);
    let handle = pending.await;

    handle
}

pub fn update_last_focused_window(window_id: WindowId) {
    LAST_FOCUSED_WINDOW.with(|last| *last.borrow_mut() = Some(window_id));
}

// ============================================================================
// WindowId → AppState mapping
// ============================================================================

/// Register AppState for a window.
/// Called when a window is created to enable direct state access.
pub fn register_window_state(window_id: WindowId, state: AppState) {
    WINDOW_STATES.with(|states| {
        states.borrow_mut().insert(window_id, state);
    });
}

/// Unregister AppState when window closes.
/// Called in use_drop to clean up the mapping.
pub fn unregister_window_state(window_id: WindowId) {
    WINDOW_STATES.with(|states| {
        states.borrow_mut().remove(&window_id);
    });
}

/// Get AppState by WindowId.
///
/// Windows are registered via `register_window_state()` during App component
/// initialization (in `use_context_provider`), and automatically unregistered
/// via `unregister_window_state()` when the window closes (in `use_drop`).
///
/// Returns None if the window is not registered in the WINDOW_STATES mapping.
pub fn get_window_state(window_id: WindowId) -> Option<AppState> {
    WINDOW_STATES.with(|states| states.borrow().get(&window_id).copied())
}

/// Get the last focused window's AppState.
///
/// This provides O(1) access to the last focused window's state via the
/// WINDOW_STATES mapping, enabling direct reads from AppState Signals.
///
/// Returns None if no window is focused or if the window is not registered.
pub fn get_last_focused_window_state() -> Option<AppState> {
    get_last_focused_window().and_then(get_window_state)
}

pub(crate) fn get_last_focused_window() -> Option<WindowId> {
    LAST_FOCUSED_WINDOW.with(|last| *last.borrow())
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
