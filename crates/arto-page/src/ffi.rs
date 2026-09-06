//! C ABI for the macOS Quick Look preview extension.
//!
//! Compiled only with the `ffi` feature, into the `staticlib` that the Swift
//! shim under platform/macos/quicklook links. The extension passes a file
//! path in and receives the finished page as a C string.

use crate::{Config, PageOptions};
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::ptr;

/// Render the Markdown file at `path_utf8` to a full self-contained HTML page.
///
/// The page follows the user's `config.json` when it can be read; the
/// extension's sandbox may hide the app's configuration directory, in which
/// case the built-in defaults apply. A preview must never fail because of a
/// configuration problem, so those errors are swallowed here.
///
/// Returns a Rust-allocated C string that the caller must release only with
/// [`arto_page_free_string`] (never libc `free`). Returns null on any error
/// (an unreadable, non-regular, or oversized file, or a rendering failure).
///
/// # Safety
///
/// `path_utf8` must be either null or a valid pointer to a NUL-terminated C
/// string that stays valid for the duration of the call. The returned pointer
/// must be freed exactly once with [`arto_page_free_string`] and not otherwise.
#[no_mangle]
pub unsafe extern "C" fn arto_page_render_markdown_file(path_utf8: *const c_char) -> *mut c_char {
    if path_utf8.is_null() {
        return ptr::null_mut();
    }

    // Swift passes the file-system representation (raw bytes), which is not
    // guaranteed to be valid UTF-8, so build the path from bytes directly.
    let path = path_from_ffi_bytes(CStr::from_ptr(path_utf8).to_bytes());

    let options = Config::load_preferences()
        .map(|config| PageOptions::from_config(&config))
        .unwrap_or_default();
    let Ok(document) = crate::render_file(&path, &options) else {
        return ptr::null_mut();
    };

    match CString::new(document) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string previously returned by [`arto_page_render_markdown_file`].
///
/// # Safety
///
/// `ptr` must be either null or a pointer returned by
/// [`arto_page_render_markdown_file`] that has not already been freed. Passing
/// any other pointer, or freeing the same pointer twice, is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn arto_page_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
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
