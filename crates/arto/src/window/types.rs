use crate::state::{Position, Size};

/// Window metrics captured from the actual window state.
///
/// This struct represents the current state of a window (position and size)
/// as reported by the windowing system. It is used to:
/// - Initialize app state position/size on window creation
/// - Calculate outer-to-inner size deltas for window decorations
/// - Persist window state when closing
///
/// Unlike configuration values (which may contain percentages or relative positions),
/// `WindowMetrics` always contains absolute pixel coordinates and dimensions.
#[derive(Clone, Debug, Default)]
pub struct WindowMetrics {
    /// Window position in logical pixels (top-left corner)
    pub position: Position,
    /// Window size in logical pixels (inner content area)
    pub size: Size,
}
