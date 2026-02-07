use std::time::{Duration, Instant};

use crate::config::KeybindingsConfig;
use crate::keybindings::action::Action;
use crate::keybindings::context::KeyContext;
use crate::keybindings::key_input::KeyInput;
use crate::keybindings::templates::{self, ResolvedBinding};

/// Default timeout for multi-key sequences.
const DEFAULT_SEQUENCE_TIMEOUT: Duration = Duration::from_millis(1000);

/// Result of processing a key input through the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyMatchResult {
    /// No matching binding found — let the event propagate
    NoMatch,

    /// Partial match — waiting for more keys (consume the event)
    Pending,

    /// Full match — dispatch this action (consume the event)
    Matched(Action),
}

/// Key sequence state machine.
#[derive(Debug)]
enum KeySequenceState {
    /// Waiting for first key
    Idle,

    /// Received partial sequence, waiting for next key
    Pending {
        keys: Vec<KeyInput>,
        started_at: Instant,
    },
}

/// The keybinding engine that maps key inputs to actions.
///
/// Each window has its own engine instance. The engine is rebuilt when
/// config changes (template switch, custom binding edits).
pub struct KeybindingEngine {
    /// Resolved bindings (template + custom, reserved keys filtered)
    bindings: Vec<ResolvedBinding>,

    /// Current key sequence state
    state: KeySequenceState,

    /// Timeout for multi-key sequences
    sequence_timeout: Duration,
}

impl KeybindingEngine {
    /// Create a new engine from keybindings configuration.
    pub fn new(config: &KeybindingsConfig) -> Self {
        let custom: Vec<(String, String, Option<String>)> = config
            .custom
            .iter()
            .map(|b| (b.key.clone(), b.action.clone(), b.context.clone()))
            .collect();

        let bindings = templates::resolve_bindings(config.template, &custom);

        tracing::debug!(
            template = ?config.template,
            custom_count = config.custom.len(),
            resolved_count = bindings.len(),
            "Keybinding engine initialized"
        );

        Self {
            bindings,
            state: KeySequenceState::Idle,
            sequence_timeout: DEFAULT_SEQUENCE_TIMEOUT,
        }
    }

    /// Create an engine with specific bindings (for testing).
    #[cfg(test)]
    fn with_bindings(bindings: Vec<ResolvedBinding>) -> Self {
        Self {
            bindings,
            state: KeySequenceState::Idle,
            sequence_timeout: DEFAULT_SEQUENCE_TIMEOUT,
        }
    }

    /// Process a key input and return the match result.
    ///
    /// The `context` parameter determines which bindings are visible:
    /// - Context-specific bindings matching the current context have highest priority
    /// - Global bindings (context=None) serve as fallback
    /// - Bindings for a different context are invisible
    ///
    /// Key repeat behavior:
    /// - Single-key bindings: accept repeat events (enables continuous scrolling)
    /// - Sequence bindings: ignore repeat events (prevents accidental completion)
    pub fn process_key(
        &mut self,
        input: &KeyInput,
        is_repeat: bool,
        context: Option<KeyContext>,
    ) -> KeyMatchResult {
        match &mut self.state {
            KeySequenceState::Idle => self.process_idle(input, is_repeat, context),
            KeySequenceState::Pending { .. } => self.process_pending(input, is_repeat, context),
        }
    }

    /// Reset the sequence state to Idle.
    ///
    /// Called when:
    /// - Window focus changes
    /// - ActionCancel is dispatched
    /// - Timeout expires (checked lazily in process_pending)
    pub fn reset_state(&mut self) {
        self.state = KeySequenceState::Idle;
    }

    /// Check if the engine is in a pending sequence state.
    pub fn is_pending(&self) -> bool {
        matches!(self.state, KeySequenceState::Pending { .. })
    }

    /// Process a key in Idle state.
    fn process_idle(
        &mut self,
        input: &KeyInput,
        _is_repeat: bool,
        context: Option<KeyContext>,
    ) -> KeyMatchResult {
        let keys = vec![input.clone()];

        // Check for exact single-key match
        if let Some(action) = self.find_exact_match(&keys, context) {
            return KeyMatchResult::Matched(action);
        }

        // Check for prefix match (start of a multi-key sequence)
        if self.has_prefix_match(&keys, context) {
            self.state = KeySequenceState::Pending {
                keys,
                started_at: Instant::now(),
            };
            return KeyMatchResult::Pending;
        }

        KeyMatchResult::NoMatch
    }

