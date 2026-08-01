use dioxus::prelude::*;
use std::path::PathBuf;

use super::source_ops::build_path_with_range;
use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};

/// "Copy Table As..." submenu: TSV / CSV / Markdown / Path with Range
#[component]
pub(super) fn CopyTableAsSubmenu(
    table_tsv: Option<String>,
    table_csv: Option<String>,
    table_markdown: Option<String>,
    current_file: Option<PathBuf>,
    table_source_line: Option<u32>,
    table_source_line_end: Option<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let table_path_with_range = build_path_with_range(
        current_file.as_ref(),
        table_source_line,
        table_source_line_end,
    );

    rsx! {
        ContextMenuSubmenu {
            label: "Copy Table As...",

            if let Some(tsv) = table_tsv {
                ContextMenuItem {
                    label: "TSV",
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&tsv);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some(csv) = table_csv {
                ContextMenuItem {
                    label: "CSV",
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&csv);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some(markdown) = table_markdown {
                ContextMenuItem {
                    label: "Markdown",
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&markdown);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some((path_value, start, end)) = table_path_with_range.clone() {
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
