//! Turning what the user pointed at into an [`OpenEvent`].
//!
//! Paths arrive from the command line and from the OS (Finder, file
//! associations). Both are canonicalized and sorted into files and
//! directories here; the protocol crate only knows the result.

use super::{OpenEvent, OpenRequest};
use crate::cli::CliInvocation;
use arto_ipc::{classify_path, PathKind};
use std::path::Path;

/// The event a CLI invocation asks for: an open request when it names any
/// valid file or directory, otherwise a plain reopen.
pub fn open_event_for_invocation(invocation: &CliInvocation) -> OpenEvent {
    match build_open_request(invocation) {
        Some(request) => OpenEvent::Open(request),
        None => OpenEvent::Reopen {
            behavior: invocation.open_mode.to_file_open_behavior(),
        },
    }
}

/// Build an OpenRequest from CLI invocation.
///
/// Path handling:
/// - files are collected into `files`
/// - first directory is used as root (unless `--directory` is provided)
/// - invalid paths are skipped
pub fn build_open_request(invocation: &CliInvocation) -> Option<OpenRequest> {
    let mut directory = invocation
        .directory
        .as_ref()
        .and_then(canonicalize_directory);

    let mut files = Vec::new();

    for path in &invocation.paths {
        match classify_path(path) {
            Some(PathKind::File(canonical)) => files.push(canonical),
            Some(PathKind::Directory(canonical)) => {
                if directory.is_none() {
                    directory = Some(canonical);
                }
            }
            None => tracing::warn!(?path, "Skipping invalid path (not a file or directory)"),
        }
    }

    if files.is_empty() && directory.is_none() {
        return None;
    }

    Some(OpenRequest {
        files,
        directory,
        behavior: invocation.open_mode.to_file_open_behavior(),
    })
}

/// Validate and categorize a path from non-CLI sources (e.g., Finder).
///
/// Finder events do not override `fileOpen`, so behavior is `None`.
pub fn validate_path(path: impl AsRef<Path>) -> Option<OpenEvent> {
    match classify_path(path.as_ref()) {
        Some(PathKind::File(canonical)) => Some(OpenEvent::Open(OpenRequest {
            files: vec![canonical],
            directory: None,
            behavior: None,
        })),
        Some(PathKind::Directory(canonical)) => Some(OpenEvent::Open(OpenRequest {
            files: Vec::new(),
            directory: Some(canonical),
            behavior: None,
        })),
        None => {
            tracing::warn!(
                path = ?path.as_ref(),
                "Skipping invalid path (not a file or directory)"
            );
            None
        }
    }
}

fn canonicalize_directory(path: impl AsRef<Path>) -> Option<std::path::PathBuf> {
    match classify_path(path.as_ref()) {
        Some(PathKind::Directory(canonical)) => Some(canonical),
        _ => {
            tracing::warn!(
                path = ?path.as_ref(),
                "Skipping --directory because it is not a valid directory"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliOpenMode;
    use crate::config::FileOpenBehavior;
    use std::path::PathBuf;

    #[test]
    fn build_open_request_uses_positional_directory_when_directory_option_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("docs");
        let file = temp.path().join("README.md");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(&file, "# test").unwrap();

        let invocation = CliInvocation {
            paths: vec![directory.clone(), file.clone()],
            directory: None,
            open_mode: CliOpenMode::LastFocused,
        };

        let request = build_open_request(&invocation).unwrap();
        assert_eq!(request.files, vec![file.canonicalize().unwrap()]);
        assert_eq!(request.directory, Some(directory.canonicalize().unwrap()));
        assert_eq!(request.behavior, Some(FileOpenBehavior::LastFocused));
    }

    #[test]
    fn build_open_request_prefers_directory_option_over_positional_directory() {
        let temp = tempfile::tempdir().unwrap();
        let option_directory = temp.path().join("option");
        let positional_directory = temp.path().join("positional");
        std::fs::create_dir_all(&option_directory).unwrap();
        std::fs::create_dir_all(&positional_directory).unwrap();

        let invocation = CliInvocation {
            paths: vec![positional_directory.clone()],
            directory: Some(option_directory.clone()),
            open_mode: CliOpenMode::LastFocused,
        };

        let request = build_open_request(&invocation).unwrap();
        assert_eq!(request.files, Vec::<PathBuf>::new());
        assert_eq!(
            request.directory,
            Some(option_directory.canonicalize().unwrap())
        );
    }

    #[test]
    fn build_open_request_maps_open_mode_to_behavior() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("README.md");
        std::fs::write(&file, "# test").unwrap();

        let invocation = CliInvocation {
            paths: vec![file],
            directory: None,
            open_mode: CliOpenMode::CurrentScreen,
        };

        let request = build_open_request(&invocation).unwrap();
        assert_eq!(request.behavior, Some(FileOpenBehavior::CurrentScreen));
    }

    #[test]
    fn build_open_request_maps_config_mode_to_none_behavior() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("README.md");
        std::fs::write(&file, "# test").unwrap();

        let invocation = CliInvocation {
            paths: vec![file],
            directory: None,
            open_mode: CliOpenMode::Config,
        };

        let request = build_open_request(&invocation).unwrap();
        assert_eq!(request.behavior, None);
    }

    #[test]
    fn open_event_for_invocation_falls_back_to_reopen_without_paths() {
        let invocation = CliInvocation {
            paths: Vec::new(),
            directory: None,
            open_mode: CliOpenMode::NewWindow,
        };

        assert_eq!(
            open_event_for_invocation(&invocation),
            OpenEvent::Reopen {
                behavior: Some(FileOpenBehavior::NewWindow)
            }
        );
    }

    #[test]
    fn validate_path_rejects_missing_paths() {
        let temp = tempfile::tempdir().unwrap();
        assert!(validate_path(temp.path().join("missing.md")).is_none());
    }
}
