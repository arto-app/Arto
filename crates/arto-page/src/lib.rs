//! Self-contained HTML pages of Arto-rendered Markdown.
//!
//! The desktop app renders Markdown into a fragment and lets the WebView load
//! the stylesheet and the frontend bundle as assets. Everything else that wants
//! Arto's rendering outside the app (the `arto-page` CLI, `arto page`, and the
//! macOS Quick Look preview) needs a single document that carries the styles
//! and the bundle inline, because it has no asset server to fall back on. This
//! crate builds that document.
//!
//! The body comes from Arto's real Markdown pipeline (`arto-markdown`), which
//! emits placeholder markup for Mermaid diagrams and math. The embedded
//! frontend bundle (`window.ArtoRenderer`) turns those placeholders into
//! rendered diagrams and formulas once `init()` runs inside the page.
//!
//! What a page embeds is decided by the body it is written for: a document
//! with no diagram carries no Mermaid, one with no formula carries neither
//! KaTeX nor its fonts, and one whose code blocks name only the usual
//! languages carries a seventh of highlight.js. The page pays for every byte
//! again on every preview, and most documents are prose.
//!
//! # Security
//!
//! Quick Look previews are generated passively (pressing Space in Finder) for
//! files the user has not chosen to trust, and the page runs JavaScript so the
//! bundle can draw diagrams and math. To stop untrusted Markdown from
//! injecting executable script (raw `<script>` tags, `on*` handlers,
//! `javascript:` URLs), the page carries a strict `Content-Security-Policy`
//! whose `script-src` allowlists only the SHA-256 hashes of the first-party
//! inline scripts embedded here. [`PageOptions`] can switch the policy off
//! for callers that render trusted input.
//!
//! The library bundles and the choice among them come from `arto-frontend`,
//! which also serves an application that places `arto-markdown`'s output in a
//! page of its own rather than writing one.

pub use arto_config::{
    CjkFontLanguage, ColorTheme, Config, ConfigError, FontFamilyChoice, Theme, ThemeConfig,
    TypographyConfig,
};
pub use arto_markdown::RenderOptions;

use document::build_document;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(feature = "cli")]
pub mod cli;
mod css;
mod document;
#[cfg(feature = "ffi")]
pub mod ffi;

/// Maximum size of a Markdown file this crate will read. Guards against
/// unbounded reads from a `.md`-named file that is really a symlink to an
/// endless source such as `/dev/zero`.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// How the page is rendered.
#[derive(Debug, Clone)]
pub struct PageOptions {
    /// Markdown rendering choices, shared with every other consumer of
    /// arto-markdown (the app reads the same struct from `config.json`).
    pub render: RenderOptions,
    /// The colour theme the page opens in. `Auto` follows the viewer's
    /// `prefers-color-scheme`.
    pub theme: Theme,
    /// Which of GitHub's themes light mode paints.
    pub light_theme: ColorTheme,
    /// Which of GitHub's themes dark mode paints.
    pub dark_theme: ColorTheme,
    /// How the text is set: line length, line height, face and size.
    pub typography: TypographyConfig,
    /// Emit the `Content-Security-Policy` that restricts script execution to
    /// the embedded frontend. Leave it on for untrusted input.
    pub content_security_policy: bool,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            render: RenderOptions::default(),
            theme: Theme::default(),
            light_theme: ThemeConfig::default().light_theme,
            dark_theme: ThemeConfig::default().dark_theme,
            typography: TypographyConfig::default(),
            content_security_policy: true,
        }
    }
}

impl PageOptions {
    /// The options the user's configuration asks for: the app's rendering
    /// options, its default theme and its typography. The policy stays on; a
    /// configuration file must not be able to switch off the protection for
    /// untrusted input.
    pub fn from_config(config: &Config) -> Self {
        Self {
            render: config.markdown.clone(),
            theme: config.theme.default_theme,
            light_theme: config.theme.light_theme,
            dark_theme: config.theme.dark_theme,
            typography: config.typography.clone(),
            content_security_policy: true,
        }
    }
}

