//! Native menu accelerators for menu shortcuts.
//!
//! arto-keybindings resolves a shortcut to a physical code; this is the one
//! step that needs muda, so it stays in the app.

use dioxus::html::Modifiers;
use dioxus_desktop::muda::accelerator::{Accelerator, Modifiers as MudaModifiers};

use arto_keybindings::single_chord_code;

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
    let (modifiers, code) = single_chord_code(key)?;
    let modifiers = modifiers_to_muda(modifiers);
    let modifiers = (!modifiers.is_empty()).then_some(modifiers);
    Some(Accelerator::new(modifiers, code))
}

/// Map keyboard modifier flags to muda modifiers.
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

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_desktop::muda::accelerator::Code;

    /// muda modifier a parsed `Cmd`/`Meta` chord resolves to on the host: `SUPER`
    /// (Cmd) on macOS, `CONTROL` on Windows/Linux where the primary modifier is
    /// remapped META → CONTROL at parse time.
    #[cfg(target_os = "macos")]
    const PRIMARY_MUDA: MudaModifiers = MudaModifiers::SUPER;
    #[cfg(not(target_os = "macos"))]
    const PRIMARY_MUDA: MudaModifiers = MudaModifiers::CONTROL;

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
}
