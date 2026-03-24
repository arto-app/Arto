use dioxus::events::KeyboardEvent;
use dioxus::html::{Key, Modifiers};
use dioxus::prelude::ModifiersInteraction;
use std::fmt;
use std::str::FromStr;

/// Normalized single key press: logical Key + modifier flags.
///
/// Uses `dioxus::html::Key` (logical key, not physical `Code`) for correct
/// international keyboard support. `Key::Character("j")` is the same on
/// QWERTY, AZERTY, and QWERTZ layouts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: Key,
    pub modifiers: Modifiers,
}

/// Space-separated sequence of key chords (e.g., `"g g"` → two KeyChords).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShortcutSequence {
    pub chords: Vec<KeyChord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutParseError {
    Empty,
    MissingKey,
    UnknownKey(String),
    UnknownModifier(String),
}

impl fmt::Display for ShortcutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty shortcut string"),
            Self::MissingKey => write!(f, "modifier(s) without a key"),
            Self::UnknownKey(k) => write!(f, "unknown key: {k:?}"),
            Self::UnknownModifier(m) => write!(f, "unknown modifier: {m:?}"),
        }
    }
}

impl std::error::Error for ShortcutParseError {}

impl KeyChord {
    /// Normalize: uppercase Character → SHIFT + lowercase.
    ///
    /// When the user presses Shift+J, the browser reports `key: "J"` with `shiftKey: true`.
    /// We normalize `Key::Character("J")` to `Key::Character("j")` + `Modifiers::SHIFT`
    /// so that config-defined `"Shift+j"` matches correctly.
    ///
    /// This is the single normalization entry point used by both config parsing
    /// and JS event processing to ensure consistent key representation.
    fn normalize(mut self) -> Self {
        if let Key::Character(ref ch) = self.key {
            if ch.len() == 1 {
                let c = ch.chars().next().unwrap();
                if c.is_ascii_uppercase() {
                    self.key = Key::Character(c.to_ascii_lowercase().to_string());
                    self.modifiers |= Modifiers::SHIFT;
                }
            }
        }
        self
    }

    /// Build a KeyChord from JS interceptor data.
    ///
    /// `key_str` is the `event.key` value (e.g., "j", "Enter", "G").
    /// `modifier_bits` is a bitmask: bit 0=ALT, 1=CTRL, 2=META, 3=SHIFT.
    pub fn from_js_event(key_str: &str, modifier_bits: u32) -> Self {
        let key = match Key::from_str(key_str) {
            Ok(Key::Character(s)) if s.len() > 1 => parse_key(key_str).unwrap_or(Key::Character(s)),
            Ok(k) => k,
            Err(_) => parse_key(key_str).unwrap_or(Key::Unidentified),
        };
        let modifiers = Modifiers::from_bits_truncate(modifier_bits);
        KeyChord { key, modifiers }.normalize()
    }

    /// Returns true if this chord is a modifier-only key press (should be ignored).
    pub fn is_modifier_only(&self) -> bool {
        matches!(self.key, Key::Control | Key::Shift | Key::Alt | Key::Meta)
    }
}

