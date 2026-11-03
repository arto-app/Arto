use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::icon::{Icon, IconName};
use crate::state::Unreadable;

/// `path` marks the page with the file it stands for, which the ready
/// reporter tells apart from an error page left over from another file.
#[component]
pub fn FileErrorView(filename: String, path: String, reason: Unreadable) -> Element {
    let hint = match &reason {
        Unreadable::Missing => t!("file_error.missing").to_string(),
        Unreadable::NotText => t!("file_error.not_text").to_string(),
        Unreadable::Other(error) => t!("file_error.other", error = error).to_string(),
    };
    rsx! {
        div {
            class: "no-file file-error",
            "data-arto-loaded": path,
            div {
                class: "no-file-container",
                div {
                    class: "no-file-icon file-error-icon",
                    Icon { name: IconName::AlertTriangle, size: 64 }
                }
                h2 {
                    class: "no-file-title file-error-title",
                    {t!("file_error.title").to_string()}
                }
                p {
                    class: "no-file-description file-error-filename",
                    "{filename}"
                }
                div {
                    class: "no-file-hints",
                    div {
                        class: "no-file-hint",
                        span {
                            class: "no-file-hint-icon",
                            Icon { name: IconName::AlertCircle, size: 20 }
                        }
                        span {
                            class: "no-file-hint-text",
                            "{hint}"
                        }
                    }
                }
            }
        }
    }
}
