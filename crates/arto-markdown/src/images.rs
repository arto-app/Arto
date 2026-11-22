//! Local images: which file a reference names, and what the `src` or
//! `srcset` naming it becomes under each [`ImageResolution`].

use base64::{engine::general_purpose, Engine as _};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::path::Path;

use crate::urls::{
    file_url_to_path, has_foreign_scheme, needs_no_resolution, parse_srcset, SrcsetCandidate,
};
use crate::{DeferredImage, FileStat, Files, ImageResolution};

/// The largest local image, in bytes, that a render inlines as a data URL or
/// hands to the host under [`ImageResolution::Deferred`].
///
/// Prevents accidental misreferences (e.g. log files, device files like `/dev/zero`)
/// from freezing the UI or exhausting memory during Markdown rendering. Legitimate
/// screenshots, photos, and diagrams comfortably fit within this limit. A host
/// that serves deferred images reads the file later than the render did, so it
/// applies the same bound again when it does.
pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// Infer MIME type from file extension
fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        _ => "image/png", // Default
    }
}

/// A local image reference as a `data:` URL, or `None` when it cannot be
/// inlined and must be left as written.
///
/// `http(s):` and `data:` references address something that needs no
/// resolution and are declined. Everything else is a path: `file:` URLs
/// (including `file://`, `file://localhost/…` and `file:/…`) are parsed via
/// `url::Url` so percent-encoding and platform differences are handled, and
/// other values are plain paths — absolute ones used as-is, relative ones
/// joined onto `base_dir` — answered by `files`. A path that leads outside
/// `base_dir` is still read, matching standard Markdown viewer behavior.
/// Reads are bounded by
/// `MAX_IMAGE_BYTES` so a misreferenced huge file or a device file
/// cannot freeze the UI. An empty file is declined as well: it holds no image,
/// and its data URL would end in the `base64,` comma, which `srcset` parsing
/// strips back off into a malformed URL.
fn inline_local_image(src: &str, base_dir: &Path, files: &dyn Files) -> Option<String> {
    let FileStat {
        path: canonical_path,
        len,
        ..
    } = resolve_local_image(src, base_dir, files)?;
    if len > MAX_IMAGE_BYTES {
        tracing::debug!(
            ?canonical_path,
            size = len,
            limit = MAX_IMAGE_BYTES,
            "Image exceeds inline size limit; skipping"
        );
        return None;
    }
    let Some(image_data) = files.read(&canonical_path, MAX_IMAGE_BYTES) else {
        tracing::debug!(
            ?canonical_path,
            limit = MAX_IMAGE_BYTES,
            "Image could not be read within the inline size limit; skipping"
        );
        return None;
    };
    if image_data.is_empty() {
        tracing::debug!(?canonical_path, "Image file is empty; nothing to inline");
        return None;
    }
    let mime_type = get_mime_type(&canonical_path);
    let base64_data = general_purpose::STANDARD.encode(&image_data);
    Some(format!("data:{mime_type};base64,{base64_data}"))
}

/// The file a local image reference names, or `None` when the reference
/// addresses something that needs no resolution or nothing that exists.
fn resolve_local_image(src: &str, base_dir: &Path, files: &dyn Files) -> Option<FileStat> {
    if needs_no_resolution(src) {
        return None;
    }

    let absolute_path = if src.starts_with("file:") {
        // A URL that names no file here (another host, or one that does not
        // parse) has its scheme stripped and the rest read as a path.
        file_url_to_path(src).unwrap_or_else(|| {
            let raw = src
                .strip_prefix("file://")
                .or_else(|| src.strip_prefix("file:/"))
                .or_else(|| src.strip_prefix("file:"))
                .unwrap_or(src);
            let path = Path::new(raw);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            }
        })
    } else {
        let path = Path::new(src);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_dir.join(path)
        }
    };

    files.stat(&absolute_path)
}

/// The images a render handed to the host, in the order first referenced.
///
/// Deduplicated by id, so the same file drawn several times is one entry and
/// one URL — which is also what lets the host cache it.
#[derive(Default)]
pub(crate) struct DeferredImages {
    pub(crate) entries: Vec<DeferredImage>,
}

impl DeferredImages {
    fn url(&mut self, image: DeferredImage, base_url: &str) -> String {
        // The extension rides along after the id, which the host ignores when
        // it looks the id up. Nothing needs it to resolve the image, but a
        // reader of the document and the frontend both do: the rasteriser
        // scales an SVG differently from a photograph, and it has only the
        // URL to tell them apart.
        let url = match image
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some(extension) => format!("{base_url}/{}.{extension}", image.id),
            None => format!("{base_url}/{}", image.id),
        };
        if !self.entries.iter().any(|entry| entry.id == image.id) {
            self.entries.push(image);
        }
        url
    }
}

