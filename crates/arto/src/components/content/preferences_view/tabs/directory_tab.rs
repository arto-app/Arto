use super::super::form_controls::{DirectoryPicker, OptionCardItem, OptionCards};
use crate::config::{Config, NewWindowBehavior, StartupBehavior};
use dioxus::prelude::*;
use std::path::PathBuf;

#[component]
pub fn DirectoryTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    current_directory: Option<PathBuf>,
) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let directory = config.read().directory.clone();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Directory" }
                    p { class: "preference-description", "The directory to open when no specific directory is specified." }
                }
                DirectoryPicker {
                    value: directory.default_directory.to_owned(),
                    placeholder: "Not set".to_string(),
                    on_change: move |new_value| {
                        config.write().directory.default_directory = new_value;
                        has_changes.set(true);
                    },
                    current_directory: current_directory.clone(),
                }
            }

            h3 { class: "preference-section-title", "Behavior" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On Startup" }
                    p { class: "preference-description", "Which directory to open when the application starts." }
                }
                OptionCards {
                    name: "dir-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default directory".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::LastClosed,
                            title: "Last Closed".to_string(),
                            description: Some("Resume from last closed window".to_string()),
                        },
                    ],
                    selected: directory.on_startup,
                    on_change: move |new_behavior| {
                        config.write().directory.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On New Window" }
                    p { class: "preference-description", "Which directory to open in new windows." }
                }
                OptionCards {
                    name: "dir-new-window".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default directory".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::LastFocused,
                            title: "Last Focused".to_string(),
                            description: Some("Same as current window".to_string()),
                        },
                    ],
                    selected: directory.on_new_window,
                    on_change: move |new_behavior| {
                        config.write().directory.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
