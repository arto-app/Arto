use dioxus::html::{Key, Modifiers};
use dioxus_desktop::muda::accelerator::{Accelerator, Code, Modifiers as MudaModifiers};

use crate::config::BindingSet;
use crate::shortcut::ShortcutSequence;

/// Convert a single-chord config key string into a native muda `Accelerator`.
///
/// Menu shortcuts are dispatched by the OS via muda accelerators, which only
/// support a single modifier+key chord. This returns `None` for:
/// - multi-chord sequences (e.g. vim `g g`)
/// - keys with no physical `Code` mapping
///
/// In those cases the caller keeps the cosmetic menu hint and lets the
/// keybinding engine handle the shortcut instead.
pub fn accelerator_for_key(key: &str) -> Option<Accelerator> {
    let sequence: ShortcutSequence = key.parse().ok()?;
    let [chord] = sequence.chords.as_slice() else {
        // Only single-chord shortcuts can become native accelerators.
        return None;
    };

    let code = key_to_code(&chord.key)?;
    let modifiers = modifiers_to_muda(chord.modifiers);
    let modifiers = (!modifiers.is_empty()).then_some(modifiers);
    Some(Accelerator::new(modifiers, code))
}

/// Canonical skip key (`"<modifier-bits>:<physical-code>"`) for a menu shortcut,
/// matching the JS interceptor's `canonicalChord`.
///
/// The key part is the **physical** `Code` (e.g. `"KeyW"`), matching how muda
/// dispatches accelerators, not the logical key. This is required because
/// Alt/Option remaps `KeyboardEvent.key` on macOS (e.g. `Cmd+Alt+w` reports a
/// different glyph), which would break a logical-key comparison; `event.code`
/// is modifier-stable.
///
/// Returns `None` unless the key is a single chord that maps to a physical
/// code — only those are OS-dispatched and would otherwise double-fire.
pub fn menu_accelerator_skip_key(key: &str) -> Option<String> {
    let sequence: ShortcutSequence = key.parse().ok()?;
    let [chord] = sequence.chords.as_slice() else {
        return None;
    };
    let code = key_to_code(&chord.key)?;
    Some(format!("{}:{}", chord.modifiers.bits(), code))
}

/// Canonical skip keys for all native menu shortcuts in a binding set.
///
/// The keybinding engine forwards these to the JS interceptor so it can avoid
/// double-dispatching chords the OS menu already handles. Only entries whose
/// action actually maps to a menu item are included — a shortcut bound to a
/// non-menu action attaches to no menu item, so skipping it would leave the
/// chord dead.
pub fn menu_accelerator_skip_keys(bindings: &BindingSet) -> Vec<String> {
    bindings
        .menu_shortcuts
        .iter()
        .filter(|ka| super::is_menu_action(&ka.action))
        .filter_map(|ka| menu_accelerator_skip_key(&ka.key))
        .collect()
}

/// Map dioxus modifier flags to muda (keyboard_types) modifiers.
///
/// `META` (Cmd on macOS) maps to `SUPER`; `Accelerator::new` also normalizes
/// `META` to `SUPER` internally, so this keeps the two representations aligned.
fn modifiers_to_muda(modifiers: Modifiers) -> MudaModifiers {
    let mut out = MudaModifiers::empty();
    if modifiers.contains(Modifiers::CONTROL) {
        out |= MudaModifiers::CONTROL;
    }
    if modifiers.contains(Modifiers::ALT) {
        out |= MudaModifiers::ALT;
    }
    if modifiers.contains(Modifiers::SHIFT) {
        out |= MudaModifiers::SHIFT;
    }
    if modifiers.contains(Modifiers::META) {
        out |= MudaModifiers::SUPER;
    }
    out
}

/// Map a logical `Key` to a physical muda `Code`, or `None` when unmappable.
fn key_to_code(key: &Key) -> Option<Code> {
    let code = match key {
        Key::Character(c) => return char_to_code(c),
        Key::Enter => Code::Enter,
        Key::Escape => Code::Escape,
        Key::Tab => Code::Tab,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::ArrowUp => Code::ArrowUp,
        Key::ArrowDown => Code::ArrowDown,
        Key::ArrowLeft => Code::ArrowLeft,
        Key::ArrowRight => Code::ArrowRight,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::F1 => Code::F1,
        Key::F2 => Code::F2,
        Key::F3 => Code::F3,
        Key::F4 => Code::F4,
        Key::F5 => Code::F5,
        Key::F6 => Code::F6,
        Key::F7 => Code::F7,
        Key::F8 => Code::F8,
        Key::F9 => Code::F9,
        Key::F10 => Code::F10,
        Key::F11 => Code::F11,
        Key::F12 => Code::F12,
        _ => return None,
    };
    Some(code)
}

