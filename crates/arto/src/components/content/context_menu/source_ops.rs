use dioxus::prelude::*;
use rust_i18n::t;
use std::path::PathBuf;

use crate::components::context_menu::ContextMenuItem;
use crate::components::icon::IconName;
use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::{shortcut_hint_for_context_action, Action, KeyContext};
use crate::utils::source_lines::format_location;

/// Build a "path:start-end" or "path:line" string from file path and line range.
/// Returns `None` if file or either line number is unavailable.
pub(super) fn build_path_with_range(
    file: Option<&PathBuf>,
    start: Option<u32>,
    end: Option<u32>,
) -> Option<(String, u32, u32)> {
    let (file, start, end) = (file?, start?, end?);
    Some((format_location(file, start, end), start, end))
}

#[component]
pub(super) fn LinkContextItems(href: String, on_close: EventHandler<()>) -> Element {
    let state = use_context::<crate::state::AppState>();
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);

    rsx! {
        ContextMenuItem {
            label: t!("context_menu.link.open").to_string(),
            shortcut: shortcut("file.open_link"),
            icon: Some(IconName::Link),
            on_click: {
                let on_close = on_close;
                move |_| {
                    dispatch_action(&Action::FileOpenLink, state);
                    on_close.call(());
                }
            },
        }

        ContextMenuItem {
            label: t!("context_menu.link.open_in_new_window").to_string(),
            shortcut: shortcut("file.open_link_in_new_window"),
            icon: Some(IconName::AppWindow),
            on_click: {
                let on_close = on_close;
                move |_| {
                    dispatch_action(&Action::FileOpenLinkInNewWindow, state);
                    on_close.call(());
                }
            },
        }

        ContextMenuItem {
            label: t!("context_menu.link.copy_path").to_string(),
            shortcut: shortcut("clipboard.copy_link_path"),
            icon: Some(IconName::Copy),
            on_click: {
                let href = href.clone();
                let on_close = on_close;
                move |_| {
                    crate::utils::clipboard::copy_text(&href);
                    crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                    on_close.call(());
                }
            },
        }
    }
}
