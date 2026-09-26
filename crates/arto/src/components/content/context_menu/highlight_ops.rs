use dioxus::prelude::*;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;
use crate::highlights::{HighlightColor, HighlightId};
use crate::keybindings::dispatcher::highlight_selection;
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};
use crate::state::AppState;

/// "Highlight" (a colour for the selection) and "Remove Highlight" (the
/// highlights the selection, or the click, touches).
#[component]
pub(super) fn HighlightItems(
    has_selection: bool,
    highlight_ids: Vec<String>,
    on_close: EventHandler<()>,
) -> Element {
    let state = use_context::<AppState>();
    let shortcut = shortcut_hint_for_context_action(KeyContext::Content, "highlight.add");
    let remove = shortcut_hint_for_context_action(KeyContext::Content, "highlight.remove");

    rsx! {
        if has_selection {
            ContextMenuSubmenu {
                label: "Highlight",
                icon: Some(IconName::Highlight),
                for color in HighlightColor::ALL {
                    ContextMenuItem {
                        key: "{color.to_js_name()}",
                        label: color_label(color),
                        // The key draws in the colour picked last; the hint
                        // stands beside that colour.
                        shortcut: if color == crate::highlights::last_color() { shortcut.clone() } else { None },
                        on_click: move |_| {
                            highlight_selection(&state, color, true);
                            on_close.call(());
                        },
                    }
                }
            }
        }

        if !highlight_ids.is_empty() {
            ContextMenuItem {
                label: "Remove Highlight",
                icon: Some(IconName::Trash),
                shortcut: remove,
                on_click: move |_| {
                    if let Some(file) = state.current_file() {
                        let ids: Vec<HighlightId> =
                            highlight_ids.iter().cloned().map(HighlightId::from).collect();
                        crate::highlights::remove_highlights(&file, &ids);
                    }
                    on_close.call(());
                },
            }
        }
    }
}

fn color_label(color: HighlightColor) -> &'static str {
    match color {
        HighlightColor::Green => "Green",
        HighlightColor::Blue => "Blue",
        HighlightColor::Pink => "Pink",
        HighlightColor::Orange => "Orange",
        HighlightColor::Purple => "Purple",
    }
}
