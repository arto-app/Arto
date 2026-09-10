use serde::{Deserialize, Serialize};

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

/// What becomes of raw HTML written into the Markdown.
///
/// A document may embed markup the Markdown syntax cannot say — a `<kbd>`, a
/// `<details>`, an `<img>` with a width — and passing it through is what
/// makes that work. The same passthrough lets a `<style>` or a `<script>`
/// restyle the page around the document, which is why the middle choice is
/// the default rather than the permissive one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawHtml {
    /// Write every raw HTML node through untouched.
    Allow,
    /// GFM's tagfilter: neutralize the tags that restyle or script the page
    /// (`<style>`, `<script>`, `<iframe>` and friends) by escaping their
    /// leading `<`, and leave all other markup working.
    #[default]
    Filter,
    /// Escape every raw HTML node, so the document shows its own markup.
    Escape,
}

/// Choices that change how Markdown is rendered.
///
/// This is also the `markdown` section of Arto's `config.json`, so every
/// consumer of the pipeline (the app, `arto page`, Quick Look) exposes the
/// same options and reads the same user preferences. The struct carries
/// `#[serde(default)]`, so a configuration file naming one option gets the
/// documented default for every other.
///
/// The GFM baseline — tables, task lists, strikethrough, footnotes — is not
/// here. Those constructs are what a Markdown document written for GitHub
/// contains, and a reader that renders them as literal pipes and brackets is
/// broken rather than configured. What is here is the layer above that
/// baseline, where a construct's syntax collides with prose somebody actually
/// writes: `$5 and $10` is not a formula, and `[[a]]` is not always a link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RenderOptions {
    /// Turn bare URLs into links.
    pub auto_link_urls: bool,
    /// Read `$…$` and `$$…$$` as formulas for the math renderer.
    ///
    /// Off leaves a document that writes `$` for something else alone: two
    /// shell variables in one line (`echo $HOME/$USER`) otherwise pair up
    /// into a formula spanning both.
    pub math: bool,
    /// Read `[[target]]` and `[[target|label]]` as links to another document.
    pub wiki_links: bool,
    /// Read `^text^` as superscript.
    pub superscript: bool,
    /// Read `~text~` as subscript.
    ///
    /// Off keeps a lone `~` literal, which is what a document using tildes
    /// for approximation (`~5 minutes`) means by it.
    pub subscript: bool,
    /// Read PHP Markdown Extra / mdBook definition lists.
    pub definition_lists: bool,
    /// Read a trailing `{#id .class}` on a heading as the id and classes to
    /// render it with, rather than as part of its text.
    pub heading_attributes: bool,
    /// Replace quotes, dashes and ellipses with their typographic forms.
    pub smart_punctuation: bool,
    /// Let emphasis pair when its delimiters sit against East Asian
    /// punctuation, so `**強調。**` is emphasized.
    ///
    /// CommonMark's flanking rules read the punctuation on either side of a
    /// `*` run to decide whether it may open or close, and East Asian
    /// punctuation blocks the run the way Latin punctuation does — except
    /// that CJK text sets punctuation directly against the words, where Latin
    /// text puts a space in between. Off is what GitHub renders.
    pub cjk_emphasis: bool,
    /// Append a `#` link to each heading, the way GitHub does.
    pub heading_permalinks: bool,
    /// What becomes of raw HTML written into the Markdown.
    pub raw_html: RawHtml,
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
            math: true,
            wiki_links: true,
            superscript: true,
            subscript: true,
            definition_lists: true,
            heading_attributes: true,
            smart_punctuation: true,
            cjk_emphasis: true,
            // Off, like GitHub's own rendering: Arto's headings are reached
            // from the contents gutter rather than by copying a link out of
            // the page.
            heading_permalinks: false,
            raw_html: RawHtml::default(),
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

    /// Naming one option must not turn the others off, or every release that
    /// adds one would silently change how an existing document renders.
    #[test]
    fn test_naming_one_option_leaves_the_rest_at_their_defaults() {
        let parsed: RenderOptions = serde_json::from_str(r#"{"math":false}"#).unwrap();
        assert!(!parsed.math);
        assert!(parsed.auto_link_urls);
        assert!(parsed.cjk_emphasis);
        assert_eq!(parsed.raw_html, RawHtml::Filter);
    }

    #[test]
    fn test_serializes_with_camel_case_keys() {
        let json = serde_json::to_string(&RenderOptions {
            auto_link_urls: false,
            wiki_links: false,
            ..Default::default()
        })
        .unwrap();
        assert!(json.contains(r#""autoLinkUrls":false"#), "{json}");
        assert!(json.contains(r#""wikiLinks":false"#), "{json}");
        assert!(json.contains(r#""rawHtml":"filter""#), "{json}");
    }

    #[test]
    fn test_raw_html_names_are_snake_case() {
        for (value, name) in [
            (RawHtml::Allow, r#""allow""#),
            (RawHtml::Filter, r#""filter""#),
            (RawHtml::Escape, r#""escape""#),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), name);
            assert_eq!(serde_json::from_str::<RawHtml>(name).unwrap(), value);
        }
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
