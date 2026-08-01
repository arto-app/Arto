use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use display_info::DisplayInfo;
use mouse_position::mouse_position::Mouse;

trait DisplayLike {
    fn x(&self) -> i32;
    fn y(&self) -> i32;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn is_primary(&self) -> bool;
}

impl DisplayLike for DisplayInfo {
    fn x(&self) -> i32 {
        self.x
    }

    fn y(&self) -> i32 {
        self.y
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn is_primary(&self) -> bool {
        self.is_primary
    }
}

fn primary_display_index<T: DisplayLike>(displays: &[T]) -> Option<usize> {
    displays
        .iter()
        .position(|display| display.is_primary())
        .or_else(|| (!displays.is_empty()).then_some(0))
}

fn find_display_index<T: DisplayLike>(x: i32, y: i32, displays: &[T]) -> Option<usize> {
    displays.iter().position(|display| {
        let left = display.x();
        let top = display.y();
        let right = left + display.width() as i32;
        let bottom = top + display.height() as i32;
        x >= left && x < right && y >= top && y < bottom
    })
}

fn flip_y<T: DisplayLike>(y: i32, display: &T) -> i32 {
    display.y() + display.height() as i32 - y
}

/// Get the current display bounds (origin and size) in logical pixels.
///
/// Returns the bounds of the display where the cursor is currently located,
/// falling back to the primary display if cursor position cannot be determined.
///
/// # Returns
///
/// - `Some((origin, size))` - Tuple of logical position and logical size
/// - `None` - If no displays are available or scale factor is invalid
pub fn get_current_display_bounds() -> Option<(LogicalPosition<i32>, LogicalSize<u32>)> {
    let display = get_cursor_display().or_else(get_primary_display)?;
    display_info_logical_bounds(&display)
}

pub fn display_info_logical_bounds(
    display: &DisplayInfo,
) -> Option<(LogicalPosition<i32>, LogicalSize<u32>)> {
    let scale = display.scale_factor as f64;
    if scale <= 0.0 {
        return None;
    }
    let origin = to_logical_position_from_parts(display.x, display.y, scale);
    let size = to_logical_size_from_parts(display.width, display.height, scale);
    Some((origin, size))
}

/// Get the primary display information.
///
/// Returns the display marked as primary, or the first display if no primary is set.
///
/// # Returns
///
/// - `Some(DisplayInfo)` - Primary display info
/// - `None` - If display enumeration fails or no displays are available
pub fn get_primary_display() -> Option<DisplayInfo> {
    let displays = DisplayInfo::all().ok()?;
    displays
        .iter()
        .find(|display| display.is_primary)
        .cloned()
        .or_else(|| displays.first().cloned())
}

/// Normalize a raw cursor position to logical coordinates.
///
/// `Mouse::get_mouse_position()` does not report the same coordinate space on
/// every platform:
/// - macOS (core-graphics) reports logical points already.
/// - Windows (`GetCursorPos`) reports physical pixels, because tao marks the
///   process `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
///
/// Everything the drag code compares against is logical: DOM
/// `screen_coordinates()`, the tab bar bounds measured from the DOM, and the
/// rects `drag::is_point_in_window` derives as `physical / scale`. Feeding a
/// physical value into that world silently breaks it — at 150% scaling the
/// cursor reads 1.5x too far, so every window hit test misses, the drag detaches
/// immediately, and the preview window is positioned far off target.
///
/// `scale_factor` is the drag source window's scale factor. With monitors on
/// different DPIs, a cursor dragged onto another monitor is normalized with the
/// source window's factor rather than the one it is over.
pub fn cursor_position_to_logical(x: f64, y: f64, scale_factor: f64) -> (f64, f64) {
    #[cfg(target_os = "windows")]
    {
        if scale_factor > 0.0 {
            return (x / scale_factor, y / scale_factor);
        }
        (x, y)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = scale_factor;
        (x, y)
    }
}

/// Get the current cursor position in logical coordinates.
///
/// See [`cursor_position_to_logical`] for why normalization is needed.
/// Returns `None` if the cursor position cannot be determined.
pub fn get_cursor_logical_position(scale_factor: f64) -> Option<(f64, f64)> {
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => {
            Some(cursor_position_to_logical(x as f64, y as f64, scale_factor))
        }
        Mouse::Error => None,
    }
}

/// Get the display where the cursor is currently located.
///
/// # Returns
///
/// - `Some(DisplayInfo)` - Display containing the cursor
/// - `None` - If cursor position cannot be determined
pub fn get_cursor_display() -> Option<DisplayInfo> {
    let (x, y) = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => return None,
    };

    if let Ok(display) = DisplayInfo::from_point(x, y) {
        return Some(display);
    }

    let displays = DisplayInfo::all().ok()?;
    if let Some(index) = primary_display_index(&displays) {
        // Some platforms report cursor Y in an inverted coordinate space; try a flipped Y fallback.
        let flipped_y = flip_y(y, &displays[index]);
        if let Ok(display) = DisplayInfo::from_point(x, flipped_y) {
            return Some(display);
        }
    }

    find_display_index(x, y, &displays)
        .and_then(|index| displays.get(index).cloned())
        .or_else(|| displays.first().cloned())
}

