use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use mouse_position::mouse_position::Mouse;
use std::path::PathBuf;

use crate::components::right_sidebar::RightSidebarTab;
use crate::config::{
    NewWindowBehavior, StartupBehavior, WindowDimension, WindowDimensionUnit, WindowPosition,
    WindowPositionMode, WindowSize, CONFIG,
};
use crate::state::{PersistedState, Position, Size};
use crate::theme::Theme;
use crate::utils::screen::{get_current_display_bounds, get_cursor_display, get_primary_display};
use crate::window::main::get_last_focused_window_state;

const MIN_WINDOW_DIMENSION: f64 = 100.0;

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
    pub open: bool,
    pub width: f64,
    pub show_all_files: bool,
}

pub struct TocPreference {
    pub open: bool,
    pub width: f64,
    pub tab: RightSidebarTab,
}

pub struct WindowSizePreference {
    pub size: LogicalSize<u32>,
}

pub struct WindowPositionPreference {
    pub position: LogicalPosition<i32>,
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
    let max_x = screen_origin.x + available_width;
    let max_y = screen_origin.y + available_height;
    let clamped_x = absolute_position.x.clamp(screen_origin.x, max_x);
    let clamped_y = absolute_position.y.clamp(screen_origin.y, max_y);

    LogicalPosition::new(clamped_x, clamped_y)
}

fn resolve_window_position_from_cursor(
    window_size: LogicalSize<u32>,
) -> Option<LogicalPosition<i32>> {
    let (x, y) = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x as f64, y as f64),
        Mouse::Error => return None,
    };
    let display = get_cursor_display().or_else(get_primary_display)?;
    let scale = display.scale_factor as f64;
    if scale <= 0.0 {
        return None;
    }
    let display_x = display.x as f64 / scale;
    let display_y = display.y as f64 / scale;
    let display_width = display.width as f64 / scale;
    let display_height = display.height as f64 / scale;
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

fn window_position_from_state(position: Position) -> WindowPosition {
    WindowPosition {
        x: WindowDimension {
            value: position.x as f64,
            unit: WindowDimensionUnit::Pixels,
        },
        y: WindowDimension {
            value: position.y as f64,
            unit: WindowDimensionUnit::Pixels,
        },
    }
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
) -> (WindowPosition, WindowPositionMode, WindowSize) {
    let cfg = CONFIG.read();
    let position = choose_by_behavior(
        is_first_window,
        cfg.window_position.on_startup,
        cfg.window_position.on_new_window,
        || cfg.window_position.default_position,
        || {
            get_last_focused_metrics()
                .map(|(pos, _)| window_position_from_state(pos))
                .unwrap_or_else(|| {
                    window_position_from_state(PersistedState::load().window_position)
                })
        },
    );
    let position_mode = choose_by_behavior(
        is_first_window,
        cfg.window_position.on_startup,
        cfg.window_position.on_new_window,
        || cfg.window_position.default_position_mode,
        || WindowPositionMode::Coordinates,
    );
    let size = choose_by_behavior(
        is_first_window,
        cfg.window_size.on_startup,
        cfg.window_size.on_new_window,
        || cfg.window_size.default_size,
        || {
            get_last_focused_metrics()
                .map(|(_, sz)| window_size_from_state(sz))
                .unwrap_or_else(|| window_size_from_state(PersistedState::load().window_size))
        },
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
            get_last_focused_window_state()
                .map(|state| *state.current_theme.read())
                .unwrap_or_else(|| PersistedState::load().theme)
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
    choose_by_behavior(
        is_first_window,
        cfg.sidebar.on_startup,
        cfg.sidebar.on_new_window,
        || SidebarPreference {
            open: cfg.sidebar.default_open,
            width: cfg.sidebar.default_width,
            show_all_files: cfg.sidebar.default_show_all_files,
        },
        || {
            if let Some(state) = get_last_focused_window_state() {
                let sidebar = state.sidebar.read();
                SidebarPreference {
                    open: sidebar.open,
                    width: sidebar.width,
                    show_all_files: sidebar.show_all_files,
                }
            } else {
                let persisted = PersistedState::load();
                SidebarPreference {
                    open: persisted.sidebar_open,
                    width: persisted.sidebar_width,
                    show_all_files: persisted.sidebar_show_all_files,
                }
            }
        },
    )
}

pub fn get_toc_preference(is_first_window: bool) -> TocPreference {
    let cfg = CONFIG.read();
    choose_by_behavior(
        is_first_window,
        cfg.right_sidebar.on_startup,
        cfg.right_sidebar.on_new_window,
        || TocPreference {
            open: cfg.right_sidebar.default_open,
            width: cfg.right_sidebar.default_width,
            tab: cfg.right_sidebar.default_tab,
        },
        || {
            if let Some(state) = get_last_focused_window_state() {
                TocPreference {
                    open: *state.right_sidebar_open.read(),
                    width: *state.right_sidebar_width.read(),
                    tab: *state.right_sidebar_tab.read(),
                }
            } else {
                let persisted = PersistedState::load();
                TocPreference {
                    open: persisted.right_sidebar_open,
                    width: persisted.right_sidebar_width,
                    tab: persisted.right_sidebar_tab,
                }
            }
        },
    )
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
        WindowPositionMode::Coordinates => {
            resolve_window_position(position, screen_origin, screen_size, resolved_size)
        }
        WindowPositionMode::Mouse => resolve_window_position_from_cursor(resolved_size)
            .unwrap_or_else(|| LogicalPosition::new(0, 0)),
    };
    WindowPositionPreference {
        position: resolved_position,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_get_sidebar_preference_new_window() {
        let result = get_sidebar_preference(false);
        // Should return a SidebarPreference
        assert!(result.width > 0.0);
    }

    #[test]
    fn test_get_toc_preference_first_window() {
        let result = get_toc_preference(true);
        // Should return a TocPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.tab,
            RightSidebarTab::Contents | RightSidebarTab::Search
        ));
    }

    #[test]
    fn test_get_toc_preference_new_window() {
        let result = get_toc_preference(false);
        // Should return a TocPreference
        assert!(result.width > 0.0);
        assert!(matches!(
            result.tab,
            RightSidebarTab::Contents | RightSidebarTab::Search
        ));
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
}
