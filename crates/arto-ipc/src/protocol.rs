use arto_config::{FileOpenBehavior, Theme};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A window's top-left corner in screen coordinates, in logical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPoint {
    pub x: i32,
    pub y: i32,
}

/// A window's size in logical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowExtent {
    pub width: u32,
    pub height: u32,
}

/// What a launch asks of the window it lands in, beyond what to read.
///
/// Every field is absent by default, and an absent field leaves the running
/// instance's own answer — the preferences, or the window as it already
/// stands — untouched. Automation is what these exist for: placing a window
/// where a screen capture expects it, and pinning the theme so the capture
/// does not change with the time of day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<WindowPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<WindowExtent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

impl WindowOptions {
    /// Whether the launch asked for nothing at all, in which case the window
    /// is left exactly as the running instance would have made it.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// What the *other* windows of one launch inherit.
    ///
    /// A launch naming several files gives each one a window. The theme is
    /// how this invocation's windows should look, so all of them take it;
    /// a position and a size name one place and one shape, and a place holds
    /// one window — stacking the rest exactly on top of the first would hide
    /// them behind it.
    pub fn without_geometry(&self) -> Self {
        Self {
            position: None,
            size: None,
            theme: self.theme,
        }
    }
}

/// What a launch asks the running instance to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    #[serde(default)]
    pub files: Vec<PathBuf>,
    pub directory: Option<PathBuf>,
    /// Which window should receive the request; `None` leaves the choice
    /// to the running instance's configuration.
    pub behavior: Option<FileOpenBehavior>,
    /// Leave the frontmost app frontmost: apply the request without
    /// activating Arto or moving the keyboard focus.
    #[serde(default)]
    pub behind: bool,
    /// Geometry and theme for the window this request lands in.
    #[serde(default)]
    pub window: WindowOptions,
}

/// A request in the form the running instance handles: the wire messages,
/// legacy variants included, all normalize to one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenEvent {
    /// Open files and/or change root directory in a target window.
    Open(OpenRequest),
    /// Bring the app forward without opening anything (app icon clicked,
    /// or a launch with no paths).
    Reopen {
        behavior: Option<FileOpenBehavior>,
        behind: bool,
        window: WindowOptions,
    },
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
        #[serde(default)]
        behind: bool,
        #[serde(default, skip_serializing_if = "WindowOptions::is_empty")]
        window: WindowOptions,
        /// Hold the connection open until the target window has drawn what
        /// this request asked for, and answer with [`ReadyReply`] then.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        wait_ready: bool,
    },
    /// Reopen/activate the application (no paths provided).
    Reopen {
        #[serde(default)]
        behavior: Option<FileOpenBehavior>,
        #[serde(default)]
        behind: bool,
        #[serde(default, skip_serializing_if = "WindowOptions::is_empty")]
        window: WindowOptions,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        wait_ready: bool,
    },
}

/// The one line the primary writes back, and only to a launch that asked to
/// wait: the target window has drawn what the request asked for.
///
/// A reply travels the other way down the same connection, so it carries its
/// own `type` tag rather than reusing [`IpcMessage`]'s — a future reply of a
/// different kind is then a variant here, not a message the primary would
/// have to distinguish from a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReadyReply {
    /// The window drew the request.
    Ready,
    /// The primary gave up waiting; the request itself was still applied.
    Timeout,
}

impl IpcMessage {
    /// The wire form of `event`, with the transport's own `wait_ready` flag.
    ///
    /// Waiting is not part of the event: it says what the *connection* does
    /// after the request is handed over, which is the sender's business and
    /// none of the app's.
    pub fn from_event(event: OpenEvent, wait_ready: bool) -> Self {
        match event {
            OpenEvent::Open(request) => IpcMessage::Open {
                files: request.files,
                directory: request.directory,
                behavior: request.behavior,
                behind: request.behind,
                window: request.window,
                wait_ready,
            },
            OpenEvent::Reopen {
                behavior,
                behind,
                window,
            } => IpcMessage::Reopen {
                behavior,
                behind,
                window,
                wait_ready,
            },
        }
    }

