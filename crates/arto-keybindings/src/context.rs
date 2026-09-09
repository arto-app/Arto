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
    /// The panel, whichever of its faces is showing. There used to be one of
    /// these per face; the keys that walk a list of documents are the same
    /// keys whether the list is a tree, a history or a set of stars, and
    /// having to bind them three times was three chances to bind them
    /// differently.
    Sidebar,
    Search,
    Palette,
    /// The contents, while they are open by name rather than under the
    /// pointer. A list of headings answers the same keys the panel's lists do.
    Contents,
}

impl KeyContext {
    /// Every context, in the order they are listed to the reader.
    ///
    /// What walks this rather than spelling the contexts out: resolving a
    /// binding set for the engine, and the preferences' own list of sections.
    /// A context added to the enum is therefore added to both.
    pub const ALL: [Self; 5] = [
        Self::Content,
        Self::Sidebar,
        Self::Search,
        Self::Palette,
        Self::Contents,
    ];

    /// The name this context is listed under.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Content => "Content",
            // The panel is what the reader sees; `sidebar` is the name the
            // configuration has always used for it.
            Self::Sidebar => "Panel",
            Self::Search => "Search",
            Self::Palette => "Palette",
            Self::Contents => "Contents",
        }
    }

    /// Whether this context is a field being typed into.
    ///
    /// What is typed there is text, not shortcuts, so a bare key belongs to
    /// the field: only this context's own bindings answer it, and the global
    /// ones — which is where `j` means "scroll" — stay out of the way. A chord
    /// with a modifier is nobody's idea of typing, so those still fall through
    /// to the global set, and Cmd+W closes the window from inside a search.
    pub fn owns_input(&self) -> bool {
        matches!(self, Self::Search | Self::Palette)
    }
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
            Self::Search => f.write_str("search"),
            Self::Palette => f.write_str("palette"),
            Self::Contents => f.write_str("contents"),
        }
    }
}

impl FromStr for KeyContext {
    type Err = KeyContextParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "content" => Ok(Self::Content),
            "sidebar" => Ok(Self::Sidebar),
            "search" => Ok(Self::Search),
            "palette" => Ok(Self::Palette),
            "contents" => Ok(Self::Contents),
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
            KeyContext::Search,
            KeyContext::Palette,
        ];
        for ctx in &contexts {
            let s = ctx.to_string();
            let parsed: KeyContext = s.parse().unwrap();
            assert_eq!(*ctx, parsed);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let ctx = KeyContext::Sidebar;
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, r#""sidebar""#);
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

    #[test]
    fn a_field_owns_its_bare_keys() {
        assert!(KeyContext::Search.owns_input());
        assert!(KeyContext::Palette.owns_input());
        assert!(!KeyContext::Content.owns_input());
        assert!(!KeyContext::Sidebar.owns_input());
    }
}
