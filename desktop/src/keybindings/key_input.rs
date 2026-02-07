use std::fmt;
use std::str::FromStr;

use bitflags::bitflags;

bitflags! {
    /// Keyboard modifier flags.
    ///
    /// Order is significant for display: Ctrl+Alt+Shift+Cmd
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b0001;
        const CTRL  = 0b0010;
        const ALT   = 0b0100;
        const CMD   = 0b1000;
    }
}

/// Parsed representation of a single key press (normalized form).
///
/// Normalization rules:
/// - Modifier order: Ctrl+Alt+Shift+Cmd
/// - Alpha keys: lowercase ("a"-"z")
/// - Digit keys: as-is ("0"-"9")
/// - Special keys: PascalCase ("Escape", "Enter", "BracketLeft", "Space")
/// - Shift+alpha: `Shift+` prefix with lowercase key (`Shift+g`, not `G`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyInput {
    /// Normalized key name
    pub key: String,
    pub modifiers: Modifiers,
}

/// A key sequence is one or more KeyInputs.
///
/// Examples:
/// - `"g g"` → `KeySequence([KeyInput("g", 0), KeyInput("g", 0)])`
/// - `"Ctrl+d"` → `KeySequence([KeyInput("d", CTRL)])`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySequence(pub Vec<KeyInput>);

/// Error type for parsing key notations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    EmptyInput,
    EmptyKeyPart,
    UnknownModifier(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyInput => write!(f, "empty key notation"),
            ParseError::EmptyKeyPart => write!(f, "empty key part in notation"),
            ParseError::UnknownModifier(m) => write!(f, "unknown modifier: {m}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl KeyInput {
    /// Create a new KeyInput with the given key and modifiers.
    #[cfg(test)]
    pub fn new(key: impl Into<String>, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
        }
    }

    /// Parse a single key notation string into a KeyInput.
    ///
    /// Normalizes the input:
    /// - `"G"` → `Shift+g`
    /// - `"ctrl+D"` → `Ctrl+Shift+d`
    /// - `"Escape"` → `Escape` (no modifiers)
    /// - `"Cmd+Shift+w"` → `Cmd+Shift+w`
    pub fn parse(notation: &str) -> Result<Self, ParseError> {
        let notation = notation.trim();
        if notation.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        let parts: Vec<&str> = notation.split('+').collect();
        let mut modifiers = Modifiers::empty();

        // All parts except the last are modifiers
        for &part in &parts[..parts.len() - 1] {
            let modifier = parse_modifier(part)?;
            modifiers |= modifier;
        }

        // Last part is the key
        let raw_key = parts[parts.len() - 1];
        if raw_key.is_empty() {
            return Err(ParseError::EmptyKeyPart);
        }

        let (key, extra_modifiers) = normalize_key(raw_key);
        modifiers |= extra_modifiers;

        Ok(KeyInput { key, modifiers })
    }
}

impl FromStr for KeyInput {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for KeyInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display in canonical order: Ctrl+Alt+Shift+Cmd+key
        if self.modifiers.contains(Modifiers::CTRL) {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.contains(Modifiers::ALT) {
            write!(f, "Alt+")?;
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            write!(f, "Shift+")?;
        }
        if self.modifiers.contains(Modifiers::CMD) {
            write!(f, "Cmd+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl KeySequence {
    /// Parse a key sequence notation (space-separated key inputs).
    ///
    /// Examples:
    /// - `"g g"` → two-key sequence
    /// - `"Ctrl+d"` → single-key sequence
    /// - `"g Shift+t"` → mixed sequence
    pub fn parse(notation: &str) -> Result<Self, ParseError> {
        let notation = notation.trim();
        if notation.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        let keys: Result<Vec<KeyInput>, ParseError> =
            notation.split_whitespace().map(KeyInput::parse).collect();

        Ok(KeySequence(keys?))
    }

    /// Returns the number of key inputs in the sequence.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if this sequence starts with the given prefix.
    pub fn starts_with(&self, prefix: &[KeyInput]) -> bool {
        if prefix.len() > self.0.len() {
            return false;
        }
        self.0[..prefix.len()] == *prefix
    }
}

impl FromStr for KeySequence {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, key) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{key}")?;
        }
        Ok(())
    }
}

