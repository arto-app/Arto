//! The local images the app agreed to serve, by the id its documents name.
//!
//! A render hands back the images it referenced instead of the bytes, and
//! this is where they are kept until a WebView asks for one. What the app
//! will serve is exactly what some render put here: an id nobody registered
//! is a 404, which is what keeps a URL from naming any file on the disk.
//!
//! The map is process-wide, not per window, because the window that asks is
//! not always the window that rendered: opening an image in its own window
//! hands that window the URL to resolve for itself.
//!
//! Entries are only ever added. Each is a path and a content type, so even a
//! reading session of thousands of images stays well under a megabyte, and
//! nothing has to decide when an id stops being valid — a document being
//! re-rendered would otherwise race the WebView still loading the last one.

use arto_markdown::DeferredImage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

/// What serving an image needs: where it is, and what to call it.
#[derive(Clone)]
pub struct Registered {
    pub path: PathBuf,
    pub mime: &'static str,
}

static REGISTRY: LazyLock<RwLock<HashMap<String, Registered>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Agree to serve every image in `images`.
pub fn register(images: Vec<DeferredImage>) {
    if images.is_empty() {
        return;
    }
    let mut registry = REGISTRY.write().expect("image registry is not poisoned");
    for image in images {
        registry.entry(image.id).or_insert(Registered {
            path: image.path,
            mime: image.mime,
        });
    }
}

/// The image an id stands for, or `None` when no render ever named it.
pub fn resolve(id: &str) -> Option<Registered> {
    REGISTRY
        .read()
        .expect("image registry is not poisoned")
        .get(id)
        .cloned()
}

/// The file behind one of the app's own image URLs, or `None` when `src` is
/// not one of them.
pub fn path_for(src: &str) -> Option<PathBuf> {
    Some(resolve(id_in(src)?)?.path)
}

/// Every one of the app's image URLs in `text`, replaced by the file it
/// stands for.
///
/// What the reader means by an image is the file on their disk. The URL is an
/// arrangement between the renderer and the WebView, and it resolves nowhere
/// else — so anything copied out of the app, a Markdown link or a path, says
/// where the image actually is.
pub fn with_paths_for_urls(text: &str) -> String {
    let mut rewritten = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find(URL_PREFIX) {
        rewritten.push_str(&rest[..start]);
        let url_end = rest[start..]
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '>'))
            .map_or(rest.len(), |end| start + end);
        let url = &rest[start..url_end];

        match path_for(url) {
            Some(path) => rewritten.push_str(&path.to_string_lossy()),
            None => rewritten.push_str(url),
        }
        rest = &rest[url_end..];
    }

    rewritten.push_str(rest);
    rewritten
}

/// The id inside one of the app's image URLs.
///
/// The whole origin has to match, not just the `/img/` segment: `https://…
/// /img/<something>.png` is somebody else's URL, and resolving it against this
/// registry would hand a remote reference a local file.
fn id_in(src: &str) -> Option<&str> {
    src.strip_prefix(URL_PREFIX)?
        .split(['.', '?', '#'])
        .next()
        .filter(|id| !id.is_empty())
}

/// How one of those URLs starts, whichever shape the platform uses.
const URL_PREFIX: &str = if cfg!(windows) {
    "http://artoasset.localhost/img/"
} else {
    "artoasset://localhost/img/"
};

/// One of the app's own image URLs as a `data:` URL, or `None` when `src` is
/// not one of them.
///
/// For the paths that have to put an image somewhere other than in a
/// document: the clipboard, mostly. A WebView cannot rasterize an image it
/// fetched from the app's origin — the canvas it draws to is tainted and
/// reading it back throws — so the bytes go the other way instead.
pub fn data_url_for(src: &str) -> Option<String> {
    use base64::Engine as _;

    let id = id_in(src)?;
    let image = resolve(id)?;
    // Bounded for the same reason serving one is: the file is read now, not
    // when the document was rendered, and by now it may have grown or been
    // replaced by something that is not an image at all.
    let bytes = super::read_bounded(&image.path, super::MAX_IMAGE_SIZE)?;

    Some(format!(
        "data:{};base64,{}",
        image.mime,
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, path: &str) -> DeferredImage {
        DeferredImage {
            id: id.to_string(),
            path: PathBuf::from(path),
            mime: "image/png",
        }
    }

    #[test]
    fn an_unregistered_id_resolves_to_nothing() {
        assert!(resolve("never-rendered-this-one").is_none());
    }

    #[test]
    fn a_registered_id_resolves_to_its_file() {
        register(vec![image("test-registered", "/tmp/hero.png")]);

        let found = resolve("test-registered").expect("registered");
        assert_eq!(found.path, PathBuf::from("/tmp/hero.png"));
        assert_eq!(found.mime, "image/png");
    }

    /// What leaves the app names the file, not the arrangement between the
    /// renderer and the WebView.
    #[test]
    fn copied_text_names_the_file_rather_than_the_url() {
        register(vec![image("testrewrite", "/tmp/hero.png")]);
        let url = format!("{URL_PREFIX}testrewrite.png");

        assert_eq!(with_paths_for_urls(&url), "/tmp/hero.png");
        assert_eq!(
            with_paths_for_urls(&format!("![alt]({url})")),
            "![alt](/tmp/hero.png)"
        );
    }

    /// An id from a session that is gone, or a URL of somebody else's, is
    /// left exactly as it was rather than turned into nothing.
    #[test]
    fn text_that_names_no_registered_image_is_untouched() {
        let unknown = format!("![alt]({URL_PREFIX}0123456789abcdef.png)");
        assert_eq!(with_paths_for_urls(&unknown), unknown);

        let remote = "![alt](https://example.com/hero.png)";
        assert_eq!(with_paths_for_urls(remote), remote);
    }

    /// A remote URL that happens to be shaped like one of the app's own is
    /// still a remote URL: the registry answers for this origin only.
    #[test]
    fn a_foreign_url_shaped_like_the_apps_resolves_to_nothing() {
        register(vec![image("testforeign", "/tmp/hero.png")]);

        assert!(path_for("https://example.com/img/testforeign.png").is_none());
        assert!(data_url_for("https://example.com/img/testforeign.png").is_none());
    }

    /// Re-rendering the same document registers the same ids again, which
    /// must not disturb what the WebView is already loading.
    #[test]
    fn registering_an_id_twice_keeps_the_first_answer() {
        register(vec![image("test-twice", "/tmp/first.png")]);
        register(vec![image("test-twice", "/tmp/second.png")]);

        assert_eq!(
            resolve("test-twice").expect("registered").path,
            PathBuf::from("/tmp/first.png")
        );
    }
}
