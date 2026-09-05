//! Viewport clamping for context menus and their submenu flyouts.

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

/// Decide whether the submenu flyout should open to the left of the menu
/// instead of the right, to avoid spilling past the viewport's right edge.
pub fn submenu_opens_left(
    menu_x: i32,
    menu_width: i32,
    submenu_width: i32,
    viewport_width: i32,
    margin: i32,
) -> bool {
    menu_x + menu_width + submenu_width + margin > viewport_width
}

/// Clamp the submenu flyout's top so its full height stays within the viewport.
///
/// Returns the top the flyout should render at: `anchor_y` when it fits, a
/// smaller value when it would spill past the bottom edge, and `margin` when the
/// flyout is taller than the available space (its top stays visible).
pub fn clamp_submenu_top(
    anchor_y: i32,
    submenu_height: i32,
    viewport_height: i32,
    margin: i32,
) -> i32 {
    let max_top = (viewport_height - margin - submenu_height).max(margin);
    anchor_y.clamp(margin, max_top)
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

    #[test]
    fn submenu_opens_right_with_room() {
        // Plenty of horizontal room: the flyout opens to the right.
        assert!(!submenu_opens_left(100, 220, 208, 1000, MARGIN));
    }

    #[test]
    fn submenu_flips_left_near_right_edge() {
        // Menu hugging the right edge: right-side flyout would spill, so flip.
        assert!(submenu_opens_left(700, 220, 208, 1000, MARGIN));
    }

    #[test]
    fn submenu_top_unchanged_when_flyout_fits() {
        // Plenty of room below the anchor: the flyout stays at its anchor.
        let top = clamp_submenu_top(100, 200, 800, MARGIN);
        assert_eq!(top, 100);
    }

    #[test]
    fn submenu_top_shifts_up_near_bottom_edge() {
        // Anchor near the bottom: the top is pulled up so the flyout's bottom
        // sits at viewport_height - margin.
        let top = clamp_submenu_top(700, 200, 800, MARGIN);
        assert_eq!(top, 800 - MARGIN - 200); // 592
    }

    #[test]
    fn submenu_top_pins_to_margin_when_taller_than_viewport() {
        // A flyout taller than the available space keeps its top visible.
        let top = clamp_submenu_top(700, 1000, 800, MARGIN);
        assert_eq!(top, MARGIN);
    }
}
