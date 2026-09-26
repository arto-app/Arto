use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the reading of a document is measured and shown, and what Arto marks
/// on it beyond what it says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ReadingConfig {
    /// Show how long a document takes to read, and how long is left, in the header.
    pub show_time: bool,
    /// Words read per minute, for scripts written in words.
    #[schemars(range(min = 1))]
    pub words_per_minute: u32,
    /// Characters read per minute, for Chinese, Japanese and Korean.
    #[schemars(range(min = 1))]
    pub characters_per_minute: u32,
    /// Documents shorter than this many minutes show no reading time.
    pub min_minutes: u32,
    /// Mark what changed in a document since it was last read: a line in the
    /// margin beside each block added or rewritten, a hairline where text
    /// was taken out, and a dot on the headings they fall under. Turned off,
    /// Arto stops keeping a copy of each document read.
    pub show_changes: bool,
    /// Take a line whose words are only spaced differently to be unchanged.
    pub ignore_whitespace_changes: bool,
}

impl Default for ReadingConfig {
    fn default() -> Self {
        Self {
            show_time: true,
            words_per_minute: 230,
            characters_per_minute: 500,
            min_minutes: 3,
            show_changes: true,
            ignore_whitespace_changes: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_field_takes_its_default() {
        let parsed: ReadingConfig = serde_json::from_str(r#"{"wordsPerMinute": 300}"#).unwrap();
        assert_eq!(
            parsed,
            ReadingConfig {
                words_per_minute: 300,
                ..ReadingConfig::default()
            }
        );
    }

    #[test]
    fn changes_are_shown_and_spacing_ignored_unless_turned_off() {
        let config = ReadingConfig::default();
        assert!(config.show_changes);
        assert!(config.ignore_whitespace_changes);

        let parsed: ReadingConfig = serde_json::from_str(r#"{"showChanges":false}"#).unwrap();
        assert!(!parsed.show_changes);
        assert!(parsed.ignore_whitespace_changes);
    }

    #[test]
    fn it_survives_a_round_trip() {
        let config = ReadingConfig {
            show_time: false,
            words_per_minute: 180,
            characters_per_minute: 400,
            min_minutes: 5,
            show_changes: false,
            ignore_whitespace_changes: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            json,
            r#"{"showTime":false,"wordsPerMinute":180,"charactersPerMinute":400,"minMinutes":5,"showChanges":false,"ignoreWhitespaceChanges":false}"#
        );
        assert_eq!(
            serde_json::from_str::<ReadingConfig>(&json).unwrap(),
            config
        );
    }
}
