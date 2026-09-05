use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::context_menu::ContextMenuItem;
use crate::components::icon::IconName;
use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::{shortcut_hint_for_context_action, Action, KeyContext};

/// Build a "path:start-end" or "path:line" string from file path and line range.
/// Returns `None` if file or either line number is unavailable.
pub(super) fn build_path_with_range(
    file: Option<&PathBuf>,
    start: Option<u32>,
    end: Option<u32>,
) -> Option<(String, u32, u32)> {
    let f = file?;
    let start = start?;
    let end = end?;
    let path_str = f.display().to_string();
    let formatted = if start != end {
        format!("{path_str}:{start}-{end}")
    } else {
        format!("{path_str}:{start}")
    };
    Some((formatted, start, end))
}

#[component]
pub(super) fn LinkContextItems(href: String, on_close: EventHandler<()>) -> Element {
    let state = use_context::<crate::state::AppState>();
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);

    rsx! {
        ContextMenuItem {
            label: "Open Link",
            shortcut: shortcut("file.open_link"),
            icon: Some(IconName::ExternalLink),
            on_click: {
                let on_close = on_close;
                move |_| {
                    dispatch_action(&Action::FileOpenLink, state);
                    on_close.call(());
                }
            },
        }

        ContextMenuItem {
            label: "Open Link in New Tab",
            shortcut: shortcut("file.open_link_in_new_tab"),
            icon: Some(IconName::Add),
            on_click: {
                let on_close = on_close;
                move |_| {
                    dispatch_action(&Action::FileOpenLinkInNewTab, state);
                    on_close.call(());
                }
            },
        }

        ContextMenuItem {
            label: "Copy Link Path",
            shortcut: shortcut("clipboard.copy_link_path"),
            icon: Some(IconName::Copy),
            on_click: {
                let href = href.clone();
                let on_close = on_close;
                move |_| {
                    crate::utils::clipboard::copy_text(&href);
                    crate::keybindings::dispatcher::show_action_feedback("Copied");
                    on_close.call(());
                }
            },
        }
    }
}
