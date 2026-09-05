use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOpenMode {
    /// Use the behavior from config.json (fileOpen setting).
    Config,
    LastFocused,
    CurrentScreen,
    NewWindow,
}

impl CliOpenMode {
    pub(crate) fn to_file_open_behavior(self) -> Option<crate::config::FileOpenBehavior> {
        match self {
            Self::Config => None,
            Self::LastFocused => Some(crate::config::FileOpenBehavior::LastFocused),
            Self::CurrentScreen => Some(crate::config::FileOpenBehavior::CurrentScreen),
            Self::NewWindow => Some(crate::config::FileOpenBehavior::NewWindow),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliInvocation {
    pub paths: Vec<PathBuf>,
    pub directory: Option<PathBuf>,
    pub open_mode: CliOpenMode,
}
