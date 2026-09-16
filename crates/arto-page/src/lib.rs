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
//! # Security
//!
//! Quick Look previews are generated passively (pressing Space in Finder) for
//! files the user has not chosen to trust, and the page runs JavaScript so the
//! bundle can draw diagrams and math. To stop untrusted Markdown from
//! injecting executable script (raw `<script>` tags, `on*` handlers,
//! `javascript:` URLs), the page carries a strict `Content-Security-Policy`
//! whose `script-src` allowlists only the SHA-256 hashes of the two
//! first-party inline scripts embedded here. [`PageOptions`] can switch the
//! policy off for callers that render trusted input.

pub use arto_config::{ColorTheme, Config, ConfigError, Theme, ThemeConfig};
pub use arto_markdown::RenderOptions;

use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "ffi")]
pub mod ffi;

/// The frontend stylesheet, embedded at compile time.
const FRONTEND_CSS: &str = include_str!("../assets/frontend/main.css");

/// The frontend bundle (IIFE build exposing `window.ArtoRenderer`), embedded
/// at compile time.
const FRONTEND_JS: &str = include_str!("../assets/frontend/main.iife.js");

/// Style overrides for a standalone page, applied after [`FRONTEND_CSS`].
///
/// The shared stylesheet sets `body { overflow: hidden }` because the app
/// scrolls inside an inner `.content` container. A standalone document has no
/// such container, so inheriting that rule clips long documents and leaves the
/// page unscrollable. Restore natural document scrolling.
const STANDALONE_OVERRIDE_CSS: &str = "html,body{overflow:auto!important;height:auto!important;}";

/// [`FRONTEND_CSS`] without the KaTeX font faces.
///
/// Those faces carry their woff2 inline and are most of the stylesheet by
/// weight, yet nothing but typeset math ever has a glyph to set in them.
/// Computed once per process: Quick Look builds a page on every press of
/// Space, and the scan runs over a megabyte-scale string.
static CSS_WITHOUT_MATH_FONTS: LazyLock<String> = LazyLock::new(|| {
    let mut css = strip_font_faces(FRONTEND_CSS, "KaTeX_");
    // The buffer was sized for a stylesheet with nothing to strip, and what is
    // left is a fraction of that. It is held for the life of the process.
    css.shrink_to_fit();
    css
});

/// Remove every `@font-face` block that names a font family starting with
/// `prefix`.
fn strip_font_faces(css: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(at) = rest.find("@font-face") {
        let Some(open) = rest[at..].find('{').map(|i| at + i) else {
            break;
        };
        let Some(end) = block_end(rest, open) else {
            break;
        };
        out.push_str(if rest[open..end].contains(prefix) {
            &rest[..at]
        } else {
            &rest[..end]
        });
        rest = &rest[end..];
    }

    out.push_str(rest);
    out
}

/// Remove the `[data-theme=…]` rules for every theme but `light` and `dark`.
///
/// The stylesheet carries all of GitHub's themes because the app lets the
/// reader pick any of them at any time. A page is written for one pair: the
/// bootstrap writes one of these two names onto the root element and nothing
/// afterwards can name a third.
fn strip_unused_themes(css: &str, light: &str, dark: &str) -> String {
    const MARKER: &str = "[data-theme=";

    let mut out = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(at) = rest.find(MARKER) {
        let name_start = at + MARKER.len();
        let Some(name_end) = rest[name_start..].find(']').map(|i| name_start + i) else {
            break;
        };
        let Some(open) = rest[name_end..].find('{').map(|i| name_end + i) else {
            break;
        };
        let Some(end) = block_end(rest, open) else {
            break;
        };
        let name = &rest[name_start..name_end];
        if name == light || name == dark {
            out.push_str(&rest[..end]);
        } else {
            // A theme can share its rule with another selector
            // (`:root:not([data-theme]),[data-theme=light]{…}`). Cutting at the
            // selector itself would leave the rest of the list behind, to bind
            // to whatever rule follows — so cut where the rule starts.
            let rule_start = rest[..at].rfind(['}', '{', ';']).map_or(0, |i| i + 1);
            out.push_str(&rest[..rule_start]);
        }
        rest = &rest[end..];
    }

    out.push_str(rest);
    out
}

