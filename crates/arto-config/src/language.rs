use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The language Arto's own interface is shown in.
///
/// The documents read are never translated; this only chooses the words of
/// menus, preferences and the like. `auto` follows the system language and
/// falls back to English when the system asks for one Arto does not have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// The system language, or English when Arto has no translation for it
    #[default]
    Auto,
    /// English
    En,
    /// Japanese (日本語)
    Ja,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_language_codes() {
        assert_eq!(serde_json::to_string(&Language::Auto).unwrap(), r#""auto""#);
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), r#""en""#);
        assert_eq!(serde_json::to_string(&Language::Ja).unwrap(), r#""ja""#);
    }

    #[test]
    fn deserializes_from_language_codes() {
        let parsed: Language = serde_json::from_str(r#""ja""#).unwrap();
        assert_eq!(parsed, Language::Ja);
    }
}
