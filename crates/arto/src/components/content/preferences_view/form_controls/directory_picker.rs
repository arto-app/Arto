use crate::components::icon::{Icon, IconName};
use dioxus::prelude::*;
use std::path::PathBuf;

/// Directory picker component with browse button and "Use Current" option
#[component]
pub fn DirectoryPicker(
    value: Option<PathBuf>,
    placeholder: String,
    on_change: EventHandler<Option<PathBuf>>,
    current_directory: Option<PathBuf>,
) -> Element {
    let handle_browse = move |_| {
        spawn(async move {
            if let Some(path) = pick_directory().await {
                on_change.call(Some(path));
            }
        });
    };

    let handle_use_current = {
        let current_dir = current_directory.clone();
        move |_| {
            if current_dir.is_some() {
                on_change.call(current_dir.clone());
            }
        }
    };

    rsx! {
        div {
            class: "directory-input",
            input {
                r#type: "text",
                placeholder: "{placeholder}",
                value: value.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                oninput: move |evt| {
                    let value = evt.value();
                    let new_value = if value.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(value))
                    };
                    on_change.call(new_value);
                },
            }
            button {
                class: "icon-button",
                title: "Browse...",
                onclick: handle_browse,
                Icon { name: IconName::FolderOpen, size: 18 }
            }
            button {
                class: "use-current-button",
                disabled: current_directory.is_none(),
                onclick: handle_use_current,
                "Use Current"
            }
        }
    }
}

/// Helper function to open native directory picker dialog (async to prevent UI freeze)
async fn pick_directory() -> Option<PathBuf> {
    use rfd::AsyncFileDialog;
    AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}
