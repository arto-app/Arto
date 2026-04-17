use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use mouse_position::mouse_position::Mouse;
use std::path::PathBuf;

use crate::config::{
    normalize_zoom_level as normalize_sidebar_zoom_level, NewWindowBehavior, StartupBehavior,
    WindowDimension, WindowDimensionUnit, WindowPosition, WindowPositionMode, WindowSize, CONFIG,
};
use crate::state::{PersistedState, Position, SidebarPanel, Size};
use crate::theme::Theme;
use crate::utils::screen::{
    get_current_display_bounds, get_cursor_display, get_display_bounds_for_logical_point,
    get_primary_display,
};
use crate::window::main::get_last_focused_window_state;

const MIN_WINDOW_DIMENSION: f64 = 100.0;

// ============================================================================
// Zoom Helpers
// ============================================================================

/// Normalize zoom level to the nearest 0.1 step to prevent precision drift
/// and ensure consistent behavior with menu zoom in/out actions.
pub fn normalize_zoom_level(zoom: f64) -> f64 {
    ((zoom * 10.0).round() / 10.0).clamp(0.5, 5.0)
}

// ============================================================================
// Preference Types
// ============================================================================

pub struct ThemePreference {
    pub theme: Theme,
}

pub struct DirectoryPreference {
    pub directory: Option<PathBuf>,
}

pub struct SidebarPreference {
    pub pinned: bool,
    pub panel: SidebarPanel,
    pub width: f64,
    pub show_all_files: bool,
    pub zoom_level: f64,
}

pub struct RightSidebarPreference {
    pub pinned: bool,
    pub width: f64,
    pub panel: SidebarPanel,
    pub zoom_level: f64,
}

pub struct WindowSizePreference {
    pub size: LogicalSize<u32>,
}

pub struct WindowPositionPreference {
    pub position: LogicalPosition<i32>,
}

pub struct ZoomPreference {
    pub zoom_level: f64,
}

#[derive(Debug, Clone, Copy)]
enum WindowPositionSource {
    Relative(WindowPosition),
    Absolute(Position),
}

// ============================================================================
// Helper Functions
// ============================================================================

fn choose_by_behavior<T>(
    is_first_window: bool,
    on_startup: StartupBehavior,
    on_new_window: NewWindowBehavior,
    default: impl FnOnce() -> T,
    last: impl FnOnce() -> T,
) -> T {
    if is_first_window {
        match on_startup {
            StartupBehavior::Default => default(),
            StartupBehavior::LastClosed => last(),
        }
    } else {
        match on_new_window {
            NewWindowBehavior::Default => default(),
            NewWindowBehavior::LastFocused => last(),
        }
    }
}

/// Resolve a preference by trying the last focused window's AppState first,
/// falling back to PersistedState.
///
/// **Fallback chain:** last focused window's `AppState` → `PersistedState` (state.json)
fn resolve_from_state_or_persisted<T>(
    from_state: impl FnOnce(&crate::state::AppState) -> T,
    from_persisted: impl FnOnce(&PersistedState) -> T,
) -> T {
    if let Some(state) = get_last_focused_window_state() {
        from_state(&state)
    } else {
        from_persisted(&PersistedState::load())
    }
}

fn resolve_window_size(config: WindowSize, max_size: LogicalSize<u32>) -> LogicalSize<u32> {
    let max_size_f64 = LogicalSize::new(max_size.width as f64, max_size.height as f64);
    let size = config.to_logical_size(&max_size_f64);
    let width = size
        .width
        .max(MIN_WINDOW_DIMENSION)
        .min(max_size.width as f64)
        .round() as u32;
    let height = size
        .height
        .max(MIN_WINDOW_DIMENSION)
        .min(max_size.height as f64)
        .round() as u32;
    LogicalSize::new(width, height)
}

