use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Context in which a keybinding is active.
///
/// Bindings with a context only trigger when the matching panel has focus.
/// Bindings without a context (None) are global and trigger in any panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyContext {
    Sidebar,
    RightSidebar,
    QuickAccess,
}

/// Error type for parsing context strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeyContextError(pub String);

impl fmt::Display for ParseKeyContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown key context: {}", self.0)
    }
}

impl std::error::Error for ParseKeyContextError {}

impl FromStr for KeyContext {
    type Err = ParseKeyContextError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sidebar" => Ok(KeyContext::Sidebar),
            "right_sidebar" => Ok(KeyContext::RightSidebar),
            "quick_access" => Ok(KeyContext::QuickAccess),
            _ => Err(ParseKeyContextError(s.to_string())),
        }
    }
}

impl fmt::Display for KeyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyContext::Sidebar => write!(f, "sidebar"),
            KeyContext::RightSidebar => write!(f, "right_sidebar"),
            KeyContext::QuickAccess => write!(f, "quick_access"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_contexts() {
        assert_eq!(
            "sidebar".parse::<KeyContext>().unwrap(),
            KeyContext::Sidebar
        );
        assert_eq!(
            "right_sidebar".parse::<KeyContext>().unwrap(),
            KeyContext::RightSidebar
        );
        assert_eq!(
            "quick_access".parse::<KeyContext>().unwrap(),
            KeyContext::QuickAccess
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert!("content".parse::<KeyContext>().is_err());
        assert!("unknown".parse::<KeyContext>().is_err());
    }

    #[test]
    fn test_display_roundtrip() {
        let contexts = [
            KeyContext::Sidebar,
            KeyContext::RightSidebar,
            KeyContext::QuickAccess,
        ];
        for ctx in &contexts {
            let s = ctx.to_string();
            let parsed: KeyContext = s.parse().unwrap();
            assert_eq!(&parsed, ctx, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let ctx = KeyContext::RightSidebar;
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, r#""right_sidebar""#);
        let parsed: KeyContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ctx);
    }
}
