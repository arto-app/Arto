use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::icon::{Icon, IconName};

#[component]
pub(super) fn DragDropOverlay() -> Element {
    rsx! {
        div {
            class: "drag-drop-overlay",
            div {
                class: "drag-drop-content",
                div {
                    class: "drag-drop-icon",
                    Icon { name: IconName::FileUpload, size: 64 }
                }
                div {
                    class: "drag-drop-text",
                    {t!("drop_overlay.message").to_string()}
                }
            }
        }
    }
}
