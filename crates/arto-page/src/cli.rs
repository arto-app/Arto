//! Command-line surface shared by the `arto-page` binary and `arto page`.
//!
//! The arguments are defined once here so both entry points accept exactly
//! the same flags. Errors are reported as [`PageError`]; the binaries decide
//! how to print them.

use crate::{render_file, PageError, PageOptions, RenderOptions};
use clap::Args;
use std::io::Write;
use std::path::PathBuf;

/// Render a Markdown file into a self-contained HTML page.
#[derive(Debug, Clone, Args)]
pub struct PageArgs {
    /// Markdown file to render
    pub input: PathBuf,
    /// Write the page to this file instead of standard output
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Omit the Content-Security-Policy that blocks scripts embedded in the
    /// Markdown; only for input you trust
    #[arg(long)]
    pub no_csp: bool,
    /// Leave bare URLs as plain text instead of turning them into links
    #[arg(long)]
    pub no_auto_link_urls: bool,
}

impl PageArgs {
    /// The rendering options these arguments describe.
    pub fn options(&self) -> PageOptions {
        PageOptions {
            render: RenderOptions {
                auto_link_urls: !self.no_auto_link_urls,
            },
            content_security_policy: !self.no_csp,
        }
    }
}

/// Render `args.input` and write the page to `args.output` or standard output.
pub fn run(args: &PageArgs) -> Result<(), PageError> {
    let html = render_file(&args.input, &args.options())?;
    match &args.output {
        Some(output) => std::fs::write(output, html).map_err(|source| PageError::Write {
            path: output.clone(),
            source,
        }),
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(html.as_bytes())
                .and_then(|()| stdout.flush())
                .map_err(PageError::WriteStdout)
        }
    }
}
