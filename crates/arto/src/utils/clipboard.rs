//! Unified clipboard operations using arboard
//!
//! This module provides cross-platform clipboard functionality for both text and images.
//! The clipboard instance is held for the application's lifetime to ensure proper
//! clipboard ownership on Linux.

use arboard::{Clipboard, ImageData};
use base64::Engine;
use std::sync::LazyLock;
use std::sync::Mutex;

use super::image::extract_base64_from_data_url;

/// Global clipboard instance held for the application lifetime.
///
/// On Linux, clipboard contents are owned by the application that placed them,
/// so we keep the clipboard alive to prevent data loss.
static CLIPBOARD: LazyLock<Mutex<Clipboard>> =
    LazyLock::new(|| Mutex::new(Clipboard::new().expect("Failed to initialize clipboard")));

/// Copy text to the system clipboard.
///
/// # Examples
///
/// ```no_run
/// use arto::utils::clipboard::copy_text;
///
/// copy_text("Hello, world!");
/// ```
pub fn copy_text(text: impl AsRef<str>) {
    let mut clipboard = CLIPBOARD.lock().unwrap();
    if let Err(e) = clipboard.set_text(text.as_ref()) {
        tracing::error!(%e, "Failed to copy text to clipboard");
    }
}

/// Copy an image from a data URL to the system clipboard.
///
/// The data URL should be in the format: `data:image/png;base64,<base64-encoded-data>`
///
/// # Examples
///
/// ```no_run
/// use arto::utils::clipboard::copy_image_from_data_url;
///
/// let data_url = "data:image/png;base64,iVBORw0KGgo...";
/// copy_image_from_data_url(data_url);
/// ```
pub fn copy_image_from_data_url(data_url: impl AsRef<str>) {
    let data_url = data_url.as_ref();

    // Extract base64 data from data URL
    let base64_data = match extract_base64_from_data_url(data_url) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(%e, "Failed to extract base64 data from data URL");
            return;
        }
    };

    // Decode base64 to bytes
    let image_bytes = match base64::prelude::BASE64_STANDARD.decode(base64_data) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!(%e, "Failed to decode base64 image data");
            return;
        }
    };

    // Load image and convert to RGBA
    let img = match image::load_from_memory(&image_bytes) {
        Ok(img) => img,
        Err(e) => {
            tracing::error!(%e, "Failed to load image from bytes");
            return;
        }
    };

    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();

    // Create ImageData for arboard
    let image_data = ImageData {
        width: width as usize,
        height: height as usize,
        bytes: rgba_img.into_raw().into(),
    };

    // Copy to clipboard
    let mut clipboard = CLIPBOARD.lock().unwrap();
    if let Err(e) = clipboard.set_image(image_data) {
        tracing::error!(%e, "Failed to copy image to clipboard");
    }
}
