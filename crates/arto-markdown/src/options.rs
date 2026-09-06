use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Choices that change how Markdown is rendered.
///
/// This is also the `markdown` section of Arto's `config.json`, so every
/// consumer of the pipeline (the app, `arto page`, Quick Look) exposes the
/// same options and reads the same user preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    /// Turn bare URLs into links (default: true).
    #[serde(default = "default_true")]
    pub auto_link_urls: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            auto_link_urls: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_fields_fall_back_to_defaults() {
        let parsed: RenderOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, RenderOptions::default());
        assert!(parsed.auto_link_urls);
    }

    #[test]
    fn test_serializes_with_camel_case_keys() {
        let json = serde_json::to_string(&RenderOptions {
            auto_link_urls: false,
        })
        .unwrap();
        assert_eq!(json, r#"{"autoLinkUrls":false}"#);
    }
}
