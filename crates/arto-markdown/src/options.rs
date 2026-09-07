use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// How a local image reaches the document.
///
/// The two consumers of the pipeline want opposite things from an image. A
/// page that has to stand on its own — what `arto page` writes and what Quick
/// Look previews — can only carry the bytes. The app has a window it serves
/// itself, and inlining there costs it the memory twice over, in the HTML it
/// builds and in the WebView that parses it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImageResolution {
    /// Read the file and inline it as a `data:` URL.
    #[default]
    DataUrl,
    /// Emit `{base_url}/{id}` and report which path each id stands for, for a
    /// host that serves the bytes at that URL itself.
    ///
    /// An id rather than the path itself, because a URL carrying an arbitrary
    /// path would have to survive percent-encoding and the shape of a Windows
    /// path. It is not a secret — the same file always hashes the same way —
    /// so what a host may serve is decided by the ids a render actually
    /// handed it, never by the id being hard to guess.
    Deferred { base_url: String },
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
    /// How local images reach the document.
    ///
    /// Deliberately not part of `config.json`: which of the two a consumer
    /// needs follows from what that consumer is, not from anything the reader
    /// would choose. Keeping it here rather than in a second parameter keeps
    /// the pipeline's entry points to one options argument.
    #[serde(skip)]
    pub images: ImageResolution,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            auto_link_urls: true,
            images: ImageResolution::default(),
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
            ..Default::default()
        })
        .unwrap();
        assert_eq!(json, r#"{"autoLinkUrls":false}"#);
    }

    /// The image strategy is the host's business, so `config.json` neither
    /// carries it nor can set it.
    #[test]
    fn test_image_resolution_stays_out_of_the_configuration_file() {
        let json = serde_json::to_string(&RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        })
        .unwrap();
        assert!(!json.contains("images"), "leaked into config.json: {json}");

        let parsed: RenderOptions =
            serde_json::from_str(r#"{"images":{"deferred":{"baseUrl":"x"}}}"#).unwrap();
        assert_eq!(parsed.images, ImageResolution::DataUrl);
    }
}
