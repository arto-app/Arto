use dioxus::document;
use dioxus::prelude::*;

use crate::keybindings::{self, Action, KeyContext};

use super::keybinding_engine::KeyEventData;

const MOD_CONTROL: u32 = 0x08;

#[derive(Clone, PartialEq)]
pub(super) struct ShortcutHelpItem {
    key: String,
    action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShortcutOverlayVisibility {
    Hidden,
    Visible,
    Closing,
}

/// Build effective shortcut list for the currently focused context.
///
/// Context-specific bindings override global bindings when the same key sequence exists.
pub(super) fn build_shortcut_help_items(context: KeyContext) -> Vec<ShortcutHelpItem> {
    use std::collections::BTreeMap;

    let bindings = keybindings::resolve_bindings(&crate::config::CONFIG.read().keybindings);
    let mut map = BTreeMap::<String, (Option<KeyContext>, Action)>::new();

    for binding in bindings {
        if binding.context.is_some() && binding.context != Some(context) {
            continue;
        }

        let sequence = binding.sequence.to_string();
        match map.get(&sequence) {
            None => {
                map.insert(sequence, (binding.context, binding.action));
            }
            Some((existing_context, _)) => {
                if existing_context.is_none() && binding.context.is_some() {
                    map.insert(sequence, (binding.context, binding.action));
                }
            }
        }
    }

    map.into_iter()
        .map(|(key, (_source_ctx, action))| ShortcutHelpItem {
            key,
            action: action_label(action),
        })
        .collect()
}

fn shortcut_column_count(window_width: u32) -> usize {
    match window_width {
        0..=760 => 1,
        761..=1080 => 2,
        1081..=1400 => 3,
        1401..=1720 => 4,
        _ => 5,
    }
}

pub(super) fn split_shortcut_help_columns(
    items: Vec<ShortcutHelpItem>,
    window_width: u32,
) -> Vec<Vec<ShortcutHelpItem>> {
    if items.is_empty() {
        return Vec::new();
    }
    let columns = shortcut_column_count(window_width).min(items.len());
    let rows_per_column = items.len().div_ceil(columns);

    let mut result = Vec::with_capacity(columns);
    for col in 0..columns {
        let start = col * rows_per_column;
        if start >= items.len() {
            break;
        }
        let end = (start + rows_per_column).min(items.len());
        result.push(items[start..end].to_vec());
    }
    result
}

fn action_label(action: Action) -> String {
    let action_name = action.to_string();
    let display_name = action_name
        .strip_prefix("clipboard.")
        .or_else(|| action_name.strip_prefix("help."))
        .unwrap_or(action_name.as_str());

    display_name
        .split('.')
        .flat_map(|part| part.split('_'))
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn is_shortcut_overlay_visible(visibility: Signal<ShortcutOverlayVisibility>) -> bool {
    !matches!(*visibility.read(), ShortcutOverlayVisibility::Hidden)
}

pub(super) fn close_shortcut_overlay(mut visibility: Signal<ShortcutOverlayVisibility>) {
    if matches!(*visibility.read(), ShortcutOverlayVisibility::Hidden) {
        return;
    }
    visibility.set(ShortcutOverlayVisibility::Closing);
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(140)).await;
        if matches!(*visibility.read(), ShortcutOverlayVisibility::Closing) {
            visibility.set(ShortcutOverlayVisibility::Hidden);
        }
    });
}

pub(super) fn toggle_shortcut_overlay(mut visibility: Signal<ShortcutOverlayVisibility>) {
    let current = *visibility.read();
    match current {
        ShortcutOverlayVisibility::Hidden => visibility.set(ShortcutOverlayVisibility::Visible),
        ShortcutOverlayVisibility::Visible | ShortcutOverlayVisibility::Closing => {
            close_shortcut_overlay(visibility);
        }
    }
}

pub(super) fn handle_shortcut_overlay_scroll_key(data: &KeyEventData) -> bool {
    let base = data.key.as_str();
    let ctrl_only = data.modifiers == MOD_CONTROL;
    let no_mod = data.modifiers == 0;
    let delta = if no_mod && (base == "j" || base == "ArrowDown") {
        Some(56)
    } else if no_mod && (base == "k" || base == "ArrowUp") {
        Some(-56)
    } else if ctrl_only && (base.eq_ignore_ascii_case("n")) {
        Some(56)
    } else if ctrl_only && (base.eq_ignore_ascii_case("p")) {
        Some(-56)
    } else {
        None
    };
    if let Some(px) = delta {
        scroll_shortcut_help_list(px);
        return true;
    }
    false
}

pub(super) fn handle_shortcut_overlay_close_key(
    data: &KeyEventData,
    visibility: Signal<ShortcutOverlayVisibility>,
) -> bool {
    if data.modifiers == 0 && data.key == "Escape" {
        close_shortcut_overlay(visibility);
        return true;
    }
    false
}

fn scroll_shortcut_help_list(px: i32) {
    let script = format!(
        r#"
        (() => {{
            const list = document.querySelector('.shortcut-help-list');
            if (!list) return;
            list.scrollBy({{ top: {px}, behavior: 'smooth' }});
        }})();
        "#
    );
    let _ = document::eval(&script);
}

#[component]
pub(super) fn ShortcutHelpOverlay(
    columns: Vec<Vec<ShortcutHelpItem>>,
    is_closing: bool,
    on_close: EventHandler<()>,
) -> Element {
    let columns_len = columns.len().max(1);
    let columns_style = format!("--shortcut-columns: {columns_len};");

    rsx! {
        div {
            class: if is_closing { "shortcut-help-overlay is-closing" } else { "shortcut-help-overlay" },
            onclick: move |_| on_close.call(()),
            div {
                class: if is_closing { "shortcut-help-panel is-closing" } else { "shortcut-help-panel" },
                onclick: move |evt| evt.stop_propagation(),
                div {
                    class: "shortcut-help-list",
                    div {
                        class: "shortcut-help-columns",
                        style: "{columns_style}",
                        for (index, column_items) in columns.into_iter().enumerate() {
                            div {
                                class: if index == 0 { "shortcut-help-column" } else { "shortcut-help-column shortcut-help-column--with-separator" },
                                table {
                                    class: "shortcut-help-table",
                                    tbody {
                                        for item in column_items {
                                            tr {
                                                class: "shortcut-help-row",
                                                td {
                                                    class: "shortcut-help-cell shortcut-help-cell--key",
                                                    span { class: "shortcut-help-key", title: "{item.key}", "{item.key}" }
                                                }
                                                td {
                                                    class: "shortcut-help-cell shortcut-help-cell--action",
                                                    span { class: "shortcut-help-action", title: "{item.action}", "{item.action}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_count_breakpoints() {
        assert_eq!(shortcut_column_count(700), 1);
        assert_eq!(shortcut_column_count(800), 2);
        assert_eq!(shortcut_column_count(1200), 3);
        assert_eq!(shortcut_column_count(1500), 4);
        assert_eq!(shortcut_column_count(1900), 5);
    }

    #[test]
    fn split_shortcuts_into_balanced_columns() {
        let items = (0..10)
            .map(|i| ShortcutHelpItem {
                key: format!("k{i}"),
                action: format!("a{i}"),
            })
            .collect::<Vec<_>>();
        let columns = split_shortcut_help_columns(items, 1200);
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].len(), 4);
        assert_eq!(columns[1].len(), 4);
        assert_eq!(columns[2].len(), 2);
    }
}
