use super::super::form_controls::{OptionCardItem, OptionCards, SliderInput};
use crate::components::right_sidebar::RightSidebarTab as RightSidebarTabKind;
use crate::config::{
    normalize_sidebar_zoom, Config, NewWindowBehavior, StartupBehavior, MAX_SIDEBAR_ZOOM,
    MIN_SIDEBAR_ZOOM, ZOOM_STEP,
};
use crate::state::AppState;
use dioxus::prelude::*;

#[component]
pub fn RightSidebarTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    mut state: AppState,
) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let right_sidebar_cfg = config.read().right_sidebar.clone();
    let current_width = state.right_sidebar.read().width;
    let current_zoom = state.right_sidebar.read().zoom_level;

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Current Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Current Zoom Level" }
                    p { class: "preference-description", "The zoom level for the current window's right sidebar." }
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
                        state.right_sidebar.write().zoom_level = normalize_sidebar_zoom(new_zoom);
                    },
                    default_value: Some(right_sidebar_cfg.default_zoom_level),
                }
            }

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Pinned by Default" }
                    p { class: "preference-description", "Whether the right sidebar panel is pinned to the layout when starting." }
                }
                OptionCards {
                    name: "right-sidebar-default-pinned".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: false,
                            title: "Unpinned".to_string(),
                            description: Some("Right sidebar shown as overlay on hover".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: true,
                            title: "Pinned".to_string(),
                            description: Some("Right sidebar pinned to the layout".to_string()),
                        },
                    ],
                    selected: right_sidebar_cfg.default_pinned,
                    on_change: move |new_state| {
                        config.write().right_sidebar.default_pinned = new_state;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Tab" }
                    p { class: "preference-description", "Which tab is active when the right sidebar opens." }
                }
                OptionCards {
                    name: "right-sidebar-default-tab".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RightSidebarTabKind::Contents,
                            title: "Contents".to_string(),
                            description: Some("Show table of contents".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RightSidebarTabKind::Search,
                            title: "Search".to_string(),
                            description: Some("Show document search".to_string()),
                        },
                    ],
                    selected: right_sidebar_cfg.default_tab,
                    on_change: move |new_tab| {
                        config.write().right_sidebar.default_tab = new_tab;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Width" }
                    p { class: "preference-description", "The default right sidebar panel width in pixels." }
                }
                SliderInput {
                    value: right_sidebar_cfg.default_width,
                    min: 220.0,
                    max: 400.0,
                    step: 10.0,
                    unit: "px".to_string(),
                    on_change: move |new_width| {
                        config.write().right_sidebar.default_width = new_width;
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
                    p { class: "preference-description", "The default zoom level applied to the right sidebar content." }
                }
                SliderInput {
                    value: right_sidebar_cfg.default_zoom_level,
                    min: MIN_SIDEBAR_ZOOM,
                    max: MAX_SIDEBAR_ZOOM,
                    step: ZOOM_STEP,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        config.write().right_sidebar.default_zoom_level = new_zoom;
                        has_changes.set(true);
                    },
                    current_value: Some(current_zoom),
                }
            }

            h3 { class: "preference-section-title", "Behavior" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On Startup" }
                    p { class: "preference-description", "Right sidebar panel state when the application starts." }
                }
                OptionCards {
                    name: "right-sidebar-startup".to_string(),
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
                    selected: right_sidebar_cfg.on_startup,
                    on_change: move |new_behavior| {
                        config.write().right_sidebar.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On New Window" }
                    p { class: "preference-description", "Right sidebar panel state in new windows." }
                }
                OptionCards {
                    name: "right-sidebar-new-window".to_string(),
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
                    selected: right_sidebar_cfg.on_new_window,
                    on_change: move |new_behavior| {
                        config.write().right_sidebar.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
