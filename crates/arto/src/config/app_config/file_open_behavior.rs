use serde::{Deserialize, Serialize};

/// Behavior for routing externally opened files/directories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOpenBehavior {
    /// Always create a new window.
    NewWindow,
    /// Open in the last focused visible window when possible.
    #[default]
    LastFocused,
    /// Open in a visible window on the cursor's current screen when possible.
    CurrentScreen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_open_behavior_default() {
        assert_eq!(FileOpenBehavior::default(), FileOpenBehavior::LastFocused);
    }

    #[test]
    fn test_file_open_behavior_serialization_roundtrip() {
        let cases = [
            (FileOpenBehavior::NewWindow, r#""new_window""#),
            (FileOpenBehavior::LastFocused, r#""last_focused""#),
            (FileOpenBehavior::CurrentScreen, r#""current_screen""#),
        ];

        for (behavior, expected_json) in cases {
            let json = serde_json::to_string(&behavior).unwrap();
            assert_eq!(json, expected_json);
            let parsed: FileOpenBehavior = serde_json::from_str(expected_json).unwrap();
            assert_eq!(parsed, behavior);
        }
    }
}
