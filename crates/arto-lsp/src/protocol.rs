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
    ///
    /// Left out of the JSON entirely when the launch asked for none of it,
    /// so the request a plain `arto README.md` sends is the object it has
    /// always been — this struct is written to the wire directly, flattened
    /// into the parameters of `arto/open`.
    #[serde(default, skip_serializing_if = "WindowOptions::is_empty")]
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
    use arto_config::Theme;

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
    fn window_options_are_left_out_of_a_request_that_named_none() {
        // `OpenRequest` is written to the wire directly, flattened into the
        // parameters of `arto/open`, so an empty object here would be an
        // empty object there.
        let request = OpenRequest {
            files: vec![PathBuf::from("/a.md")],
            directory: None,
            behavior: None,
            behind: false,
            window: WindowOptions::default(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("window"), "{json}");
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