fn clamp_window_position_to_display(
    position: LogicalPosition<i32>,
    screen_origin: LogicalPosition<i32>,
    screen_size: LogicalSize<u32>,
    window_size: LogicalSize<u32>,
) -> LogicalPosition<i32> {
    let available_width_u32 = screen_size.width.saturating_sub(window_size.width);
    let available_height_u32 = screen_size.height.saturating_sub(window_size.height);
    let available_width = available_width_u32.min(i32::MAX as u32) as i32;
    let available_height = available_height_u32.min(i32::MAX as u32) as i32;
    let max_x = screen_origin.x + available_width;
    let max_y = screen_origin.y + available_height;

    LogicalPosition::new(
        position.x.clamp(screen_origin.x, max_x),
        position.y.clamp(screen_origin.y, max_y),
    )
}

fn resolve_window_position(
    config: WindowPosition,
    screen_origin: LogicalPosition<i32>,
    screen_size: LogicalSize<u32>,
    window_size: LogicalSize<u32>,
) -> LogicalPosition<i32> {
    // Clamp to i32::MAX to prevent overflow when casting from u32
    let available_width_u32 = screen_size.width.saturating_sub(window_size.width);
    let available_height_u32 = screen_size.height.saturating_sub(window_size.height);
    let available_width = available_width_u32.min(i32::MAX as u32) as i32;
    let available_height = available_height_u32.min(i32::MAX as u32) as i32;
    let available_size = LogicalSize::new(available_width, available_height);
    let position = config.to_logical_position(available_size);
    let absolute_position =
        LogicalPosition::new(screen_origin.x + position.x, screen_origin.y + position.y);

    // Clamp position to ensure window stays on screen
    // This prevents off-screen windows when monitors are removed or repositioned
    clamp_window_position_to_display(absolute_position, screen_origin, screen_size, window_size)
}

fn resolve_absolute_window_position(
    position: Position,
    screen_origin: LogicalPosition<i32>,
    screen_size: LogicalSize<u32>,
    window_size: LogicalSize<u32>,
) -> LogicalPosition<i32> {
    clamp_window_position_to_display(
        LogicalPosition::new(position.x, position.y),
        screen_origin,
        screen_size,
        window_size,
    )
}

fn resolve_window_position_from_cursor(
    window_size: LogicalSize<u32>,
) -> Option<LogicalPosition<i32>> {
    let (x, y) = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x as f64, y as f64),
        Mouse::Error => return None,
    };
    let display = get_cursor_display().or_else(get_primary_display)?;
    #[cfg(target_os = "macos")]
    let (display_x, display_y, display_width, display_height) = (
        display.x as f64,
        display.y as f64,
        display.width as f64,
        display.height as f64,
    );

    #[cfg(not(target_os = "macos"))]
    let (display_x, display_y, display_width, display_height) = {
        let scale = display.scale_factor as f64;
        if scale <= 0.0 {
            return None;
        }
        (
            display.x as f64 / scale,
            display.y as f64 / scale,
            display.width as f64 / scale,
            display.height as f64 / scale,
        )
    };

    let (cursor_x, cursor_y) = (x, y);
    let window_width = window_size.width as f64;
    let window_height = window_size.height as f64;
    let max_x = (display_x + display_width - window_width).max(display_x);
    let max_y = (display_y + display_height - window_height).max(display_y);
    let clamped_x = cursor_x.clamp(display_x, max_x);
    let clamped_y = cursor_y.clamp(display_y, max_y);
    Some(LogicalPosition::new(
        clamped_x.round() as i32,
        clamped_y.round() as i32,
    ))
}

fn window_size_from_state(size: Size) -> WindowSize {
    WindowSize {
        width: WindowDimension {
            value: size.width as f64,
            unit: WindowDimensionUnit::Pixels,
        },
        height: WindowDimension {
            value: size.height as f64,
            unit: WindowDimensionUnit::Pixels,
        },
    }
}

fn has_persisted_window_position(persisted: &PersistedState) -> bool {
    persisted.window_position != Position::default()
}

fn has_persisted_window_size(persisted: &PersistedState) -> bool {
    persisted.window_size != Size::default()
}