/// The index just past the `}` that closes the block opening at `open`.
fn block_end(css: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, c) in css[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The inline bootstrap script. Settles the theme (the frontend reads
/// `data-theme` on the root element during initialization) and then starts the
/// frontend.
///
/// The theme choices travel in `data-theme-preference`, `data-light-theme`
/// and `data-dark-theme` on `<html>` rather than in this script, so the script
/// stays byte-identical across pages and its CSP hash can be a constant. The
/// preference `light` and `dark` are taken as-is; anything else follows
/// `prefers-color-scheme`. The mode then picks one of the two theme names.
const BOOTSTRAP_JS: &str = r#"(function(){
  try {
    var root = document.documentElement;
    var preference = root.getAttribute('data-theme-preference');
    var dark = preference === 'dark';
    if (preference !== 'light' && preference !== 'dark') {
      dark = !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
    }
    var theme = root.getAttribute(dark ? 'data-dark-theme' : 'data-light-theme');
    root.setAttribute('data-theme', theme || (dark ? 'dark' : 'light'));
  } catch (e) {}
  if (window.ArtoRenderer && typeof window.ArtoRenderer.init === 'function') { window.ArtoRenderer.init(); }
})();"#;

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
            content_security_policy: true,
        }
    }
}

impl PageOptions {
    /// The options the user's configuration asks for: the app's rendering
    /// options and its default theme. The policy stays on; a configuration
    /// file must not be able to switch off the protection for untrusted input.
    pub fn from_config(config: &Config) -> Self {
        Self {
            render: config.markdown.clone(),
            theme: config.theme.default_theme,
            light_theme: config.theme.light_theme,
            dark_theme: config.theme.dark_theme,
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
    Render(#[source] anyhow::Error),
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
    Ok(build_document(&rendered.html, options))
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

/// Base64-encoded SHA-256 digest, as required by a CSP `'sha256-...'` source.
fn sha256_base64(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest)
}

/// Assemble a full, self-contained HTML document from a rendered Markdown body.
///
/// The embedded stylesheet and frontend bundle are inlined so the page needs no
/// network or filesystem access. With [`PageOptions::content_security_policy`]
/// on, a `Content-Security-Policy` restricts script execution to the two
/// first-party inline scripts (by SHA-256 hash), so raw HTML in an untrusted
/// Markdown body cannot run JavaScript.
fn build_document(body_html: &str, options: &PageOptions) -> String {
    // Guard against premature `<script>` termination: if the minified bundle
    // ever contains the literal `</script` (only possible inside a JS string
    // or regex, where `<\/script` is equivalent), the HTML parser would close
    // our inline script early and `ArtoRenderer` would never be defined.
    let bundle = FRONTEND_JS.replace("</script", r"<\/script");

    // Allowlist exactly the two inline scripts we emit; everything else the
    // Markdown body may contain (script tags, event handlers, javascript: URLs)
    // is blocked. `style-src 'unsafe-inline'` is required because the page
    // carries its stylesheet inline and the frontend sets inline styles at
    // runtime.
    let csp_meta = if options.content_security_policy {
        format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
             script-src 'sha256-{bundle_hash}' 'sha256-{bootstrap_hash}'; \
             style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; base-uri 'none'\">\n",
            bundle_hash = sha256_base64(&bundle),
            bootstrap_hash = sha256_base64(BOOTSTRAP_JS),
        )
    } else {
        String::new()
    };

    // `data-theme` is what the stylesheet reads, so a fixed theme applies
    // before any script runs (and with scripts blocked). `Auto` starts light
    // and lets the bootstrap consult `prefers-color-scheme`.
    let light_theme = options.light_theme.as_str();
    let dark_theme = options.dark_theme.as_str();
    let (initial_theme, theme_preference) = match options.theme {
        Theme::Auto => (light_theme, "auto"),
        Theme::Light => (light_theme, "light"),
        Theme::Dark => (dark_theme, "dark"),
    };

    // Both passes drop what this page can never paint with. Math is typeset in
    // the page rather than in the HTML handed to it, so a body with no
    // `preprocessed-math*` container never reaches for a KaTeX glyph; and the
    // stylesheet's other themes have no name left that could select them.
    let css = {
        let with_math_settled = if body_html.contains("preprocessed-math") {
            FRONTEND_CSS
        } else {
            &CSS_WITHOUT_MATH_FONTS
        };
        strip_unused_themes(with_math_settled, light_theme, dark_theme)
    };

    format!(
        r#"<!DOCTYPE html><html data-theme="{initial_theme}" data-theme-preference="{theme_preference}" data-light-theme="{light_theme}" data-dark-theme="{dark_theme}"><head><meta charset="utf-8">
{csp_meta}<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{css}</style>
<style>{standalone_override}</style></head>
<body>
<div class="markdown-viewer"><article class="markdown-body">{body}</article></div>
<script>{bundle}</script>
<script>{bootstrap}</script>
</body></html>"#,
        csp_meta = csp_meta,
        css = css,
        standalone_override = STANDALONE_OVERRIDE_CSS,
        initial_theme = initial_theme,
        theme_preference = theme_preference,
        light_theme = light_theme,
        dark_theme = dark_theme,
        body = body_html,
        bundle = bundle,
        bootstrap = BOOTSTRAP_JS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_document_wraps_body_and_embeds_assets() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // Body is wrapped in the markdown-viewer container (which supplies the
        // app's padding, background, and centered max-width) and article.
        assert!(html.contains(
            r#"<div class="markdown-viewer"><article class="markdown-body"><p>hi</p></article></div>"#
        ));
        // Frontend is bootstrapped.
        assert!(html.contains("ArtoRenderer.init"));
        // Styles and scripts are inlined.
        assert!(html.contains("<style>"));
        assert!(html.contains("</style>"));
        // A standalone document has no inner `.content` scroll container, so
        // the shared `body { overflow: hidden }` must be overridden or the
        // page cannot scroll long documents.
        assert!(html.contains(STANDALONE_OVERRIDE_CSS));
        assert!(html.contains("overflow:auto"));
        assert!(html.contains("<script>"));
        assert!(html.contains("</script>"));
        // Theme defaults are present: start light, let the bootstrap follow
        // the system.
        assert!(html.contains(r#"<html data-theme="light" data-theme-preference="auto" data-light-theme="light" data-dark-theme="dark">"#));
        assert!(html.contains("prefers-color-scheme"));
    }

    #[test]
    fn test_strip_font_faces_removes_only_the_matching_faces() {
        let css = "a{color:red}@font-face{font-family:KaTeX_Main;src:url(x)}b{color:blue}@font-face{font-family:Other;src:url(y)}c{color:green}";

        assert_eq!(
            strip_font_faces(css, "KaTeX_"),
            "a{color:red}b{color:blue}@font-face{font-family:Other;src:url(y)}c{color:green}"
        );
    }

    #[test]
    fn test_strip_font_faces_leaves_a_stylesheet_without_a_match_alone() {
        let css = "a{color:red}@font-face{font-family:Other;src:url(y)}";

        assert_eq!(strip_font_faces(css, "KaTeX_"), css);
    }

    #[test]
    fn test_build_document_embeds_the_math_fonts_only_for_a_document_with_math() {
        let plain = build_document("<p>hi</p>", &PageOptions::default());
        let math = build_document(
            r#"<span class="preprocessed-math-inline" data-original-content="x">x</span>"#,
            &PageOptions::default(),
        );

        // The KaTeX faces carry their woff2 inline, so leaving them out of a
        // document that sets no math saves megabytes.
        assert!(math.len() > plain.len() + 1_000_000);
        assert!(math.contains("@font-face"));
        // Both are still whole pages.
        assert!(plain.contains("ArtoRenderer.init"));
        assert!(math.contains("ArtoRenderer.init"));
    }

    #[test]
    fn test_strip_unused_themes_keeps_only_the_named_pair() {
        let css = "a{color:red}[data-theme=light]{--x:1}[data-theme=dark_dimmed]{--x:2}[data-theme=dark]{--x:3}b{color:blue}";

        assert_eq!(
            strip_unused_themes(css, "light", "dark"),
            "a{color:red}[data-theme=light]{--x:1}[data-theme=dark]{--x:3}b{color:blue}"
        );
    }

    #[test]
    fn test_strip_unused_themes_drops_the_whole_selector_list_of_a_dropped_theme() {
        let css = "a{color:red}:root:not([data-theme]),[data-theme=light]{--x:1}@media print{b{color:blue}}";

        assert_eq!(
            strip_unused_themes(css, "light_high_contrast", "dark"),
            "a{color:red}@media print{b{color:blue}}"
        );
    }

    #[test]
    fn test_strip_unused_themes_allows_the_same_theme_on_both_sides() {
        let css = "[data-theme=light]{--x:1}[data-theme=dark]{--x:2}";

        assert_eq!(
            strip_unused_themes(css, "light", "light"),
            "[data-theme=light]{--x:1}"
        );
    }

    #[test]
    fn test_build_document_carries_only_the_themes_the_page_can_paint() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        assert!(html.contains("[data-theme=light]{"));
        assert!(html.contains("[data-theme=dark]{"));
        assert!(!html.contains("[data-theme=dark_dimmed]{"));
        assert!(!html.contains("[data-theme=light_high_contrast]{"));
    }

    #[test]
    fn test_build_document_keeps_the_themes_the_options_name() {
        let html = build_document(
            "<p>hi</p>",
            &PageOptions {
                light_theme: ColorTheme::LightHighContrast,
                dark_theme: ColorTheme::DarkDimmed,
                ..PageOptions::default()
            },
        );

        assert!(html.contains("[data-theme=light_high_contrast]{"));
        assert!(html.contains("[data-theme=dark_dimmed]{"));
        assert!(!html.contains("[data-theme=light]{"));
        assert!(!html.contains("[data-theme=dark]{"));
    }

    #[test]
    fn test_build_document_has_no_premature_script_termination() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // The only `</script` occurrences must be our own closing tags, i.e.
        // the embedded bundle must not smuggle in a `</script` that would
        // close the inline script early.
        assert_eq!(html.matches("</script>").count(), 2);
        assert_eq!(html.matches("</script").count(), 2);
    }

    #[test]
    fn test_build_document_has_csp_allowlisting_only_inline_scripts() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // The page carries a CSP that allowlists scripts by hash (no
        // 'unsafe-inline'), so injected <script>/event handlers cannot run.
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(html.contains("script-src 'sha256-"));
        assert!(!html.contains("'unsafe-inline'; script"));
        // The hashes must match the exact inline script contents we emit.
        let bundle = FRONTEND_JS.replace("</script", r"<\/script");
        assert!(html.contains(&format!("'sha256-{}'", sha256_base64(&bundle))));
        assert!(html.contains(&format!("'sha256-{}'", sha256_base64(BOOTSTRAP_JS))));
    }

    #[test]
    fn test_build_document_can_omit_csp() {
        let options = PageOptions {
            content_security_policy: false,
            ..PageOptions::default()
        };
        let html = build_document("<p>hi</p>", &options);

        assert!(!html.contains("Content-Security-Policy"));
        // The rest of the document is unchanged.
        assert!(html.contains("ArtoRenderer.init"));
        assert!(html.contains(r#"<meta name="viewport""#));
    }

    #[test]
    fn test_build_document_applies_a_fixed_theme_before_scripts_run() {
        let dark = build_document(
            "<p>hi</p>",
            &PageOptions {
                theme: Theme::Dark,
                ..PageOptions::default()
            },
        );
        assert!(dark.contains(r#"<html data-theme="dark" data-theme-preference="dark" data-light-theme="light" data-dark-theme="dark">"#));

        let light = build_document(
            "<p>hi</p>",
            &PageOptions {
                theme: Theme::Light,
                ..PageOptions::default()
            },
        );
        assert!(light.contains(r#"<html data-theme="light" data-theme-preference="light" data-light-theme="light" data-dark-theme="dark">"#));

        // The bootstrap is byte-identical whatever the theme, so its CSP hash
        // stays valid.
        let hash = format!("'sha256-{}'", sha256_base64(BOOTSTRAP_JS));
        assert!(dark.contains(&hash));
        assert!(light.contains(&hash));
    }

    #[test]
    fn test_options_from_config_take_render_options_and_theme_but_keep_csp() {
        let config = Config {
            markdown: RenderOptions {
                auto_link_urls: false,
                ..Default::default()
            },
            theme: arto_config::ThemeConfig {
                default_theme: Theme::Dark,
                ..Default::default()
            },
            ..Default::default()
        };

        let options = PageOptions::from_config(&config);
        assert!(!options.render.auto_link_urls);
        assert_eq!(options.theme, Theme::Dark);
        assert!(options.content_security_policy);
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
