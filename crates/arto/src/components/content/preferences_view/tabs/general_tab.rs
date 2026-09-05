use super::super::form_controls::{OptionCardItem, OptionCards};
use crate::config::{Config, FileOpenBehavior};
use dioxus::prelude::*;

#[component]
pub fn GeneralTab(config: Signal<Config>, has_changes: Signal<bool>) -> Element {
    // Extract values upfront to avoid holding read guard across closures
    let file_open = config.read().file_open;

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "External Open" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "File Open Behavior" }
                    p {
                        class: "preference-description",
                        "How files or directories opened from Finder/CLI/IPC are routed."
                    }
                }
                OptionCards {
                    name: "external-file-open-behavior".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::NewWindow,
                            title: "New Window".to_string(),
                            description: Some("Always create a new window".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::LastFocused,
                            title: "Last Focused".to_string(),
                            description: Some("Open in the last focused visible window".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::CurrentScreen,
                            title: "Current Screen".to_string(),
                            description: Some("Open in a visible window on the cursor screen".to_string()),
                        },
                    ],
                    selected: file_open,
                    on_change: move |new_behavior| {
                        config.write().file_open = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