/// Why a page could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum PageError {
    /// The Markdown file could not be read (missing, not a regular file, or
    /// larger than [`MAX_FILE_BYTES`]).
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The Markdown pipeline failed.
    #[error("failed to render Markdown")]
    Render(#[source] arto_markdown::RenderError),
    /// The page could not be written to the requested file.
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The page could not be written to standard output.
    #[error("cannot write to standard output: {0}")]
    WriteStdout(#[source] std::io::Error),
    /// The user's configuration could not be read or parsed.
    #[error("cannot load the configuration: {0}")]
    Config(#[from] ConfigError),
}

/// Render the Markdown file at `path` into a self-contained HTML page.
///
/// Relative links and images in the Markdown resolve against the file's
/// directory, as they do in the app.
pub fn render_file(path: impl AsRef<Path>, options: &PageOptions) -> Result<String, PageError> {
    let path = path.as_ref();
    let markdown = read_capped(path).map_err(|source| PageError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    render_markdown(markdown, path, options)
}

/// Render Markdown text into a self-contained HTML page.
///
/// `base_path` is the file the text came from (or a path in the directory it
/// should be interpreted in); relative links and images resolve against it.
pub fn render_markdown(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &PageOptions,
) -> Result<String, PageError> {
    // Imposed rather than assumed: the page is one file, with nothing to
    // serve an image from, so a caller that had set `Deferred` in the shared
    // render options would otherwise get a document of URLs that answer to
    // nobody.
    let render = arto_markdown::RenderOptions {
        images: arto_markdown::ImageResolution::DataUrl,
        ..options.render.clone()
    };
    let rendered =
        arto_markdown::render_to_html(markdown, base_path, &render).map_err(PageError::Render)?;
    // The policy loads media only from `data:` URLs, so what would come from
    // elsewhere is swapped for its stand-in before the page is built: an
    // image for its alt text, a video for the link it holds. Left in, the
    // page would show an empty frame where the document had something to say.
    let html = if options.content_security_policy {
        arto_markdown::without_remote_media(&rendered.html)
    } else {
        rendered.html
    };
    Ok(build_document(&html, options))
}

/// Read a file to a string, refusing to read more than [`MAX_FILE_BYTES`].
///
/// Validates the file via `metadata` *before* opening it: a non-regular file
/// (a FIFO or device) is rejected, because opening a FIFO would block the
/// caller indefinitely. `metadata` follows symlinks and, unlike `open`, does
/// not block on a FIFO. The bounded `take` read is kept as a backstop in case
/// the file grows between the check and the read.
fn read_capped(path: &Path) -> std::io::Result<String> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }

    let file = fs::File::open(path)?;
    let mut content = String::new();
    file.take(MAX_FILE_BYTES + 1).read_to_string(&mut content)?;
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_leaves_a_link_where_the_policy_blocks_a_video() {
        let url = "https://github.com/user-attachments/assets/2f1c9e0a";
        let markdown = format!("{url}\n");
        let base = Path::new("/nonexistent/doc.md");

        let html = render_markdown(&markdown, base, &PageOptions::default()).unwrap();
        assert!(!html.contains("<video"), "{html}");
        assert!(html.contains(&format!(r#"<a href="{url}">"#)), "{html}");

        let options = PageOptions {
            content_security_policy: false,
            ..PageOptions::default()
        };
        let html = render_markdown(&markdown, base, &options).unwrap();
        assert!(html.contains(&format!(r#"<video src="{url}""#)), "{html}");
    }

    #[test]
    fn test_options_from_config_take_render_options_theme_and_typography_but_keep_csp() {
        let config = Config {
            markdown: RenderOptions {
                auto_link_urls: false,
                ..Default::default()
            },
            theme: arto_config::ThemeConfig {
                default_theme: Theme::Dark,
                ..Default::default()
            },
            typography: TypographyConfig {
                measure: 45.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let options = PageOptions::from_config(&config);
        assert!(!options.render.auto_link_urls);
        assert_eq!(options.theme, Theme::Dark);
        assert!(options.content_security_policy);
        assert_eq!(options.typography.measure, 45.0);
    }

    #[test]
    fn test_every_sample_renders_to_a_well_formed_page() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
        let mut rendered = 0;
        for entry in fs::read_dir(&samples).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let html = render_file(&path, &PageOptions::default())
                .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            assert!(
                html.contains(r#"<article class="markdown-body">"#),
                "{} lacks the article wrapper",
                path.display()
            );
            // The body may legitimately mention `</script>` (a sample about
            // raw HTML does), so only the tail is checked: both first-party
            // scripts must still close the document.
            assert!(
                html.ends_with("</script>\n</body></html>"),
                "{} does not end with the bootstrap script",
                path.display()
            );
            rendered += 1;
        }
        assert!(rendered > 0, "no samples found under {}", samples.display());
    }

    #[test]
    fn test_render_file_produces_page_for_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# Title\n\nSee https://example.com\n").unwrap();

        let html = render_file(&path, &PageOptions::default()).unwrap();
        assert!(html.contains("<h1"));
        assert!(html.contains("Title"));
        assert!(html.contains(r#"href="https://example.com""#));

        let plain = render_file(
            &path,
            &PageOptions {
                render: RenderOptions {
                    auto_link_urls: false,
                    ..Default::default()
                },
                ..PageOptions::default()
            },
        )
        .unwrap();
        assert!(!plain.contains(r#"href="https://example.com""#));
    }

    #[test]
    fn test_render_file_reports_unreadable_input() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.md");

        let err = render_file(&missing, &PageOptions::default()).unwrap_err();
        assert!(matches!(err, PageError::Read { .. }));
        assert!(err.to_string().contains("missing.md"));
    }

    #[test]
    fn test_read_capped_accepts_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.md");
        std::fs::write(&path, "# ok\n").unwrap();
        assert!(read_capped(&path).is_ok());
    }

    #[test]
    fn test_read_capped_rejects_non_regular_file() {
        // A directory is not a regular file and must be rejected before open
        // (the same guard rejects FIFOs/devices without blocking).
        let dir = tempfile::tempdir().unwrap();
        assert!(read_capped(dir.path()).is_err());
    }
}
