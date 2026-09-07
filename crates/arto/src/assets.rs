//! What the app serves to its own WebViews, and the protocol it serves it on.
//!
//! # Why a protocol of its own
//!
//! The obvious alternative, writing the stylesheet and the script straight
//! into each window's HTML, costs a megabyte of markup per window, cannot be
//! cached, and makes the sources unreadable in the developer tools. A URL
//! keeps all three.
//!
//! `dioxus://` would have been the natural origin, but Dioxus builds its own
//! asset handler after `Config` is consumed, so nothing can be added to it
//! from here. A protocol registered on the `Config` is a second origin, which
//! is why the responses carry `Access-Control-Allow-Origin` and why the icon
//! sprite is inlined into the document instead of being fetched: a
//! `<use href="…#id">` across origins is refused, and no CORS header changes
//! that.
//!
//! # The origin
//!
//! Windows' WebView2 cannot register a non-standard scheme, so wry rewrites
//! `{protocol}://` requests to `http://{protocol}.` and undoes the rewrite
//! before the handler sees them. The handler therefore reads one origin on
//! every platform, and only the URL written into the HTML differs.
//!
//! The host is `localhost` because of what that rewrite leaves behind: on
//! Windows these become plain `http://` requests, and a document Dioxus
//! serves from a secure origin refuses insecure subresources. Chromium counts
//! `*.localhost` as potentially trustworthy, so the stylesheet and the script
//! load rather than being blocked as mixed content. A host such as
//! `artoasset.assets` would be neither trustworthy nor obviously unresolvable.

mod frontend;
pub mod images;

use arto_keybindings::BindingSet;
use dioxus::desktop::wry::http::{Request, Response};
use dioxus::desktop::wry::{RequestAsyncResponder, WebViewId};
use dioxus::desktop::Config;
use std::borrow::Cow;
use std::io::Read;

/// The scheme the app answers on.
///
/// Not `arto`: wry's Windows rewrite catches `http://{protocol}.*`, and with
/// `arto` that would take requests to real hosts such as `arto.app` with it.
const PROTOCOL: &str = "artoasset";

/// The origin the HTML asks for. See the module docs for the two shapes.
#[cfg(windows)]
const ORIGIN: &str = "http://artoasset.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "artoasset://localhost";

/// Answer requests for the frontend, and for the images a document
/// references, on this configuration's windows.
///
/// Registered on the `Config` rather than through `use_asset_handler`, which
/// only exists after the first render — far too late for the stylesheet the
/// custom head asks for while the page is still parsing.
///
/// The asynchronous form, because one of the two things served is a file of
/// unknown size: an image is read on a thread of its own rather than on the
/// one the window is drawn on.
pub fn with_asset_protocol(config: Config) -> Config {
    config.with_asynchronous_custom_protocol(
        PROTOCOL,
        |_id: WebViewId, request: Request<Vec<u8>>, responder: RequestAsyncResponder| {
            let path = request.uri().path().to_string();

            if let Some(segment) = path.strip_prefix("/img/") {
                // The id is hexadecimal; anything from the first dot on is the
                // extension the render put there for the reader's benefit.
                let id = segment.split('.').next().unwrap_or_default().to_string();
                std::thread::spawn(move || responder.respond(respond_with_image(&id)));
                return;
            }

            responder.respond(respond(&path));
        },
    )
}

fn respond(path: &str) -> Response<Cow<'static, [u8]>> {
    let Some(file) = path.strip_prefix("/assets/").and_then(frontend::bundled) else {
        return not_found();
    };
    let Some(bytes) = frontend::bytes(file) else {
        return not_found();
    };

    Response::builder()
        .status(200)
        .header("Content-Type", file.mime)
        // The document is served from `dioxus://`, so every one of these
        // requests is cross-origin.
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", CACHE_CONTROL)
        .body(bytes)
        .expect("a response with a body and valid headers")
}

