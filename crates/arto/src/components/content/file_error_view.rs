use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::icon::{Icon, IconName};

#[component]
pub fn FileErrorView(filename: String, error_message: String) -> Element {
    rsx! {
        div {
            class: "no-file file-error",
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
                            {t!("file_error.hint").to_string()}
                        }
                    }
                    div {
                        class: "no-file-hint",
                        span {
                            class: "no-file-hint-icon",
                            Icon { name: IconName::AlertCircle, size: 20 }
                        }
                        span {
                            class: "no-file-hint-text",
                            {t!("file_error.message", error = error_message).to_string()}
                        }
                    }
                }
            }
        }
    }
}