fn should_restore_last_closed_position(
    is_first_window: bool,
    startup_behavior: StartupBehavior,
    default_position_mode: WindowPositionMode,
    persisted: &PersistedState,
) -> bool {
    if !is_first_window || !has_persisted_window_position(persisted) {
        return false;
    }

    matches!(startup_behavior, StartupBehavior::LastClosed)
        || matches!(startup_behavior, StartupBehavior::Default)
            && matches!(default_position_mode, WindowPositionMode::Coordinates)
}

fn should_restore_last_closed_size(
    is_first_window: bool,
    startup_behavior: StartupBehavior,
    persisted: &PersistedState,
) -> bool {
    is_first_window
        && has_persisted_window_size(persisted)
        && matches!(
            startup_behavior,
            StartupBehavior::LastClosed | StartupBehavior::Default
        )
}

/// Get window metrics from last focused window's AppState.
/// Uses WINDOW_STATES mapping for O(1) access instead of iterating windows.
fn get_last_focused_metrics() -> Option<(Position, Size)> {
    get_last_focused_window_state().map(|state| {
        let position = (*state.position.read()).into();
        let size = (*state.size.read()).into();
        (position, size)
    })
}

fn resolve_window_settings(
    is_first_window: bool,
) -> (WindowPositionSource, WindowPositionMode, WindowSize) {
    let cfg = CONFIG.read();
    let persisted = PersistedState::load();
    let restore_last_closed_position = should_restore_last_closed_position(
        is_first_window,
        cfg.window_position.on_startup,
        cfg.window_position.default_position_mode,
        &persisted,
    );
    let restore_last_closed_size =
        should_restore_last_closed_size(is_first_window, cfg.window_size.on_startup, &persisted);

    let position = if restore_last_closed_position {
        get_last_focused_metrics()
            .map(|(pos, _)| WindowPositionSource::Absolute(pos))
            .unwrap_or_else(|| WindowPositionSource::Absolute(persisted.window_position))
    } else {
        choose_by_behavior(
            is_first_window,
            cfg.window_position.on_startup,
            cfg.window_position.on_new_window,
            || WindowPositionSource::Relative(cfg.window_position.default_position),
            || {
                get_last_focused_metrics()
                    .map(|(pos, _)| WindowPositionSource::Absolute(pos))
                    .unwrap_or_else(|| WindowPositionSource::Absolute(persisted.window_position))
            },
        )
    };
    let position_mode = if restore_last_closed_position {
        WindowPositionMode::Coordinates
    } else {
        choose_by_behavior(
            is_first_window,
            cfg.window_position.on_startup,
            cfg.window_position.on_new_window,
            || cfg.window_position.default_position_mode,
            || WindowPositionMode::Coordinates,
        )
    };
    let size = if restore_last_closed_size {
        get_last_focused_metrics()
            .map(|(_, sz)| window_size_from_state(sz))
            .unwrap_or_else(|| window_size_from_state(persisted.window_size))
    } else {
        choose_by_behavior(
            is_first_window,
            cfg.window_size.on_startup,
            cfg.window_size.on_new_window,
            || cfg.window_size.default_size,
            || {
                get_last_focused_metrics()
                    .map(|(_, sz)| window_size_from_state(sz))
                    .unwrap_or_else(|| window_size_from_state(persisted.window_size))
            },
        )
    };

    tracing::debug!(
        is_first_window,
        startup_position_behavior = ?cfg.window_position.on_startup,
        startup_size_behavior = ?cfg.window_size.on_startup,
        restore_last_closed_position,
        restore_last_closed_size,
        position_mode = ?position_mode,
        position = ?position,
        size = ?size,
        persisted_position = ?persisted.window_position,
        persisted_size = ?persisted.window_size,
        "Resolved window settings"
    );

    (position, position_mode, size)
}

// ============================================================================
// Public API
// ============================================================================

