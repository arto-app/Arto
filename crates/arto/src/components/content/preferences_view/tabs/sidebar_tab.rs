use super::super::form_controls::{OptionCardItem, OptionCards, SliderInput};
use crate::config::{
    normalize_sidebar_zoom, Config, NewWindowBehavior, RecentTrace, StartupBehavior,
    MAX_SIDEBAR_ZOOM, MIN_CONTENT_WIDTH_RANGE, MIN_SIDEBAR_ZOOM, ZOOM_STEP,
};
use crate::events::{SidebarSide, SET_SIDEBAR_ZOOM_IN_WINDOW};
use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

#[component]
pub fn SidebarTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    /// The window these "Current Settings" act on.
    window_id: WindowId,
    current_width: f64,
    current_zoom: f64,
) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let sidebar_cfg = config.read().sidebar.clone();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Current Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Current Zoom Level" }
                    p { class: "preference-description", "The zoom level for the current window's sidebar." }
                }
                SliderInput {
                    value: current_zoom,
                    min: MIN_SIDEBAR_ZOOM,
                    max: MAX_SIDEBAR_ZOOM,
                    step: ZOOM_STEP,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        // Normalize to 0.1 step and clamp to valid range
                        let _ = SET_SIDEBAR_ZOOM_IN_WINDOW.send((
                            window_id,
                            SidebarSide::Left,
                            normalize_sidebar_zoom(new_zoom),
                        ));
                    },
                    default_value: Some(sidebar_cfg.default_zoom_level),
                }
            }

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Pinned by Default" }
                    p { class: "preference-description", "Whether the sidebar is pinned to the layout when starting." }
                }
                OptionCards {
                    name: "sidebar-default-pinned".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: false,
                            title: "Unpinned".to_string(),
                            description: Some("Sidebar shown as overlay on hover".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: true,
                            title: "Pinned".to_string(),
                            description: Some("Sidebar pinned to the layout".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.default_pinned,
                    on_change: move |new_state| {
                        config.write().sidebar.default_pinned = new_state;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Width" }
                    p { class: "preference-description", "The default sidebar width in pixels." }
                }
                SliderInput {
                    value: sidebar_cfg.default_width,
                    min: 200.0,
                    max: 600.0,
                    step: 10.0,
                    unit: "px".to_string(),
                    on_change: move |new_width| {
                        config.write().sidebar.default_width = new_width;
                        has_changes.set(true);
                    },
                    current_value: Some(current_width),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Zoom Level" }
                    p { class: "preference-description", "The default zoom level applied to the sidebar content." }
                }
                SliderInput {
                    value: sidebar_cfg.default_zoom_level,
                    min: MIN_SIDEBAR_ZOOM,
                    max: MAX_SIDEBAR_ZOOM,
                    step: ZOOM_STEP,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        config.write().sidebar.default_zoom_level = new_zoom;
                        has_changes.set(true);
                    },
                    current_value: Some(current_zoom),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Show All Files" }
                    p { class: "preference-description", "Whether to show non-markdown files in the file explorer." }
                }
                OptionCards {
                    name: "sidebar-show-all-files".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: false,
                            title: "Markdown Only".to_string(),
                            description: Some("Show only markdown files".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: true,
                            title: "All Files".to_string(),
                            description: Some("Show all file types".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.default_show_all_files,
                    on_change: move |new_state| {
                        config.write().sidebar.default_show_all_files = new_state;
                        has_changes.set(true);
                    },
                }
            }

            h3 { class: "preference-section-title", "Reading Width" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Minimum Content Width" }
                    p {
                        class: "preference-description",
                        "The width the document keeps while anything else can give way instead. Everything around the page folds at this number plus its own width, from the outside in: the margin trace, then the panel, then the contents gutter, then the rail. Raise it and a wide window folds them sooner."
                    }
                }
                SliderInput {
                    value: sidebar_cfg.min_content_width,
                    min: *MIN_CONTENT_WIDTH_RANGE.start(),
                    max: *MIN_CONTENT_WIDTH_RANGE.end(),
                    step: 10.0,
                    unit: "px".to_string(),
                    on_change: move |new_width| {
                        config.write().sidebar.min_content_width = new_width;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Margin Trace" }
                    p {
                        class: "preference-description",
                        "The documents read before this one, left at the edge of the page. It is the one window on the history nobody asks for, so it is the one you can turn off. A window too narrow to keep the document readable hides it whatever is chosen here."
                    }
                }
                OptionCards {
                    name: "sidebar-recent-trace".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Never,
                            title: "Never".to_string(),
                            description: Some("Keep the margin empty".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::HiddenWhenSidebar,
                            title: "Not with the panel".to_string(),
                            description: Some("Hidden while the panel is out".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::HiddenWhenFullWidth,
                            title: "Not at full width".to_string(),
                            description: Some("Hidden while the document fills the width".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Always,
                            title: "Always".to_string(),
                            description: Some("Drawn whenever it fits".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.recent_trace,
                    on_change: move |new_trace| {
                        config.write().sidebar.recent_trace = new_trace;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Documents in the Trace" }
                    p { class: "preference-description", "How many documents the margin trace names." }
                }
                SliderInput {
                    value: sidebar_cfg.recent_trace_count as f64,
                    min: 1.0,
                    max: 12.0,
                    step: 1.0,
                    unit: String::new(),
                    on_change: move |new_count: f64| {
                        config.write().sidebar.recent_trace_count = new_count.max(1.0) as usize;
                        has_changes.set(true);
                    },
                }
            }

            h3 { class: "preference-section-title", "Behavior" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On Startup" }
                    p { class: "preference-description", "Sidebar state when the application starts." }
                }
                OptionCards {
                    name: "sidebar-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default settings".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::LastClosed,
                            title: "Last Closed".to_string(),
                            description: Some("Resume from last closed window".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.on_startup,
                    on_change: move |new_behavior| {
                        config.write().sidebar.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On New Window" }
                    p { class: "preference-description", "Sidebar state in new windows." }
                }
                OptionCards {
                    name: "sidebar-new-window".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default settings".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::LastFocused,
                            title: "Last Focused".to_string(),
                            description: Some("Same as current window".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.on_new_window,
                    on_change: move |new_behavior| {
                        config.write().sidebar.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
