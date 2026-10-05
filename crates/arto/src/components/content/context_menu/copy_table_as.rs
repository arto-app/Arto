use dioxus::prelude::*;
use rust_i18n::t;
use std::path::PathBuf;

use super::source_ops::build_path_with_range;
use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;

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
            label: t!("context_menu.copy_table_as").to_string(),
            icon: Some(IconName::Table),

            if let Some(tsv) = table_tsv {
                ContextMenuItem {
                    label: "TSV",
                    icon: Some(IconName::Table),
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&tsv);
                            crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some(csv) = table_csv {
                ContextMenuItem {
                    label: "CSV",
                    icon: Some(IconName::Table),
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&csv);
                            crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some(markdown) = table_markdown {
                ContextMenuItem {
                    label: "Markdown",
                    icon: Some(IconName::Markdown),
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&markdown);
                            crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some((path_value, start, end)) = table_path_with_range.clone() {
                ContextMenuItem {
                    label: if start != end {
                        t!("context_menu.path_with_range", start = start, end = end).to_string()
                    } else {
                        t!("context_menu.path_with_line", line = start).to_string()
                    },
                    on_click: {
                        move |_| {
                            crate::utils::clipboard::copy_text(&path_value);
                            crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                            on_close.call(());
                        }
                    },
                }
            }
        }
    }
}
