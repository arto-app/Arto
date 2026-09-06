use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Context for keybinding matching.
///
/// When a key chord matches both a context-specific and global binding,
/// the context-specific binding takes priority.
/// Bindings from a different context are invisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyContext {
    Content,
    Sidebar,
    RightSidebar,
    QuickAccess,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyContextParseError(pub(crate) String);

impl fmt::Display for KeyContextParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown key context: {:?}", self.0)
    }
}

impl std::error::Error for KeyContextParseError {}

impl fmt::Display for KeyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content => f.write_str("content"),
            Self::Sidebar => f.write_str("sidebar"),
            Self::RightSidebar => f.write_str("right_sidebar"),
            Self::QuickAccess => f.write_str("quick_access"),
            Self::Search => f.write_str("search"),
        }
    }
}

impl FromStr for KeyContext {
    type Err = KeyContextParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "content" => Ok(Self::Content),
            "sidebar" => Ok(Self::Sidebar),
            "right_sidebar" => Ok(Self::RightSidebar),
            "quick_access" => Ok(Self::QuickAccess),
            "search" => Ok(Self::Search),
            _ => Err(KeyContextParseError(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip() {
        let contexts = [
            KeyContext::Content,
            KeyContext::Sidebar,
            KeyContext::RightSidebar,
            KeyContext::QuickAccess,
            KeyContext::Search,
        ];
        for ctx in &contexts {
            let s = ctx.to_string();
            let parsed: KeyContext = s.parse().unwrap();
            assert_eq!(*ctx, parsed);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let ctx = KeyContext::RightSidebar;
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, r#""right_sidebar""#);
        let parsed: KeyContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ctx);
    }

    #[test]
    fn content_serde() {
        let ctx = KeyContext::Content;
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, r#""content""#);
        let parsed: KeyContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ctx);
    }

    #[test]
    fn parse_invalid() {
        assert!("unknown".parse::<KeyContext>().is_err());
        assert!("".parse::<KeyContext>().is_err());
    }
}