/// Answer with the image `id` stands for, if any render ever named it.
///
/// The bound is the one the rendering pipeline applies when it inlines an
/// image instead. It has to be repeated here because the file is only read
/// now: it may have grown, or been replaced by something that is not an
/// image at all, since the document was rendered.
fn respond_with_image(id: &str) -> Response<Cow<'static, [u8]>> {
    let Some(image) = images::resolve(id) else {
        tracing::debug!(id, "Image was never registered by a render");
        return not_found();
    };

    let Some(bytes) = read_bounded(&image.path, MAX_IMAGE_SIZE) else {
        return not_found();
    };

    Response::builder()
        .status(200)
        .header("Content-Type", image.mime)
        .header("Access-Control-Allow-Origin", "*")
        // The id changes when the file does, so what it names cannot.
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(Cow::Owned(bytes))
        .expect("a response with a body and valid headers")
}

/// The largest image the app will read, matching the pipeline's own bound.
/// Guards against a path that has come to name a device file or a log.
const MAX_IMAGE_SIZE: u64 = 32 * 1024 * 1024;

fn read_bounded(path: &std::path::Path, max_size: u64) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path)
        .inspect_err(|error| tracing::debug!(?path, %error, "Image could not be opened"))
        .ok()?;

    let mut bytes = Vec::new();
    file.take(max_size + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > max_size {
        tracing::debug!(?path, limit = max_size, "Image is over the size limit");
        return None;
    }
    Some(bytes)
}

fn not_found() -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(404)
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Borrowed(&b""[..]))
        .expect("a response with a body and valid headers")
}

/// A release binary's bundle cannot change while it runs, and the URL carries
/// the version it was built from. A debug build reads the file on every
/// request, so that rebuilding the bundle and reloading a window is enough to
/// see the change.
#[cfg(not(debug_assertions))]
const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
#[cfg(debug_assertions)]
const CACHE_CONTROL: &str = "no-store";

/// The URL of one bundled file.
///
/// The app's version is in the query so that an upgrade cannot be answered
/// from the cache of the version before it — which matters because a release
/// answers with `immutable` and a year of freshness.
///
/// `ARTO_BUILD_VERSION`, not `CARGO_PKG_VERSION`: the workspace manifest says
/// `0.0.0` and only the release workflow patches it, so a Nix build (which
/// stamps the version through the environment instead) and every local
/// release build would otherwise share one query for every version they ever
/// have.
fn url(file: frontend::Bundled) -> String {
    format!(
        "{ORIGIN}/assets/{}?v={}",
        file.name,
        env!("ARTO_BUILD_VERSION")
    )
}

/// The URL of the module every window imports.
pub fn main_script_url() -> String {
    url(frontend::MAIN_SCRIPT)
}

/// Where a rendered document points at the images the app serves for it.
pub fn image_base_url() -> String {
    format!("{ORIGIN}/img")
}

/// The `<head>` markup that loads the main stylesheet, for
/// `Config::with_custom_head` on every window.
///
/// Kept as static head markup on purpose. `document::Stylesheet {}` would be
/// the idiomatic rsx form, but it is inserted by the runtime after the first
/// render, so the window would paint unstyled for a moment on every open.
/// Head markup is parsed with the page and applies before anything shows.
pub fn main_stylesheet_head() -> String {
    format!(
        r#"<link rel="stylesheet" href="{}">"#,
        url(frontend::MAIN_STYLE)
    )
}

/// The welcome page, with its images and shortcuts filled in.
pub fn get_default_markdown_content() -> String {
    let template = include_str!("../assets/welcome.md");

    // The template names its images by their source-relative paths so that it
    // stays a document that renders on its own; here they become data URLs,
    // which the page can carry because they are small and are drawn once.
    let template = template
        .replace(
            "../assets/arto-header-welcome-light.png",
            &data_url(
                "image/png",
                include_bytes!("../assets/arto-header-welcome-light.png"),
            ),
        )
        .replace(
            "../assets/arto-header-welcome-dark.png",
            &data_url(
                "image/png",
                include_bytes!("../assets/arto-header-welcome-dark.png"),
            ),
        );

    with_current_shortcuts(&template)
}

/// The app icon, for the About tab.
///
/// Encoded once: the tab re-renders on every configuration change, and the
/// icon it shows is the same 31 KB either way.
pub fn app_icon_data_url() -> &'static str {
    static ICON: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| data_url("image/png", crate::window::icon::APP_ICON_PNG));
    &ICON
}

