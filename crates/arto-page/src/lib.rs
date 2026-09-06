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

pub use arto_markdown::RenderOptions;

use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

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

/// The inline bootstrap script. Picks the initial theme from
/// `prefers-color-scheme` (the frontend reads `document.body`'s `data-theme`
/// during initialization) and then starts the frontend.
const BOOTSTRAP_JS: &str = r#"(function(){
  // Quick Look loads this page from an opaque origin, which is not a secure
  // context, so crypto.randomUUID (used by Mermaid) is undefined. Polyfill it
  // with a non-cryptographic UUID — Mermaid only needs unique element ids.
  try {
    if (window.crypto && typeof window.crypto.randomUUID !== 'function') {
      window.crypto.randomUUID = function () {
        return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function (c) {
          var r = (Math.random() * 16) | 0;
          return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
        });
      };
    }
  } catch (e) {}
  try {
    var m = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.body.setAttribute('data-theme', m ? 'dark' : 'light');
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
    /// Emit the `Content-Security-Policy` that restricts script execution to
    /// the embedded frontend. Leave it on for untrusted input.
    pub content_security_policy: bool,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            render: RenderOptions::default(),
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
    let body_html = arto_markdown::render_to_html(markdown, base_path, &options.render)
        .map_err(PageError::Render)?;
    Ok(build_document(&body_html, options))
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
    // is blocked. `style-src 'unsafe-inline'` is required because the frontend
    // injects theme <style> elements and inline styles at runtime.
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

    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8">
{csp_meta}<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{css}</style>
<style>{standalone_override}</style></head>
<body data-theme="light">
<div class="markdown-viewer"><article class="markdown-body">{body}</article></div>
<script>{bundle}</script>
<script>{bootstrap}</script>
</body></html>"#,
        csp_meta = csp_meta,
        css = FRONTEND_CSS,
        standalone_override = STANDALONE_OVERRIDE_CSS,
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
        // Theme defaults are present.
        assert!(html.contains(r#"<body data-theme="light">"#));
        assert!(html.contains("prefers-color-scheme"));
        // Mermaid needs crypto.randomUUID, which an opaque origin lacks, so
        // the bootstrap must polyfill it.
        assert!(html.contains("crypto.randomUUID"));
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