/// The image `file` is, or `None` when it is one no consumer could show.
///
/// The bound and the emptiness check are the same ones [`inline_local_image`]
/// applies, and they are made here rather than left to the host because the
/// answer changes the markup: a `srcset` candidate that cannot be shown is
/// dropped so the browser falls back to one that can, and the host serving
/// the URL is far too late to drop it.
fn deferred_image(file: FileStat) -> Option<DeferredImage> {
    if file.len > MAX_IMAGE_BYTES {
        tracing::debug!(
            path = ?file.path,
            size = file.len,
            limit = MAX_IMAGE_BYTES,
            "Image exceeds the size limit; not served"
        );
        return None;
    }
    if file.len == 0 {
        tracing::debug!(path = ?file.path, "Image file is empty; nothing to serve");
        return None;
    }

    Some(DeferredImage {
        id: image_id(&file),
        mime: get_mime_type(&file.path),
        path: file.path,
    })
}

/// An id standing for `file` as it is right now.
///
/// The path is hashed rather than the bytes, so naming an image costs no
/// read; it is the one spelling [`Files::stat`] gives, so two spellings of
/// one file agree.
/// The size and modification time go in as well, which is what makes an
/// edited image a different URL — the app reloads a document when its file
/// changes, and a URL that stayed the same would let the WebView answer the
/// re-render from its cache with the bytes the reader just replaced.
///
/// The id is not a secret: anyone who can guess a path can compute its hash.
/// What limits what the host will serve is the host's own registry — it
/// answers for the ids a render gave it, and for nothing else.
fn image_id(file: &FileStat) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file.path.as_os_str().as_encoded_bytes());
    hasher.update(file.len.to_le_bytes());
    if let Some(modified) = file.modified {
        if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
            hasher.update(since_epoch.as_nanos().to_le_bytes());
        }
    }
    hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The `<img src>` value a local image reference becomes, or `None` when it
/// is to be left as written.
pub(crate) fn resolved_image_src(
    src: &str,
    base_dir: &Path,
    files: &dyn Files,
    resolution: &ImageResolution,
    collected: &RefCell<DeferredImages>,
) -> Option<String> {
    match resolution {
        ImageResolution::DataUrl => inline_local_image(src, base_dir, files),
        ImageResolution::Deferred { base_url } => {
            let image = deferred_image(resolve_local_image(src, base_dir, files)?)?;
            Some(collected.borrow_mut().url(image, base_url))
        }
    }
}

/// A `srcset` value with every local candidate inlined as a `data:` URL, or
/// `None` when the value needs no change. An empty string means no candidate
/// survived and the attribute should go.
///
/// A candidate that carries a scheme of its own (`http(s):`, `data:`, …) is
/// left as written. A local one that could not be read is dropped, because the
/// rendered page carries no base URL — `arto-page` is a self-contained
/// document and the app's WebView resolves against its asset server — so such
/// a candidate can never load, and keeping it would let the browser choose it
/// over a candidate that did inline: a `2x` candidate wins on every Retina
/// display. This is why a `srcset` is treated differently from an `img[src]`,
/// which keeps an unreadable path because there is no alternative to fall back
/// on.
pub(crate) fn inline_srcset(
    value: &str,
    base_dir: &Path,
    files: &dyn Files,
    resolution: &ImageResolution,
    collected: &RefCell<DeferredImages>,
) -> Option<String> {
    let mut changed = false;
    let candidates: Vec<String> = parse_srcset(value)
        .into_iter()
        .filter_map(|SrcsetCandidate { url, descriptor }| {
            let url = match resolved_image_src(&url, base_dir, files, resolution, collected) {
                Some(resolved) => {
                    changed = true;
                    resolved
                }
                None if has_foreign_scheme(&url) => url,
                None => {
                    changed = true;
                    return None;
                }
            };
            Some(if descriptor.is_empty() {
                url
            } else {
                format!("{url} {descriptor}")
            })
        })
        .collect();
    changed.then(|| candidates.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mime_type() {
        assert_eq!(get_mime_type(Path::new("test.png")), "image/png");
        assert_eq!(get_mime_type(Path::new("test.jpg")), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.jpeg")), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.gif")), "image/gif");
        assert_eq!(get_mime_type(Path::new("test.svg")), "image/svg+xml");
        assert_eq!(get_mime_type(Path::new("test.webp")), "image/webp");
        assert_eq!(get_mime_type(Path::new("test.bmp")), "image/bmp");
        assert_eq!(get_mime_type(Path::new("test.ico")), "image/x-icon");
        assert_eq!(get_mime_type(Path::new("test.unknown")), "image/png");
    }
}
