use serde::{Deserialize, Serialize};

/// Keybinding configuration stored in config.json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeybindingsConfig {
    /// Template: "vim", "emacs", or "none"
    pub template: KeybindingTemplate,

    /// User-defined bindings that override or extend the template
    pub custom: Vec<KeyBinding>,
}

/// Available keybinding templates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeybindingTemplate {
    #[default]
    None,
    Vim,
    Emacs,
}

/// A single key-to-action binding in config.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Key notation (e.g., "g g", "Ctrl+d", "Shift+g")
    pub key: String,

    /// Action identifier (e.g., "scroll.top", "tab.next")
    pub action: String,

    /// Optional context that restricts when this binding is active
    /// (e.g., "sidebar", "right_sidebar", "quick_access").
    /// When None, the binding is global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_default_template_is_none() {
        let config = KeybindingsConfig::default();
        assert_eq!(config.template, KeybindingTemplate::None);
        assert!(config.custom.is_empty());
    }

    #[test]
    fn test_template_serialization() {
        assert_eq!(
            serde_json::to_string(&KeybindingTemplate::None).unwrap(),
            r#""none""#
        );
        assert_eq!(
            serde_json::to_string(&KeybindingTemplate::Vim).unwrap(),
            r#""vim""#
        );
        assert_eq!(
            serde_json::to_string(&KeybindingTemplate::Emacs).unwrap(),
            r#""emacs""#
        );
    }

    #[test]
    fn test_config_roundtrip() {
        let config = KeybindingsConfig {
            template: KeybindingTemplate::Vim,
            custom: vec![
                KeyBinding {
                    key: "g g".to_string(),
                    action: "scroll.top".to_string(),
                    context: None,
                },
                KeyBinding {
                    key: "j".to_string(),
                    action: "cursor.down".to_string(),
                    context: Some("sidebar".to_string()),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: KeybindingsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_context_not_serialized_when_none() {
        let binding = KeyBinding {
            key: "j".to_string(),
            action: "scroll.down".to_string(),
            context: None,
        };
        let json = serde_json::to_string(&binding).unwrap();
        assert!(!json.contains("context"));
    }

    #[test]
    fn test_deserialize_binding_with_context() {
        let json = indoc! {r#"
            {
              "template": "vim",
              "custom": [
                { "key": "j", "action": "cursor.down", "context": "sidebar" }
              ]
            }
        "#};

        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.custom[0].context, Some("sidebar".to_string()));
    }

    #[test]
    fn test_deserialize_from_json() {
        let json = indoc! {r#"
            {
              "template": "vim",
              "custom": [
                { "key": "j", "action": "zoom.in" }
              ]
            }
        "#};

        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.template, KeybindingTemplate::Vim);
        assert_eq!(config.custom.len(), 1);
        assert_eq!(config.custom[0].key, "j");
        assert_eq!(config.custom[0].action, "zoom.in");
    }

    #[test]
    fn test_deserialize_empty_object() {
        let json = "{}";
        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.template, KeybindingTemplate::None);
        assert!(config.custom.is_empty());
    }

    #[test]
    fn test_deserialize_template_only() {
        let json = r#"{"template": "emacs"}"#;
        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.template, KeybindingTemplate::Emacs);
        assert!(config.custom.is_empty());
    }
}