/// Parse a modifier name (case-insensitive) into a Modifiers flag.
fn parse_modifier(name: &str) -> Result<Modifiers, ParseError> {
    match name.to_lowercase().as_str() {
        "ctrl" | "control" => Ok(Modifiers::CTRL),
        "alt" | "option" | "opt" => Ok(Modifiers::ALT),
        "shift" => Ok(Modifiers::SHIFT),
        "cmd" | "command" | "super" | "meta" => Ok(Modifiers::CMD),
        _ => Err(ParseError::UnknownModifier(name.to_string())),
    }
}

/// Normalize a key name, returning the normalized key and any implicit modifiers.
///
/// Rules:
/// - Single uppercase letter `"G"` → `("g", SHIFT)`
/// - Single lowercase letter `"g"` → `("g", empty)`
/// - Single digit `"0"` → `("0", empty)`
/// - Special key names are PascalCased
fn normalize_key(raw: &str) -> (String, Modifiers) {
    // Single character
    if raw.len() == 1 {
        let ch = raw.chars().next().unwrap();

        if ch.is_ascii_uppercase() {
            // Uppercase letter implies Shift
            return (ch.to_ascii_lowercase().to_string(), Modifiers::SHIFT);
        }

        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            return (raw.to_string(), Modifiers::empty());
        }

        // Punctuation and symbols: use the key code name mapping
        if let Some(name) = punctuation_to_name(ch) {
            return (name.to_string(), Modifiers::empty());
        }

        return (raw.to_string(), Modifiers::empty());
    }

    // Multi-character: normalize known special key names
    let normalized = normalize_special_key(raw);
    (normalized, Modifiers::empty())
}

/// Map punctuation characters to their key code names.
fn punctuation_to_name(ch: char) -> Option<&'static str> {
    match ch {
        '/' => Some("Slash"),
        '.' => Some("Period"),
        ',' => Some("Comma"),
        ';' => Some("Semicolon"),
        '\'' => Some("Quote"),
        '[' => Some("BracketLeft"),
        ']' => Some("BracketRight"),
        '\\' => Some("Backslash"),
        '-' => Some("Minus"),
        '=' => Some("Equal"),
        '`' => Some("Backquote"),
        ' ' => Some("Space"),
        _ => None,
    }
}

/// Normalize special key names to PascalCase.
fn normalize_special_key(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "escape" | "esc" => "Escape".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "Space".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "arrowup" | "up" => "ArrowUp".to_string(),
        "arrowdown" | "down" => "ArrowDown".to_string(),
        "arrowleft" | "left" => "ArrowLeft".to_string(),
        "arrowright" | "right" => "ArrowRight".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "bracketleft" => "BracketLeft".to_string(),
        "bracketright" => "BracketRight".to_string(),
        "backslash" => "Backslash".to_string(),
        "slash" => "Slash".to_string(),
        "period" => "Period".to_string(),
        "comma" => "Comma".to_string(),
        "semicolon" => "Semicolon".to_string(),
        "quote" => "Quote".to_string(),
        "minus" => "Minus".to_string(),
        "equal" => "Equal".to_string(),
        "backquote" => "Backquote".to_string(),
        _ => raw.to_string(),
    }
}

/// Construct a KeyInput from JavaScript keydown event data.
///
/// This is the entry point for converting raw JS events into normalized KeyInputs.
/// The JS interceptor sends `{key, modifiers}` where:
/// - `key` is `event.key` (e.g., "a", "Escape", "Enter")
/// - `modifiers` is a bitmask: SHIFT=1, CTRL=2, ALT=4, CMD=8
pub fn from_js_event(key: &str, modifier_bits: u8) -> KeyInput {
    let modifiers = Modifiers::from_bits_truncate(modifier_bits);

    // Normalize the JS event.key value
    let (normalized_key, extra_modifiers) = normalize_js_key(key);

    KeyInput {
        key: normalized_key,
        modifiers: modifiers | extra_modifiers,
    }
}

