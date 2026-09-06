//! Turning a [`BindingSet`] into bindings the engine can match against.

use std::str::FromStr;

use crate::action::{Action, ActionParseError};
use crate::bindings::{BindingSet, KeyAction};
use crate::context::KeyContext;
use crate::shortcut::{ShortcutParseError, ShortcutSequence};

/// A fully resolved keybinding ready for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub sequence: ShortcutSequence,
    pub action: Action,
    pub context: Option<KeyContext>,
}

/// An entry of a binding set that cannot be resolved and is skipped.
///
/// These come from hand-edited configuration; the presets shipped with the
/// crate are validated at load time and never produce them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    #[error("invalid key {key:?}: {source}")]
    InvalidKey {
        key: String,
        #[source]
        source: ShortcutParseError,
    },
    #[error("unknown action {action:?}: {source}")]
    UnknownAction {
        action: String,
        #[source]
        source: ActionParseError,
    },
}

/// Resolve a binding set for engine consumption.
///
/// The caller supplies the effective binding set; there is no hidden base
/// layer merged underneath it.
pub fn resolve_bindings(bindings: &BindingSet) -> Vec<ResolvedBinding> {
    bindings.clone().into_resolved_bindings()
}

impl BindingSet {
    /// Resolve every entry, returning the usable bindings and the entries
    /// that had to be skipped, in one pass.
    ///
    /// Platforms with a native menu (macOS/Linux) let the OS dispatch menu
    /// shortcuts, so they are excluded. Windows has no native menu, so the
    /// engine must own them or they would have no dispatch path at all.
    pub fn resolve(self) -> (Vec<ResolvedBinding>, Vec<BindingError>) {
        self.resolve_with(cfg!(target_os = "windows"))
    }

    /// Flatten into resolved bindings for engine consumption, dropping the
    /// entries that fail to resolve. See [`BindingSet::resolve`] to get both.
    pub fn into_resolved_bindings(self) -> Vec<ResolvedBinding> {
        self.resolve().0
    }

    /// The entries [`BindingSet::resolve`] would skip.
    pub fn binding_errors(&self) -> Vec<BindingError> {
        self.clone().resolve().1
    }

    /// Core of [`BindingSet::resolve`] with the menu-shortcut folding decision
    /// passed explicitly, so both platform branches can be exercised from a
    /// single-host test run.
    #[cfg(test)]
    pub(crate) fn into_resolved_bindings_with(
        self,
        fold_menu_shortcuts: bool,
    ) -> Vec<ResolvedBinding> {
        self.resolve_with(fold_menu_shortcuts).0
    }

    fn resolve_with(self, fold_menu_shortcuts: bool) -> (Vec<ResolvedBinding>, Vec<BindingError>) {
        let mut resolved = Vec::new();
        let mut errors = Vec::new();
        let mut resolve = |actions: Vec<KeyAction>, context: Option<KeyContext>| {
            resolve_field(actions, context, &mut resolved, &mut errors);
        };
        if fold_menu_shortcuts {
            resolve(self.menu_shortcuts, None);
        }
        resolve(self.global, None);
        resolve(self.content, Some(KeyContext::Content));
        resolve(self.sidebar, Some(KeyContext::Sidebar));
        resolve(self.quick_access, Some(KeyContext::QuickAccess));
        resolve(self.right_sidebar, Some(KeyContext::RightSidebar));
        resolve(self.search, Some(KeyContext::Search));
        (resolved, errors)
    }
}

/// Parse key actions into resolved bindings, collecting the entries that
/// cannot be resolved instead of dropping them silently.
fn resolve_field(
    actions: Vec<KeyAction>,
    context: Option<KeyContext>,
    resolved: &mut Vec<ResolvedBinding>,
    errors: &mut Vec<BindingError>,
) {
    for ka in actions {
        let sequence = match ShortcutSequence::from_str(&ka.key) {
            Ok(seq) => seq,
            Err(source) => {
                errors.push(BindingError::InvalidKey {
                    key: ka.key,
                    source,
                });
                continue;
            }
        };
        let action = match Action::from_str(&ka.action) {
            Ok(a) => a,
            Err(source) => {
                errors.push(BindingError::UnknownAction {
                    action: ka.action,
                    source,
                });
                continue;
            }
        };
        resolved.push(ResolvedBinding {
            sequence,
            action,
            context,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn invalid_bindings_skipped_and_reported() {
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

        // One pass yields both the usable bindings and the skipped entries.
        let (resolved, errors) = set.clone().resolve();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].action, Action::ScrollUp);
        assert_eq!(errors.len(), 2);
        assert!(matches!(&errors[0], BindingError::InvalidKey { key, .. } if key.is_empty()));
        assert!(matches!(
            &errors[1],
            BindingError::UnknownAction { action, .. } if action == "invalid.action"
        ));

        // The convenience views agree with the combined result.
        assert_eq!(resolve_bindings(&set), resolved);
        assert_eq!(set.binding_errors(), errors);
    }
}
