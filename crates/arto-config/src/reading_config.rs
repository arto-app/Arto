use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the reading of a document is measured and shown.
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
}

impl Default for ReadingConfig {
    fn default() -> Self {
        Self {
            show_time: true,
            words_per_minute: 230,
            characters_per_minute: 500,
            min_minutes: 3,
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
    fn it_survives_a_round_trip() {
        let config = ReadingConfig {
            show_time: false,
            words_per_minute: 180,
            characters_per_minute: 400,
            min_minutes: 5,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            json,
            r#"{"showTime":false,"wordsPerMinute":180,"charactersPerMinute":400,"minMinutes":5}"#
        );
        assert_eq!(
            serde_json::from_str::<ReadingConfig>(&json).unwrap(),
            config
        );
    }
}