/// Normalize a JavaScript `event.key` value.
///
/// JS event.key uses different conventions than our internal format:
/// - Single uppercase letter (with Shift held) → lowercase + SHIFT flag
/// - Special keys like "Escape", "Enter" → already PascalCase
fn normalize_js_key(key: &str) -> (String, Modifiers) {
    // Single character
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();

        if ch.is_ascii_uppercase() {
            return (ch.to_ascii_lowercase().to_string(), Modifiers::SHIFT);
        }

        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            return (key.to_string(), Modifiers::empty());
        }

        // Punctuation
        if let Some(name) = punctuation_to_name(ch) {
            return (name.to_string(), Modifiers::empty());
        }

        return (key.to_string(), Modifiers::empty());
    }

    // Multi-character: normalize known JS key names
    let normalized = normalize_special_key(key);
    (normalized, Modifiers::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let input = KeyInput::parse("j").unwrap();
        assert_eq!(input.key, "j");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_parse_uppercase_implies_shift() {
        let input = KeyInput::parse("G").unwrap();
        assert_eq!(input.key, "g");
        assert_eq!(input.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn test_parse_modifier_key() {
        let input = KeyInput::parse("Ctrl+d").unwrap();
        assert_eq!(input.key, "d");
        assert_eq!(input.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn test_parse_shift_lowercase() {
        let input = KeyInput::parse("Shift+g").unwrap();
        assert_eq!(input.key, "g");
        assert_eq!(input.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn test_parse_cmd_shift() {
        let input = KeyInput::parse("Cmd+Shift+w").unwrap();
        assert_eq!(input.key, "w");
        assert_eq!(input.modifiers, Modifiers::CMD | Modifiers::SHIFT);
    }

    #[test]
    fn test_parse_special_key() {
        let input = KeyInput::parse("Escape").unwrap();
        assert_eq!(input.key, "Escape");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_parse_punctuation() {
        let input = KeyInput::parse("/").unwrap();
        assert_eq!(input.key, "Slash");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_parse_digit() {
        let input = KeyInput::parse("1").unwrap();
        assert_eq!(input.key, "1");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_parse_alt_shift_comma() {
        let input = KeyInput::parse("Alt+Shift+,").unwrap();
        assert_eq!(input.key, "Comma");
        assert_eq!(input.modifiers, Modifiers::ALT | Modifiers::SHIFT);
    }

    #[test]
    fn test_display_roundtrip() {
        let cases = [
            "j",
            "Shift+g",
            "Ctrl+d",
            "Ctrl+Shift+a",
            "Cmd+Shift+w",
            "Escape",
        ];
        for notation in cases {
            let input = KeyInput::parse(notation).unwrap();
            let displayed = input.to_string();
            let reparsed = KeyInput::parse(&displayed).unwrap();
            assert_eq!(input, reparsed, "roundtrip failed for {notation}");
        }
    }

    #[test]
    fn test_parse_sequence() {
        let seq = KeySequence::parse("g g").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.0[0].key, "g");
        assert_eq!(seq.0[1].key, "g");
    }

    #[test]
    fn test_parse_mixed_sequence() {
        let seq = KeySequence::parse("g Shift+t").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.0[0].key, "g");
        assert_eq!(seq.0[0].modifiers, Modifiers::empty());
        assert_eq!(seq.0[1].key, "t");
        assert_eq!(seq.0[1].modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn test_sequence_display() {
        let seq = KeySequence::parse("g g").unwrap();
        assert_eq!(seq.to_string(), "g g");
    }

    #[test]
    fn test_sequence_starts_with() {
        let seq = KeySequence::parse("g g").unwrap();
        let prefix = vec![KeyInput::new("g", Modifiers::empty())];
        assert!(seq.starts_with(&prefix));
    }

    #[test]
    fn test_from_js_event_lowercase() {
        let input = from_js_event("j", 0);
        assert_eq!(input.key, "j");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_from_js_event_uppercase_with_shift() {
        // JS sends uppercase letter when Shift is held
        let input = from_js_event("G", 1); // SHIFT=1
        assert_eq!(input.key, "g");
        assert_eq!(input.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn test_from_js_event_ctrl() {
        let input = from_js_event("d", 2); // CTRL=2
        assert_eq!(input.key, "d");
        assert_eq!(input.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn test_from_js_event_escape() {
        let input = from_js_event("Escape", 0);
        assert_eq!(input.key, "Escape");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_from_js_event_slash() {
        let input = from_js_event("/", 0);
        assert_eq!(input.key, "Slash");
        assert_eq!(input.modifiers, Modifiers::empty());
    }

    #[test]
    fn test_parse_error_empty() {
        assert_eq!(KeyInput::parse(""), Err(ParseError::EmptyInput));
    }

    #[test]
    fn test_parse_error_unknown_modifier() {
        assert!(matches!(
            KeyInput::parse("Foo+a"),
            Err(ParseError::UnknownModifier(_))
        ));
    }

    #[test]
    fn test_modifier_case_insensitive() {
        let input = KeyInput::parse("ctrl+d").unwrap();
        assert_eq!(input.modifiers, Modifiers::CTRL);

        let input = KeyInput::parse("CTRL+d").unwrap();
        assert_eq!(input.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn test_esc_alias() {
        let input = KeyInput::parse("Esc").unwrap();
        assert_eq!(input.key, "Escape");
    }
}
