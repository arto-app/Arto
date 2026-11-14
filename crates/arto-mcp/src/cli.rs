//! `arto mcp`: the arguments, and running the server on standard I/O.

use crate::instance::Local;
use crate::roots::Roots;
use crate::tools::Arto;
use arto_lsp::PeerInfo;
use clap::Args;
use rmcp::model::Implementation;
use rmcp::ServiceExt;
use std::ffi::OsString;
use std::path::PathBuf;

/// Serve the Model Context Protocol on standard input and output, so an
/// agent can show the user files in Arto and read where they are.
#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    /// Let the agent open files and directories under DIR. Repeat it for
    /// more than one; given none, the directories ARTO_MCP_ALLOW lists, as
    /// PATH does, or else the directory this command runs in
    #[arg(long = "allow", value_name = "DIR")]
    pub allow: Vec<PathBuf>,
}

/// The variable a client that cannot change the command line names the
/// allowed directories in.
pub const ALLOW_VARIABLE: &str = "ARTO_MCP_ALLOW";

fn allowed(flags: &[PathBuf], variable: Option<OsString>) -> Vec<PathBuf> {
    if !flags.is_empty() {
        return flags.to_vec();
    }
    variable
        .map(|list| {
            std::env::split_paths(&list)
                .filter(|dir| !dir.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Why the server stopped other than by its client leaving.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("could not find the directory this command runs in: {0}")]
    WorkingDirectory(#[source] std::io::Error),
    #[error("could not find this program, to start Arto with: {0}")]
    Program(#[source] std::io::Error),
    #[error("could not start the async runtime: {0}")]
    Runtime(#[source] std::io::Error),
    #[error("the MCP session failed: {0}")]
    Session(String),
}

/// Serve until the client closes standard input.
///
/// `client_info` is what this server reports of itself, both to the MCP
/// client and to the running Arto — the app's name and build, which this
/// crate does not know.
pub fn run(args: &McpArgs, client_info: PeerInfo) -> Result<(), McpError> {
    let base = std::env::current_dir().map_err(McpError::WorkingDirectory)?;
    let roots = Roots::new(allowed(&args.allow, std::env::var_os(ALLOW_VARIABLE)), base);
    let program = std::env::current_exe().map_err(McpError::Program)?;

    let server_info = Implementation::new(
        client_info.name.clone(),
        client_info.version.clone().unwrap_or_default(),
    );
    let server = Arto::new(Local::new(client_info, program), roots, server_info);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(McpError::Runtime)?;
    runtime.block_on(async move {
        let service = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|error| McpError::Session(error.to_string()))?;
        service
            .waiting()
            .await
            .map_err(|error| McpError::Session(error.to_string()))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(paths: &[&str]) -> OsString {
        std::env::join_paths(paths).unwrap()
    }

    #[test]
    fn the_variable_names_the_directories_when_no_flag_does() {
        let dirs = allowed(&[], Some(joined(&["/notes", "/src"])));
        assert_eq!(dirs, [PathBuf::from("/notes"), PathBuf::from("/src")]);
    }

    #[test]
    fn a_flag_overrides_the_variable() {
        let dirs = allowed(&[PathBuf::from("/flag")], Some(joined(&["/notes"])));
        assert_eq!(dirs, [PathBuf::from("/flag")]);
    }

    #[test]
    fn empty_entries_in_the_variable_name_nothing() {
        let dirs = allowed(&[], Some(joined(&["", "/notes", ""])));
        assert_eq!(dirs, [PathBuf::from("/notes")]);
    }

    #[test]
    fn neither_names_nothing() {
        assert!(allowed(&[], None).is_empty());
        assert!(allowed(&[], Some(OsString::new())).is_empty());
    }
}
