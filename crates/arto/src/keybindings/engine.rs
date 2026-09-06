use std::time::{Duration, Instant};

use crate::config::BindingSet;
use crate::shortcut::KeyChord;

use super::action::Action;
use super::context::KeyContext;
use super::presets::{resolve_bindings, ResolvedBinding};

/// Result of processing a key input through the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyMatchResult {
    /// No binding matched; input consumed.
    NoMatch,
    /// Partial match; waiting for more keys (sequence in progress).
    Pending,
    /// A binding matched; execute the returned action.
    Matched(Action),
}

/// State machine for keybinding sequence matching.
///
/// Tracks the current key sequence state (Idle or Pending) and matches
/// incoming key chords against resolved bindings with context awareness.
pub struct KeybindingEngine {
    bindings: Vec<ResolvedBinding>,
    state: KeySequenceState,
    sequence_timeout: Duration,
}

enum KeySequenceState {
    Idle,
    Pending {
        keys: Vec<KeyChord>,
        started_at: Instant,
    },
}

/// Default sequence timeout: 1 second.
const DEFAULT_SEQUENCE_TIMEOUT: Duration = Duration::from_millis(1000);

impl KeybindingEngine {
    /// Create a new engine from a binding set.
    pub fn new(bindings: &BindingSet) -> Self {
        Self {
            bindings: resolve_bindings(bindings),
            state: KeySequenceState::Idle,
            sequence_timeout: DEFAULT_SEQUENCE_TIMEOUT,
        }
    }

    /// Process a key input and return the match result.
    ///
    /// `context` is the current focused panel's key context.
    /// `is_repeat` controls key repeat behavior: single-chord bindings allow repeat,
    /// multi-chord (sequence) bindings ignore repeat events.
    pub fn process_key(
        &mut self,
        input: &KeyChord,
        is_repeat: bool,
        context: KeyContext,
    ) -> KeyMatchResult {
        // Check timeout for pending state
        if let KeySequenceState::Pending { started_at, .. } = &self.state {
            if started_at.elapsed() >= self.sequence_timeout {
                self.state = KeySequenceState::Idle;
            }
        }

        // Build the current key sequence
        let current_keys = match &mut self.state {
            KeySequenceState::Idle => {
                vec![input.clone()]
            }
            KeySequenceState::Pending { keys, .. } => {
                // Ignore repeat events during sequence input
                if is_repeat {
                    return KeyMatchResult::Pending;
                }
                keys.push(input.clone());
                keys.clone()
            }
        };

        // Find matching bindings
        let (exact_match, has_prefix) = self.find_match(&current_keys, context);

        if let Some(action) = exact_match {
            self.state = KeySequenceState::Idle;
            return KeyMatchResult::Matched(action);
        }

        if has_prefix {
            self.state = KeySequenceState::Pending {
                keys: current_keys,
                started_at: match &self.state {
                    KeySequenceState::Pending { started_at, .. } => *started_at,
                    KeySequenceState::Idle => Instant::now(),
                },
            };
            return KeyMatchResult::Pending;
        }

        // No match at all
        self.state = KeySequenceState::Idle;

        // Vim-style fallback: a failed sequence doesn't consume the last key.
        // For example, typing "g x" (no binding) should still process "x" as
        // a fresh single-key input, potentially matching scroll.down etc.
        if current_keys.len() > 1 {
            let single = vec![input.clone()];
            let (exact_match, has_prefix) = self.find_match(&single, context);

            if let Some(action) = exact_match {
                return KeyMatchResult::Matched(action);
            }

            if has_prefix {
                self.state = KeySequenceState::Pending {
                    keys: single,
                    started_at: Instant::now(),
                };
                return KeyMatchResult::Pending;
            }
        }

        KeyMatchResult::NoMatch
    }

    /// Build an engine with the menu-shortcut folding decision forced, so tests
    /// can exercise both the native-menu (macOS/Linux) and folded (Windows)
    /// branches regardless of the host platform.
    #[cfg(test)]
    fn new_with_menu_folding(bindings: &BindingSet, fold_menu_shortcuts: bool) -> Self {
        Self {
            bindings: bindings
                .clone()
                .into_resolved_bindings_with(fold_menu_shortcuts),
            state: KeySequenceState::Idle,
            sequence_timeout: DEFAULT_SEQUENCE_TIMEOUT,
        }
    }

    /// Reset the engine state (e.g., on focus change or cancel).
    pub fn reset(&mut self) {
        self.state = KeySequenceState::Idle;
    }

    /// Returns true if a sequence is in progress.
    #[cfg(test)]
    pub fn is_pending(&self) -> bool {
        matches!(self.state, KeySequenceState::Pending { .. })
    }

