use super::super::form_controls::{OptionCardItem, OptionCards, SliderInput};
use crate::components::right_sidebar::RightSidebarTab as RightSidebarTabKind;
use crate::config::{Config, NewWindowBehavior, StartupBehavior};
use dioxus::prelude::*;

#[component]
pub fn RightSidebarTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    current_right_sidebar_width: f64,
) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let right_sidebar = config.read().right_sidebar.clone();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Open by Default" }
                    p { class: "preference-description", "Whether the right sidebar panel is open when starting." }
                }
                OptionCards {
                    name: "right-sidebar-default-open".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: false,
                            title: "Closed".to_string(),
                            description: Some("Right sidebar closed by default".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: true,
                            title: "Open".to_string(),
                            description: Some("Right sidebar open by default".to_string()),
                        },
                    ],
                    selected: right_sidebar.default_open,
                    on_change: move |new_state| {
                        config.write().right_sidebar.default_open = new_state;
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
                    selected: right_sidebar.default_tab,
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
                    value: right_sidebar.default_width,
                    min: 150.0,
                    max: 400.0,
                    step: 10.0,
                    unit: "px".to_string(),
                    on_change: move |new_width| {
                        config.write().right_sidebar.default_width = new_width;
                        has_changes.set(true);
                    },
                    current_value: Some(current_right_sidebar_width),
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
                    selected: right_sidebar.on_startup,
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
                    selected: right_sidebar.on_new_window,
                    on_change: move |new_behavior| {
                        config.write().right_sidebar.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
