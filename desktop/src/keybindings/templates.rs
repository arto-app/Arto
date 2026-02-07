mod emacs;
mod vim;

use crate::config::KeybindingTemplate;
use crate::keybindings::action::Action;
use crate::keybindings::context::KeyContext;
use crate::keybindings::key_input::KeySequence;

/// A resolved binding ready for the engine.
#[derive(Debug, Clone)]
pub struct ResolvedBinding {
    pub sequence: KeySequence,
    pub action: Action,
    pub context: Option<KeyContext>,
}

/// Set of reserved Cmd+Key combinations that muda intercepts.
/// These are excluded from Engine bindings even if defined in templates/custom.
static RESERVED_CMD_KEYS: &[&str] = &[
    "Cmd+n",
    "Cmd+t",
    "Cmd+w",
    "Cmd+o",
    "Cmd+f",
    "Cmd+b",
    "Cmd+Comma",
    // Cmd+Shift variants
    "Cmd+Shift+o",
    "Cmd+Shift+w",
    "Cmd+Shift+r",
    // Cmd+digit/symbol
    "Cmd+0",
    "Cmd+Equal",
    "Cmd+Minus",
    "Cmd+BracketLeft",
    "Cmd+BracketRight",
    // OS standard (PredefinedMenuItem)
    "Cmd+q",
    "Cmd+x",
    "Cmd+c",
    "Cmd+v",
    "Cmd+a",
];

/// Check if a key sequence is a reserved menu shortcut.
fn is_reserved(sequence: &KeySequence) -> bool {
    if sequence.len() != 1 {
        return false;
    }
    let input = &sequence.0[0];
    let notation = input.to_string();
    let lower = notation.to_lowercase();
    RESERVED_CMD_KEYS.iter().any(|&r| r.to_lowercase() == lower)
}

