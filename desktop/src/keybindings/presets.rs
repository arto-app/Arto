use std::collections::HashSet;
use std::str::FromStr;

use crate::config::{BindingSet, KeyAction};
use crate::shortcut::ShortcutSequence;

use super::action::Action;
use super::context::KeyContext;

pub mod default {
    use crate::config::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/default.json"), "default")
    }
}

pub mod vim {
    use crate::config::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/vim.json"), "vim")
    }
}

pub mod emacs {
    use crate::config::BindingSet;

    pub fn bindings() -> BindingSet {
        super::parse_bindings_json(include_str!("presets/emacs.json"), "emacs")
    }
}

/// A fully resolved keybinding ready for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub sequence: ShortcutSequence,
    pub action: Action,
    pub context: Option<KeyContext>,
}

/// Default bindings for fresh installs.
///
/// Delegates to the Default preset (browser-style Cmd+Key shortcuts).
/// Used by `Config::load()` to auto-populate when no keybindings exist.
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
            super::accelerator_for_key(&ka.key).is_some(),
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

/// Resolve bindings from configuration for engine consumption.
///
/// All bindings live in `config.custom`. There is no hidden base layer.
/// `Config::load()` auto-populates custom with defaults for fresh installs.
pub fn resolve_bindings(bindings: &BindingSet) -> Vec<ResolvedBinding> {
    bindings.clone().into_resolved_bindings()
}

impl BindingSet {
    /// Flatten into resolved bindings for engine consumption.
    pub fn into_resolved_bindings(self) -> Vec<ResolvedBinding> {
        let mut result = Vec::new();
        // Platforms with a native menu (macOS/Linux) let the OS dispatch menu
        // shortcuts, so they are excluded here. Windows has no native menu, so
        // the engine must own them or they would have no dispatch path at all.
        #[cfg(target_os = "windows")]
        resolve_field(self.menu_shortcuts, None, &mut result);
        resolve_field(self.global, None, &mut result);
        resolve_field(self.content, Some(KeyContext::Content), &mut result);
        resolve_field(self.sidebar, Some(KeyContext::Sidebar), &mut result);
        resolve_field(
            self.quick_access,
            Some(KeyContext::QuickAccess),
            &mut result,
        );
        resolve_field(
            self.right_sidebar,
            Some(KeyContext::RightSidebar),
            &mut result,
        );
        resolve_field(self.search, Some(KeyContext::Search), &mut result);
        result
    }
}

/// Parse key actions into resolved bindings, logging warnings for invalid entries.
fn resolve_field(
    actions: Vec<KeyAction>,
    context: Option<KeyContext>,
    result: &mut Vec<ResolvedBinding>,
) {
    for ka in actions {
        let sequence = match ShortcutSequence::from_str(&ka.key) {
            Ok(seq) => seq,
            Err(e) => {
                tracing::warn!(key = %ka.key, error = %e, "Skipping keybinding: invalid key");
                continue;
            }
        };
        let action = match Action::from_str(&ka.action) {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(action = %ka.action, error = %e, "Skipping keybinding: unknown action");
                continue;
            }
        };
        result.push(ResolvedBinding {
            sequence,
            action,
            context,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Preset serde validation -----------------------------------------

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

    // -- Resolution engine logic -----------------------------------------

    #[test]
    fn empty_resolves_to_nothing() {
        let bindings = resolve_bindings(&BindingSet::default());
        assert!(bindings.is_empty());
    }

    #[test]
    fn bindings_resolve_directly() {
        let set = BindingSet {
            global: vec![KeyAction {
                key: "x".to_string(),
                action: "tab.close".to_string(),
            }],
            ..Default::default()
        };
        let bindings = resolve_bindings(&set);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action, Action::TabClose);
    }

    #[test]
    fn context_binding() {
        let set = BindingSet {
            sidebar: vec![KeyAction {
                key: "j".to_string(),
                action: "cursor.down".to_string(),
            }],
            ..Default::default()
        };
        let bindings = resolve_bindings(&set);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].context, Some(KeyContext::Sidebar));
        assert_eq!(bindings[0].action, Action::CursorDown);
    }

    #[test]
    fn invalid_bindings_skipped() {
        let set = BindingSet {
            global: vec![
                KeyAction {
                    key: "".to_string(),
                    action: "scroll.down".to_string(),
                },
                KeyAction {
                    key: "j".to_string(),
                    action: "invalid.action".to_string(),
                },
                KeyAction {
                    key: "k".to_string(),
                    action: "scroll.up".to_string(),
                },
            ],
            ..Default::default()
        };
        let bindings = resolve_bindings(&set);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action, Action::ScrollUp);
    }
}