    /// Find a matching binding for the given key sequence and context.
    ///
    /// Returns (exact_match, has_prefix_match):
    /// - exact_match: Some(Action) if a binding exactly matches the key sequence
    /// - has_prefix_match: true if some binding starts with this key sequence
    ///
    /// Context matching priority:
    /// 1. Context-specific match (exact context) → highest priority
    /// 2. Global match (context=None) → fallback
    /// 3. Different context → invisible (ignored)
    fn find_match(&self, keys: &[KeyChord], context: KeyContext) -> (Option<Action>, bool) {
        let mut exact_context_match: Option<Action> = None;
        let mut exact_global_match: Option<Action> = None;
        let mut has_prefix = false;

        for binding in &self.bindings {
            // Check context visibility
            let is_visible = match (&binding.context, &context) {
                // Binding is global → always visible
                (None, _) => true,
                // Binding has context, panel has same context → visible
                (Some(bc), pc) if bc == pc => true,
                // Binding has context but panel doesn't match → invisible
                _ => false,
            };

            if !is_visible {
                continue;
            }

            let binding_chords = &binding.sequence.chords;

            // Check exact match
            if binding_chords.len() == keys.len() && chords_match(binding_chords, keys) {
                if binding.context.is_some() {
                    // Context-specific match takes priority
                    exact_context_match = Some(binding.action);
                } else if exact_context_match.is_none() {
                    exact_global_match = Some(binding.action);
                }
            }

            // Check prefix match (binding is longer than current keys)
            if binding_chords.len() > keys.len()
                && chords_match(&binding_chords[..keys.len()], keys)
            {
                has_prefix = true;
            }
        }

        let exact = exact_context_match.or(exact_global_match);
        (exact, has_prefix)
    }
}