/// The icon sprite, inlined into every window's document.
///
/// Not served from the protocol: `<use href="…#id">` is same-origin only, so
/// a sprite behind a second origin would leave every icon blank, with nothing
/// reported anywhere.
pub fn icon_sprite() -> &'static str {
    include_str!("../assets/frontend/icons/tabler-sprite.svg")
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// What a shortcut reads as when the action has none bound.
const UNBOUND_SHORTCUT: &str = "—";

/// Replace every `{{action}}` in the welcome page with the shortcut bound to
/// that action right now.
///
/// The page is the first thing a reader sees, and every shortcut on it is
/// rebindable, so printing the defaults would be printing something untrue
/// for anyone who has changed one — or who uses the Emacs or Vim preset.
fn with_current_shortcuts(template: &str) -> String {
    let config = crate::config::CONFIG.read();
    substitute_shortcuts(template, &config.keybindings)
}

fn substitute_shortcuts(template: &str, bindings: &BindingSet) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        rendered.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            rendered.push_str(&rest[open..]);
            return rendered;
        };
        let action = after[..close].trim();
        let hint = arto_keybindings::hint_for_action(bindings, action, None);
        rendered.push_str(hint.as_deref().unwrap_or(UNBOUND_SHORTCUT));
        rest = &after[close + 2..];
    }
    rendered.push_str(rest);
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_keybindings::KeyAction;

    fn bound(key: &str, action: &str) -> BindingSet {
        BindingSet {
            global: vec![KeyAction {
                key: key.to_string(),
                action: action.to_string(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn shortcuts_come_from_the_bindings_given() {
        let rendered =
            substitute_shortcuts("Open with `{{file.open}}`.", &bound("Cmd+p", "file.open"));

        assert!(!rendered.contains("{{"), "placeholder left unrendered");
        assert!(
            !rendered.contains(UNBOUND_SHORTCUT),
            "a bound action rendered as unbound: {rendered}"
        );
    }

    #[test]
    fn an_action_nobody_bound_says_so() {
        let rendered = substitute_shortcuts("`{{file.open}}`", &BindingSet::default());
        assert_eq!(rendered, format!("`{UNBOUND_SHORTCUT}`"));
    }

    #[test]
    fn text_around_and_between_placeholders_survives() {
        let rendered = substitute_shortcuts(
            "before {{file.open}} between {{file.open}} after {{",
            &bound("Cmd+p", "file.open"),
        );
        assert!(rendered.starts_with("before "));
        assert!(
            rendered.ends_with(" after {{"),
            "unterminated tail lost: {rendered}"
        );
    }

    /// A request for anything but a bundled file is refused rather than
    /// answered from the filesystem.
    #[test]
    fn the_protocol_serves_the_bundle_and_nothing_else() {
        assert_eq!(respond("/assets/main.css").status(), 200);
        assert_eq!(respond("/assets/main.js").status(), 200);
        assert_eq!(respond("/assets/../../etc/passwd").status(), 404);
        assert_eq!(respond("/etc/passwd").status(), 404);
        assert_eq!(respond("/assets/").status(), 404);
    }

    /// The registry, not the path in the URL, is what decides that an image
    /// may be read: an id no render handed over names nothing.
    #[test]
    fn an_image_nobody_registered_is_refused() {
        assert_eq!(respond_with_image("0123456789abcdef").status(), 404);
        assert_eq!(respond_with_image("").status(), 404);
    }

    /// The extension is for whoever reads the URL; the id alone resolves it.
    #[test]
    fn a_registered_image_is_served_under_its_extension() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("hero.png");
        std::fs::write(&path, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        images::register(vec![arto_markdown::DeferredImage {
            id: "testserved".to_string(),
            path,
            mime: "image/png",
        }]);

        let response = respond_with_image("testserved");
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["Content-Type"], "image/png");
        assert_eq!(response.body().as_ref(), [0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn the_stylesheet_is_asked_for_over_the_protocol() {
        let head = main_stylesheet_head();
        assert!(head.contains(ORIGIN), "{head}");
        assert!(head.contains("main.css"), "{head}");
    }
}