    /// Whether the sender is holding the connection open for a reply.
    pub fn wait_ready(&self) -> bool {
        match self {
            IpcMessage::File { .. } | IpcMessage::Directory { .. } => false,
            IpcMessage::Open { wait_ready, .. } | IpcMessage::Reopen { wait_ready, .. } => {
                *wait_ready
            }
        }
    }

    /// Normalize into the event the running instance handles.
    pub fn into_open_event(self) -> OpenEvent {
        match self {
            IpcMessage::File { path } => OpenEvent::Open(OpenRequest {
                files: vec![path],
                directory: None,
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            }),
            IpcMessage::Directory { path } => OpenEvent::Open(OpenRequest {
                files: Vec::new(),
                directory: Some(path),
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            }),
            IpcMessage::Open {
                files,
                directory,
                behavior,
                behind,
                window,
                wait_ready: _,
            } => OpenEvent::Open(OpenRequest {
                files,
                directory,
                behavior,
                behind,
                window,
            }),
            IpcMessage::Reopen {
                behavior,
                behind,
                window,
                wait_ready: _,
            } => OpenEvent::Reopen {
                behavior,
                behind,
                window,
            },
        }
    }
}

impl From<OpenEvent> for IpcMessage {
    fn from(event: OpenEvent) -> Self {
        Self::from_event(event, false)
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
            behind: false,
            window: WindowOptions::default(),
            wait_ready: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused","behind":false}"#
        );
    }

    #[test]
    fn a_launch_that_asks_for_nothing_writes_the_line_it_always_wrote() {
        // Geometry, theme and the ready wait are all absent by default and
        // are left out of the line entirely, so a primary from before they
        // existed reads exactly what it used to.
        let msg = IpcMessage::Reopen {
            behavior: None,
            behind: false,
            window: WindowOptions::default(),
            wait_ready: false,
        };
        assert_eq!(
            serde_json::to_string(&msg).unwrap(),
            r#"{"type":"reopen","behavior":null,"behind":false}"#
        );
    }

    #[test]
    fn window_options_round_trip_through_the_wire_format() {
        let msg = IpcMessage::Open {
            files: Vec::new(),
            directory: None,
            behavior: Some(FileOpenBehavior::NewWindow),
            behind: false,
            window: WindowOptions {
                position: Some(WindowPoint { x: 120, y: 64 }),
                size: Some(WindowExtent {
                    width: 1400,
                    height: 920,
                }),
                theme: Some(Theme::Light),
            },
            wait_ready: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"type":"open","files":[],"directory":null,"behavior":"new_window","behind":false,"window":{"position":{"x":120,"y":64},"size":{"width":1400,"height":920},"theme":"light"},"wait_ready":true}"#
        );
        assert_eq!(serde_json::from_str::<IpcMessage>(&json).unwrap(), msg);
    }

    #[test]
    fn waiting_is_the_connections_business_and_not_the_events() {
        // The event the app handles says nothing about waiting; the flag
        // rides beside it on the wire and is read straight off the message.
        let event = OpenEvent::Reopen {
            behavior: None,
            behind: false,
            window: WindowOptions::default(),
        };
        let waiting = IpcMessage::from_event(event.clone(), true);
        assert!(waiting.wait_ready());
        assert_eq!(waiting.into_open_event(), event);

        let plain = IpcMessage::from_event(event.clone(), false);
        assert!(!plain.wait_ready());
        assert_eq!(plain.into_open_event(), event);
    }

    #[test]
    fn a_legacy_message_never_asks_to_wait() {
        let legacy: IpcMessage =
            serde_json::from_str(r#"{"type":"file","path":"/tmp/a.md"}"#).unwrap();
        assert!(!legacy.wait_ready());
    }

    #[test]
    fn the_other_windows_of_a_launch_take_the_theme_but_not_the_place() {
        let asked = WindowOptions {
            position: Some(WindowPoint { x: 120, y: 64 }),
            size: Some(WindowExtent {
                width: 1400,
                height: 920,
            }),
            theme: Some(Theme::Dark),
        };
        assert_eq!(
            asked.without_geometry(),
            WindowOptions {
                position: None,
                size: None,
                theme: Some(Theme::Dark),
            }
        );
    }

    #[test]
    fn a_launch_that_asked_for_nothing_passes_nothing_on() {
        assert!(WindowOptions::default().without_geometry().is_empty());
    }

    #[test]
    fn ready_replies_carry_their_own_tag() {
        assert_eq!(
            serde_json::to_string(&ReadyReply::Ready).unwrap(),
            r#"{"type":"ready"}"#
        );
        assert_eq!(
            serde_json::to_string(&ReadyReply::Timeout).unwrap(),
            r#"{"type":"timeout"}"#
        );
    }

    #[test]
    fn reopen_serializes_with_type_tag() {
        let msg = IpcMessage::Reopen {
            behavior: Some(FileOpenBehavior::LastFocused),
            behind: false,
            window: WindowOptions::default(),
            wait_ready: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"type":"reopen","behavior":"last_focused","behind":false}"#
        );
    }

    #[test]
    fn open_deserializes() {
        let json = r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused","behind":true}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            IpcMessage::Open {
                files: vec![PathBuf::from("/path/to/file.md")],
                directory: Some(PathBuf::from("/path/to/dir")),
                behavior: Some(FileOpenBehavior::LastFocused),
                behind: true,
                window: WindowOptions::default(),
                wait_ready: false,
            }
        );
    }

    #[test]
    fn open_without_behind_opens_in_front() {
        let open: IpcMessage = serde_json::from_str(
            r#"{"type":"open","files":["/a.md"],"directory":null,"behavior":null}"#,
        )
        .unwrap();
        assert_eq!(
            open,
            IpcMessage::Open {
                files: vec![PathBuf::from("/a.md")],
                directory: None,
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
                wait_ready: false,
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
                behavior: Some(FileOpenBehavior::LastFocused),
                behind: false,
                window: WindowOptions::default(),
                wait_ready: false,
            }
        );

        let legacy: IpcMessage = serde_json::from_str(r#"{"type":"reopen"}"#).unwrap();
        assert_eq!(
            legacy,
            IpcMessage::Reopen {
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
                wait_ready: false,
            }
        );
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
                behind: false,
                window: WindowOptions::default(),
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
                behind: false,
                window: WindowOptions::default(),
            })
        );
        assert_eq!(
            IpcMessage::Open {
                files: vec![PathBuf::from("/test.md")],
                directory: Some(PathBuf::from("/test/dir")),
                behavior: Some(FileOpenBehavior::CurrentScreen),
                behind: true,
                window: WindowOptions::default(),
                wait_ready: false,
            }
            .into_open_event(),
            OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/test.md")],
                directory: Some(PathBuf::from("/test/dir")),
                behavior: Some(FileOpenBehavior::CurrentScreen),
                behind: true,
                window: WindowOptions::default(),
            })
        );
        assert_eq!(
            IpcMessage::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused),
                behind: true,
                window: WindowOptions::default(),
                wait_ready: false,
            }
            .into_open_event(),
            OpenEvent::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused),
                behind: true,
                window: WindowOptions::default(),
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
                behind: false,
                window: WindowOptions::default(),
            }),
            OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/a.md")],
                directory: None,
                behavior: None,
                behind: true,
                window: WindowOptions::default(),
            }),
            OpenEvent::Reopen {
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            },
            OpenEvent::Reopen {
                behavior: None,
                behind: true,
                window: WindowOptions::default(),
            },
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
            {"type":"open","files":["/file1.md"],"directory":null,"behavior":"last_focused","behind":false}
            {"type":"open","files":[],"directory":"/dir","behavior":"new_window","behind":true}
            {"type":"reopen","behavior":"current_screen","behind":false}
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
                    behind: false,
                    window: WindowOptions::default(),
                    wait_ready: false,
                },
                IpcMessage::Open {
                    files: Vec::new(),
                    directory: Some(PathBuf::from("/dir")),
                    behavior: Some(FileOpenBehavior::NewWindow),
                    behind: true,
                    window: WindowOptions::default(),
                    wait_ready: false,
                },
                IpcMessage::Reopen {
                    behavior: Some(FileOpenBehavior::CurrentScreen),
                    behind: false,
                    window: WindowOptions::default(),
                    wait_ready: false,
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
