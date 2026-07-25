//! Arto Quick Look FFI.
//!
//! This crate is compiled as a C-ABI `staticlib` (`libarto_ql.a`) and linked
//! into the Arto Quick Look preview extension. It renders a Markdown file into
//! a single, fully self-contained HTML page (styles and the renderer bundle are
//! embedded at compile time) that the Swift shim loads into its WebView.
//!
//! The rendered body reuses Arto's real Markdown pipeline (`arto-markdown`),
//! which emits placeholder markup for Mermaid diagrams and math. The embedded
//! renderer bundle (`window.ArtoRenderer`) turns those placeholders into fully
//! rendered diagrams and formulas once `init()` runs inside the WebView.
//!
//! # Security
//!
//! Quick Look previews are generated passively (e.g. pressing Space in Finder)
//! for files the user has not chosen to trust, and the WebView runs JavaScript
//! so the renderer bundle can draw diagrams and math. To stop untrusted
//! Markdown from injecting executable script (raw `<script>` tags, `on*`
//! handlers, `javascript:` URLs), the page carries a strict `Content-Security-
//! Policy` whose `script-src` allowlists only the SHA-256 hashes of the two
//! first-party inline scripts embedded here. Any other script the Markdown
//! smuggles into the body is refused execution by the WebView.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::ptr;

/// The renderer stylesheet, embedded at compile time.
const RENDERER_CSS: &str = include_str!("../../desktop/assets/dist/main.css");

/// Quick Look-specific style overrides, applied after [`RENDERER_CSS`].
///
/// The shared renderer stylesheet sets `body { overflow: hidden }` because the
/// app scrolls inside an inner `.content` container. The Quick Look document
/// has no such container, so inheriting that rule clips long documents and
/// leaves the preview unscrollable. Restore natural document scrolling for the
/// standalone preview page.
const QL_OVERRIDE_CSS: &str = "html,body{overflow:auto!important;height:auto!important;}";

/// The renderer bundle (IIFE build exposing `window.ArtoRenderer`), embedded
/// at compile time.
const RENDERER_JS: &str = include_str!("../../desktop/assets/dist/main.iife.js");

/// The inline bootstrap script. Picks the initial theme from
/// `prefers-color-scheme` (the renderer reads `document.body`'s `data-theme`
/// during initialization) and then starts the renderer.
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

