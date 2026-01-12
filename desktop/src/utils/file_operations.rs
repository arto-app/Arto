use std::path::Path;
use std::process::Command;

/// Reveal a file in the platform's file manager.
///
/// - macOS: Opens Finder with the file selected
/// - Windows: Opens Explorer with the file selected
/// - Linux: Opens the parent directory (file manager varies by desktop environment)
pub fn reveal_in_finder(path: impl AsRef<Path>) {
    let path = path.as_ref();

    #[cfg(target_os = "macos")]
    {
        // Use `open -R` to reveal the file in Finder
        if let Err(e) = Command::new("open").arg("-R").arg(path).spawn() {
            tracing::error!(%e, ?path, "Failed to reveal in Finder");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use `explorer /select,<path>` to reveal the file in Explorer
        if let Err(e) = Command::new("explorer").arg("/select,").arg(path).spawn() {
            tracing::error!(%e, ?path, "Failed to reveal in Explorer");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, open the parent directory (file manager varies by DE)
        // TODO: Consider using DBus to invoke the file manager with selection
        if let Some(parent) = path.parent() {
            if let Err(e) = open::that(parent) {
                tracing::error!(%e, ?parent, "Failed to open parent directory");
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Fallback: open the parent directory
        if let Some(parent) = path.parent() {
            if let Err(e) = open::that(parent) {
                tracing::error!(%e, ?parent, "Failed to open parent directory");
            }
        }
    }
}
