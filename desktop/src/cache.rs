use std::fs;
use std::path::PathBuf;

/// Build identifier embedded at compile time (changes on every rebuild)
const BUILD_ID: &str = compile_time::datetime_str!();

/// Check if the build has changed since last launch,
/// and clear WebView cache if it has.
///
/// Must be called before Dioxus LaunchBuilder to ensure
/// the WebView starts with a clean cache after upgrades.
pub fn clear_stale_webview_cache_if_needed() {
    let stored = read_stored_build_id();

    match stored.as_deref() {
        Some(id) if id == BUILD_ID => {
            tracing::debug!(build_id = BUILD_ID, "Build unchanged, skipping cache clear");
        }
        Some(old_id) => {
            tracing::info!(
                old_build_id = old_id,
                new_build_id = BUILD_ID,
                "Build changed, clearing WebView cache"
            );
            clear_webview_cache();
            write_current_build_id();
        }
        None => {
            // First launch with this feature — clear cache to handle upgrade
            // from older versions that didn't write .build-id
            tracing::info!(
                build_id = BUILD_ID,
                "No stored build ID found, clearing WebView cache"
            );
            clear_webview_cache();
            write_current_build_id();
        }
    }
}

// ---------------------------------------------------------------------------
// Build ID persistence
// ---------------------------------------------------------------------------

/// Path: ~/Library/Application Support/arto/.build-id
fn build_id_path() -> PathBuf {
    const FILENAME: &str = ".build-id";
    if let Some(mut path) = dirs::config_dir() {
        path.push("arto");
        path.push(FILENAME);
        return path;
    }
    if let Some(mut path) = dirs::home_dir() {
        path.push(".arto");
        path.push(FILENAME);
        return path;
    }
    PathBuf::from(FILENAME)
}

fn read_stored_build_id() -> Option<String> {
    let path = build_id_path();
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

fn write_current_build_id() {
    let path = build_id_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!(
                path = %path.display(),
                ?e,
                "Failed to create directory for build ID file"
            );
            return;
        }
    }
    if let Err(e) = fs::write(&path, BUILD_ID) {
        tracing::warn!(path = %path.display(), ?e, "Failed to write build ID file");
    }
}

// ---------------------------------------------------------------------------
// WebView cache clearing
// ---------------------------------------------------------------------------

fn clear_webview_cache() {
    let cache_dirs = collect_webview_cache_dirs();
    for dir in &cache_dirs {
        if dir.exists() {
            match fs::remove_dir_all(dir) {
                Ok(()) => {
                    tracing::info!(path = %dir.display(), "Cleared WebView cache directory");
                }
                Err(e) => {
                    tracing::warn!(path = %dir.display(), ?e, "Failed to clear WebView cache");
                }
            }
        }
    }
}

fn collect_webview_cache_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // macOS: ~/Library/Caches/
    if let Some(cache_dir) = dirs::cache_dir() {
        dirs.push(cache_dir.join("com.lambdalisue.Arto"));
        dirs.push(cache_dir.join("arto"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_id_format() {
        // compile_time::datetime_str!() format: yyyy-MM-ddThh:mm:ssZ
        // Example: "2026-01-31T12:34:56Z"
        assert!(BUILD_ID.len() >= 20, "BUILD_ID should be ISO 8601 format");
        assert!(BUILD_ID.contains('T'), "BUILD_ID should contain 'T'");
        assert!(BUILD_ID.contains('Z'), "BUILD_ID should contain 'Z'");
    }

    #[test]
    fn test_collect_webview_cache_dirs_returns_expected_paths() {
        let dirs = collect_webview_cache_dirs();
        // Skip test in environments where dirs::cache_dir() is None (CI/containers)
        if dirs.is_empty() {
            return;
        }
        assert!(dirs.iter().any(|d| d.ends_with("com.lambdalisue.Arto")));
    }
}
