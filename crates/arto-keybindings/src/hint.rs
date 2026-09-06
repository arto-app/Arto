use crate::bindings::{BindingSet, KeyAction};
use crate::context::KeyContext;

/// Return a formatted shortcut hint for `action` from `bindings`.
///
/// Lookup order: context binding → global keybinding → menu shortcut.
pub fn hint_for_action(
    bindings: &BindingSet,
    action: &str,
    context: Option<KeyContext>,
) -> Option<String> {
    let key = context
        .and_then(|ctx| find_key_for_action(bindings_for_context(bindings, ctx), action))
        .or_else(|| find_key_for_action(&bindings.global, action))
        .or_else(|| find_key_for_action(&bindings.menu_shortcuts, action))?;
    Some(format_shortcut_hint(key))
}

fn find_key_for_action<'a>(bindings: &'a [KeyAction], action: &str) -> Option<&'a str> {
    bindings
        .iter()
        .find(|ka| ka.action == action)
        .map(|ka| ka.key.as_str())
}

fn bindings_for_context(bindings: &BindingSet, context: KeyContext) -> &[KeyAction] {
    match context {
        KeyContext::Content => &bindings.content,
        KeyContext::Sidebar => &bindings.sidebar,
        KeyContext::QuickAccess => &bindings.quick_access,
        KeyContext::RightSidebar => &bindings.right_sidebar,
        KeyContext::Search => &bindings.search,
    }
}

/// Convert keybinding notation into a platform-appropriate shortcut hint.
///
/// macOS uses the compact menu-style symbol form; Windows/Linux use the plain
/// text form that matches their native conventions:
/// - `Cmd+Shift+o` -> `⌘⇧O` (macOS) / `Ctrl+Shift+O` (Windows/Linux)
/// - `Ctrl+w h` -> `⌃W H` (macOS) / `Ctrl+W H` (Windows/Linux)
///
/// `Cmd`/`Meta` is the primary accelerator, so it renders as `⌘` on macOS and
/// as `Ctrl` on Windows/Linux — mirroring the `META` -> `CONTROL` remap applied
/// when the same chord is parsed for matching.
pub fn format_shortcut_hint(key: &str) -> String {
    format_shortcut_hint_impl(key, cfg!(target_os = "macos"))
}

/// Platform-agnostic core of [`format_shortcut_hint`], extracted so both the
/// macOS symbol form and the Windows/Linux text form can be unit-tested from any
/// host.
fn format_shortcut_hint_impl(key: &str, is_macos: bool) -> String {
    key.split_whitespace()
        .map(|chord| format_chord_hint(chord, is_macos))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_chord_hint(chord: &str, is_macos: bool) -> String {
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
        push_modifier_hint(&mut out, modifier, is_macos);
    }
    out.push_str(&format_key_name_hint(key_part, is_macos));
    out
}

/// Append the rendered form of a single modifier token to `out`.
///
/// macOS uses the compact menu-style glyphs; Windows/Linux use plain text and
/// collapse `Cmd`/`Meta` onto `Ctrl`, the primary accelerator there.
fn push_modifier_hint(out: &mut String, modifier: &str, is_macos: bool) {
    if is_macos {
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
    } else {
        match modifier.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "ctrl" | "control" => out.push_str("Ctrl+"),
            "shift" => out.push_str("Shift+"),
            "alt" | "option" => out.push_str("Alt+"),
            other => {
                out.push_str(other);
                out.push('+');
            }
        }
    }
}

fn format_key_name_hint(key: &str, is_macos: bool) -> String {
    if is_macos {
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
    } else {
        match key {
            "ArrowUp" => "Up".to_string(),
            "ArrowDown" => "Down".to_string(),
            "ArrowLeft" => "Left".to_string(),
            "ArrowRight" => "Right".to_string(),
            "Escape" => "Esc".to_string(),
            "Backspace" | "Enter" | "Tab" | "Space" | "PageUp" | "PageDown" | "Home" | "End" => {
                key.to_string()
            }
            _ => key.to_uppercase(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_shortcut_hint_uses_menu_style_symbols_on_macos() {
        assert_eq!(format_shortcut_hint_impl("Cmd+Shift+o", true), "⌘⇧O");
        assert_eq!(format_shortcut_hint_impl("Ctrl+w h", true), "⌃W H");
    }

    #[test]
    fn format_shortcut_hint_uses_text_form_off_macos() {
        // Cmd/Meta renders as Ctrl, the primary accelerator on Windows/Linux.
        assert_eq!(
            format_shortcut_hint_impl("Cmd+Shift+o", false),
            "Ctrl+Shift+O"
        );
        assert_eq!(format_shortcut_hint_impl("Ctrl+w h", false), "Ctrl+W H");
    }

    #[test]
    fn format_shortcut_hint_formats_arrow_keys_on_macos() {
        assert_eq!(format_shortcut_hint_impl("ArrowDown", true), "↓");
        assert_eq!(format_shortcut_hint_impl("Cmd+ArrowLeft", true), "⌘←");
    }

    #[test]
    fn format_shortcut_hint_formats_arrow_keys_off_macos() {
        assert_eq!(format_shortcut_hint_impl("ArrowDown", false), "Down");
        assert_eq!(
            format_shortcut_hint_impl("Cmd+ArrowLeft", false),
            "Ctrl+Left"
        );
    }

    #[test]
    fn format_shortcut_hint_delegates_to_host_platform() {
        // The public entry point must match the impl for the host's platform.
        assert_eq!(
            format_shortcut_hint("Cmd+Shift+o"),
            format_shortcut_hint_impl("Cmd+Shift+o", cfg!(target_os = "macos"))
        );
    }

    #[test]
    fn hint_prefers_context_then_global_then_menu() {
        let bindings = BindingSet {
            menu_shortcuts: vec![KeyAction {
                key: "Cmd+w".to_string(),
                action: "tab.close".to_string(),
            }],
            global: vec![KeyAction {
                key: "x".to_string(),
                action: "tab.close".to_string(),
            }],
            sidebar: vec![KeyAction {
                key: "d".to_string(),
                action: "tab.close".to_string(),
            }],
            ..Default::default()
        };

        assert_eq!(
            hint_for_action(&bindings, "tab.close", Some(KeyContext::Sidebar)),
            Some(format_shortcut_hint("d"))
        );
        assert_eq!(
            hint_for_action(&bindings, "tab.close", Some(KeyContext::Content)),
            Some(format_shortcut_hint("x"))
        );
        assert_eq!(
            hint_for_action(&bindings, "tab.close", None),
            Some(format_shortcut_hint("x"))
        );

        let menu_only = BindingSet {
            menu_shortcuts: bindings.menu_shortcuts.clone(),
            ..Default::default()
        };
        assert_eq!(
            hint_for_action(&menu_only, "tab.close", None),
            Some(format_shortcut_hint("Cmd+w"))
        );
        assert_eq!(hint_for_action(&menu_only, "no.such.action", None), None);
    }
}