/// Map a single logical character to a physical `Code`.
///
/// Letters/digits reuse the W3C UI Events code names via `Code`'s `FromStr`
/// (e.g. `"KeyO"`, `"Digit0"`); symbols map explicitly.
fn char_to_code(character: &str) -> Option<Code> {
    let mut chars = character.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        // Multi-character logical keys have no single physical code.
        return None;
    }

    match ch {
        'a'..='z' => format!("Key{}", ch.to_ascii_uppercase()).parse().ok(),
        'A'..='Z' => format!("Key{ch}").parse().ok(),
        '0'..='9' => format!("Digit{ch}").parse().ok(),
        '[' => Some(Code::BracketLeft),
        ']' => Some(Code::BracketRight),
        '=' => Some(Code::Equal),
        '-' => Some(Code::Minus),
        '/' => Some(Code::Slash),
        '\\' => Some(Code::Backslash),
        ',' => Some(Code::Comma),
        '.' => Some(Code::Period),
        ';' => Some(Code::Semicolon),
        '\'' => Some(Code::Quote),
        '`' => Some(Code::Backquote),
        ' ' => Some(Code::Space),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_letter_maps_to_super_key_code() {
        assert_eq!(
            accelerator_for_key("Cmd+o"),
            Some(Accelerator::new(Some(MudaModifiers::SUPER), Code::KeyO))
        );
    }

    #[test]
    fn cmd_shift_letter_includes_shift() {
        assert_eq!(
            accelerator_for_key("Cmd+Shift+o"),
            Some(Accelerator::new(
                Some(MudaModifiers::SUPER | MudaModifiers::SHIFT),
                Code::KeyO
            ))
        );
    }

    #[test]
    fn symbol_aliases_map_to_codes() {
        assert_eq!(
            accelerator_for_key("Cmd+BracketLeft"),
            Some(Accelerator::new(
                Some(MudaModifiers::SUPER),
                Code::BracketLeft
            ))
        );
        assert_eq!(
            accelerator_for_key("Cmd+Equal"),
            Some(Accelerator::new(Some(MudaModifiers::SUPER), Code::Equal))
        );
        assert_eq!(
            accelerator_for_key("Cmd+Minus"),
            Some(Accelerator::new(Some(MudaModifiers::SUPER), Code::Minus))
        );
    }

    #[test]
    fn digit_maps_to_digit_code() {
        assert_eq!(
            accelerator_for_key("Cmd+0"),
            Some(Accelerator::new(Some(MudaModifiers::SUPER), Code::Digit0))
        );
    }

    #[test]
    fn no_modifier_named_key() {
        assert_eq!(
            accelerator_for_key("ArrowDown"),
            Some(Accelerator::new(None, Code::ArrowDown))
        );
    }

    #[test]
    fn ctrl_alt_combination() {
        assert_eq!(
            accelerator_for_key("Cmd+Alt+w"),
            Some(Accelerator::new(
                Some(MudaModifiers::SUPER | MudaModifiers::ALT),
                Code::KeyW
            ))
        );
    }

    #[test]
    fn multi_chord_sequence_is_not_an_accelerator() {
        assert_eq!(accelerator_for_key("g g"), None);
    }

    #[test]
    fn invalid_key_returns_none() {
        assert_eq!(accelerator_for_key("no_such_key"), None);
        assert_eq!(accelerator_for_key(""), None);
    }

    // -- Skip-key canonical form (must match JS interceptor's event.code) --
    //
    // dioxus Modifiers bits: ALT=0x01, CONTROL=0x08, META=0x40, SHIFT=0x200.
    // Key part is the physical Code (e.g. "KeyO"), not the logical glyph.

    #[test]
    fn skip_key_cmd_letter() {
        assert_eq!(
            menu_accelerator_skip_key("Cmd+o").as_deref(),
            Some("64:KeyO")
        );
    }

    #[test]
    fn skip_key_cmd_shift_letter() {
        // META (0x40=64) | SHIFT (0x200=512) = 576
        assert_eq!(
            menu_accelerator_skip_key("Cmd+Shift+o").as_deref(),
            Some("576:KeyO")
        );
    }

    #[test]
    fn skip_key_uses_physical_code_for_symbols() {
        assert_eq!(
            menu_accelerator_skip_key("Cmd+BracketLeft").as_deref(),
            Some("64:BracketLeft")
        );
    }

    #[test]
    fn skip_key_alt_uses_stable_physical_code() {
        // META (64) | ALT (1) = 65; physical code is immune to Option remapping.
        assert_eq!(
            menu_accelerator_skip_key("Cmd+Alt+w").as_deref(),
            Some("65:KeyW")
        );
    }

    #[test]
    fn skip_key_none_for_sequence() {
        assert_eq!(menu_accelerator_skip_key("g g"), None);
    }

    #[test]
    fn skip_keys_collects_only_representable_menu_shortcuts() {
        use crate::config::KeyAction;
        let bindings = BindingSet {
            menu_shortcuts: vec![
                KeyAction {
                    key: "Cmd+o".to_string(),
                    action: "file.open".to_string(),
                },
                // A hand-edited non-representable entry is skipped, not panicked.
                KeyAction {
                    key: "g g".to_string(),
                    action: "history.back".to_string(),
                },
            ],
            ..Default::default()
        };
        assert_eq!(menu_accelerator_skip_keys(&bindings), vec!["64:KeyO"]);
    }
}