pub fn get_theme_preference(is_first_window: bool) -> ThemePreference {
    let cfg = CONFIG.read();
    let theme = choose_by_behavior(
        is_first_window,
        cfg.theme.on_startup,
        cfg.theme.on_new_window,
        || cfg.theme.default_theme,
        || {
            resolve_from_state_or_persisted(
                |state| *state.current_theme.read(),
                |persisted| persisted.theme,
            )
        },
    );
    ThemePreference { theme }
}

pub fn get_directory_preference(is_first_window: bool) -> DirectoryPreference {
    let cfg = CONFIG.read();
    let directory: Option<PathBuf> = choose_by_behavior(
        is_first_window,
        cfg.directory.on_startup,
        cfg.directory.on_new_window,
        || cfg.directory.default_directory.clone(),
        || {
            get_last_focused_window_state()
                .and_then(|state| state.sidebar.read().root_directory.clone())
                .or_else(|| PersistedState::load().directory)
                .or_else(|| cfg.directory.default_directory.clone())
        },
    );
    DirectoryPreference { directory }
}

pub fn get_sidebar_preference(is_first_window: bool) -> SidebarPreference {
    let cfg = CONFIG.read();
    let pref = choose_by_behavior(
        is_first_window,
        cfg.sidebar.on_startup,
        cfg.sidebar.on_new_window,
        || SidebarPreference {
            pinned: cfg.sidebar.default_pinned,
            panel: cfg.sidebar.default_panel,
            width: cfg.sidebar.default_width,
            show_all_files: cfg.sidebar.default_show_all_files,
            zoom_level: cfg.sidebar.default_zoom_level,
        },
        || {
            resolve_from_state_or_persisted(
                |state| {
                    let sidebar = state.sidebar.read();
                    SidebarPreference {
                        pinned: sidebar.pinned,
                        panel: *state.left_sidebar_panel.read(),
                        width: sidebar.width,
                        show_all_files: sidebar.show_all_files,
                        zoom_level: sidebar.zoom_level,
                    }
                },
                |persisted| SidebarPreference {
                    pinned: persisted.sidebar_pinned,
                    panel: persisted.left_sidebar_panel,
                    width: persisted.sidebar_width,
                    show_all_files: persisted.sidebar_show_all_files,
                    zoom_level: persisted.sidebar_zoom_level,
                },
            )
        },
    );
    // Normalize zoom level to valid range with 0.1 step
    SidebarPreference {
        zoom_level: normalize_sidebar_zoom_level(pref.zoom_level),
        ..pref
    }
}

pub fn get_right_sidebar_preference(is_first_window: bool) -> RightSidebarPreference {
    let cfg = CONFIG.read();
    let pref = choose_by_behavior(
        is_first_window,
        cfg.right_sidebar.on_startup,
        cfg.right_sidebar.on_new_window,
        || RightSidebarPreference {
            pinned: cfg.right_sidebar.default_pinned,
            width: cfg.right_sidebar.default_width,
            panel: cfg.right_sidebar.default_panel,
            zoom_level: cfg.right_sidebar.default_zoom_level,
        },
        || {
            resolve_from_state_or_persisted(
                |state| RightSidebarPreference {
                    pinned: *state.right_sidebar_pinned.read(),
                    width: *state.right_sidebar_width.read(),
                    panel: *state.right_sidebar_panel.read(),
                    zoom_level: *state.right_sidebar_zoom_level.read(),
                },
                |persisted| RightSidebarPreference {
                    pinned: persisted.right_sidebar_pinned,
                    width: persisted.right_sidebar_width,
                    panel: persisted.right_sidebar_panel,
                    zoom_level: persisted.right_sidebar_zoom_level,
                },
            )
        },
    );
    // Normalize zoom level to valid range with 0.1 step
    RightSidebarPreference {
        zoom_level: normalize_sidebar_zoom_level(pref.zoom_level),
        ..pref
    }
}

