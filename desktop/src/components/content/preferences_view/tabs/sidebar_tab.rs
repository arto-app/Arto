use super::super::form_controls::{OptionCardItem, OptionCards, SliderInput};
use crate::config::{normalize_zoom_level, Config, NewWindowBehavior, StartupBehavior};
use crate::state::{AppState, SidebarPanel};
use dioxus::prelude::*;

#[component]
pub fn SidebarTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    mut state: AppState,
) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let sidebar_cfg = config.read().sidebar.clone();
    let current_width = state.sidebar.read().width;
    let current_zoom = state.sidebar.read().zoom_level;

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
                    min: 0.5,
                    max: 2.0,
                    step: 0.1,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        // Normalize to 0.1 step and clamp to valid range
                        state.sidebar.write().zoom_level = normalize_zoom_level(new_zoom);
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
                    label { "Default Panel" }
                    p { class: "preference-description", "Which content the left sidebar shows by default." }
                }
                OptionCards {
                    name: "sidebar-default-panel".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: SidebarPanel::Directory,
                            title: "Directory".to_string(),
                            description: Some("Show the file directory tree".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: SidebarPanel::Contents,
                            title: "Contents".to_string(),
                            description: Some("Show markdown table of contents".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: SidebarPanel::Search,
                            title: "Search".to_string(),
                            description: Some("Show document search results".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.default_panel,
                    on_change: move |new_panel| {
                        config.write().sidebar.default_panel = new_panel;
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
                    min: 0.5,
                    max: 2.0,
                    step: 0.1,
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