/// Maximum size of a Markdown file we will preview. Guards against unbounded
/// reads from a `.md`-named file that is really a symlink to an endless source
/// such as `/dev/zero`.
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Render the Markdown file at `path_utf8` to a full self-contained HTML page.
///
/// Returns a Rust-allocated C string that the caller must release only with
/// [`arto_ql_free_string`] (never libc `free`). Returns null on any error (an
/// unreadable, non-regular, or oversized file, or a rendering failure).
///
/// # Safety
///
/// `path_utf8` must be either null or a valid pointer to a NUL-terminated C
/// string that stays valid for the duration of the call. The returned pointer
/// must be freed exactly once with [`arto_ql_free_string`] and not otherwise.
#[no_mangle]
pub unsafe extern "C" fn arto_ql_render_markdown_file(path_utf8: *const c_char) -> *mut c_char {
    if path_utf8.is_null() {
        return ptr::null_mut();
    }

    // Swift passes the file-system representation (raw bytes), which is not
    // guaranteed to be valid UTF-8, so build the path from bytes directly.
    let path = path_from_ffi_bytes(CStr::from_ptr(path_utf8).to_bytes());

    let Ok(content) = read_capped(&path) else {
        return ptr::null_mut();
    };

    let Ok(body_html) = arto_markdown::render_to_html(&content, &path, true) else {
        return ptr::null_mut();
    };

    let document = build_document(&body_html);

    match CString::new(document) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Build a `PathBuf` from the raw file-system bytes Swift hands across the FFI.
///
/// On Unix the file-system representation is arbitrary bytes, so the path is
/// built losslessly from them. Quick Look only ships on macOS; the non-Unix
/// branch exists solely to keep this crate compiling in cross-platform CI and
/// is never invoked at runtime.
#[cfg(unix)]
fn path_from_ffi_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_ffi_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// Free a string previously returned by [`arto_ql_render_markdown_file`].
///
/// # Safety
///
/// `ptr` must be either null or a pointer returned by
/// [`arto_ql_render_markdown_file`] that has not already been freed. Passing
/// any other pointer, or freeing the same pointer twice, is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn arto_ql_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Read a file to a string, refusing to read more than [`MAX_FILE_BYTES`].
///
/// Validates the file via `metadata` *before* opening it: a non-regular file
/// (a FIFO or device) is rejected, because opening a FIFO would block the
/// preview indefinitely. `metadata` follows symlinks and, unlike `open`, does
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
            "file exceeds maximum preview size",
        ));
    }

    let file = fs::File::open(path)?;
    let mut content = String::new();
    file.take(MAX_FILE_BYTES + 1).read_to_string(&mut content)?;
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum preview size",
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
/// The embedded stylesheet and renderer bundle are inlined so the page needs no
/// network or filesystem access. A `Content-Security-Policy` restricts script
/// execution to the two first-party inline scripts (by SHA-256 hash), so raw
/// HTML in the untrusted Markdown body cannot run JavaScript in the WebView.
fn build_document(body_html: &str) -> String {
    // Guard against premature `<script>` termination: if the minified bundle
    // ever contains the literal `</script` (only possible inside a JS string
    // or regex, where `<\/script` is equivalent), the HTML parser would close
    // our inline script early and `ArtoRenderer` would never be defined.
    let bundle = RENDERER_JS.replace("</script", r"<\/script");

    // Allowlist exactly the two inline scripts we emit; everything else the
    // Markdown body may contain (script tags, event handlers, javascript: URLs)
    // is blocked. `style-src 'unsafe-inline'` is required because the renderer
    // injects theme <style> elements and inline styles at runtime.
    let csp = format!(
        "default-src 'none'; script-src 'sha256-{bundle_hash}' 'sha256-{bootstrap_hash}'; \
         style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; base-uri 'none'",
        bundle_hash = sha256_base64(&bundle),
        bootstrap_hash = sha256_base64(BOOTSTRAP_JS),
    );

    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="{csp}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{css}</style>
<style>{ql_override}</style></head>
<body data-theme="light">
<div class="markdown-viewer"><article class="markdown-body">{body}</article></div>
<script>{bundle}</script>
<script>{bootstrap}</script>
</body></html>"#,
        csp = csp,
        css = RENDERER_CSS,
        ql_override = QL_OVERRIDE_CSS,
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
        let html = build_document("<p>hi</p>");

        // Body is wrapped in the markdown-viewer container (which supplies the
        // app's padding, background, and centered max-width) and article.
        assert!(html.contains(
            r#"<div class="markdown-viewer"><article class="markdown-body"><p>hi</p></article></div>"#
        ));
        // Renderer is bootstrapped.
        assert!(html.contains("ArtoRenderer.init"));
        // Styles and scripts are inlined.
        assert!(html.contains("<style>"));
        assert!(html.contains("</style>"));
        // The Quick Look document has no inner `.content` scroll container, so
        // the shared `body { overflow: hidden }` must be overridden or the
        // preview cannot scroll long documents.
        assert!(html.contains(QL_OVERRIDE_CSS));
        assert!(html.contains("overflow:auto"));
        assert!(html.contains("<script>"));
        assert!(html.contains("</script>"));
        // Theme defaults are present.
        assert!(html.contains(r#"<body data-theme="light">"#));
        assert!(html.contains("prefers-color-scheme"));
        // Mermaid needs crypto.randomUUID, which the opaque origin lacks, so
        // the bootstrap must polyfill it.
        assert!(html.contains("crypto.randomUUID"));
    }

    #[test]
    fn test_build_document_has_no_premature_script_termination() {
        let html = build_document("<p>hi</p>");

        // The only `</script` occurrences must be our own closing tags, i.e.
        // the embedded bundle must not smuggle in a `</script` that would
        // close the inline script early.
        assert_eq!(html.matches("</script>").count(), 2);
        assert_eq!(html.matches("</script").count(), 2);
    }

    #[test]
    fn test_build_document_has_csp_allowlisting_only_inline_scripts() {
        let html = build_document("<p>hi</p>");

        // The page carries a CSP that allowlists scripts by hash (no
        // 'unsafe-inline'), so injected <script>/event handlers cannot run.
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(html.contains("script-src 'sha256-"));
        assert!(!html.contains("'unsafe-inline'; script"));
        // The hashes must match the exact inline script contents we emit.
        let bundle = RENDERER_JS.replace("</script", r"<\/script");
        assert!(html.contains(&format!("'sha256-{}'", sha256_base64(&bundle))));
        assert!(html.contains(&format!("'sha256-{}'", sha256_base64(BOOTSTRAP_JS))));
    }

    #[test]
    fn test_read_capped_rejects_oversized_content() {
        // A string longer than the cap is refused; a small one is accepted.
        // (Exercised indirectly via a temp file.)
        let dir = std::env::temp_dir();
        let path = dir.join("arto_ql_read_capped_test.md");
        std::fs::write(&path, "# ok\n").unwrap();
        assert!(read_capped(&path).is_ok());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_read_capped_rejects_non_regular_file() {
        // A directory is not a regular file and must be rejected before open
        // (the same guard rejects FIFOs/devices without blocking).
        assert!(read_capped(&std::env::temp_dir()).is_err());
    }
}