/// Raw binding entry: (key, action, optional context string).
type RawBinding = (&'static str, &'static str, Option<&'static str>);

/// Get the raw binding definitions for a template.
fn get_template_bindings(template: KeybindingTemplate) -> Vec<RawBinding> {
    match template {
        KeybindingTemplate::None => vec![],
        KeybindingTemplate::Vim => vim::bindings(),
        KeybindingTemplate::Emacs => emacs::bindings(),
    }
}

/// Merge template and custom bindings.
///
/// Merge rules:
/// 1. Start with selected template bindings (or empty for None)
/// 2. Custom bindings override template on the same (key, context) pair
/// 3. Reserved Cmd+Key bindings are excluded with a warning
pub fn resolve_bindings(
    template: KeybindingTemplate,
    custom: &[(String, String, Option<String>)],
) -> Vec<ResolvedBinding> {
    // Build template bindings: (key notation, action, context string)
    let template_entries = get_template_bindings(template);
    let mut bindings: Vec<(String, Action, Option<String>)> = Vec::new();

    for (key, action_str, ctx) in &template_entries {
        match action_str.parse::<Action>() {
            Ok(action) => {
                bindings.push((key.to_string(), action, ctx.map(|s| s.to_string())));
            }
            Err(e) => {
                tracing::warn!(key, %e, "Skipping template binding with invalid action");
            }
        }
    }

    // Apply custom overrides: same (key, context) pair → replace
    for (key, action_str, ctx) in custom {
        match action_str.parse::<Action>() {
            Ok(action) => {
                // Remove existing binding with the same (key, context) pair
                bindings.retain(|(k, _, c)| !(k == key && c == ctx));
                bindings.push((key.clone(), action, ctx.clone()));
            }
            Err(e) => {
                tracing::warn!(key, %e, "Skipping custom binding with invalid action");
            }
        }
    }

    // Parse key sequences, resolve contexts, and filter reserved keys
    let mut resolved = Vec::new();
    for (key_notation, action, ctx_str) in bindings {
        match KeySequence::parse(&key_notation) {
            Ok(sequence) => {
                if is_reserved(&sequence) {
                    tracing::warn!(
                        key = key_notation,
                        "Ignoring binding for reserved menu shortcut"
                    );
                    continue;
                }
                let context = ctx_str.as_deref().and_then(|s| match s.parse::<KeyContext>() {
                    Ok(ctx) => Some(ctx),
                    Err(e) => {
                        tracing::warn!(key = key_notation, %e, "Skipping binding with invalid context");
                        None
                    }
                });
                // Skip if context string was provided but couldn't be parsed
                if ctx_str.is_some() && context.is_none() {
                    continue;
                }
                resolved.push(ResolvedBinding {
                    sequence,
                    action,
                    context,
                });
            }
            Err(e) => {
                tracing::warn!(key = key_notation, %e, "Skipping binding with invalid key");
            }
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_template_is_empty() {
        let bindings = resolve_bindings(KeybindingTemplate::None, &[]);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_vim_template_has_bindings() {
        let bindings = resolve_bindings(KeybindingTemplate::Vim, &[]);
        assert!(!bindings.is_empty());
        // Check that global j → scroll.down exists
        let j_binding = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "j" && b.context.is_none())
            .expect("Vim template should have global j binding");
        assert_eq!(j_binding.action, Action::ScrollDown);
    }

    #[test]
    fn test_emacs_template_has_bindings() {
        let bindings = resolve_bindings(KeybindingTemplate::Emacs, &[]);
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_custom_overrides_template_global() {
        let custom = vec![("j".to_string(), "zoom.in".to_string(), None)];
        let bindings = resolve_bindings(KeybindingTemplate::Vim, &custom);

        let j_binding = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "j" && b.context.is_none())
            .expect("Should have global j binding");
        assert_eq!(j_binding.action, Action::ZoomIn);
    }

    #[test]
    fn test_custom_overrides_same_key_context_pair() {
        // Override a context-specific binding without affecting global
        let custom = vec![(
            "j".to_string(),
            "zoom.in".to_string(),
            Some("sidebar".to_string()),
        )];
        let bindings = resolve_bindings(KeybindingTemplate::Vim, &custom);

        // The global j → scroll.down should still exist
        let global_j = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "j" && b.context.is_none())
            .expect("Global j binding should survive");
        assert_eq!(global_j.action, Action::ScrollDown);

        // The sidebar j should be overridden
        let sidebar_j = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "j" && b.context == Some(KeyContext::Sidebar))
            .expect("Sidebar j binding should exist");
        assert_eq!(sidebar_j.action, Action::ZoomIn);
    }

    #[test]
    fn test_custom_extends_template() {
        let custom = vec![("x".to_string(), "tab.new".to_string(), None)];
        let bindings = resolve_bindings(KeybindingTemplate::Vim, &custom);

        let x_binding = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "x")
            .expect("Should have custom x binding");
        assert_eq!(x_binding.action, Action::TabNew);
    }

    #[test]
    fn test_reserved_keys_filtered() {
        let custom = vec![("Cmd+w".to_string(), "tab.close".to_string(), None)];
        let bindings = resolve_bindings(KeybindingTemplate::None, &custom);

        // Cmd+W is reserved by muda, should be filtered out
        assert!(
            bindings.iter().all(|b| b.sequence.to_string() != "Cmd+w"),
            "Reserved Cmd+W should be filtered"
        );
    }

    #[test]
    fn test_custom_only_mode() {
        let custom = vec![
            ("j".to_string(), "scroll.down".to_string(), None),
            ("k".to_string(), "scroll.up".to_string(), None),
        ];
        let bindings = resolve_bindings(KeybindingTemplate::None, &custom);
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn test_invalid_action_skipped() {
        let custom = vec![("j".to_string(), "invalid.action".to_string(), None)];
        let bindings = resolve_bindings(KeybindingTemplate::None, &custom);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_invalid_key_skipped() {
        let custom = vec![("".to_string(), "scroll.down".to_string(), None)];
        let bindings = resolve_bindings(KeybindingTemplate::None, &custom);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_vim_template_has_panel_bindings() {
        let bindings = resolve_bindings(KeybindingTemplate::Vim, &[]);

        // Should have sidebar j → cursor.down
        let sidebar_j = bindings
            .iter()
            .find(|b| b.sequence.to_string() == "j" && b.context == Some(KeyContext::Sidebar));
        assert!(sidebar_j.is_some(), "Vim should have sidebar j binding");
        assert_eq!(sidebar_j.unwrap().action, Action::CursorDown);
    }
}