#[cfg(target_os = "macos")]
fn to_logical_size_from_parts(width: u32, height: u32, _scale: f64) -> LogicalSize<u32> {
    // display-info uses CGDisplayBounds on macOS, which is already in points.
    LogicalSize::new(width.max(1), height.max(1))
}

#[cfg(not(target_os = "macos"))]
fn to_logical_size_from_parts(width: u32, height: u32, scale: f64) -> LogicalSize<u32> {
    // Use safe minimum scale factor to prevent division by zero
    let safe_scale = if scale <= 0.0 { 1.0 } else { scale };
    let width = (width as f64 / safe_scale).round().max(1.0) as u32;
    let height = (height as f64 / safe_scale).round().max(1.0) as u32;
    LogicalSize::new(width, height)
}

#[cfg(target_os = "macos")]
fn to_logical_position_from_parts(x: i32, y: i32, _scale: f64) -> LogicalPosition<i32> {
    // display-info uses CGDisplayBounds on macOS, which is already in points.
    LogicalPosition::new(x, y)
}

#[cfg(not(target_os = "macos"))]
fn to_logical_position_from_parts(x: i32, y: i32, scale: f64) -> LogicalPosition<i32> {
    // Use safe minimum scale factor to prevent division by zero
    let safe_scale = if scale <= 0.0 { 1.0 } else { scale };
    let x = (x as f64 / safe_scale).round() as i32;
    let y = (y as f64 / safe_scale).round() as i32;
    LogicalPosition::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct TestDisplay {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        is_primary: bool,
    }

    impl DisplayLike for TestDisplay {
        fn x(&self) -> i32 {
            self.x
        }

        fn y(&self) -> i32 {
            self.y
        }

        fn width(&self) -> u32 {
            self.width
        }

        fn height(&self) -> u32 {
            self.height
        }

        fn is_primary(&self) -> bool {
            self.is_primary
        }
    }

    #[test]
    fn test_primary_display_index() {
        let displays = [
            TestDisplay {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
                is_primary: false,
            },
            TestDisplay {
                x: 100,
                y: 0,
                width: 100,
                height: 100,
                is_primary: true,
            },
        ];
        assert_eq!(primary_display_index(&displays), Some(1));

        let displays = [TestDisplay {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            is_primary: false,
        }];
        assert_eq!(primary_display_index(&displays), Some(0));
        assert_eq!(primary_display_index::<TestDisplay>(&[]), None);
    }

    #[test]
    fn test_find_display_index() {
        let displays = [
            TestDisplay {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
                is_primary: true,
            },
            TestDisplay {
                x: 100,
                y: 0,
                width: 100,
                height: 100,
                is_primary: false,
            },
        ];
        assert_eq!(find_display_index(10, 10, &displays), Some(0));
        assert_eq!(find_display_index(150, 50, &displays), Some(1));
        assert_eq!(find_display_index(250, 50, &displays), None);
    }

    #[test]
    fn test_flip_y() {
        let display = TestDisplay {
            x: 0,
            y: 10,
            width: 100,
            height: 120,
            is_primary: true,
        };
        assert_eq!(flip_y(15, &display), 115);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_to_logical_size_from_parts_scales() {
        let size = to_logical_size_from_parts(100, 50, 2.0);
        assert_eq!(size.width, 50);
        assert_eq!(size.height, 25);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_to_logical_size_from_parts_macos_uses_display_bounds() {
        let size = to_logical_size_from_parts(100, 50, 2.0);
        assert_eq!(size.width, 100);
        assert_eq!(size.height, 50);
    }

    #[test]
    fn test_to_logical_size_from_parts_minimum() {
        let size = to_logical_size_from_parts(0, 0, 2.0);
        assert_eq!(size.width, 1);
        assert_eq!(size.height, 1);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_to_logical_position_from_parts_scales() {
        let position = to_logical_position_from_parts(-40, 20, 2.0);
        assert_eq!(position.x, -20);
        assert_eq!(position.y, 10);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_to_logical_position_from_parts_macos_uses_display_bounds() {
        let position = to_logical_position_from_parts(-40, 20, 2.0);
        assert_eq!(position.x, -40);
        assert_eq!(position.y, 20);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cursor_position_to_logical_divides_by_scale_on_windows() {
        // GetCursorPos reports physical pixels, so a cursor at logical (400, 300)
        // on a 150% display arrives as (600, 450) and must be scaled back down.
        assert_eq!(
            cursor_position_to_logical(600.0, 450.0, 1.5),
            (400.0, 300.0)
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_cursor_position_to_logical_is_identity_off_windows() {
        // core-graphics (macOS) already reports logical points.
        assert_eq!(
            cursor_position_to_logical(600.0, 450.0, 1.5),
            (600.0, 450.0)
        );
    }

    #[test]
    fn test_cursor_position_to_logical_handles_zero_scale() {
        // A bogus scale factor must not produce NaN/inf coordinates.
        assert_eq!(
            cursor_position_to_logical(600.0, 450.0, 0.0),
            (600.0, 450.0)
        );
    }
}
