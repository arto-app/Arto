//! What fits beside the document at a given width.
//!
//! Every element around the page has a width, and at some point their sum
//! leaves the document too narrow to read. Rather than five thresholds to
//! keep in step, there is one number — the document's minimum width — and
//! everything else is that number plus the width of whatever is still out.
//!
//! The order things give way in is fixed: the margin trace first (the
//! history has three other windows), then the panel (the rail can still peek
//! it back), then the contents gutter (Cmd+J still opens it), then the rail.
//! The document itself never gives way; below the last threshold it simply
//! follows the window, because the minimum is a target and not a floor.
//!
//! What comes out is state, never settings. Narrowing a window hides the
//! panel; widening it brings the panel back exactly as it was configured.
//! The one exception lives in the caller: a panel the reader folded with
//! Cmd+B stays folded, because that was a statement of intent rather than a
//! consequence of the width.

/// The rail's width. Always the last thing to go.
pub const RAIL_WIDTH: f64 = 40.0;

/// The contents gutter's width.
pub const GUTTER_WIDTH: f64 = 24.0;

/// The margin trace's width.
pub const TRACE_WIDTH: f64 = 138.0;

/// What may be drawn beside the document at this width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visible {
    pub trace: bool,
    pub panel: bool,
    pub gutter: bool,
    pub rail: bool,
}

/// Give way from the outside in until the document keeps `min_content`.
///
/// `effective_width` is the window's content width after zoom — magnifying
/// the page is the same as narrowing the window, so the same rule covers
/// both. `panel_w` is the panel's current width, so widening the panel
/// raises the width at which it folds.
pub fn budget(effective_width: f64, min_content: f64, panel_w: f64) -> Visible {
    let rail = effective_width >= min_content + RAIL_WIDTH;
    let gutter = rail && effective_width >= min_content + RAIL_WIDTH + GUTTER_WIDTH;
    let panel = gutter && effective_width >= min_content + RAIL_WIDTH + panel_w + GUTTER_WIDTH;
    let trace =
        panel && effective_width >= min_content + RAIL_WIDTH + panel_w + TRACE_WIDTH + GUTTER_WIDTH;

    Visible {
        trace,
        panel,
        gutter,
        rail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel width the specification's threshold table assumes.
    const PANEL: f64 = 226.0;

    fn at(width: f64, min_content: f64) -> Visible {
        budget(width, min_content, PANEL)
    }

    #[test]
    fn a_wide_window_shows_everything() {
        let visible = at(1600.0, 640.0);
        assert_eq!(
            visible,
            Visible {
                trace: true,
                panel: true,
                gutter: true,
                rail: true
            }
        );
    }

    // The specification's table, from both sides of every threshold:
    // 1068 / 930 / 704 / 680 for a 640px document.

    #[test]
    fn the_trace_goes_first() {
        assert!(at(1068.0, 640.0).trace);
        assert!(!at(1067.0, 640.0).trace);
        assert!(at(1067.0, 640.0).panel);
    }

    #[test]
    fn then_the_panel() {
        assert!(at(930.0, 640.0).panel);
        assert!(!at(929.0, 640.0).panel);
        assert!(at(929.0, 640.0).gutter);
    }

    #[test]
    fn then_the_gutter() {
        assert!(at(704.0, 640.0).gutter);
        assert!(!at(703.0, 640.0).gutter);
        assert!(at(703.0, 640.0).rail);
    }

    #[test]
    fn and_the_rail_last() {
        assert!(at(680.0, 640.0).rail);
        assert!(!at(679.0, 640.0).rail);
    }

    #[test]
    fn nothing_is_left_below_the_last_threshold() {
        let visible = at(400.0, 640.0);
        assert_eq!(
            visible,
            Visible {
                trace: false,
                panel: false,
                gutter: false,
                rail: false
            }
        );
    }

    // The same table with the setting raised: 1228 / 1090 / 864 / 840.

    #[test]
    fn a_wider_document_folds_things_sooner() {
        assert!(at(1228.0, 800.0).trace);
        assert!(!at(1227.0, 800.0).trace);
        assert!(at(1090.0, 800.0).panel);
        assert!(!at(1089.0, 800.0).panel);
        assert!(at(864.0, 800.0).gutter);
        assert!(!at(863.0, 800.0).gutter);
        assert!(at(840.0, 800.0).rail);
        assert!(!at(839.0, 800.0).rail);
    }

    #[test]
    fn a_wider_panel_folds_at_a_wider_window() {
        // The panel's own width is part of its threshold, so dragging it
        // wider moves the point at which it gives way.
        assert!(budget(930.0, 640.0, 226.0).panel);
        assert!(!budget(930.0, 640.0, 280.0).panel);
        assert!(budget(984.0, 640.0, 280.0).panel);
    }

    #[test]
    fn the_order_never_inverts() {
        // Whatever the width, nothing outer survives something inner going.
        for width in (300..1600).step_by(7) {
            let visible = budget(width as f64, 640.0, PANEL);
            assert!(!visible.trace || visible.panel);
            assert!(!visible.panel || visible.gutter);
            assert!(!visible.gutter || visible.rail);
        }
    }
}
