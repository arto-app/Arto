use serde::{Deserialize, Serialize};

/// Per-context keybinding definition stored in `mappings.json`.
///
/// Two distinct kinds of shortcut live here, differing by capability:
///
/// - `menu_shortcuts`: native OS menu accelerators. Single-chord only, shown in
///   the menu bar, dispatched by the OS (so they work even when no window has
///   keyboard focus). Keyed by a menu-backed [`crate::Action`].
/// - Everything else (`global` + per-context): keybindings handled by the
///   in-window engine. Support chord sequences and per-context resolution but
///   only fire while a WebView has focus.
///
/// `global` bindings are always visible (fallback for all contexts).
/// Other context fields hold bindings active only in that specific context.
/// Presets are loaded on demand from the UI, not stored as a field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BindingSet {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu_shortcuts: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidebar: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quick_access: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub right_sidebar: Vec<KeyAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<KeyAction>,
}

/// A single key → action mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyAction {
    pub key: String,
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let set = BindingSet::default();
        assert!(set.menu_shortcuts.is_empty());
        assert!(set.global.is_empty());
        assert!(set.content.is_empty());
        assert!(set.sidebar.is_empty());
        assert!(set.quick_access.is_empty());
        assert!(set.right_sidebar.is_empty());
        assert!(set.search.is_empty());
    }

    #[test]
    fn roundtrip() {
        let set = BindingSet {
            global: vec![KeyAction {
                key: "g g".to_string(),
                action: "scroll.top".to_string(),
            }],
            sidebar: vec![KeyAction {
                key: "j".to_string(),
                action: "cursor.down".to_string(),
            }],
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&set).unwrap();
        let parsed: BindingSet = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, set);
    }

    #[test]
    fn empty_json_uses_defaults() {
        let set: BindingSet = serde_json::from_str("{}").unwrap();
        assert!(set.global.is_empty());
        assert!(set.content.is_empty());
        assert!(set.sidebar.is_empty());
        assert!(set.quick_access.is_empty());
        assert!(set.right_sidebar.is_empty());
        assert!(set.search.is_empty());
    }

    #[test]
    fn skip_serializing_empty_fields() {
        let json = serde_json::to_string(&BindingSet::default()).unwrap();
        assert!(!json.contains("global"));
        assert!(!json.contains("sidebar"));
    }

    #[test]
    fn structured_json() {
        let json = r#"{
            "global": [{"key": "x", "action": "tab.close"}],
            "sidebar": [{"key": "o", "action": "cursor.enter"}]
        }"#;
        let set: BindingSet = serde_json::from_str(json).unwrap();
        assert_eq!(set.global.len(), 1);
        assert_eq!(set.sidebar.len(), 1);
        assert!(set.content.is_empty());
    }

    #[test]
    fn non_empty_when_global_has_item() {
        let set = BindingSet {
            global: vec![KeyAction {
                key: "j".to_string(),
                action: "scroll.down".to_string(),
            }],
            ..Default::default()
        };
        assert!(!set.global.is_empty());
    }

    #[test]
    fn menu_shortcuts_roundtrip() {
        let set = BindingSet {
            menu_shortcuts: vec![KeyAction {
                key: "Cmd+o".to_string(),
                action: "file.open".to_string(),
            }],
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&set).unwrap();
        assert!(json.contains("menuShortcuts"));
        let parsed: BindingSet = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, set);
    }

    #[test]
    fn menu_shortcuts_skipped_when_empty() {
        let json = serde_json::to_string(&BindingSet::default()).unwrap();
        assert!(!json.contains("menuShortcuts"));
    }

    /// Legacy `mappings.json` files (written before `menuShortcuts` existed)
    /// must keep loading: the new field defaults to empty, and existing
    /// per-context sections are preserved.
    #[test]
    fn legacy_json_without_menu_shortcuts_loads() {
        let json = r#"{
            "global": [{"key": "Cmd+o", "action": "file.open"}],
            "sidebar": [{"key": "j", "action": "cursor.down"}]
        }"#;
        let set: BindingSet = serde_json::from_str(json).unwrap();
        assert!(set.menu_shortcuts.is_empty());
        assert_eq!(set.global.len(), 1);
        assert_eq!(set.sidebar.len(), 1);
    }
}
