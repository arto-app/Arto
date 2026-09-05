use dioxus::html::HasFileData;
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::state::AppState;

pub(super) async fn handle_dropped_files(evt: Event<DragData>, mut state: AppState) {
    let paths: Vec<PathBuf> = evt.files().into_iter().map(|f| f.path()).collect();

    // wry only reads the legacy NSFilenamesPboardType, so drags from apps that
    // publish modern pasteboard types only (e.g. VS Code) arrive with no files
    #[cfg(target_os = "macos")]
    let paths = if paths.is_empty() {
        drag_pasteboard_paths()
    } else {
        paths
    };

    for path in paths {
        open_dropped_path(path, &mut state);
    }
}

fn open_dropped_path(path: PathBuf, state: &mut AppState) {
    // Finder sidebar items arrive as symlinks
    let resolved_path = match std::fs::canonicalize(&path) {
        Ok(p) => {
            tracing::info!("Resolved path: {:?} -> {:?}", path, p);
            p
        }
        Err(e) => {
            tracing::warn!("Failed to canonicalize path {:?}: {}", path, e);
            path
        }
    };

    if resolved_path.is_dir() {
        tracing::info!("Setting dropped directory as root: {:?}", resolved_path);
        state.set_root_directory(resolved_path);
        // Pin so the tree the drop just opened is actually visible
        state.sidebar.write().pinned = true;
    } else {
        tracing::info!("Opening dropped file: {:?}", resolved_path);
        state.open_file(resolved_path);
    }
}

#[cfg(target_os = "macos")]
fn drag_pasteboard_paths() -> Vec<PathBuf> {
    use objc2_app_kit::NSPasteboard;

    // NSPasteboard is not bound as MainThreadOnly, but AppKit is only
    // documented safe from the main thread
    if objc2::MainThreadMarker::new().is_none() {
        tracing::warn!("Drag pasteboard must be read on the main thread");
        return Vec::new();
    }

    let pasteboard =
        NSPasteboard::pasteboardWithName(unsafe { objc2_app_kit::NSPasteboardNameDrag });
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            // Chromium-based apps (e.g. VS Code) publish public.url instead of
            // public.file-url; path_from_file_url keeps non-file URLs out
            item.stringForType(unsafe { objc2_app_kit::NSPasteboardTypeFileURL })
                .or_else(|| item.stringForType(unsafe { objc2_app_kit::NSPasteboardTypeURL }))
        })
        .filter_map(|url| path_from_file_url(&url.to_string()))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(any(target_os = "macos", test))]
fn path_from_file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;

    // A remote authority (file://server/...) must not resolve to a local path
    let path_part = match rest.find('/') {
        Some(idx) if matches!(&rest[..idx], "" | "localhost") => &rest[idx..],
        _ => return None,
    };

    let decoded = percent_encoding::percent_decode_str(path_part)
        .decode_utf8()
        .ok()?;

    Some(PathBuf::from(decoded.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_url_is_percent_decoded() {
        assert_eq!(
            path_from_file_url("file:///Users/alice/My%20Notes/todo.md"),
            Some(PathBuf::from("/Users/alice/My Notes/todo.md"))
        );
    }

    #[test]
    fn test_file_url_with_localhost_authority() {
        assert_eq!(
            path_from_file_url("file://localhost/Users/alice/todo.md"),
            Some(PathBuf::from("/Users/alice/todo.md"))
        );
    }

    #[test]
    fn test_file_url_with_remote_authority_is_rejected() {
        assert_eq!(
            path_from_file_url("file://example.com/Users/alice/todo.md"),
            None
        );
    }

    #[test]
    fn test_file_url_without_path_is_rejected() {
        assert_eq!(path_from_file_url("file://"), None);
        assert_eq!(path_from_file_url("file://localhost"), None);
    }

    #[test]
    fn test_non_file_url_is_rejected() {
        assert_eq!(path_from_file_url("https://example.com/a.md"), None);
        assert_eq!(path_from_file_url("/Users/alice/todo.md"), None);
    }
}
