//! Pure calculation functions for tab bar operations.
//!
//! These functions are extracted from component code to enable unit testing.
//! All functions in this module are pure (no side effects, no global state access).

use crate::state::TabContent;

/// Check if mouse movement exceeds the drag threshold.
///
/// # Arguments
/// * `dx` - Horizontal distance moved (absolute value)
/// * `dy` - Vertical distance moved (absolute value)
/// * `threshold` - Minimum distance to trigger drag
///
/// # Returns
/// `true` if either dx or dy exceeds the threshold
pub fn exceeds_drag_threshold(dx: f64, dy: f64, threshold: f64) -> bool {
    dx > threshold || dy > threshold
}

/// Calculate shift class for cross-window drag visualization.
///
/// When dragging to a target window, tabs at index >= target_index
/// shift right to make visual space for the FloatingTab.
///
/// # Arguments
/// * `is_target_window` - Whether this window is the current drag target
/// * `target_index` - The insertion index in the target window
/// * `current_index` - The index of the tab being rendered
///
/// # Returns
/// CSS class name for shifting, or None if no shift needed
pub fn calculate_shift_class(
    is_target_window: bool,
    target_index: Option<usize>,
    current_index: usize,
) -> Option<&'static str> {
    if !is_target_window {
        return None;
    }

    let target = target_index?;

    // Tabs at or after target_index shift right to make room
    if current_index >= target {
        Some("shifting-right")
    } else {
        None
    }
}

/// Convert screen X coordinate to client X coordinate.
///
/// # Arguments
/// * `screen_x` - X position in screen coordinates
/// * `outer_x` - Window outer position X (physical pixels)
/// * `chrome_x` - Window chrome inset X (physical pixels)
/// * `scale` - Display scale factor
///
/// # Returns
/// X position in client (viewport) coordinates
pub fn screen_to_client_x(screen_x: f64, outer_x: f64, chrome_x: f64, scale: f64) -> f64 {
    // outer_position() and chrome are in physical pixels, convert to logical
    // client = screen - inner_logical = screen - (outer + chrome) / scale
    screen_x - (outer_x + chrome_x) / scale
}

/// Calculate the clamped left position for a floating tab.
///
/// Ensures the floating tab stays within valid bounds (between first tab and [+] button).
///
/// # Arguments
/// * `raw_left` - Unclamped left position (client_x - grab_offset.x)
/// * `first_tab_left` - Left edge of the first tab position
/// * `tab_count` - Number of tabs in the target window
/// * `tab_width` - Fixed width of each tab in pixels
///
/// # Returns
/// Clamped left position for the floating tab
pub fn calculate_floating_tab_left(
    raw_left: f64,
    first_tab_left: f64,
    tab_count: usize,
    tab_width: f64,
) -> f64 {
    let max_left = first_tab_left + (tab_count as f64 * tab_width);
    raw_left.clamp(first_tab_left, max_left)
}

/// Calculate grab offset from click position and element bounds.
///
/// # Arguments
/// * `click_x` - X position of the click (client coordinates)
/// * `click_y` - Y position of the click (client coordinates)
/// * `element_x` - X position of the element's origin
/// * `element_y` - Y position of the element's origin
///
/// # Returns
/// Tuple of (grab_x, grab_y) representing offset within the element
pub fn calculate_grab_offset(
    click_x: f64,
    click_y: f64,
    element_x: f64,
    element_y: f64,
) -> (f64, f64) {
    (click_x - element_x, click_y - element_y)
}