/// Check if two chord slices match exactly.
fn chords_match(a: &[KeyChord], b: &[KeyChord]) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BindingSet, KeyAction};
    use crate::shortcut::ShortcutSequence;
    use std::str::FromStr;

    fn vim_bindings() -> BindingSet {
        crate::keybindings::presets::vim::bindings()
    }

    fn default_bindings() -> BindingSet {
        crate::keybindings::default_bindings()
    }

    fn chord(s: &str) -> KeyChord {
        ShortcutSequence::from_str(s)
            .unwrap()
            .chords
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn vim_scroll_down() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        let result = engine.process_key(&chord("j"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
    }

    #[test]
    fn vim_sequence_gg() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);

        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Pending);

        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollTop));
    }

    #[test]
    fn vim_sequence_timeout_resets() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        // Override timeout to zero for testing
        engine.sequence_timeout = Duration::ZERO;

        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Pending);

        // Simulate timeout by sleeping
        std::thread::sleep(Duration::from_millis(1));

        // After timeout, "g" alone doesn't match anything
        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        // "g" starts a new pending for "g g" or "g t" etc.
        assert_eq!(result, KeyMatchResult::Pending);
    }

    #[test]
    fn no_match_returns_no_match() {
        let config = default_bindings();
        let mut engine = KeybindingEngine::new(&config);
        // "x" isn't bound in defaults (only Cmd+Key shortcuts)
        let result = engine.process_key(&chord("x"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::NoMatch);
    }

    #[test]
    fn default_global_binding_cmd_r() {
        // Cmd+r (window.reload) stays an engine keybinding in the default preset.
        let config = default_bindings();
        let mut engine = KeybindingEngine::new(&config);
        let result = engine.process_key(&chord("Cmd+r"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::WindowReload));
    }

    #[test]
    fn menu_shortcut_reachable_only_when_folded() {
        // The menu-shortcut folding decision, tested on both branches from any
        // host. Cmd+o (file.open) is a menu shortcut in the default preset.
        // - Native-menu platforms (macOS/Linux): the OS dispatches it, so the
        //   engine must NOT resolve it.
        // - Windows: no native menu, so it is folded into the engine and matches.
        // Using `chord("Cmd+o")` keeps the probe in the host's own parse space,
        // so it lines up with the folded binding regardless of platform.
        let config = default_bindings();

        let mut native = KeybindingEngine::new_with_menu_folding(&config, false);
        assert_eq!(
            native.process_key(&chord("Cmd+o"), false, KeyContext::Content),
            KeyMatchResult::NoMatch
        );

        let mut folded = KeybindingEngine::new_with_menu_folding(&config, true);
        assert_eq!(
            folded.process_key(&chord("Cmd+o"), false, KeyContext::Content),
            KeyMatchResult::Matched(Action::FileOpen)
        );
    }

    #[test]
    fn folded_menu_shortcut_yields_to_later_global_binding() {
        // Menu shortcuts fold in BEFORE the `global` field, and global matches
        // are last-wins, so an idiomatic global binding on the same chord wins
        // over a folded menu shortcut. Keyed with Ctrl+n on both entries so the
        // chords collide on every host (Ctrl is never remapped), isolating the
        // fold-ordering invariant from the Cmd→Ctrl remap (covered in shortcut).
        let set = BindingSet {
            menu_shortcuts: vec![KeyAction {
                key: "Ctrl+n".to_string(),
                action: "window.new".to_string(),
            }],
            global: vec![KeyAction {
                key: "Ctrl+n".to_string(),
                action: "scroll.down".to_string(),
            }],
            ..Default::default()
        };
        let mut engine = KeybindingEngine::new_with_menu_folding(&set, true);
        assert_eq!(
            engine.process_key(&chord("Ctrl+n"), false, KeyContext::Content),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn repeat_allowed_for_single_chord() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        // First press
        let result = engine.process_key(&chord("j"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
        // Repeat (is_repeat=true) should still work for single-chord
        let result = engine.process_key(&chord("j"), true, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
    }

    #[test]
    fn repeat_ignored_during_sequence() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);

        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Pending);

        // Repeat during sequence should be ignored
        let result = engine.process_key(&chord("g"), true, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Pending);
    }

    #[test]
    fn failed_sequence_retries_last_key() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);

        // "g" starts a sequence
        let result = engine.process_key(&chord("g"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Pending);

        // "j" doesn't complete "g j" (not a binding) → fails sequence
        // Then retries "j" alone → matches scroll.down
        let result = engine.process_key(&chord("j"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
    }

    #[test]
    fn reset_clears_pending() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);

        engine.process_key(&chord("g"), false, KeyContext::Content);
        assert!(engine.is_pending());

        engine.reset();
        assert!(!engine.is_pending());
    }

    #[test]
    fn context_specific_overrides_global() {
        let mut bindings = vim_bindings();
        // Add a sidebar-specific override on top of vim preset
        bindings.sidebar.retain(|b| b.key != "j");
        bindings.sidebar.push(KeyAction {
            key: "j".to_string(),
            action: "cursor.down".to_string(),
        });
        let mut engine = KeybindingEngine::new(&bindings);

        // In sidebar context: "j" should match cursor.down (context-specific)
        let result = engine.process_key(&chord("j"), false, KeyContext::Sidebar);
        assert_eq!(result, KeyMatchResult::Matched(Action::CursorDown));

        // In Content context: "j" should match scroll.down (global)
        let result = engine.process_key(&chord("j"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
    }

    #[test]
    fn different_context_binding_invisible() {
        let bindings = BindingSet {
            sidebar: vec![KeyAction {
                key: "j".to_string(),
                action: "cursor.down".to_string(),
            }],
            ..Default::default()
        };
        let mut engine = KeybindingEngine::new(&bindings);

        // In QuickAccess context: sidebar-specific "j" should be invisible
        let result = engine.process_key(&chord("j"), false, KeyContext::QuickAccess);
        assert_eq!(result, KeyMatchResult::NoMatch);
    }

    #[test]
    fn emacs_ctrl_n_scroll() {
        let bindings = crate::keybindings::presets::emacs::bindings();
        let mut engine = KeybindingEngine::new(&bindings);
        let result = engine.process_key(&chord("Ctrl+n"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ScrollDown));
    }

    #[test]
    fn user_overrides_default_global_binding() {
        // User edits Cmd+r from window.reload to tab.new in their global config.
        let mut custom = crate::keybindings::default_bindings();
        let cmd_r = custom.global.iter_mut().find(|b| b.key == "Cmd+r").unwrap();
        cmd_r.action = "tab.new".to_string();

        let mut engine = KeybindingEngine::new(&custom);
        let result = engine.process_key(&chord("Cmd+r"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::TabNew));
    }

    #[test]
    fn escape_maps_to_cancel() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        let result = engine.process_key(&chord("Escape"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::Cancel));
    }

    #[test]
    fn content_context_bindings_visible_in_content() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        let result = engine.process_key(&chord("Ctrl+j"), false, KeyContext::Content);
        assert_eq!(result, KeyMatchResult::Matched(Action::ContentNext));
    }

    #[test]
    fn content_context_bindings_invisible_in_sidebar() {
        let config = vim_bindings();
        let mut engine = KeybindingEngine::new(&config);
        // In sidebar: content-only Ctrl+j binding is not visible.
        let result = engine.process_key(&chord("Ctrl+j"), false, KeyContext::Sidebar);
        assert_eq!(result, KeyMatchResult::NoMatch);
    }
}
