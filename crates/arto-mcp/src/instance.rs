//! The running Arto, as the tools reach it.

use arto_lsp::{AskError, Call, PeerInfo};
use std::path::{Path, PathBuf};

/// What the tools need of Arto: to be asked one thing, and to be started.
///
/// A trait so the tools can be tried against a stand-in; the one that
/// matters is [`Local`].
pub trait Instance: Send + Sync + 'static {
    /// Send one request of the socket protocol and hand back its answer.
    fn ask(&self, call: &Call) -> Result<serde_json::Value, AskError>;

    /// Start Arto with these paths, for when it is not running.
    fn launch(&self, paths: &[PathBuf], behind: bool) -> std::io::Result<()>;
}

/// The Arto running as this user on this machine.
#[derive(Debug, Clone)]
pub struct Local {
    client_info: PeerInfo,
    program: PathBuf,
}

impl Local {
    /// `program` is what to run to start Arto: the `arto` command this
    /// server was itself started as, so the app it starts is the same
    /// version as the server talking to it.
    pub fn new(client_info: PeerInfo, program: impl Into<PathBuf>) -> Self {
        Self {
            client_info,
            program: program.into(),
        }
    }
}

impl Instance for Local {
    fn ask(&self, call: &Call) -> Result<serde_json::Value, AskError> {
        arto_lsp::ask_existing_instance(self.client_info.clone(), call)
    }

    fn launch(&self, paths: &[PathBuf], behind: bool) -> std::io::Result<()> {
        launch_command(&self.program, paths, behind)
            .spawn()
            .map(drop)
    }
}

/// The command that starts Arto on `paths`, cut loose from this server.
///
/// Its standard streams are closed: this server's own are the MCP
/// connection, and an app that wrote a log line to them would corrupt it.
/// On Unix it leads a process group of its own, so the MCP client stopping
/// this server — which it may do by signalling the whole group — does not
/// close the windows the user is reading.
fn launch_command(program: &Path, paths: &[PathBuf], behind: bool) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    if behind {
        command.arg("--behind");
    }
    command
        .arg("--")
        .args(paths)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(command: &std::process::Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_launch_names_its_paths_after_the_flags() {
        // A file called `--theme=dark` is a file, not a flag.
        let command = launch_command(
            Path::new("arto"),
            &[PathBuf::from("/a/--theme=dark.md")],
            false,
        );
        assert_eq!(args(&command), ["--", "/a/--theme=dark.md"]);
    }

    #[test]
    fn a_launch_behind_keeps_the_application_the_user_is_in() {
        let command = launch_command(Path::new("arto"), &[PathBuf::from("/a.md")], true);
        assert_eq!(args(&command), ["--behind", "--", "/a.md"]);
    }
}
