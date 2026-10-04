use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;
use crate::highlights::{HighlightColor, HighlightId};
use crate::keybindings::dispatcher::highlight_selection;
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};
use crate::state::AppState;

/// "Highlight" (a colour for the selection), a note on the highlight under
/// the pointer, and "Remove Highlight" (the highlights the selection, or the
/// click, touches).
#[component]
pub(super) fn HighlightItems(
    has_selection: bool,
    highlight_ids: Vec<String>,
    highlight_under_pointer: Option<String>,
    on_close: EventHandler<()>,
) -> Element {
    let state = use_context::<AppState>();
    let shortcut = shortcut_hint_for_context_action(KeyContext::Content, "highlight.add");
    let remove = shortcut_hint_for_context_action(KeyContext::Content, "highlight.remove");
    let noted = highlight_under_pointer
        .map(HighlightId::from)
        .and_then(|id| {
            let highlights = state.highlights.read();
            let highlight = highlights.iter().find(|highlight| highlight.id == id)?;
            Some((id, note_label(highlight.note.as_deref())))
        });

    rsx! {
        if has_selection {
            ContextMenuSubmenu {
                label: t!("context_menu.highlight.title").to_string(),
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

        if let Some((id, label)) = noted {
            ContextMenuItem {
                label,
                icon: Some(IconName::Edit),
                on_click: move |_| {
                    on_close.call(());
                    crate::highlights::card::open_at(state, id.clone());
                },
            }
        }

        if !highlight_ids.is_empty() {
            ContextMenuItem {
                label: t!("context_menu.highlight.remove").to_string(),
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

/// The item that opens a highlight's card, named for what the reader will
/// do there.
fn note_label(note: Option<&str>) -> String {
    if note.is_some() {
        t!("context_menu.highlight.edit_note").to_string()
    } else {
        t!("context_menu.highlight.add_note").to_string()
    }
}

fn color_label(color: HighlightColor) -> String {
    match color {
        HighlightColor::Green => t!("context_menu.highlight.green"),
        HighlightColor::Blue => t!("context_menu.highlight.blue"),
        HighlightColor::Pink => t!("context_menu.highlight.pink"),
        HighlightColor::Orange => t!("context_menu.highlight.orange"),
        HighlightColor::Purple => t!("context_menu.highlight.purple"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_item_offers_to_add_a_note_or_to_edit_the_one_there() {
        assert_eq!(note_label(None), "Add Note\u{2026}");
        assert_eq!(note_label(Some("remember")), "Edit Note\u{2026}");
    }
}