/// Parse a key name string from config notation into a `Key`.
///
/// This is used only for config parsing (e.g., `"Ctrl+n"`, `"BracketLeft"`),
/// NOT for runtime JS events. JS events go through `KeyChord::from_js_event()`
/// which calls `Key::from_str()` directly + `normalize()`.
///
/// Handles three categories:
/// 1. Single ASCII character → `Key::Character("x")`
/// 2. Named keys (Enter, Escape, ArrowUp, etc.) → corresponding `Key` variant
/// 3. Special symbol names (Equal, Minus, etc.) → `Key::Character("=")`, etc.
fn parse_key(token: &str) -> Result<Key, ShortcutParseError> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(ShortcutParseError::UnknownKey(token.to_string()));
    }

    // Single character → Key::Character
    if trimmed.len() == 1 {
        let ch = trimmed.chars().next().unwrap();
        // Lowercase alphabetic characters for consistency
        if ch.is_ascii_alphabetic() {
            return Ok(Key::Character(ch.to_ascii_lowercase().to_string()));
        }
        // Digits and symbols pass through as-is
        return Ok(Key::Character(trimmed.to_string()));
    }

    // Try keyboard-types FromStr for named keys (Enter, Escape, ArrowUp, etc.)
    if let Ok(key) = Key::from_str(trimmed) {
        // Reject Key::Character from FromStr for multi-char strings — we handle
        // single chars above, so multi-char Character would be a user typo
        if !matches!(key, Key::Character(_)) {
            return Ok(key);
        }
    }

    // Case-insensitive named key aliases
    match trimmed.to_ascii_lowercase().as_str() {
        "enter" | "return" => Ok(Key::Enter),
        "escape" | "esc" => Ok(Key::Escape),
        "tab" => Ok(Key::Tab),
        "backspace" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "space" | " " => Ok(Key::Character(" ".to_string())),
        "arrowup" | "up" => Ok(Key::ArrowUp),
        "arrowdown" | "down" => Ok(Key::ArrowDown),
        "arrowleft" | "left" => Ok(Key::ArrowLeft),
        "arrowright" | "right" => Ok(Key::ArrowRight),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        // Symbol aliases (config convenience: "Equal" instead of "=")
        "equal" => Ok(Key::Character("=".to_string())),
        "minus" => Ok(Key::Character("-".to_string())),
        "bracketleft" => Ok(Key::Character("[".to_string())),
        "bracketright" => Ok(Key::Character("]".to_string())),
        "slash" => Ok(Key::Character("/".to_string())),
        "backslash" => Ok(Key::Character("\\".to_string())),
        "comma" => Ok(Key::Character(",".to_string())),
        "period" => Ok(Key::Character(".".to_string())),
        "semicolon" => Ok(Key::Character(";".to_string())),
        "quote" => Ok(Key::Character("'".to_string())),
        "backquote" => Ok(Key::Character("`".to_string())),
        _ => Err(ShortcutParseError::UnknownKey(token.to_string())),
    }
}

impl FromStr for ShortcutSequence {
    type Err = ShortcutParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ShortcutParseError::Empty);
        }

        let mut chords = Vec::new();
        for token in trimmed.split_whitespace() {
            chords.push(parse_chord(token)?);
        }

        Ok(Self { chords })
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display order: Ctrl+Alt+Shift+Cmd+key
        if self.modifiers.contains(Modifiers::CONTROL) {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.contains(Modifiers::ALT) {
            write!(f, "Alt+")?;
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            write!(f, "Shift+")?;
        }
        if self.modifiers.contains(Modifiers::META) {
            if cfg!(target_os = "macos") {
                write!(f, "Cmd+")?;
            } else {
                write!(f, "Win+")?;
            }
        }
        match &self.key {
            Key::Character(c) => write!(f, "{c}"),
            other => write!(f, "{other}"),
        }
    }
}

impl fmt::Display for ShortcutSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, chord) in self.chords.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{chord}")?;
        }
        Ok(())
    }
}

impl From<&KeyboardEvent> for KeyChord {
    fn from(event: &KeyboardEvent) -> Self {
        KeyChord {
            key: event.data().key(),
            modifiers: event.data().modifiers(),
        }
        .normalize()
    }
}

impl From<KeyboardEvent> for KeyChord {
    fn from(event: KeyboardEvent) -> Self {
        Self::from(&event)
    }
}

impl ShortcutSequence {
    /// Returns true if this is a single-chord sequence.
    #[cfg(test)]
    pub fn is_single(&self) -> bool {
        self.chords.len() == 1
    }
}

