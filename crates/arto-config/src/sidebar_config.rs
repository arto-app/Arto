use super::behavior::{NewWindowBehavior, StartupBehavior};
use super::zoom_config::DEFAULT_ZOOM_LEVEL;
use schemars::JsonSchema;
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

fn default_peek_on_hover() -> bool {
    true
}

/// When the margin trace — the documents read before this one, left at the
/// edge of the page — is drawn.
///
/// It is the one window on the history nobody asks for, which is why it is
/// the one that can be turned off. Whatever is chosen here, the trace is
/// drawn only where there is a margin to draw it in: a window too narrow to
/// keep the document readable hides it, and so does a document set to the
/// full width, which leaves no margin at all. That is state, not a change to
/// this setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecentTrace {
    /// Never drawn.
    Never,
    /// Hidden while the panel is out, since it already lists documents.
    #[default]
    HiddenWhenSidebar,
    /// Always drawn, where there is a margin for it.
    ///
    /// `hidden_when_full_width` is the old name of this: it named a choice
    /// that is now simply what happens, since a document at full width has no
    /// margin for the trace to be in. A configuration that still says it
    /// keeps working and means this.
    #[serde(alias = "hidden_when_full_width")]
    Always,
}

/// What the panel does once a document has been opened from one of its rows.
///
/// Two readings of the same list, and both are ordinary. One reader keeps the
/// panel out and works down it, opening a document after a document; the other
/// goes to it to fetch one thing and wants the page to themselves once they
/// have it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpenFromPanel {
    /// The list stays where it is.
    #[default]
    KeepOpen,
    /// The panel puts itself away, leaving the document alone on screen. A
    /// pinned panel unpins: what was asked for is the document by itself.
    ClosePanel,
}

/// Which click on a glyph of the rail holds the panel beside the document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PinPanelOn {
    /// A click holds the face; a click on the face already held puts the
    /// panel away.
    #[default]
    Click,
    /// A click brings the face out over the document, as resting on the
    /// glyph does, and it goes again when the pointer leaves. A double click
    /// holds it; a double click on the face already held puts the panel away.
    DoubleClick,
}

/// Configuration for sidebar-related settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// What the panel does once a document has been opened from it.
    #[serde(default)]
    pub on_open: OpenFromPanel,
    /// Whether resting the pointer on the rail brings the panel out.
    ///
    /// Off, the panel comes out only when a glyph on the rail is clicked, and
    /// stays until it is put away again.
    #[serde(default = "default_peek_on_hover")]
    pub peek_on_hover: bool,
    /// Which click on the rail holds the panel beside the document.
    #[serde(default)]
    pub pin_on: PinPanelOn,
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
            on_open: OpenFromPanel::default(),
            peek_on_hover: default_peek_on_hover(),
            pin_on: PinPanelOn::default(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_name_for_always_still_parses() {
        // `hidden_when_full_width` named a choice that is now simply what
        // happens: a document at full width has no margin for the trace.
        let parsed: RecentTrace = serde_json::from_str(r#""hidden_when_full_width""#).unwrap();
        assert_eq!(parsed, RecentTrace::Always);
    }

    #[test]
    fn open_from_panel_keeps_the_panel_by_default() {
        assert_eq!(OpenFromPanel::default(), OpenFromPanel::KeepOpen);
        assert_eq!(SidebarConfig::default().on_open, OpenFromPanel::KeepOpen);
    }

    #[test]
    fn open_from_panel_roundtrips_through_json() {
        let cases = [
            (OpenFromPanel::KeepOpen, r#""keep_open""#),
            (OpenFromPanel::ClosePanel, r#""close_panel""#),
        ];
        for (behavior, expected) in cases {
            assert_eq!(serde_json::to_string(&behavior).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<OpenFromPanel>(expected).unwrap(),
                behavior
            );
        }
    }

    #[test]
    fn a_config_written_before_the_setting_existed_still_loads() {
        let json = serde_json::to_value(SidebarConfig::default()).unwrap();
        let mut without = json.as_object().unwrap().clone();
        without.remove("onOpen").unwrap();
        let parsed: SidebarConfig = serde_json::from_value(without.into()).unwrap();
        assert_eq!(parsed.on_open, OpenFromPanel::KeepOpen);
    }

    #[test]
    fn a_single_click_pins_the_panel_by_default() {
        assert_eq!(PinPanelOn::default(), PinPanelOn::Click);
        assert_eq!(SidebarConfig::default().pin_on, PinPanelOn::Click);
    }

    #[test]
    fn pin_panel_on_roundtrips_through_json() {
        let cases = [
            (PinPanelOn::Click, r#""click""#),
            (PinPanelOn::DoubleClick, r#""double_click""#),
        ];
        for (gesture, expected) in cases {
            assert_eq!(serde_json::to_string(&gesture).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<PinPanelOn>(expected).unwrap(),
                gesture
            );
        }
    }

    #[test]
    fn a_config_without_pin_on_pins_on_a_click() {
        let json = serde_json::to_value(SidebarConfig::default()).unwrap();
        let mut without = json.as_object().unwrap().clone();
        without.remove("pinOn").unwrap();
        let parsed: SidebarConfig = serde_json::from_value(without.into()).unwrap();
        assert_eq!(parsed.pin_on, PinPanelOn::Click);
    }

    #[test]
    fn the_rail_peeks_on_hover_by_default() {
        assert!(SidebarConfig::default().peek_on_hover);
    }

    #[test]
    fn a_config_without_peek_on_hover_still_peeks() {
        let json = serde_json::to_value(SidebarConfig::default()).unwrap();
        let mut without = json.as_object().unwrap().clone();
        without.remove("peekOnHover").unwrap();
        let parsed: SidebarConfig = serde_json::from_value(without.into()).unwrap();
        assert!(parsed.peek_on_hover);
    }
}