    /// Process a key in Pending state.
    fn process_pending(
        &mut self,
        input: &KeyInput,
        is_repeat: bool,
        context: Option<KeyContext>,
    ) -> KeyMatchResult {
        // Ignore key repeats during sequence building
        if is_repeat {
            return KeyMatchResult::Pending;
        }

        // Check timeout
        let (mut keys, started_at) =
            match std::mem::replace(&mut self.state, KeySequenceState::Idle) {
                KeySequenceState::Pending { keys, started_at } => (keys, started_at),
                KeySequenceState::Idle => unreachable!(),
            };

        if started_at.elapsed() > self.sequence_timeout {
            // Timeout: reset and try the new key as a fresh start
            return self.process_idle(input, false, context);
        }

        keys.push(input.clone());

        // Check for exact match
        if let Some(action) = self.find_exact_match(&keys, context) {
            return KeyMatchResult::Matched(action);
        }

        // Check for continued prefix match
        if self.has_prefix_match(&keys, context) {
            self.state = KeySequenceState::Pending { keys, started_at };
            return KeyMatchResult::Pending;
        }

        // No match at all — reset state
        // The key that broke the sequence is NOT re-processed as a new sequence start.
        // This prevents unexpected behavior like "g x" triggering "x" if "x" is bound.
        KeyMatchResult::NoMatch
    }

    /// Find an exact match for the given key sequence with context priority.
    ///
    /// Priority:
    /// 1. Context-specific binding matching the current context → immediate return
    /// 2. Global binding (context=None) → fallback
    /// 3. Binding for a different context → invisible (skipped)
    fn find_exact_match(&self, keys: &[KeyInput], context: Option<KeyContext>) -> Option<Action> {
        let mut global_match = None;
        for b in &self.bindings {
            if b.sequence.0 != *keys {
                continue;
            }
            match b.context {
                Some(ctx) if Some(ctx) == context => return Some(b.action.clone()),
                None if global_match.is_none() => global_match = Some(b.action.clone()),
                _ => {} // different context → invisible
            }
        }
        global_match
    }

    /// Check if any visible binding starts with the given key sequence prefix.
    ///
    /// Only considers bindings that are visible in the current context:
    /// - Bindings with matching context
    /// - Global bindings (context=None)
    fn has_prefix_match(&self, keys: &[KeyInput], context: Option<KeyContext>) -> bool {
        self.bindings.iter().any(|b| {
            b.sequence.len() > keys.len()
                && b.sequence.starts_with(keys)
                && (b.context.is_none() || b.context == context)
        })
    }
}

