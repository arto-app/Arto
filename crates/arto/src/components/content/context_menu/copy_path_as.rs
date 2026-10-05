use dioxus::prelude::*;
use rust_i18n::t;
use std::path::PathBuf;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;

/// "Copy Path As..." submenu: Path / Path with Line / Path with Range
#[component]
pub(super) fn CopyPathAsSubmenu(
    current_file: PathBuf,
    source_line: Option<u32>,
    source_line_end: Option<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let has_range =
        source_line.is_some() && source_line_end.is_some() && source_line != source_line_end;
    let path_str = current_file.display().to_string();

    rsx! {
        ContextMenuSubmenu {
            label: t!("context_menu.copy_path_as").to_string(),
            icon: Some(IconName::Copy),

            ContextMenuItem {
                label: t!("context_menu.path").to_string(),
                icon: Some(IconName::File),
                on_click: {
                    let path_str = path_str.clone();
                    move |_| {
                        crate::utils::clipboard::copy_text(&path_str);
                        crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                        on_close.call(());
                    }
                },
            }

            if let Some(line) = source_line {
                ContextMenuItem {
                    label: t!("context_menu.path_with_line", line = line).to_string(),
                    icon: Some(IconName::File),
                    on_click: {
                        let value = format!("{path_str}:{line}");
                        move |_| {
                            crate::utils::clipboard::copy_text(&value);
                            crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                            on_close.call(());
                        }
                    },
                }
            }

            if has_range {
                if let (Some(start), Some(end)) = (source_line, source_line_end) {
                    ContextMenuItem {
                        label: t!("context_menu.path_with_range", start = start, end = end).to_string(),
                        icon: Some(IconName::File),
                        on_click: {
                            let value = format!("{path_str}:{start}-{end}");
                            move |_| {
                                crate::utils::clipboard::copy_text(&value);
                                crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                                on_close.call(());
                            }
                        },
                    }
                }
            }
        }
    }
}
