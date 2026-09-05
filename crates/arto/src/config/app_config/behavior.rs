use serde::{Deserialize, Serialize};

/// Behavior when application starts
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupBehavior {
    /// Use the default setting
    #[default]
    Default,
    /// Use the setting from the last closed window
    LastClosed,
}

/// Behavior when opening a new window
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewWindowBehavior {
    /// Use the default setting
    #[default]
    Default,
    /// Use the setting from the last focused window
    LastFocused,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_behavior_serialization() {
        let default = StartupBehavior::Default;
        let last_closed = StartupBehavior::LastClosed;

        let default_json = serde_json::to_string(&default).unwrap();
        let last_closed_json = serde_json::to_string(&last_closed).unwrap();

        assert_eq!(default_json, r#""default""#);
        assert_eq!(last_closed_json, r#""last_closed""#);

        let parsed_default: StartupBehavior = serde_json::from_str(&default_json).unwrap();
        let parsed_last: StartupBehavior = serde_json::from_str(&last_closed_json).unwrap();

        assert_eq!(parsed_default, StartupBehavior::Default);
        assert_eq!(parsed_last, StartupBehavior::LastClosed);
    }

    #[test]
    fn test_new_window_behavior_serialization() {
        let default = NewWindowBehavior::Default;
        let last_focused = NewWindowBehavior::LastFocused;

        let default_json = serde_json::to_string(&default).unwrap();
        let last_focused_json = serde_json::to_string(&last_focused).unwrap();

        assert_eq!(default_json, r#""default""#);
        assert_eq!(last_focused_json, r#""last_focused""#);
    }
}
