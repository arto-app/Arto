//! Image utility functions for saving and processing images.
//!
//! This module provides utilities for:
//! - Saving images from data URLs or HTTP/HTTPS URLs to files
//! - Extracting information from data URLs (MIME type, base64 data)
//! - Downloading images from external URLs

use base64::Engine;
use std::time::Duration;

/// Maximum allowed image size (20 MiB) to prevent memory exhaustion.
const MAX_IMAGE_SIZE: u64 = 20 * 1024 * 1024;

/// HTTP request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Save an image from a URL (data URL or HTTP/HTTPS) to a file using a native save dialog.
///
/// Opens a file save dialog and writes the image to the selected path.
/// Supports:
/// - Data URLs: `data:image/png;base64,<base64-data>`
/// - HTTP/HTTPS URLs: `https://example.com/image.png`
///
/// # Examples
///
/// ```no_run
/// use arto::utils::image::save_image;
///
/// // Data URL
/// save_image("data:image/png;base64,iVBORw0KGgo...");
///
/// // External URL
/// save_image("https://example.com/image.png");
/// ```
pub fn save_image(src: impl AsRef<str>) {
    use rfd::FileDialog;

    let src = src.as_ref();

    // Determine image source type and get bytes + file info
    let (image_bytes, filter_name, extensions, default_filename) = if src.starts_with("data:") {
        // Data URL: extract MIME type and decode base64
        let mime_type = extract_mime_type_from_data_url(src);
        let (filter_name, extensions, ext) = get_file_info_from_mime_type(mime_type);

        let base64_data = match extract_base64_from_data_url(src) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(%e, "Failed to extract base64 data from data URL");
                return;
            }
        };

        let bytes = match base64::prelude::BASE64_STANDARD.decode(base64_data) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!(%e, "Failed to decode base64 image data");
                return;
            }
        };

        (bytes, filter_name, extensions, format!("image.{}", ext))
    } else if src.starts_with("http://") || src.starts_with("https://") {
        // External URL: download the image
        let (bytes, content_type) = match download_image(src) {
            Ok(result) => result,
            Err(e) => {
                tracing::error!(%e, %src, "Failed to download image");
                return;
            }
        };

        // Determine file info from content type or URL extension
        let (filter_name, extensions, ext) = if content_type.is_some() {
            get_file_info_from_mime_type(content_type.as_deref())
        } else {
            // Fall back to URL extension
            let url_ext = extract_extension_from_url(src);
            get_file_info_from_mime_type(url_ext.map(|e| match e {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                "bmp" => "image/bmp",
                _ => "",
            }))
        };

        // Extract filename from URL or use default
        let filename = extract_filename_from_url(src).unwrap_or_else(|| format!("image.{}", ext));

        (bytes, filter_name, extensions, filename)
    } else {
        tracing::error!(%src, "Unsupported image source format");
        return;
    };

    // Show save dialog
    let Some(path) = FileDialog::new()
        .add_filter(filter_name, &extensions)
        .set_file_name(default_filename)
        .save_file()
    else {
        return; // User cancelled
    };

    // Write to file
    if let Err(e) = std::fs::write(&path, image_bytes) {
        tracing::error!(%e, ?path, "Failed to save image to file");
    }
}

/// Get file filter info (filter name, extensions, default extension) from MIME type.
///
/// Returns a tuple of (filter_name, extensions, default_extension).
fn get_file_info_from_mime_type(
    mime_type: Option<&str>,
) -> (&'static str, Vec<&'static str>, &'static str) {
    match mime_type {
        Some("image/png") => ("PNG Image", vec!["png"], "png"),
        Some("image/jpeg") => ("JPEG Image", vec!["jpg", "jpeg"], "jpg"),
        Some("image/gif") => ("GIF Image", vec!["gif"], "gif"),
        Some("image/webp") => ("WebP Image", vec!["webp"], "webp"),
        Some("image/svg+xml") => ("SVG Image", vec!["svg"], "svg"),
        Some("image/bmp") => ("BMP Image", vec!["bmp"], "bmp"),
        _ => ("Image", vec!["png", "jpg", "gif", "webp"], "png"),
    }
}

