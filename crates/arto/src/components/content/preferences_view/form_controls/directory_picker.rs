use super::ResetLine;
use crate::components::icon::{Icon, IconName};
use dioxus::prelude::*;
use std::path::PathBuf;

/// Directory picker component with browse button and "Use Current" option
///
/// `shipped` nests two `Option`s on purpose: the outer one says whether there
/// is a shipped value to offer at all, the inner one is that value — and for
/// this setting the shipped value *is* "no folder".
#[component]
pub fn DirectoryPicker(
    value: Option<PathBuf>,
    placeholder: String,
    on_change: EventHandler<Option<PathBuf>>,
    current_directory: Option<PathBuf>,
    shipped: Option<Option<PathBuf>>,
) -> Element {
    let reset_to =
        shipped
            .clone()
            .filter(|shipped| shipped != &value)
            .map(|shipped| match shipped {
                Some(path) => path.display().to_string(),
                None => "no folder".to_string(),
            });

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
        ResetLine {
            shipped: reset_to,
            on_reset: move |_| {
                if let Some(shipped) = shipped.clone() {
                    on_change.call(shipped);
                }
            },
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
