use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;

/// "Copy As..." submenu: Text / Markdown
#[component]
pub(super) fn CopyAsSubmenu(
    selected_text: String,
    current_file: Option<PathBuf>,
    source_line: Option<u32>,
    source_line_end: Option<u32>,
    on_close: EventHandler<()>,
) -> Element {
    // Show "Markdown" option when file and at least start line are known.
    let has_markdown_source = current_file.is_some() && source_line.is_some();

    rsx! {
        ContextMenuSubmenu {
            label: "Copy As...",
            icon: Some(IconName::Copy),

            ContextMenuItem {
                label: "Text",
                icon: Some(IconName::LetterT),
                on_click: {
                    let text = selected_text.clone();
                    move |_| {
                        crate::utils::clipboard::copy_text(&text);
                        on_close.call(());
                    }
                },
            }

            if has_markdown_source {
                ContextMenuItem {
                    label: "Markdown",
                    icon: Some(IconName::Markdown),
                    on_click: {
                        let file = current_file.clone().unwrap();
                        let start = source_line.unwrap();
                        let end = source_line_end.unwrap_or(start);
                        let selected_text = selected_text.clone();
                        move |_| {
                            super::copy_markdown_source_direct(
                                file.clone(),
                                start,
                                end,
                                selected_text.clone(),
                            );
                            on_close.call(());
                        }
                    },
                }
            }
        }
    }
}
