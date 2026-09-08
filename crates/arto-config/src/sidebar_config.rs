use super::behavior::{NewWindowBehavior, StartupBehavior};
use super::zoom_config::DEFAULT_ZOOM_LEVEL;
use serde::{Deserialize, Serialize};

/// Default sidebar width in pixels
pub const DEFAULT_SIDEBAR_WIDTH: f64 = 280.0;

/// Default width the document is not narrowed below while anything else can
/// give way instead.
pub const DEFAULT_MIN_CONTENT_WIDTH: f64 = 640.0;

/// The narrowest and widest the minimum content width may be set to.
pub const MIN_CONTENT_WIDTH_RANGE: std::ops::RangeInclusive<f64> = 360.0..=900.0;

/// How many documents the margin trace names by default.
pub const DEFAULT_RECENT_TRACE_COUNT: usize = 6;

fn default_sidebar_width() -> f64 {
    DEFAULT_SIDEBAR_WIDTH
}

fn default_min_content_width() -> f64 {
    DEFAULT_MIN_CONTENT_WIDTH
}

fn default_recent_trace_count() -> usize {
    DEFAULT_RECENT_TRACE_COUNT
}

fn default_sidebar_zoom_level() -> f64 {
    DEFAULT_ZOOM_LEVEL
}

/// When the margin trace — the documents read before this one, left at the
/// edge of the page — is drawn.
///
/// It is the one window on the history nobody asks for, which is why it is
/// the one that can be turned off. Whatever is chosen here, a window too
/// narrow to keep the document readable hides it anyway; that is state, not
/// a change to this setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecentTrace {
    /// Never drawn.
    Never,
    /// Hidden while the panel is out, since it already lists documents.
    #[default]
    HiddenWhenSidebar,
    /// Hidden while the document fills the width, where the margin is the
    /// reading measure rather than spare room.
    HiddenWhenFullWidth,
    /// Always drawn, width permitting.
    Always,
}

/// Configuration for sidebar-related settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarConfig {
    /// Whether sidebar is pinned to the layout by default
    pub default_pinned: bool,
    /// Default sidebar width in pixels
    #[serde(default = "default_sidebar_width")]
    pub default_width: f64,
    /// Whether to show all files (including non-markdown) by default
    pub default_show_all_files: bool,
    /// Default zoom level for sidebar content
    #[serde(default = "default_sidebar_zoom_level")]
    pub default_zoom_level: f64,
    /// The width the document keeps while anything else can give way instead.
    ///
    /// Every threshold in the layout is this number plus the width of what
    /// is still drawn beside the document, so raising it folds the trace,
    /// the panel, the gutter and the rail at correspondingly wider windows.
    #[serde(default = "default_min_content_width")]
    pub min_content_width: f64,
    /// When the margin trace is drawn.
    #[serde(default)]
    pub recent_trace: RecentTrace,
    /// How many documents the margin trace names.
    #[serde(default = "default_recent_trace_count")]
    pub recent_trace_count: usize,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            default_pinned: false,
            default_width: default_sidebar_width(),
            default_show_all_files: false,
            default_zoom_level: default_sidebar_zoom_level(),
            min_content_width: default_min_content_width(),
            recent_trace: RecentTrace::default(),
            recent_trace_count: default_recent_trace_count(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
