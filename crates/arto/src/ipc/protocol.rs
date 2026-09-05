use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    #[serde(default)]
    pub files: Vec<PathBuf>,
    pub directory: Option<PathBuf>,
    pub behavior: Option<crate::config::FileOpenBehavior>,
}

/// Open event types sent from CLI/Finder/IPC into the main-thread queue.
#[derive(Debug, Clone)]
pub enum OpenEvent {
    /// Open files and/or change root directory in a target window.
    Open(OpenRequest),
    /// App icon clicked (reopen event).
    Reopen {
        behavior: Option<crate::config::FileOpenBehavior>,
    },
}

/// IPC message types sent between instances as JSON Lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum IpcMessage {
    /// Legacy: open one file.
    ///
    /// Kept for backward compatibility so newer versions can still accept
    /// messages from an older secondary instance during rolling upgrades.
    File { path: PathBuf },
    /// Legacy: open one directory as root.
    ///
    /// Kept for backward compatibility so newer versions can still accept
    /// messages from an older secondary instance during rolling upgrades.
    Directory { path: PathBuf },
    /// Open files and/or set root directory.
    Open {
        #[serde(default)]
        files: Vec<PathBuf>,
        directory: Option<PathBuf>,
        behavior: Option<crate::config::FileOpenBehavior>,
    },
    /// Reopen/activate the application (no paths provided).
    Reopen {
        #[serde(default)]
        behavior: Option<crate::config::FileOpenBehavior>,
    },
}

impl IpcMessage {
    /// Convert to OpenEvent for internal use.
    pub(super) fn into_open_event(self) -> OpenEvent {
        match self {
            IpcMessage::File { path } => OpenEvent::Open(OpenRequest {
                files: vec![path],
                directory: None,
                behavior: None,
            }),
            IpcMessage::Directory { path } => OpenEvent::Open(OpenRequest {
                files: Vec::new(),
                directory: Some(path),
                behavior: None,
            }),
            IpcMessage::Open {
                files,
                directory,
                behavior,
            } => OpenEvent::Open(OpenRequest {
                files,
                directory,
                behavior,
            }),
            IpcMessage::Reopen { behavior } => OpenEvent::Reopen { behavior },
        }
    }

    pub(super) fn from_open_request(request: OpenRequest) -> Self {
        Self::Open {
            files: request.files,
            directory: request.directory,
            behavior: request.behavior,
        }
    }
}

/// Build an OpenRequest from CLI invocation.
///
/// Path handling:
/// - files are collected into `files`
/// - first directory is used as root (unless `--directory` is provided)
/// - invalid paths are skipped
pub fn build_open_request(invocation: &crate::cli::CliInvocation) -> Option<OpenRequest> {
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

fn canonicalize(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn canonicalize_directory(path: impl AsRef<Path>) -> Option<PathBuf> {
    let canonical = canonicalize(path.as_ref());
    if canonical.is_dir() {
        Some(canonical)
    } else {
        tracing::warn!(
            path = ?path.as_ref(),
            "Skipping --directory because it is not a valid directory"
        );
        None
    }
}

enum PathKind {
    File(PathBuf),
    Directory(PathBuf),
}

fn classify_path(path: impl AsRef<Path>) -> Option<PathKind> {
    let canonical = canonicalize(path.as_ref());
    if canonical.is_file() {
        return Some(PathKind::File(canonical));
    }
    if canonical.is_dir() {
        return Some(PathKind::Directory(canonical));
    }
    None
}

/// Result of trying to send paths to an existing instance.
pub enum SendResult {
    /// Successfully sent paths to existing instance - caller should exit
    Sent,
    /// No existing instance found - caller should become primary
    NoExistingInstance,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CliInvocation, CliOpenMode};

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
        assert_eq!(
            request.behavior,
            Some(crate::config::FileOpenBehavior::LastFocused)
        );
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
        assert_eq!(
            request.behavior,
            Some(crate::config::FileOpenBehavior::CurrentScreen)
        );
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
    fn ipc_message_file_deserializes_for_backward_compatibility() {
        let json = r#"{"type":"file","path":"/tmp/a.md"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::File { path } if path == Path::new("/tmp/a.md")));
    }

    #[test]
    fn ipc_message_directory_deserializes_for_backward_compatibility() {
        let json = r#"{"type":"directory","path":"/tmp/docs"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::Directory { path } if path == Path::new("/tmp/docs")));
    }

    #[test]
    fn ipc_message_file_maps_to_open_event() {
        let event = IpcMessage::File {
            path: PathBuf::from("/tmp/a.md"),
        }
        .into_open_event();

        assert!(matches!(
            event,
            OpenEvent::Open(OpenRequest { files, directory: None, behavior: None })
            if files == vec![PathBuf::from("/tmp/a.md")]
        ));
    }
}