/// Check if a URL points to an SVG image.
///
/// Detects SVG by:
/// - Data URLs: checks if the MIME type is `image/svg+xml`
/// - HTTP/HTTPS URLs: checks if the file extension is `.svg`
pub fn is_svg_url(src: impl AsRef<str>) -> bool {
    let src = src.as_ref();
    if src.starts_with("data:") {
        extract_mime_type_from_data_url(src) == Some("image/svg+xml")
    } else {
        matches!(extract_extension_from_url(src), Some(ext) if ext.eq_ignore_ascii_case("svg"))
    }
}

/// Download an image from an HTTP/HTTPS URL.
///
/// Returns the image bytes and the content-type header if available.
///
/// # Security
///
/// This function includes several safety mechanisms:
/// - Request timeout to prevent indefinite hangs
/// - Maximum content length check to prevent memory exhaustion
/// - Content-type validation to ensure the response is an image
pub(crate) fn download_image(url: &str) -> Result<(Vec<u8>, Option<String>), String> {
    use std::io::Read;

    let response = ureq::get(url)
        .config()
        .timeout_global(Some(Duration::from_secs(REQUEST_TIMEOUT_SECS)))
        .build()
        .call()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    // Extract and normalize content-type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());

    // Ensure the URL points to an image if content-type is provided
    if let Some(ct) = &content_type {
        if !ct.starts_with("image/") {
            return Err(format!(
                "URL does not point to an image (content-type: {})",
                ct
            ));
        }
    }

    // Enforce a maximum content length based on the Content-Length header, if present
    if let Some(len) = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        if len > MAX_IMAGE_SIZE {
            return Err(format!(
                "Image is too large ({} bytes, max {} bytes)",
                len, MAX_IMAGE_SIZE
            ));
        }
    }

    // Read the response body with a hard cap on the number of bytes
    let reader = response.into_body().into_reader();
    let mut bytes = Vec::new();

    // Limit the reader to MAX_IMAGE_SIZE + 1 to detect oversized responses
    reader
        .take(MAX_IMAGE_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if bytes.len() as u64 > MAX_IMAGE_SIZE {
        return Err(format!(
            "Image is too large (received more than {} bytes)",
            MAX_IMAGE_SIZE
        ));
    }

    Ok((bytes, content_type))
}

/// Download an image from an HTTP/HTTPS URL and encode it as a data URL.
///
/// This is useful for bypassing browser CORS restrictions when the image
/// needs to be drawn on a Canvas and exported via `toDataURL()`.
pub(crate) fn download_image_as_data_url(url: &str) -> Result<String, String> {
    let (bytes, content_type) = download_image(url)?;
    // Prefer Content-Type header; fall back to URL extension inference
    let mime = content_type
        .as_deref()
        .or_else(|| {
            extract_extension_from_url(url).and_then(|ext| {
                match ext.to_ascii_lowercase().as_str() {
                    "png" => Some("image/png"),
                    "jpg" | "jpeg" => Some("image/jpeg"),
                    "gif" => Some("image/gif"),
                    "webp" => Some("image/webp"),
                    "svg" => Some("image/svg+xml"),
                    "bmp" => Some("image/bmp"),
                    _ => None,
                }
            })
        })
        .unwrap_or("image/png");
    let b64 = base64::prelude::BASE64_STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Extract the filename from a URL path.
///
/// This function:
/// - Removes query strings from the URL
/// - Decodes URL-encoded characters (e.g., `%20` → space)
/// - Sanitizes the filename by removing path separators and dangerous characters
fn extract_filename_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next()?; // Remove query string
    let filename = path.rsplit('/').next()?;
    if filename.is_empty() || !filename.contains('.') {
        return None;
    }

    // Decode URL-encoded characters
    let decoded = percent_encoding::percent_decode_str(filename)
        .decode_utf8()
        .ok()?;

    // Sanitize: remove path separators and other dangerous characters
    let sanitized = sanitize_filename(&decoded);
    if sanitized.is_empty() || !sanitized.contains('.') {
        return None;
    }

    Some(sanitized)
}