pub fn get_zoom_preference(is_first_window: bool) -> ZoomPreference {
    let cfg = CONFIG.read();
    let zoom_level = choose_by_behavior(
        is_first_window,
        cfg.zoom.on_startup,
        cfg.zoom.on_new_window,
        || cfg.zoom.default_zoom_level,
        || {
            resolve_from_state_or_persisted(
                |state| *state.zoom_level.read(),
                |persisted| persisted.zoom_level,
            )
        },
    );
    // Normalize to 0.1 step grid and clamp to safe range
    ZoomPreference {
        zoom_level: normalize_zoom_level(zoom_level),
    }
}

pub fn get_window_size_preference(is_first_window: bool) -> WindowSizePreference {
    let (_, _, size) = resolve_window_settings(is_first_window);
    let (_, screen_size) = get_current_display_bounds()
        .unwrap_or_else(|| (LogicalPosition::new(0, 0), LogicalSize::new(1000, 800)));
    let resolved_size = resolve_window_size(size, screen_size);
    WindowSizePreference {
        size: resolved_size,
    }
}

pub fn get_window_position_preference(is_first_window: bool) -> WindowPositionPreference {
    let (position, position_mode, size) = resolve_window_settings(is_first_window);
    let (screen_origin, screen_size) = get_current_display_bounds()
        .unwrap_or_else(|| (LogicalPosition::new(0, 0), LogicalSize::new(1000, 800)));
    let resolved_size = resolve_window_size(size, screen_size);
    let resolved_position = match position_mode {
        WindowPositionMode::Coordinates => match position {
            WindowPositionSource::Relative(position) => {
                resolve_window_position(position, screen_origin, screen_size, resolved_size)
            }
            WindowPositionSource::Absolute(position) => {
                let persisted_point = LogicalPosition::new(position.x, position.y);
                let (origin, size) = get_display_bounds_for_logical_point(persisted_point)
                    .unwrap_or((screen_origin, screen_size));
                resolve_absolute_window_position(position, origin, size, resolved_size)
            }
        },
        WindowPositionMode::Mouse => resolve_window_position_from_cursor(resolved_size)
            .unwrap_or_else(|| LogicalPosition::new(0, 0)),
    };
    tracing::debug!(
        is_first_window,
        position_mode = ?position_mode,
        position = ?position,
        resolved_size = ?resolved_size,
        resolved_position = ?resolved_position,
        "Resolved window position preference"
    );
    WindowPositionPreference {
        position: resolved_position,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choose_by_behavior_startup_default() {
        let result = choose_by_behavior(
            true,
            StartupBehavior::Default,
            NewWindowBehavior::LastFocused,
            || "default_value",
            || "last_value",
        );
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_choose_by_behavior_startup_last_closed() {
        let result = choose_by_behavior(
            true,
            StartupBehavior::LastClosed,
            NewWindowBehavior::Default,
            || "default_value",
            || "last_value",
        );
        assert_eq!(result, "last_value");
    }

    #[test]
    fn test_choose_by_behavior_new_window_default() {
        let result = choose_by_behavior(
            false,
            StartupBehavior::LastClosed,
            NewWindowBehavior::Default,
            || "default_value",
            || "last_value",
        );
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_choose_by_behavior_new_window_last_focused() {
        let result = choose_by_behavior(
            false,
            StartupBehavior::Default,
            NewWindowBehavior::LastFocused,
            || "default_value",
            || "last_value",
        );
        assert_eq!(result, "last_value");
    }

    #[test]
    fn test_should_restore_last_closed_position_for_first_window_coordinates() {
        let persisted = PersistedState {
            window_position: Position { x: 120, y: 80 },
            ..Default::default()
        };

        assert!(should_restore_last_closed_position(
            true,
            StartupBehavior::Default,
            WindowPositionMode::Coordinates,
            &persisted,
        ));
    }

    #[test]
    fn test_should_not_restore_last_closed_position_for_mouse_mode() {
        let persisted = PersistedState {
            window_position: Position { x: 120, y: 80 },
            ..Default::default()
        };

        assert!(!should_restore_last_closed_position(
            true,
            StartupBehavior::Default,
            WindowPositionMode::Mouse,
            &persisted,
        ));
    }

    #[test]
    fn test_should_restore_last_closed_size_for_first_window() {
        let persisted = PersistedState {
            window_size: Size {
                width: 1200,
                height: 800,
            },
            ..Default::default()
        };

        assert!(should_restore_last_closed_size(
            true,
            StartupBehavior::Default,
            &persisted,
        ));
    }

    #[test]
    fn test_get_theme_preference_first_window() {
        let result = get_theme_preference(true);
        // Should return a ThemePreference
        assert!(matches!(
            result.theme,
            Theme::Auto | Theme::Light | Theme::Dark
        ));
    }

    #[test]
    fn test_get_theme_preference_new_window() {
        let result = get_theme_preference(false);
        // Should return a ThemePreference
        assert!(matches!(
            result.theme,
            Theme::Auto | Theme::Light | Theme::Dark
        ));
    }

    #[test]
    fn test_get_directory_preference_first_window() {
        let result = get_directory_preference(true);
        // Should return a DirectoryPreference (directory may be None)
        // We only check that it returns successfully
        let _ = result.directory;
    }

    #[test]
    fn test_get_directory_preference_new_window() {
        let result = get_directory_preference(false);
        // Should return a DirectoryPreference (directory may be None)
        // We only check that it returns successfully
        let _ = result.directory;
    }

    #[test]
    fn test_get_sidebar_preference_first_window() {
        let result = get_sidebar_preference(true);
        // Should return a SidebarPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.panel,
            SidebarPanel::Directory | SidebarPanel::Contents | SidebarPanel::Search
        ));
    }

    #[test]
    fn test_get_sidebar_preference_new_window() {
        let result = get_sidebar_preference(false);
        // Should return a SidebarPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.panel,
            SidebarPanel::Directory | SidebarPanel::Contents | SidebarPanel::Search
        ));
    }

    #[test]
    fn test_get_right_sidebar_preference_first_window() {
        let result = get_right_sidebar_preference(true);
        // Should return a RightSidebarPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.panel,
            SidebarPanel::Directory | SidebarPanel::Contents | SidebarPanel::Search
        ));
    }

    #[test]
    fn test_get_right_sidebar_preference_new_window() {
        let result = get_right_sidebar_preference(false);
        // Should return a RightSidebarPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.panel,
            SidebarPanel::Directory | SidebarPanel::Contents | SidebarPanel::Search
        ));
    }

    #[test]
    fn test_get_zoom_preference_first_window() {
        let result = get_zoom_preference(true);
        assert!(result.zoom_level > 0.0);
    }

    #[test]
    fn test_get_zoom_preference_new_window() {
        let result = get_zoom_preference(false);
        assert!(result.zoom_level > 0.0);
    }

    #[test]
    fn test_get_window_size_preference_first_window() {
        let result = get_window_size_preference(true);
        assert!(result.size.width > 0);
        assert!(result.size.height > 0);
    }

    #[test]
    fn test_get_window_size_preference_new_window() {
        let result = get_window_size_preference(false);
        assert!(result.size.width > 0);
        assert!(result.size.height > 0);
    }

    #[test]
    fn test_get_window_position_preference_first_window() {
        let result = get_window_position_preference(true);
        // Position can be any value including negative
        // We only check that it returns successfully
        let _ = result.position;
    }

    #[test]
    fn test_get_window_position_preference_new_window() {
        let result = get_window_position_preference(false);
        // Position can be any value including negative
        // We only check that it returns successfully
        let _ = result.position;
    }

    #[test]
    fn test_resolve_window_size_enforces_minimum() {
        let size = WindowSize {
            width: WindowDimension {
                value: 10.0,
                unit: WindowDimensionUnit::Pixels,
            },
            height: WindowDimension {
                value: 5.0,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        let resolved = resolve_window_size(size, LogicalSize::new(800, 600));
        assert_eq!(resolved.width, MIN_WINDOW_DIMENSION as u32);
        assert_eq!(resolved.height, MIN_WINDOW_DIMENSION as u32);
    }

    #[test]
    fn test_resolve_window_position_percent() {
        let position = WindowPosition {
            x: WindowDimension {
                value: 50.0,
                unit: WindowDimensionUnit::Percent,
            },
            y: WindowDimension {
                value: 25.0,
                unit: WindowDimensionUnit::Percent,
            },
        };
        let screen_origin = LogicalPosition::new(0, 0);
        let screen_size = LogicalSize::new(1000, 800);
        let window_size = LogicalSize::new(200, 100);
        let resolved = resolve_window_position(position, screen_origin, screen_size, window_size);
        assert_eq!(resolved.x, 400);
        assert_eq!(resolved.y, 175);
    }

    #[test]
    fn test_resolve_window_position_with_negative_origin() {
        let position = WindowPosition {
            x: WindowDimension {
                value: 10.0,
                unit: WindowDimensionUnit::Pixels,
            },
            y: WindowDimension {
                value: 20.0,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        let screen_origin = LogicalPosition::new(-300, -200);
        let screen_size = LogicalSize::new(800, 600);
        let window_size = LogicalSize::new(200, 100);
        let resolved = resolve_window_position(position, screen_origin, screen_size, window_size);
        assert_eq!(resolved.x, -290);
        assert_eq!(resolved.y, -180);
    }

    #[test]
    fn test_resolve_absolute_window_position_preserves_absolute_coordinates() {
        let position = Position { x: 1200, y: 160 };
        let screen_origin = LogicalPosition::new(1000, 0);
        let screen_size = LogicalSize::new(800, 600);
        let window_size = LogicalSize::new(400, 300);

        let resolved =
            resolve_absolute_window_position(position, screen_origin, screen_size, window_size);

        assert_eq!(resolved.x, 1200);
        assert_eq!(resolved.y, 160);
    }

    #[test]
    fn test_resolve_absolute_window_position_clamps_to_display() {
        let position = Position { x: 1700, y: 500 };
        let screen_origin = LogicalPosition::new(1000, 0);
        let screen_size = LogicalSize::new(800, 600);
        let window_size = LogicalSize::new(400, 300);

        let resolved =
            resolve_absolute_window_position(position, screen_origin, screen_size, window_size);

        assert_eq!(resolved.x, 1400);
        assert_eq!(resolved.y, 300);
    }

    #[test]
    fn test_resolve_window_size_clamps_to_screen() {
        let size = WindowSize {
            width: WindowDimension {
                value: 2000.0,
                unit: WindowDimensionUnit::Pixels,
            },
            height: WindowDimension {
                value: 1600.0,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        let resolved = resolve_window_size(size, LogicalSize::new(1200, 900));
        assert_eq!(resolved.width, 1200);
        assert_eq!(resolved.height, 900);
    }

    #[test]
    fn test_normalize_zoom_level_rounds_to_nearest_tenth() {
        assert_eq!(normalize_zoom_level(1.05), 1.1);
        assert_eq!(normalize_zoom_level(1.04), 1.0);
        assert_eq!(normalize_zoom_level(1.95), 2.0);
        assert_eq!(normalize_zoom_level(0.99), 1.0);
    }

    #[test]
    fn test_normalize_zoom_level_clamps_to_range() {
        assert_eq!(normalize_zoom_level(0.3), 0.5);
        assert_eq!(normalize_zoom_level(10.0), 5.0);
        assert_eq!(normalize_zoom_level(-1.0), 0.5);
    }

    #[test]
    fn test_normalize_zoom_level_preserves_aligned_values() {
        assert_eq!(normalize_zoom_level(1.0), 1.0);
        assert_eq!(normalize_zoom_level(1.5), 1.5);
        assert_eq!(normalize_zoom_level(2.0), 2.0);
    }
}
