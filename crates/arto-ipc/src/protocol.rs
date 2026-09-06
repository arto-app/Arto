use arto_config::FileOpenBehavior;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a launch asks the running instance to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    #[serde(default)]
    pub files: Vec<PathBuf>,
    pub directory: Option<PathBuf>,
    /// Which window should receive the request; `None` leaves the choice
    /// to the running instance's configuration.
    pub behavior: Option<FileOpenBehavior>,
}

/// A request in the form the running instance handles: the wire messages,
/// legacy variants included, all normalize to one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenEvent {
    /// Open files and/or change root directory in a target window.
    Open(OpenRequest),
    /// Bring the app forward without opening anything (app icon clicked,
    /// or a launch with no paths).
    Reopen { behavior: Option<FileOpenBehavior> },
}

/// One line of the JSON Lines protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
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
        behavior: Option<FileOpenBehavior>,
    },
    /// Reopen/activate the application (no paths provided).
    Reopen {
        #[serde(default)]
        behavior: Option<FileOpenBehavior>,
    },
}

impl IpcMessage {
    /// Normalize into the event the running instance handles.
    pub fn into_open_event(self) -> OpenEvent {
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
}

impl From<OpenEvent> for IpcMessage {
    fn from(event: OpenEvent) -> Self {
        match event {
            OpenEvent::Open(request) => IpcMessage::Open {
                files: request.files,
                directory: request.directory,
                behavior: request.behavior,
            },
            OpenEvent::Reopen { behavior } => IpcMessage::Reopen { behavior },
        }
    }
}

/// A path that exists, canonicalized and sorted by what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathKind {
    File(PathBuf),
    Directory(PathBuf),
}

/// Canonicalize a path and say whether it is a file or a directory.
///
/// Returns `None` for anything else (missing, unreadable, special files).
/// Canonicalizing first matters on macOS, where `/tmp` and friends are
/// symlinks and two spellings of one file must compare equal.
pub fn classify_path(path: impl AsRef<Path>) -> Option<PathKind> {
    let path = path.as_ref();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if canonical.is_file() {
        return Some(PathKind::File(canonical));
    }
    if canonical.is_dir() {
        return Some(PathKind::Directory(canonical));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn open_serializes_with_type_tag() {
        let msg = IpcMessage::Open {
            files: vec![PathBuf::from("/path/to/file.md")],
            directory: Some(PathBuf::from("/path/to/dir")),
            behavior: Some(FileOpenBehavior::LastFocused),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused"}"#
        );
    }

    #[test]
    fn reopen_serializes_with_type_tag() {
        let msg = IpcMessage::Reopen {
            behavior: Some(FileOpenBehavior::LastFocused),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"reopen","behavior":"last_focused"}"#);
    }

    #[test]
    fn open_deserializes() {
        let json = r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            IpcMessage::Open {
                files: vec![PathBuf::from("/path/to/file.md")],
                directory: Some(PathBuf::from("/path/to/dir")),
                behavior: Some(FileOpenBehavior::LastFocused),
            }
        );
    }

    #[test]
    fn reopen_deserializes_with_and_without_behavior() {
        let msg: IpcMessage =
            serde_json::from_str(r#"{"type":"reopen","behavior":"last_focused"}"#).unwrap();
        assert_eq!(
            msg,
            IpcMessage::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused)
            }
        );

        let legacy: IpcMessage = serde_json::from_str(r#"{"type":"reopen"}"#).unwrap();
        assert_eq!(legacy, IpcMessage::Reopen { behavior: None });
    }

    #[test]
    fn legacy_file_and_directory_messages_still_parse() {
        let file: IpcMessage =
            serde_json::from_str(r#"{"type":"file","path":"/tmp/a.md"}"#).unwrap();
        assert_eq!(
            file,
            IpcMessage::File {
                path: PathBuf::from("/tmp/a.md")
            }
        );

        let directory: IpcMessage =
            serde_json::from_str(r#"{"type":"directory","path":"/tmp/docs"}"#).unwrap();
        assert_eq!(
            directory,
            IpcMessage::Directory {
                path: PathBuf::from("/tmp/docs")
            }
        );
    }

    #[test]
    fn every_message_normalizes_to_an_open_event() {
        assert_eq!(
            IpcMessage::File {
                path: PathBuf::from("/tmp/a.md")
            }
            .into_open_event(),
            OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/tmp/a.md")],
                directory: None,
                behavior: None,
            })
        );
        assert_eq!(
            IpcMessage::Directory {
                path: PathBuf::from("/tmp/docs")
            }
            .into_open_event(),
            OpenEvent::Open(OpenRequest {
                files: Vec::new(),
                directory: Some(PathBuf::from("/tmp/docs")),
                behavior: None,
            })
        );
        assert_eq!(
            IpcMessage::Open {
                files: vec![PathBuf::from("/test.md")],
                directory: Some(PathBuf::from("/test/dir")),
                behavior: Some(FileOpenBehavior::CurrentScreen),
            }
            .into_open_event(),
            OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/test.md")],
                directory: Some(PathBuf::from("/test/dir")),
                behavior: Some(FileOpenBehavior::CurrentScreen),
            })
        );
        assert_eq!(
            IpcMessage::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused)
            }
            .into_open_event(),
            OpenEvent::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused)
            }
        );
    }

    #[test]
    fn open_event_round_trips_through_the_wire_format() {
        let events = [
            OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/a.md")],
                directory: Some(PathBuf::from("/dir")),
                behavior: Some(FileOpenBehavior::NewWindow),
            }),
            OpenEvent::Reopen { behavior: None },
        ];
        for event in events {
            let json = serde_json::to_string(&IpcMessage::from(event.clone())).unwrap();
            let back: IpcMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(back.into_open_event(), event);
        }
    }

    #[test]
    fn json_lines_parse_one_message_per_line() {
        let input = indoc! {r#"
            {"type":"open","files":["/file1.md"],"directory":null,"behavior":"last_focused"}
            {"type":"open","files":[],"directory":"/dir","behavior":"new_window"}
            {"type":"reopen","behavior":"current_screen"}
        "#};

        let messages: Vec<IpcMessage> = input
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(
            messages,
            vec![
                IpcMessage::Open {
                    files: vec![PathBuf::from("/file1.md")],
                    directory: None,
                    behavior: Some(FileOpenBehavior::LastFocused),
                },
                IpcMessage::Open {
                    files: Vec::new(),
                    directory: Some(PathBuf::from("/dir")),
                    behavior: Some(FileOpenBehavior::NewWindow),
                },
                IpcMessage::Reopen {
                    behavior: Some(FileOpenBehavior::CurrentScreen),
                },
            ]
        );
    }

    #[test]
    fn classify_path_sorts_files_and_directories() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("docs");
        let file = temp.path().join("README.md");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(&file, "# test").unwrap();

        assert_eq!(
            classify_path(&file),
            Some(PathKind::File(file.canonicalize().unwrap()))
        );
        assert_eq!(
            classify_path(&directory),
            Some(PathKind::Directory(directory.canonicalize().unwrap()))
        );
        assert_eq!(classify_path(temp.path().join("missing")), None);
    }
}