/// Sanitize a filename by removing path separators and dangerous characters.
///
/// This prevents path traversal attacks and ensures the filename is safe for the file system.
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| {
            // Allow alphanumeric, dots, hyphens, underscores, and spaces
            // Reject path separators and other special characters
            c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ')
        })
        .collect::<String>()
        // Remove leading/trailing dots and spaces for safety
        .trim_matches(|c| c == '.' || c == ' ')
        .to_string()
}

/// Extract the file extension from a URL path.
fn extract_extension_from_url(url: &str) -> Option<&str> {
    let path = url.split('?').next()?; // Remove query string
    let filename = path.rsplit('/').next()?;
    let ext = filename.rsplit('.').next()?;
    if ext == filename {
        return None; // No extension found
    }
    Some(ext)
}

/// Extract the MIME type from a data URL.
///
/// Expects format: `data:<mime-type>;base64,<base64-data>`
/// Returns the MIME type portion (e.g., "image/png").
///
/// # Examples
///
/// ```no_run
/// use arto::utils::image::extract_mime_type_from_data_url;
///
/// assert_eq!(
///     extract_mime_type_from_data_url("data:image/png;base64,iVBORw0KGgo="),
///     Some("image/png")
/// );
/// assert_eq!(extract_mime_type_from_data_url("not-a-data-url"), None);
/// ```
pub fn extract_mime_type_from_data_url(data_url: &str) -> Option<&str> {
    // data:image/png;base64,<data>
    let stripped = data_url.strip_prefix("data:")?;
    let semicolon_pos = stripped.find(';')?;
    Some(&stripped[..semicolon_pos])
}

