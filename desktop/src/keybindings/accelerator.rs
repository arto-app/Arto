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

/// Letters bound as a bare primary-modifier chord anywhere in the active
/// bindings, lowercased (e.g. `["c", "v", "x"]`).
///
/// The primary modifier is Cmd (`META`) on macOS and Ctrl (`CONTROL`) on
/// Windows/Linux, matching the parse-time remap in `shortcut.rs`. A chord counts
/// when it uses *exactly* the primary modifier (no Shift/Alt/secondary) and a
/// single ASCII letter, in *any* position of *any* sequence, in *any* context —
/// so the later `Ctrl+c` of emacs `Ctrl+x Ctrl+c` is included alongside the
/// `Ctrl+x` prefix.
///
/// The keybinding engine forwards these to the JS interceptor, which treats a
/// small set of primary+letter chords (Cmd/Ctrl + Q/C/V/X/A) as OS-reserved and
/// swallows them before the engine runs. Reporting the ones the config actually
/// binds lets the interceptor keep native clipboard/quit reserved while letting
/// bound chords (the whole emacs `C-x` prefix system, `C-v`) reach the engine.
pub fn reserved_key_overrides(bindings: &BindingSet) -> Vec<String> {
    reserved_key_overrides_impl(bindings, cfg!(target_os = "macos"))
}

