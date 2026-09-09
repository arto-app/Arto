use super::super::form_controls::{DirectoryPicker, OptionCardItem, OptionCards};
use crate::config::{Config, StartupBehavior};
use dioxus::prelude::*;
use std::path::PathBuf;

/// Where the first window is, when it opens.
///
/// Not a list of folders: the places are kept by starring one, wherever it is
/// shown, and every window already starts with all of them. What cannot be
/// said anywhere else is which folder the window is *in* — the one temporary
/// root it begins with — so that is what this pane sets.
#[component]
pub fn DirectoryTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    current_directory: Option<PathBuf>,
) -> Element {
    let on_startup = config.read().directory.on_startup;
    let default_directory = config.read().directory.default_directory.clone();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Startup Folder" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Folder to start in" }
                    p {
                        class: "preference-description",
                        "The folder the first window works in, beside the places you keep. Leave it empty to start with the places alone."
                    }
                }
                DirectoryPicker {
                    value: default_directory,
                    placeholder: "No folder — the places alone".to_string(),
                    current_directory: current_directory.clone(),
                    on_change: move |new_directory| {
                        config.write().directory.default_directory = new_directory;
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
                    p { class: "preference-description", "What the first window opens with." }
                }
                OptionCards {
                    name: "dir-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Start in the folder above".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::LastClosed,
                            title: "Last Closed".to_string(),
                            description: Some("Resume the last window's folders".to_string()),
                        },
                    ],
                    selected: on_startup,
                    on_change: move |new_behavior| {
                        config.write().directory.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