/// Extract base64 data from a data URL.
///
/// Expects format: `data:<mime-type>;base64,<base64-data>`
///
/// # Examples
///
/// ```no_run
/// use arto::utils::image::extract_base64_from_data_url;
///
/// assert_eq!(
///     extract_base64_from_data_url("data:image/png;base64,aGVsbG8="),
///     Ok("aGVsbG8=")
/// );
/// assert!(extract_base64_from_data_url("data:image/png,hello").is_err());
/// ```
pub fn extract_base64_from_data_url(data_url: &str) -> Result<&str, &'static str> {
    // data:image/png;base64,<data>
    let Some(comma_pos) = data_url.find(',') else {
        return Err("Invalid data URL: missing comma separator");
    };

    let prefix = &data_url[..comma_pos];
    if !prefix.contains(";base64") {
        return Err("Invalid data URL: not base64 encoded");
    }

    Ok(&data_url[comma_pos + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_base64_from_data_url() {
        let data_url = "data:image/png;base64,iVBORw0KGgo=";
        let base64 = extract_base64_from_data_url(data_url).unwrap();
        assert_eq!(base64, "iVBORw0KGgo=");
    }

    #[test]
    fn test_extract_base64_from_data_url_invalid() {
        let data_url = "not a data url";
        assert!(extract_base64_from_data_url(data_url).is_err());

        let data_url = "data:image/png,notbase64";
        assert!(extract_base64_from_data_url(data_url).is_err());
    }

    #[test]
    fn test_extract_mime_type_from_data_url() {
        assert_eq!(
            extract_mime_type_from_data_url("data:image/png;base64,abc"),
            Some("image/png")
        );
        assert_eq!(
            extract_mime_type_from_data_url("data:image/jpeg;base64,abc"),
            Some("image/jpeg")
        );
        assert_eq!(
            extract_mime_type_from_data_url("data:image/svg+xml;base64,abc"),
            Some("image/svg+xml")
        );
    }

    #[test]
    fn test_extract_mime_type_from_data_url_invalid() {
        assert_eq!(extract_mime_type_from_data_url("not a data url"), None);
        assert_eq!(extract_mime_type_from_data_url("data:image/png"), None);
    }

    #[test]
    fn test_extract_filename_from_url() {
        // Basic filename extraction
        assert_eq!(
            extract_filename_from_url("https://example.com/images/photo.png"),
            Some("photo.png".to_string())
        );
        assert_eq!(
            extract_filename_from_url("https://example.com/image.jpg?size=large"),
            Some("image.jpg".to_string())
        );
        assert_eq!(extract_filename_from_url("https://example.com/path/"), None);
        assert_eq!(
            extract_filename_from_url("https://example.com/noextension"),
            None
        );
    }

    #[test]
    fn test_extract_filename_from_url_decoding() {
        // URL decoding
        assert_eq!(
            extract_filename_from_url("https://example.com/my%20image.png"),
            Some("my image.png".to_string())
        );
        assert_eq!(
            extract_filename_from_url("https://example.com/photo%20%282%29.jpg"),
            Some("photo 2.jpg".to_string()) // Parentheses are filtered out
        );
    }

    #[test]
    fn test_extract_filename_from_url_sanitization() {
        // Path traversal attempts should be sanitized
        assert_eq!(
            extract_filename_from_url("https://example.com/..%2F..%2Fevil.png"),
            Some("evil.png".to_string())
        );
        // Slashes in filename should be removed
        assert_eq!(
            extract_filename_from_url("https://example.com/file%2Fwith%2Fslashes.png"),
            Some("filewithslashes.png".to_string())
        );
    }

    #[test]
    fn test_sanitize_filename() {
        // Normal filenames
        assert_eq!(sanitize_filename("image.png"), "image.png");
        assert_eq!(sanitize_filename("my-photo_2024.jpg"), "my-photo_2024.jpg");

        // Path separators removed
        assert_eq!(sanitize_filename("../../evil.png"), "evil.png");
        assert_eq!(sanitize_filename("path/to/file.png"), "pathtofile.png");
        assert_eq!(sanitize_filename("path\\to\\file.png"), "pathtofile.png");

        // Leading/trailing dots removed
        assert_eq!(sanitize_filename(".hidden.png"), "hidden.png");
        assert_eq!(sanitize_filename("file.png."), "file.png");

        // Special characters removed
        assert_eq!(sanitize_filename("file<>:\"|?*.png"), "file.png");
    }

    #[test]
    fn test_is_svg_url_data_url() {
        assert!(is_svg_url("data:image/svg+xml;base64,PHN2Zz4="));
        assert!(!is_svg_url("data:image/png;base64,iVBORw0KGgo="));
        assert!(!is_svg_url("data:image/jpeg;base64,/9j/4AAQ="));
    }

    #[test]
    fn test_is_svg_url_http_url() {
        assert!(is_svg_url("https://example.com/diagram.svg"));
        assert!(is_svg_url("https://example.com/path/image.svg?v=2"));
        assert!(is_svg_url("https://example.com/diagram.SVG"));
        assert!(is_svg_url("https://example.com/diagram.Svg"));
        assert!(!is_svg_url("https://example.com/photo.png"));
        assert!(!is_svg_url("https://example.com/photo.jpg"));
    }

    #[test]
    fn test_extract_extension_from_url() {
        assert_eq!(
            extract_extension_from_url("https://example.com/image.png"),
            Some("png")
        );
        assert_eq!(
            extract_extension_from_url("https://example.com/photo.jpeg?q=80"),
            Some("jpeg")
        );
        assert_eq!(
            extract_extension_from_url("https://example.com/noext"),
            None
        );
    }
}