impl std::fmt::Debug for KeybindingEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeybindingEngine")
            .field("bindings_count", &self.bindings.len())
            .field("state", &self.state)
            .field("sequence_timeout", &self.sequence_timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingTemplate;
    use crate::keybindings::key_input::{KeySequence, Modifiers};

    fn make_binding(key: &str, action: Action) -> ResolvedBinding {
        ResolvedBinding {
            sequence: KeySequence::parse(key).unwrap(),
            action,
            context: None,
        }
    }

    fn make_ctx_binding(key: &str, action: Action, ctx: KeyContext) -> ResolvedBinding {
        ResolvedBinding {
            sequence: KeySequence::parse(key).unwrap(),
            action,
            context: Some(ctx),
        }
    }

    #[test]
    fn test_single_key_match() {
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("j", Action::ScrollDown),
            make_binding("k", Action::ScrollUp),
        ]);

        let j = KeyInput::new("j", Modifiers::empty());
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_single_key_no_match() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("j", Action::ScrollDown)]);

        let x = KeyInput::new("x", Modifiers::empty());
        assert_eq!(engine.process_key(&x, false, None), KeyMatchResult::NoMatch);
    }

    #[test]
    fn test_modifier_key_match() {
        let mut engine = KeybindingEngine::with_bindings(vec![make_binding(
            "Ctrl+d",
            Action::ScrollHalfPageDown,
        )]);

        let ctrl_d = KeyInput::new("d", Modifiers::CTRL);
        assert_eq!(
            engine.process_key(&ctrl_d, false, None),
            KeyMatchResult::Matched(Action::ScrollHalfPageDown)
        );
    }

    #[test]
    fn test_sequence_match() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("g g", Action::ScrollTop)]);

        let g = KeyInput::new("g", Modifiers::empty());

        // First g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);

        // Second g → Matched
        assert_eq!(
            engine.process_key(&g, false, None),
            KeyMatchResult::Matched(Action::ScrollTop)
        );
    }

    #[test]
    fn test_sequence_no_match() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("g g", Action::ScrollTop)]);

        let g = KeyInput::new("g", Modifiers::empty());
        let x = KeyInput::new("x", Modifiers::empty());

        // First g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);

        // x (not a valid continuation) → NoMatch
        assert_eq!(engine.process_key(&x, false, None), KeyMatchResult::NoMatch);
    }

    #[test]
    fn test_sequence_timeout() {
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("g g", Action::ScrollTop),
            make_binding("j", Action::ScrollDown),
        ]);
        engine.sequence_timeout = Duration::from_millis(1);

        let g = KeyInput::new("g", Modifiers::empty());
        let j = KeyInput::new("j", Modifiers::empty());

        // First g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(5));

        // j after timeout → treated as new input, matches single-key binding
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_key_repeat_single_key_accepted() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("j", Action::ScrollDown)]);

        let j = KeyInput::new("j", Modifiers::empty());

        // First press
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );

        // Repeat press — should still match for continuous scrolling
        assert_eq!(
            engine.process_key(&j, true, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_key_repeat_in_sequence_ignored() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("g g", Action::ScrollTop)]);

        let g = KeyInput::new("g", Modifiers::empty());

        // First g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);

        // Repeat g during pending → still Pending (repeat ignored)
        assert_eq!(engine.process_key(&g, true, None), KeyMatchResult::Pending);

        // Actual second g → Matched
        assert_eq!(
            engine.process_key(&g, false, None),
            KeyMatchResult::Matched(Action::ScrollTop)
        );
    }

    #[test]
    fn test_reset_state() {
        let mut engine =
            KeybindingEngine::with_bindings(vec![make_binding("g g", Action::ScrollTop)]);

        let g = KeyInput::new("g", Modifiers::empty());

        // Start a sequence
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);
        assert!(engine.is_pending());

        // Reset
        engine.reset_state();
        assert!(!engine.is_pending());
    }

    #[test]
    fn test_single_and_sequence_disambiguation() {
        // When both "g" (single) and "g g" (sequence) exist,
        // "g" should trigger Pending (sequence takes priority for prefix match)
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("g", Action::ScrollDown),  // single-key
            make_binding("g g", Action::ScrollTop), // sequence
        ]);

        let g = KeyInput::new("g", Modifiers::empty());

        // First g → there's both an exact match AND a prefix match.
        // But the exact match wins immediately because we check exact before prefix.
        // This means "g" action fires and "g g" is never reachable.
        //
        // This is intentional: if a user has both "g" and "g g", the single "g"
        // will always fire immediately. Templates should not define conflicting
        // single + sequence bindings for the same key.
        assert_eq!(
            engine.process_key(&g, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_from_config() {
        let config = KeybindingsConfig {
            template: KeybindingTemplate::Vim,
            custom: vec![],
        };
        let mut engine = KeybindingEngine::new(&config);

        // In content context (None), j should be scroll.down
        let j = KeyInput::new("j", Modifiers::empty());
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_from_config_with_custom_override() {
        let config = KeybindingsConfig {
            template: KeybindingTemplate::Vim,
            custom: vec![crate::config::KeyBinding {
                key: "j".to_string(),
                action: "zoom.in".to_string(),
                context: None,
            }],
        };
        let mut engine = KeybindingEngine::new(&config);

        let j = KeyInput::new("j", Modifiers::empty());
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ZoomIn)
        );
    }

    #[test]
    fn test_mixed_sequence_gt() {
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("g t", Action::TabNext),
            make_binding("g Shift+t", Action::TabPrev),
        ]);

        let g = KeyInput::new("g", Modifiers::empty());
        let t = KeyInput::new("t", Modifiers::empty());
        let shift_t = KeyInput::new("t", Modifiers::SHIFT);

        // g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);
        // t → TabNext
        assert_eq!(
            engine.process_key(&t, false, None),
            KeyMatchResult::Matched(Action::TabNext)
        );

        // g → Pending
        assert_eq!(engine.process_key(&g, false, None), KeyMatchResult::Pending);
        // Shift+T → TabPrev
        assert_eq!(
            engine.process_key(&shift_t, false, None),
            KeyMatchResult::Matched(Action::TabPrev)
        );
    }

    // ====================================================================
    // Context-aware matching tests
    // ====================================================================

    #[test]
    fn test_context_binding_overrides_global() {
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("j", Action::ScrollDown),
            make_ctx_binding("j", Action::CursorDown, KeyContext::Sidebar),
        ]);

        let j = KeyInput::new("j", Modifiers::empty());

        // In sidebar context: context binding wins
        assert_eq!(
            engine.process_key(&j, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Matched(Action::CursorDown)
        );

        // In content context (None): global binding used
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_context_binding_invisible_in_other_context() {
        let mut engine = KeybindingEngine::with_bindings(vec![make_ctx_binding(
            "h",
            Action::CursorCollapse,
            KeyContext::Sidebar,
        )]);

        let h = KeyInput::new("h", Modifiers::empty());

        // In sidebar: matches
        assert_eq!(
            engine.process_key(&h, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Matched(Action::CursorCollapse)
        );

        // In right sidebar: invisible → NoMatch
        assert_eq!(
            engine.process_key(&h, false, Some(KeyContext::RightSidebar)),
            KeyMatchResult::NoMatch
        );

        // In content (None): invisible → NoMatch
        assert_eq!(engine.process_key(&h, false, None), KeyMatchResult::NoMatch);
    }

    #[test]
    fn test_global_binding_as_fallback_in_context() {
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("g t", Action::TabNext), // global sequence
        ]);

        let g = KeyInput::new("g", Modifiers::empty());
        let t = KeyInput::new("t", Modifiers::empty());

        // In sidebar context: global g t should still work as fallback
        assert_eq!(
            engine.process_key(&g, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Pending
        );
        assert_eq!(
            engine.process_key(&t, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Matched(Action::TabNext)
        );
    }

    #[test]
    fn test_context_shadows_global_for_same_key() {
        // When sidebar has j → cursor.down and global has j → scroll.down,
        // sidebar context should get cursor.down (not scroll.down)
        let mut engine = KeybindingEngine::with_bindings(vec![
            make_binding("j", Action::ScrollDown),
            make_ctx_binding("j", Action::CursorDown, KeyContext::Sidebar),
            make_ctx_binding("j", Action::CursorDown, KeyContext::RightSidebar),
            make_ctx_binding("j", Action::CursorDown, KeyContext::QuickAccess),
        ]);

        let j = KeyInput::new("j", Modifiers::empty());

        assert_eq!(
            engine.process_key(&j, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Matched(Action::CursorDown)
        );
        assert_eq!(
            engine.process_key(&j, false, Some(KeyContext::RightSidebar)),
            KeyMatchResult::Matched(Action::CursorDown)
        );
        assert_eq!(
            engine.process_key(&j, false, Some(KeyContext::QuickAccess)),
            KeyMatchResult::Matched(Action::CursorDown)
        );
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }

    #[test]
    fn test_from_config_sidebar_j_is_cursor_down() {
        let config = KeybindingsConfig {
            template: KeybindingTemplate::Vim,
            custom: vec![],
        };
        let mut engine = KeybindingEngine::new(&config);

        let j = KeyInput::new("j", Modifiers::empty());

        // In sidebar, j should be cursor.down
        assert_eq!(
            engine.process_key(&j, false, Some(KeyContext::Sidebar)),
            KeyMatchResult::Matched(Action::CursorDown)
        );

        // In content, j should be scroll.down
        assert_eq!(
            engine.process_key(&j, false, None),
            KeyMatchResult::Matched(Action::ScrollDown)
        );
    }
}