/// Platform-agnostic core of [`reserved_key_overrides`], extracted so both
/// primary-modifier branches can be unit-tested from any host.
fn reserved_key_overrides_impl(bindings: &BindingSet, is_macos: bool) -> Vec<String> {
    let primary = if is_macos {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let mut letters = std::collections::BTreeSet::new();
    for binding in bindings.clone().into_resolved_bindings() {
        for chord in &binding.sequence.chords {
            if chord.modifiers != primary {
                continue;
            }
            if let Key::Character(ch) = &chord.key {
                let mut chars = ch.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphabetic() => {
                        letters.insert(c.to_ascii_lowercase().to_string());
                    }
                    _ => {}
                }
            }
        }
    }
    letters.into_iter().collect()
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

    /// muda modifier a parsed `Cmd`/`Meta` chord resolves to on the host: `SUPER`
    /// (Cmd) on macOS, `CONTROL` on Windows/Linux where the primary modifier is
    /// remapped META → CONTROL at parse time.
    #[cfg(target_os = "macos")]
    const PRIMARY_MUDA: MudaModifiers = MudaModifiers::SUPER;
    #[cfg(not(target_os = "macos"))]
    const PRIMARY_MUDA: MudaModifiers = MudaModifiers::CONTROL;

    /// dioxus modifier bits the primary accelerator carries in a skip key:
    /// META (0x40) on macOS, CONTROL (0x08) on Windows/Linux.
    #[cfg(target_os = "macos")]
    const PRIMARY_BITS: u32 = 0x40;
    #[cfg(not(target_os = "macos"))]
    const PRIMARY_BITS: u32 = 0x08;

    #[test]
    fn cmd_letter_maps_to_primary_key_code() {
        assert_eq!(
            accelerator_for_key("Cmd+o"),
            Some(Accelerator::new(Some(PRIMARY_MUDA), Code::KeyO))
        );
    }

    #[test]
    fn cmd_shift_letter_includes_shift() {
        assert_eq!(
            accelerator_for_key("Cmd+Shift+o"),
            Some(Accelerator::new(
                Some(PRIMARY_MUDA | MudaModifiers::SHIFT),
                Code::KeyO
            ))
        );
    }

    #[test]
    fn symbol_aliases_map_to_codes() {
        assert_eq!(
            accelerator_for_key("Cmd+BracketLeft"),
            Some(Accelerator::new(Some(PRIMARY_MUDA), Code::BracketLeft))
        );
        assert_eq!(
            accelerator_for_key("Cmd+Equal"),
            Some(Accelerator::new(Some(PRIMARY_MUDA), Code::Equal))
        );
        assert_eq!(
            accelerator_for_key("Cmd+Minus"),
            Some(Accelerator::new(Some(PRIMARY_MUDA), Code::Minus))
        );
    }

    #[test]
    fn digit_maps_to_digit_code() {
        assert_eq!(
            accelerator_for_key("Cmd+0"),
            Some(Accelerator::new(Some(PRIMARY_MUDA), Code::Digit0))
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
                Some(PRIMARY_MUDA | MudaModifiers::ALT),
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
    // Key part is the physical Code (e.g. "KeyO"), not the logical glyph. The
    // primary modifier (Cmd) is META on macOS and remapped to CONTROL elsewhere,
    // so the leading bits differ per platform (see PRIMARY_BITS).

    #[test]
    fn skip_key_cmd_letter() {
        assert_eq!(
            menu_accelerator_skip_key("Cmd+o"),
            Some(format!("{PRIMARY_BITS}:KeyO"))
        );
    }

    #[test]
    fn skip_key_cmd_shift_letter() {
        // primary | SHIFT (0x200=512)
        assert_eq!(
            menu_accelerator_skip_key("Cmd+Shift+o"),
            Some(format!("{}:KeyO", PRIMARY_BITS | 0x200))
        );
    }

    #[test]
    fn skip_key_uses_physical_code_for_symbols() {
        assert_eq!(
            menu_accelerator_skip_key("Cmd+BracketLeft"),
            Some(format!("{PRIMARY_BITS}:BracketLeft"))
        );
    }

    #[test]
    fn skip_key_alt_uses_stable_physical_code() {
        // primary | ALT (1); physical code is immune to Option remapping.
        assert_eq!(
            menu_accelerator_skip_key("Cmd+Alt+w"),
            Some(format!("{}:KeyW", PRIMARY_BITS | 0x01))
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
        assert_eq!(
            menu_accelerator_skip_keys(&bindings),
            vec![format!("{PRIMARY_BITS}:KeyO")]
        );
    }

    // -- Reserved-key overrides (config-aware OS-reserved gate) --------------

    #[test]
    fn reserved_overrides_off_macos_include_emacs_ctrl_prefix_letters() {
        // On Windows/Linux the primary modifier is Ctrl. The emacs preset binds
        // Ctrl+x (prefix), Ctrl+v (page down), and Ctrl+x Ctrl+c (quit) — the
        // reserved letters x, v, and the later-chord c must all be reported so
        // the interceptor stops swallowing them.
        let overrides =
            reserved_key_overrides_impl(&crate::keybindings::presets::emacs::bindings(), false);
        assert!(overrides.contains(&"x".to_string()));
        assert!(overrides.contains(&"v".to_string()));
        assert!(overrides.contains(&"c".to_string()));
    }

    #[test]
    fn reserved_overrides_on_macos_exclude_ctrl_only_chords() {
        // On macOS the primary modifier is Cmd, so emacs' Ctrl-based chords are
        // not primary chords and the gate never fires on them — they must not be
        // reported (and Cmd+C/V/X/A stay OS-reserved for native clipboard).
        let overrides =
            reserved_key_overrides_impl(&crate::keybindings::presets::emacs::bindings(), true);
        assert!(!overrides.contains(&"x".to_string()));
        assert!(!overrides.contains(&"v".to_string()));
        assert!(!overrides.contains(&"c".to_string()));
    }

    #[test]
    fn reserved_overrides_default_preset_leave_clipboard_reserved() {
        // The default preset binds none of Q/C/V/X/A with the primary modifier,
        // so on neither platform should those letters be overridden — the
        // interceptor keeps native clipboard/select-all/quit working.
        let bindings = crate::keybindings::default_bindings();
        for is_macos in [true, false] {
            let overrides = reserved_key_overrides_impl(&bindings, is_macos);
            for reserved in ["q", "c", "v", "x", "a"] {
                assert!(
                    !overrides.contains(&reserved.to_string()),
                    "default preset must not override reserved {reserved:?} (is_macos={is_macos})"
                );
            }
        }
    }

    #[test]
    fn reserved_overrides_ignore_shift_and_alt_variants() {
        use crate::config::KeyAction;
        // Only bare primary+letter chords count; Shift/Alt variants never trip
        // the interceptor's reserved gate, so they must not be reported.
        let bindings = BindingSet {
            global: vec![
                KeyAction {
                    key: "Ctrl+Shift+x".to_string(),
                    action: "tab.close".to_string(),
                },
                KeyAction {
                    key: "Ctrl+Alt+v".to_string(),
                    action: "scroll.page_down".to_string(),
                },
            ],
            ..Default::default()
        };
        assert!(reserved_key_overrides_impl(&bindings, false).is_empty());
    }
}
