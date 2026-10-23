//! Local images: which file a reference names, and what the `src` or
//! `srcset` naming it becomes under each [`ImageResolution`].

use base64::{engine::general_purpose, Engine as _};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::urls::{
    file_url_to_path, has_foreign_scheme, needs_no_resolution, parse_srcset, SrcsetCandidate,
};
use crate::{DeferredImage, ImageResolution};

/// Maximum byte size of a local image that will be inlined as a data URL.
///
/// Prevents accidental misreferences (e.g. log files, device files like `/dev/zero`)
/// from freezing the UI or exhausting memory during Markdown rendering. Legitimate
/// screenshots, photos, and diagrams comfortably fit within this limit.
const MAX_INLINE_IMAGE_SIZE: u64 = 32 * 1024 * 1024;

/// Read a file for inlining, rejecting anything over `max_size` bytes.
///
/// Uses `metadata` as a fast path for regular files, then falls back to a bounded
/// `Read::take` so files whose reported length is unreliable (device files, files
/// that grow between stat and read) cannot exceed the limit.
fn read_image_bounded(path: &Path, max_size: u64) -> Option<Vec<u8>> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > max_size {
            tracing::debug!(
                ?path,
                size = metadata.len(),
                limit = max_size,
                "Image exceeds inline size limit; skipping"
            );
            return None;
        }
    }

    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(max_size + 1).read_to_end(&mut buf).ok()?;
    if buf.len() as u64 > max_size {
        tracing::debug!(
            ?path,
            limit = max_size,
            "Image exceeded inline size limit during read; skipping"
        );
        return None;
    }
    Some(buf)
}

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
/// other values are plain filesystem paths — absolute ones used as-is,
/// relative ones joined onto `canonical_base`. A path that resolves outside
/// `canonical_base` is still read (logged at `trace` level), matching
/// standard Markdown viewer behavior. Reads are bounded by
/// `MAX_INLINE_IMAGE_SIZE` so a misreferenced huge file or a device file
/// cannot freeze the UI. An empty file is declined as well: it holds no image,
/// and its data URL would end in the `base64,` comma, which `srcset` parsing
/// strips back off into a malformed URL.
fn inline_local_image(src: &str, canonical_base: &Path) -> Option<String> {
    let canonical_path = resolve_local_image(src, canonical_base)?;
    let image_data = read_image_bounded(&canonical_path, MAX_INLINE_IMAGE_SIZE)?;
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
fn resolve_local_image(src: &str, canonical_base: &Path) -> Option<PathBuf> {
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
                canonical_base.join(path)
            }
        })
    } else {
        let path = Path::new(src);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            canonical_base.join(path)
        }
    };

    let canonical_path = absolute_path.canonicalize().ok()?;
    if !canonical_path.starts_with(canonical_base) {
        tracing::trace!(
            ?src,
            "Image path resolved outside base directory; proceeding with inline read"
        );
    }
    Some(canonical_path)
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

/// The image at `path`, or `None` when it is one no consumer could show.
///
/// The bound and the emptiness check are the same ones [`read_image_bounded`]
/// applies, and they are made here rather than left to the host because the
/// answer changes the markup: a `srcset` candidate that cannot be shown is
/// dropped so the browser falls back to one that can, and the host serving
/// the URL is far too late to drop it.
fn deferred_image(path: PathBuf) -> Option<DeferredImage> {
    let metadata = std::fs::metadata(&path).ok()?;
    if metadata.len() > MAX_INLINE_IMAGE_SIZE {
        tracing::debug!(
            ?path,
            size = metadata.len(),
            limit = MAX_INLINE_IMAGE_SIZE,
            "Image exceeds the size limit; not served"
        );
        return None;
    }
    if metadata.len() == 0 {
        tracing::debug!(?path, "Image file is empty; nothing to serve");
        return None;
    }

    Some(DeferredImage {
        id: image_id(&path, &metadata),
        mime: get_mime_type(&path),
        path,
    })
}

/// An id standing for the file at `path` as it is right now.
///
/// The path is hashed rather than the bytes, so naming an image costs no
/// read; the path is already canonical, so two spellings of one file agree.
/// The size and modification time go in as well, which is what makes an
/// edited image a different URL — the app reloads a document when its file
/// changes, and a URL that stayed the same would let the WebView answer the
/// re-render from its cache with the bytes the reader just replaced.
///
/// The id is not a secret: anyone who can guess a path can compute its hash.
/// What limits what the host will serve is the host's own registry — it
/// answers for the ids a render gave it, and for nothing else.
fn image_id(path: &Path, metadata: &std::fs::Metadata) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    hasher.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified() {
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
    canonical_base: &Path,
    resolution: &ImageResolution,
    collected: &RefCell<DeferredImages>,
) -> Option<String> {
    match resolution {
        ImageResolution::DataUrl => inline_local_image(src, canonical_base),
        ImageResolution::Deferred { base_url } => {
            let image = deferred_image(resolve_local_image(src, canonical_base)?)?;
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
    canonical_base: &Path,
    resolution: &ImageResolution,
    collected: &RefCell<DeferredImages>,
) -> Option<String> {
    let mut changed = false;
    let candidates: Vec<String> = parse_srcset(value)
        .into_iter()
        .filter_map(|SrcsetCandidate { url, descriptor }| {
            let url = match resolved_image_src(&url, canonical_base, resolution, collected) {
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
    use std::fs;
    use tempfile::TempDir;

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

    /// `read_image_bounded` must reject files larger than the configured limit,
    /// both via the metadata fast path and via the bounded-read fallback.
    #[test]
    fn test_read_image_bounded_rejects_oversized() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("big.png");
        // 2 KiB file; limit 1 KiB.
        fs::write(&path, vec![0u8; 2048]).unwrap();

        assert!(
            read_image_bounded(&path, 1024).is_none(),
            "file larger than limit must be rejected"
        );
    }

    /// `read_image_bounded` must accept files at or below the limit.
    #[test]
    fn test_read_image_bounded_accepts_within_limit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("ok.png");
        fs::write(&path, vec![0u8; 1024]).unwrap();

        let data = read_image_bounded(&path, 1024).expect("exactly-limit file must be accepted");
        assert_eq!(data.len(), 1024);
    }
}
