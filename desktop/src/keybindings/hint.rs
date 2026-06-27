use crate::config::CONFIG;
use crate::keybindings::KeyContext;

/// Return a formatted shortcut hint for the given action.
///
/// Context bindings are preferred over global when both exist.
pub fn shortcut_hint_for_action(action: &str, context: Option<KeyContext>) -> Option<String> {
    let config = CONFIG.read();
    let bindings = &config.keybindings;

    let key = context
        .and_then(|ctx| find_key_for_action(bindings_for_context(bindings, ctx), action))
        .or_else(|| find_key_for_action(&bindings.global, action))?;
    Some(format_shortcut_hint(key))
}

/// Return a formatted shortcut hint from global keybindings.
pub fn shortcut_hint_for_global_action(action: &str) -> Option<String> {
    shortcut_hint_for_action(action, None)
}

/// Return the raw keybinding string from global keybindings.
pub fn shortcut_key_for_global_action(action: &str) -> Option<String> {
    let config = CONFIG.read();
    find_key_for_action(&config.keybindings.global, action).map(ToOwned::to_owned)
}

/// Return a formatted shortcut hint in the given context.
pub fn shortcut_hint_for_context_action(context: KeyContext, action: &str) -> Option<String> {
    shortcut_hint_for_action(action, Some(context))
}

fn find_key_for_action<'a>(
    bindings: &'a [crate::config::KeyAction],
    action: &str,
) -> Option<&'a str> {
    bindings
        .iter()
        .find(|ka| ka.action == action)
        .map(|ka| ka.key.as_str())
}

fn bindings_for_context(
    bindings: &crate::config::BindingSet,
    context: KeyContext,
) -> &[crate::config::KeyAction] {
    match context {
        KeyContext::Content => &bindings.content,
        KeyContext::Sidebar => &bindings.sidebar,
        KeyContext::QuickAccess => &bindings.quick_access,
        KeyContext::RightSidebar => &bindings.right_sidebar,
        KeyContext::Search => &bindings.search,
    }
}

/// Convert keybinding notation into a menu-style shortcut hint.
///
/// Examples:
/// - `Cmd+Shift+o` -> `⌘⇧O`
/// - `Ctrl+w h` -> `⌃W H`
pub fn format_shortcut_hint(key: &str) -> String {
    key.split_whitespace()
        .map(format_chord_hint)
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_chord_hint(chord: &str) -> String {
    let mut parts = chord
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return chord.to_string();
    }

    let key_part = parts.pop().unwrap();
    let mut out = String::new();
    for modifier in parts {
        match modifier.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" => out.push('⌘'),
            "ctrl" | "control" => out.push('⌃'),
            "shift" => out.push('⇧'),
            "alt" | "option" => out.push('⌥'),
            other => {
                out.push_str(other);
                out.push('+');
            }
        }
    }
    out.push_str(&format_key_name_hint(key_part));
    out
}

fn format_key_name_hint(key: &str) -> String {
    match key {
        "ArrowUp" => "↑".to_string(),
        "ArrowDown" => "↓".to_string(),
        "ArrowLeft" => "←".to_string(),
        "ArrowRight" => "→".to_string(),
        "Backspace" => "⌫".to_string(),
        "Enter" => "↩".to_string(),
        "Escape" => "⎋".to_string(),
        "Tab" => "⇥".to_string(),
        "Space" => "␠".to_string(),
        "PageUp" => "⇞".to_string(),
        "PageDown" => "⇟".to_string(),
        "Home" => "↖".to_string(),
        "End" => "↘".to_string(),
        _ => key.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_shortcut_hint_uses_menu_style_symbols() {
        assert_eq!(format_shortcut_hint("Cmd+Shift+o"), "⌘⇧O");
        assert_eq!(format_shortcut_hint("Ctrl+w h"), "⌃W H");
    }

    #[test]
    fn format_shortcut_hint_formats_arrow_keys() {
        assert_eq!(format_shortcut_hint("ArrowDown"), "↓");
        assert_eq!(format_shortcut_hint("Cmd+ArrowLeft"), "⌘←");
    }

    #[test]
    fn helper_wrappers_match_base_api() {
        assert_eq!(
            shortcut_hint_for_global_action("no.such.action"),
            shortcut_hint_for_action("no.such.action", None)
        );
        assert_eq!(
            shortcut_hint_for_context_action(KeyContext::Content, "no.such.action"),
            shortcut_hint_for_action("no.such.action", Some(KeyContext::Content))
        );
    }
}