fn parse_chord(token: &str) -> Result<KeyChord, ShortcutParseError> {
    let mut modifiers = Modifiers::empty();
    let mut key_part: Option<&str> = None;

    for part in token.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                if cfg!(target_os = "macos") {
                    modifiers.insert(Modifiers::CONTROL);
                } else {
                    modifiers.insert(Modifiers::META);
                }
            }
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "alt" | "option" => modifiers.insert(Modifiers::ALT),
            "meta" | "cmd" | "command" => {
                if cfg!(target_os = "macos") {
                    modifiers.insert(Modifiers::META);
                } else {
                    modifiers.insert(Modifiers::CONTROL);
                }
            }
            _ => {
                if key_part.is_some() {
                    return Err(ShortcutParseError::UnknownModifier(part.to_string()));
                }
                key_part = Some(part);
            }
        }
    }

    let key_part = key_part.ok_or(ShortcutParseError::MissingKey)?;
    let key = parse_key(key_part)?;

    // Apply normalization: uppercase Character + no explicit SHIFT → add SHIFT
    Ok(KeyChord { key, modifiers }.normalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_key tests ---

    #[test]
    fn parse_key_single_alpha() {
        assert_eq!(parse_key("g").unwrap(), Key::Character("g".to_string()));
        assert_eq!(parse_key("G").unwrap(), Key::Character("g".to_string()));
    }

    #[test]
    fn parse_key_single_digit() {
        assert_eq!(parse_key("0").unwrap(), Key::Character("0".to_string()));
        assert_eq!(parse_key("9").unwrap(), Key::Character("9".to_string()));
    }

    #[test]
    fn parse_key_single_symbol() {
        assert_eq!(parse_key("/").unwrap(), Key::Character("/".to_string()));
        assert_eq!(parse_key("[").unwrap(), Key::Character("[".to_string()));
        assert_eq!(parse_key("]").unwrap(), Key::Character("]".to_string()));
        assert_eq!(parse_key("-").unwrap(), Key::Character("-".to_string()));
        assert_eq!(parse_key("=").unwrap(), Key::Character("=".to_string()));
    }

    #[test]
    fn parse_key_named() {
        assert_eq!(parse_key("Enter").unwrap(), Key::Enter);
        assert_eq!(parse_key("Escape").unwrap(), Key::Escape);
        assert_eq!(parse_key("Tab").unwrap(), Key::Tab);
        assert_eq!(parse_key("Backspace").unwrap(), Key::Backspace);
        assert_eq!(parse_key("ArrowUp").unwrap(), Key::ArrowUp);
        assert_eq!(parse_key("F1").unwrap(), Key::F1);
    }

    #[test]
    fn parse_key_case_insensitive_aliases() {
        assert_eq!(parse_key("enter").unwrap(), Key::Enter);
        assert_eq!(parse_key("ESCAPE").unwrap(), Key::Escape);
        assert_eq!(parse_key("esc").unwrap(), Key::Escape);
        assert_eq!(parse_key("return").unwrap(), Key::Enter);
        assert_eq!(parse_key("del").unwrap(), Key::Delete);
    }

    #[test]
    fn parse_key_symbol_aliases() {
        assert_eq!(parse_key("Equal").unwrap(), Key::Character("=".to_string()));
        assert_eq!(parse_key("Minus").unwrap(), Key::Character("-".to_string()));
        assert_eq!(
            parse_key("BracketLeft").unwrap(),
            Key::Character("[".to_string())
        );
        assert_eq!(
            parse_key("BracketRight").unwrap(),
            Key::Character("]".to_string())
        );
    }

    #[test]
    fn parse_key_unknown() {
        assert!(parse_key("no_such_key").is_err());
        assert!(parse_key("").is_err());
    }

    // --- KeyChord normalization tests ---

    #[test]
    fn normalize_uppercase_character() {
        let chord = KeyChord {
            key: Key::Character("G".to_string()),
            modifiers: Modifiers::empty(),
        };
        let normalized = chord.normalize();
        assert_eq!(normalized.key, Key::Character("g".to_string()));
        assert!(normalized.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn normalize_uppercase_with_existing_shift() {
        let chord = KeyChord {
            key: Key::Character("G".to_string()),
            modifiers: Modifiers::SHIFT,
        };
        let normalized = chord.normalize();
        assert_eq!(normalized.key, Key::Character("g".to_string()));
        assert!(normalized.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn normalize_lowercase_unchanged() {
        let chord = KeyChord {
            key: Key::Character("g".to_string()),
            modifiers: Modifiers::empty(),
        };
        let normalized = chord.normalize();
        assert_eq!(normalized.key, Key::Character("g".to_string()));
        assert_eq!(normalized.modifiers, Modifiers::empty());
    }

    #[test]
    fn normalize_named_key_unchanged() {
        let chord = KeyChord {
            key: Key::Enter,
            modifiers: Modifiers::CONTROL,
        };
        let normalized = chord.normalize();
        assert_eq!(normalized.key, Key::Enter);
        assert_eq!(normalized.modifiers, Modifiers::CONTROL);
    }

    // --- ShortcutSequence parsing tests ---

    #[test]
    fn parse_single_key() {
        let seq = ShortcutSequence::from_str("g").unwrap();
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Character("g".to_string()),
                modifiers: Modifiers::empty()
            }]
        );
    }

    #[test]
    fn parse_with_modifiers() {
        let seq = ShortcutSequence::from_str("Ctrl+Shift+g").unwrap();
        let expected_ctrl = if cfg!(target_os = "macos") { Modifiers::CONTROL } else { Modifiers::META };
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Character("g".to_string()),
                modifiers: expected_ctrl | Modifiers::SHIFT
            }]
        );
    }

    #[test]
    fn parse_sequence() {
        let seq = ShortcutSequence::from_str("g Ctrl+g").unwrap();
        let expected_ctrl = if cfg!(target_os = "macos") { Modifiers::CONTROL } else { Modifiers::META };
        assert_eq!(
            seq.chords,
            vec![
                KeyChord {
                    key: Key::Character("g".to_string()),
                    modifiers: Modifiers::empty()
                },
                KeyChord {
                    key: Key::Character("g".to_string()),
                    modifiers: expected_ctrl
                }
            ]
        );
    }

    #[test]
    fn parse_vim_sequence_gg() {
        let seq = ShortcutSequence::from_str("g g").unwrap();
        assert_eq!(seq.chords.len(), 2);
        assert!(seq
            .chords
            .iter()
            .all(|c| c.key == Key::Character("g".to_string())));
    }

    #[test]
    fn parse_shift_key() {
        let seq = ShortcutSequence::from_str("Shift+g").unwrap();
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Character("g".to_string()),
                modifiers: Modifiers::SHIFT
            }]
        );
    }

    #[test]
    fn parse_cmd_key() {
        let seq = ShortcutSequence::from_str("Cmd+n").unwrap();
        let expected_mod = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Character("n".to_string()),
                modifiers: expected_mod
            }]
        );
    }

    #[test]
    fn parse_named_key() {
        let seq = ShortcutSequence::from_str("Enter").unwrap();
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Enter,
                modifiers: Modifiers::empty()
            }]
        );
    }

    #[test]
    fn parse_escape() {
        let seq = ShortcutSequence::from_str("Escape").unwrap();
        assert_eq!(seq.chords[0].key, Key::Escape);
    }

    #[test]
    fn parse_symbol_alias() {
        let seq = ShortcutSequence::from_str("Cmd+Equal").unwrap();
        let expected_mod = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };
        assert_eq!(
            seq.chords,
            vec![KeyChord {
                key: Key::Character("=".to_string()),
                modifiers: expected_mod
            }]
        );
    }

    #[test]
    fn parse_bracket_sequence() {
        let seq = ShortcutSequence::from_str("] ]").unwrap();
        assert_eq!(seq.chords.len(), 2);
        assert!(seq
            .chords
            .iter()
            .all(|c| c.key == Key::Character("]".to_string())));
    }

    #[test]
    fn parse_empty() {
        assert_eq!(
            ShortcutSequence::from_str("").unwrap_err(),
            ShortcutParseError::Empty
        );
    }

    #[test]
    fn parse_missing_key() {
        assert_eq!(
            ShortcutSequence::from_str("Ctrl+").unwrap_err(),
            ShortcutParseError::MissingKey
        );
    }

    #[test]
    fn parse_unknown_key() {
        assert!(matches!(
            ShortcutSequence::from_str("no_such_key").unwrap_err(),
            ShortcutParseError::UnknownKey(_)
        ));
    }

    #[test]
    fn parse_unknown_modifier() {
        // "super" is NOT a recognized modifier (unlike the old Code-based version)
        assert!(matches!(
            ShortcutSequence::from_str("foomod+g").unwrap_err(),
            ShortcutParseError::UnknownModifier(_)
        ));
    }

    // --- Display tests ---

    #[test]
    fn display_simple() {
        let chord = KeyChord {
            key: Key::Character("j".to_string()),
            modifiers: Modifiers::empty(),
        };
        assert_eq!(chord.to_string(), "j");
    }

    #[test]
    fn display_with_modifiers() {
        let chord = KeyChord {
            key: Key::Character("g".to_string()),
            modifiers: Modifiers::CONTROL | Modifiers::SHIFT,
        };
        assert_eq!(chord.to_string(), "Ctrl+Shift+g");
    }

    #[test]
    fn display_named_key() {
        let chord = KeyChord {
            key: Key::Enter,
            modifiers: Modifiers::META,
        };
        if cfg!(target_os = "macos") {
            assert_eq!(chord.to_string(), "Cmd+Enter");
        } else {
            assert_eq!(chord.to_string(), "Win+Enter");
        }
    }

    #[test]
    fn display_sequence() {
        let seq = ShortcutSequence::from_str("g g").unwrap();
        assert_eq!(seq.to_string(), "g g");
    }

    // --- from_js_event tests ---

    #[test]
    fn from_js_event_simple() {
        let chord = KeyChord::from_js_event("j", 0);
        assert_eq!(chord.key, Key::Character("j".to_string()));
        assert_eq!(chord.modifiers, Modifiers::empty());
    }

    #[test]
    fn from_js_event_with_shift() {
        // JS sends key="G" with SHIFT flag (0x200)
        let chord = KeyChord::from_js_event("G", 0x200);
        assert_eq!(chord.key, Key::Character("g".to_string()));
        assert!(chord.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn from_js_event_named_key() {
        let chord = KeyChord::from_js_event("Enter", 0);
        assert_eq!(chord.key, Key::Enter);
    }

    #[test]
    fn from_js_event_ctrl_modifier() {
        // CONTROL = 0x08
        let chord = KeyChord::from_js_event("d", 0x08);
        assert_eq!(chord.key, Key::Character("d".to_string()));
        assert!(chord.modifiers.contains(Modifiers::CONTROL));
    }

    #[test]
    fn js_modifier_bits_match_rust_modifiers() {
        // Verify JS keyboard-interceptor.ts bit constants match
        // dioxus Modifiers bitflags used in from_js_event().
        // If dioxus changes its bit layout, this test will catch the mismatch.
        assert_eq!(Modifiers::ALT.bits(), 0x01);
        assert_eq!(Modifiers::CONTROL.bits(), 0x08);
        assert_eq!(Modifiers::META.bits(), 0x40);
        assert_eq!(Modifiers::SHIFT.bits(), 0x200);
    }

    // --- Helper method tests ---

    #[test]
    fn is_modifier_only() {
        assert!(KeyChord {
            key: Key::Shift,
            modifiers: Modifiers::SHIFT
        }
        .is_modifier_only());

        assert!(!KeyChord {
            key: Key::Character("j".to_string()),
            modifiers: Modifiers::empty()
        }
        .is_modifier_only());
    }

    #[test]
    fn is_single_sequence() {
        let single = ShortcutSequence::from_str("g").unwrap();
        assert!(single.is_single());

        let multi = ShortcutSequence::from_str("g g").unwrap();
        assert!(!multi.is_single());
    }
}
