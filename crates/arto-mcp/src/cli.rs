//! `arto mcp`: the arguments, and running the server on standard I/O.

use crate::instance::Local;
use crate::roots::Roots;
use crate::tools::Arto;
use arto_lsp::PeerInfo;
use clap::Args;
use rmcp::model::Implementation;
use rmcp::ServiceExt;
use std::path::PathBuf;

/// Serve the Model Context Protocol on standard input and output, so an
/// agent can show the user files in Arto and read where they are.
#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    /// Let the agent open files and directories under DIR. Repeat it for
    /// more than one; given none, the directory this command runs in
    #[arg(long = "allow", value_name = "DIR")]
    pub allow: Vec<PathBuf>,
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
    let roots = Roots::new(args.allow.iter().cloned(), base);
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