/// Check if a tab content type is transferable to another window.
///
/// Only File tabs and FileError tabs can be transferred.
/// None, Inline, and Preferences tabs are not transferable.
pub fn is_tab_transferable(content: &TabContent) -> bool {
    matches!(content, TabContent::File(_) | TabContent::FileError(_, _))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    mod exceeds_drag_threshold {
        use super::*;

        #[test]
        fn returns_false_when_under_threshold() {
            assert!(!exceeds_drag_threshold(3.0, 3.0, 5.0));
            assert!(!exceeds_drag_threshold(0.0, 0.0, 5.0));
            assert!(!exceeds_drag_threshold(5.0, 5.0, 5.0)); // equal is not over
        }

        #[test]
        fn returns_true_when_x_exceeds() {
            assert!(exceeds_drag_threshold(6.0, 0.0, 5.0));
            assert!(exceeds_drag_threshold(10.0, 3.0, 5.0));
        }

        #[test]
        fn returns_true_when_y_exceeds() {
            assert!(exceeds_drag_threshold(0.0, 6.0, 5.0));
            assert!(exceeds_drag_threshold(3.0, 10.0, 5.0));
        }

        #[test]
        fn returns_true_when_both_exceed() {
            assert!(exceeds_drag_threshold(10.0, 10.0, 5.0));
        }
    }

    mod calculate_shift_class {
        use super::*;

        #[test]
        fn returns_none_when_not_target_window() {
            assert_eq!(calculate_shift_class(false, Some(2), 0), None);
            assert_eq!(calculate_shift_class(false, Some(2), 2), None);
            assert_eq!(calculate_shift_class(false, Some(2), 5), None);
        }

        #[test]
        fn returns_none_when_target_index_is_none() {
            assert_eq!(calculate_shift_class(true, None, 0), None);
            assert_eq!(calculate_shift_class(true, None, 5), None);
        }

        #[test]
        fn returns_none_for_tabs_before_target() {
            assert_eq!(calculate_shift_class(true, Some(3), 0), None);
            assert_eq!(calculate_shift_class(true, Some(3), 1), None);
            assert_eq!(calculate_shift_class(true, Some(3), 2), None);
        }

        #[test]
        fn returns_shifting_right_for_tabs_at_or_after_target() {
            assert_eq!(
                calculate_shift_class(true, Some(2), 2),
                Some("shifting-right")
            );
            assert_eq!(
                calculate_shift_class(true, Some(2), 3),
                Some("shifting-right")
            );
            assert_eq!(
                calculate_shift_class(true, Some(2), 10),
                Some("shifting-right")
            );
        }

        #[test]
        fn handles_target_index_zero() {
            // All tabs should shift when inserting at index 0
            assert_eq!(
                calculate_shift_class(true, Some(0), 0),
                Some("shifting-right")
            );
            assert_eq!(
                calculate_shift_class(true, Some(0), 1),
                Some("shifting-right")
            );
        }
    }

    mod screen_to_client_x {
        use super::*;

        #[test]
        fn converts_at_scale_1() {
            // screen_x=500, outer=100, chrome=50, scale=1
            // client = 500 - (100 + 50) / 1 = 500 - 150 = 350
            assert_eq!(screen_to_client_x(500.0, 100.0, 50.0, 1.0), 350.0);
        }

        #[test]
        fn converts_at_scale_2() {
            // screen_x=500, outer=100, chrome=50, scale=2
            // client = 500 - (100 + 50) / 2 = 500 - 75 = 425
            assert_eq!(screen_to_client_x(500.0, 100.0, 50.0, 2.0), 425.0);
        }

        #[test]
        fn handles_zero_chrome() {
            // screen_x=500, outer=100, chrome=0, scale=1
            // client = 500 - (100 + 0) / 1 = 400
            assert_eq!(screen_to_client_x(500.0, 100.0, 0.0, 1.0), 400.0);
        }
    }

    mod calculate_floating_tab_left {
        use super::*;

        #[test]
        fn clamps_to_minimum() {
            // raw_left=-50 should clamp to first_tab_left=100
            assert_eq!(calculate_floating_tab_left(-50.0, 100.0, 5, 140.0), 100.0);
        }

        #[test]
        fn clamps_to_maximum() {
            // first_tab_left=100, tab_count=3, tab_width=140
            // max = 100 + 3 * 140 = 520
            // raw_left=600 should clamp to 520
            assert_eq!(calculate_floating_tab_left(600.0, 100.0, 3, 140.0), 520.0);
        }

        #[test]
        fn returns_raw_when_within_bounds() {
            // first_tab_left=100, tab_count=5, tab_width=140
            // max = 100 + 5 * 140 = 800
            // raw_left=300 is within [100, 800]
            assert_eq!(calculate_floating_tab_left(300.0, 100.0, 5, 140.0), 300.0);
        }

        #[test]
        fn handles_zero_tabs() {
            // With 0 tabs, max = first_tab_left, so clamps to that
            assert_eq!(calculate_floating_tab_left(200.0, 100.0, 0, 140.0), 100.0);
        }
    }

    mod calculate_grab_offset {
        use super::*;

        #[test]
        fn calculates_offset() {
            let (x, y) = calculate_grab_offset(150.0, 80.0, 100.0, 50.0);
            assert_eq!(x, 50.0);
            assert_eq!(y, 30.0);
        }

        #[test]
        fn handles_click_at_origin() {
            let (x, y) = calculate_grab_offset(100.0, 50.0, 100.0, 50.0);
            assert_eq!(x, 0.0);
            assert_eq!(y, 0.0);
        }
    }

    mod is_tab_transferable {
        use super::*;

        #[test]
        fn file_tab_is_transferable() {
            let content = TabContent::File(PathBuf::from("/test.md"));
            assert!(is_tab_transferable(&content));
        }

        #[test]
        fn file_error_tab_is_transferable() {
            let content = TabContent::FileError(PathBuf::from("/test.md"), "error".to_string());
            assert!(is_tab_transferable(&content));
        }

        #[test]
        fn none_tab_is_not_transferable() {
            let content = TabContent::None;
            assert!(!is_tab_transferable(&content));
        }

        #[test]
        fn inline_tab_is_not_transferable() {
            let content = TabContent::Inline("content".to_string());
            assert!(!is_tab_transferable(&content));
        }

        #[test]
        fn preferences_tab_is_not_transferable() {
            let content = TabContent::Preferences;
            assert!(!is_tab_transferable(&content));
        }
    }
}
