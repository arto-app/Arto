use dioxus::prelude::*;
use std::path::PathBuf;

use super::source_ops::build_path_with_range;
use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};

/// "Copy Code As..." submenu: Code / Markdown / Path with Range
#[component]
pub(super) fn CopyCodeAsSubmenu(
    code_content: String,
    current_file: Option<PathBuf>,
    /// Block-level source line range (for Markdown and Path with Range)
    block_source_line: Option<u32>,
    block_source_line_end: Option<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let has_markdown =
        current_file.is_some() && block_source_line.is_some() && block_source_line_end.is_some();

    let block_path_with_range = build_path_with_range(
        current_file.as_ref(),
        block_source_line,
        block_source_line_end,
    );

    rsx! {
        ContextMenuSubmenu {
            label: "Copy Code As...",

            ContextMenuItem {
                label: "Code",
                on_click: {
                    let code_content = code_content.clone();
                    move |_| {
                        crate::utils::clipboard::copy_text(&code_content);
                        crate::keybindings::dispatcher::show_action_feedback("Copied");
                        on_close.call(());
                    }
                },
            }

            if has_markdown {
                ContextMenuItem {
                    label: "Markdown",
                    on_click: {
                        let file = current_file.clone().unwrap();
                        let start = block_source_line.unwrap();
                        let end = block_source_line_end.unwrap();
                        move |_| {
                            // Extract whole block source (no selected_text)
                            super::copy_markdown_source_direct(
                                file.clone(),
                                start,
                                end,
                                String::new(),
                            );
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some((path_value, start, end)) = block_path_with_range.clone() {
                ContextMenuItem {
                    label: if start != end {
                        format!("Path with Range ({start}-{end})")
                    } else {
                        format!("Path with Line ({start})")
                    },
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&path_value);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }
        }
    }
}
