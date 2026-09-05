use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Configuration for Markdown rendering behavior
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownConfig {
    /// Whether to automatically convert bare URLs to clickable links (default: true)
    #[serde(default = "default_true")]
    pub auto_link_urls: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            auto_link_urls: true,
        }
    }
}
