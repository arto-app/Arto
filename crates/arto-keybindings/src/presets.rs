//! Binding sets shipped with Arto.
//!
//! Each preset is a JSON document embedded at compile time and validated when
//! loaded, so a broken preset fails the first test or launch rather than
//! silently losing shortcuts.

use std::collections::HashSet;
use std::str::FromStr;

use crate::action::Action;
use crate::bindings::{BindingSet, KeyAction};
use crate::shortcut::ShortcutSequence;

pub mod default {
    use crate::bindings::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/default.json"), "default")
    }
}

pub mod vim {
    use crate::bindings::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/vim.json"), "vim")
    }
}

pub mod emacs {
    use crate::bindings::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/emacs.json"), "emacs")
    }
}

/// Default bindings for fresh installs.
///
/// Delegates to the Default preset (browser-style Cmd+Key shortcuts). The
/// app's config loader uses it to populate the binding set when the user
/// has none yet.
pub fn default_bindings() -> BindingSet {
    default::bindings()
}

fn parse_bindings_json(json: &str, name: &str) -> BindingSet {
    let bindings: BindingSet =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{name} preset must be valid: {e}"));
    validate_preset_bindings(name, &bindings);
    bindings
}

fn validate_preset_bindings(name: &str, bindings: &BindingSet) {
    let fields: [(&str, &Vec<KeyAction>); 6] = [
        ("global", &bindings.global),
        ("content", &bindings.content),
        ("sidebar", &bindings.sidebar),
        ("quick_access", &bindings.quick_access),
        ("right_sidebar", &bindings.right_sidebar),
        ("search", &bindings.search),
    ];

    for (context, actions) in fields {
        let mut seen_sequences = HashSet::new();
        for ka in actions {
            let sequence = ShortcutSequence::from_str(&ka.key).unwrap_or_else(|e| {
                panic!(
                    "{name} preset has invalid shortcut in {context}: key={:?}, error={}",
                    ka.key, e
                )
            });
            Action::from_str(&ka.action).unwrap_or_else(|e| {
                panic!(
                    "{name} preset has unknown action in {context}: action={:?}, error={}",
                    ka.action, e
                )
            });
            let normalized = sequence.to_string();
            assert!(
                seen_sequences.insert(normalized.clone()),
                "{name} preset has duplicate shortcut in {context}: {normalized:?}"
            );
        }
    }

    validate_menu_shortcuts(name, &bindings.menu_shortcuts);
}

/// Menu shortcuts have stricter requirements than engine keybindings: they
/// become native OS accelerators, which must be a single chord representable
/// as a muda `Accelerator`.
fn validate_menu_shortcuts(name: &str, actions: &[KeyAction]) {
    let mut seen_sequences = HashSet::new();
    for ka in actions {
        Action::from_str(&ka.action).unwrap_or_else(|e| {
            panic!(
                "{name} preset has unknown action in menu_shortcuts: action={:?}, error={}",
                ka.action, e
            )
        });
        assert!(
            crate::codes::single_chord_code(&ka.key).is_some(),
            "{name} preset menu shortcut is not a valid single-chord accelerator: key={:?}",
            ka.key
        );
        let normalized = ShortcutSequence::from_str(&ka.key).unwrap().to_string();
        assert!(
            seen_sequences.insert(normalized.clone()),
            "{name} preset has duplicate menu shortcut: {normalized:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_resolves() {
        let resolved = default::bindings().into_resolved_bindings();
        assert!(!resolved.is_empty());
    }

    #[test]
    fn vim_preset_resolves() {
        let resolved = vim::bindings().into_resolved_bindings();
        assert!(!resolved.is_empty());
    }

    #[test]
    fn emacs_preset_resolves() {
        let resolved = emacs::bindings().into_resolved_bindings();
        assert!(!resolved.is_empty());
    }

    #[test]
    fn presets_have_no_binding_errors() {
        for bindings in [default::bindings(), vim::bindings(), emacs::bindings()] {
            assert!(bindings.binding_errors().is_empty());
        }
    }
}
