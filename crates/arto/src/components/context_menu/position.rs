//! Viewport clamping for context menus.

/// Clamp a menu's top-left origin so the whole menu stays within the viewport.
///
/// If the menu is larger than the available space on an axis, it is pinned to
/// `margin` on that axis (the top/left is always visible).
pub fn clamp_menu_position(
    cursor: (i32, i32),
    menu: (i32, i32),
    viewport: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let clamp_axis = |pos: i32, size: i32, extent: i32| {
        let max_pos = (extent - margin - size).max(margin);
        pos.clamp(margin, max_pos)
    };
    (
        clamp_axis(cursor.0, menu.0, viewport.0),
        clamp_axis(cursor.1, menu.1, viewport.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MENU: (i32, i32) = (200, 300);
    const VIEWPORT: (i32, i32) = (1000, 800);
    const MARGIN: i32 = 8;

    #[test]
    fn keeps_position_unchanged_when_menu_fits() {
        // A cursor with room for the whole menu is used as-is.
        let pos = clamp_menu_position((100, 100), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (100, 100));
    }

    #[test]
    fn shifts_left_when_menu_overflows_right_edge() {
        // Cursor near the right edge: x is pulled in so the menu's right edge
        // sits at viewport_width - margin.
        let pos = clamp_menu_position((950, 100), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos.0, VIEWPORT.0 - MARGIN - MENU.0); // 1000 - 8 - 200 = 792
        assert_eq!(pos.1, 100);
    }

    #[test]
    fn shifts_up_when_menu_overflows_bottom_edge() {
        // Cursor near the bottom edge: y is pulled in so the menu's bottom edge
        // sits at viewport_height - margin.
        let pos = clamp_menu_position((100, 780), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos.0, 100);
        assert_eq!(pos.1, VIEWPORT.1 - MARGIN - MENU.1); // 800 - 8 - 300 = 492
    }

    #[test]
    fn clamps_bottom_right_corner_into_view() {
        // A corner click keeps the whole menu on-screen on both axes.
        let pos = clamp_menu_position((999, 799), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (792, 492));
    }

    #[test]
    fn pins_to_margin_when_menu_larger_than_viewport() {
        // Degenerate viewport smaller than the menu: the top-left stays visible.
        let tiny = (150, 150);
        let pos = clamp_menu_position((999, 999), MENU, tiny, MARGIN);
        assert_eq!(pos, (MARGIN, MARGIN));
    }

    #[test]
    fn never_places_origin_before_margin() {
        // A cursor above/left of the margin is pushed back to the margin.
        let pos = clamp_menu_position((0, 0), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (MARGIN, MARGIN));
    }
}
