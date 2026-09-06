use arto::cli::{CliInvocation, CliOpenMode};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

const VERSION: &str = concat!(
    env!("ARTO_BUILD_VERSION"),
    " (",
    compile_time::datetime_str!(),
    ")",
);

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OpenModeArg {
    /// Reuse a visible window on the cursor's current screen
    Screen,
    /// Always create a new window
    New,
}

/// Arto — the Art of Reading Markdown
#[derive(Parser, Debug)]
#[command(
    version = VERSION,
    about,
    long_about = "Arto — the Art of Reading Markdown\n\n\
        A local app that faithfully recreates GitHub-style Markdown rendering\n\
        for a beautiful reading experience.\n\n\
        Arto runs as a single instance — if already running, paths are sent\n\
        to the existing process instead of launching a new one.",
    after_long_help = "Examples:\n\
        \x20 arto                     Launch Arto (shows welcome screen)\n\
        \x20 arto README.md           Open a specific file\n\
        \x20 arto --open=screen README.md\n\
        \x20 arto --open=new README.md\n\
        \x20 arto --directory=. README.md\n\
        \x20 arto docs/               Open a directory in the file explorer\n\
        \x20 arto file1.md file2.md   Open multiple files in tabs\n\
        \x20 arto page README.md      Print README.md as a self-contained HTML page",
    // Subcommands and the open-paths form are exclusive, so `arto page` is
    // never mistaken for a request to open a file called `page` (use
    // `arto ./page` for that).
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Open target selection mode (default: use fileOpen setting from config.json)
    #[arg(long, value_enum)]
    open: Option<OpenModeArg>,
    /// Root directory for the file explorer sidebar
    #[arg(long)]
    directory: Option<PathBuf>,
    /// Files or directories to open
    #[arg()]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Render a Markdown file into a self-contained HTML page
    Page(arto_page::cli::PageArgs),
}

fn main() {
    // Re-exec with the canonical path if launched via a symlink.
    //
    // On macOS, `current_exe()` uses `_NSGetExecutablePath` which may return the
    // symlink path (e.g., /opt/homebrew/bin/arto) instead of the real binary
    // inside the .app bundle. Dioxus's asset resolver (`get_asset_root()`) then
    // computes the wrong Resources directory, causing CSS/JS to fail to load.
    //
    // Linux is unaffected because `current_exe()` reads `/proc/self/exe` which
    // always resolves symlinks.
    //
    // See: https://github.com/arto-app/Arto/issues/121
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(canonical) = exe.canonicalize() {
                if exe != canonical {
                    let err = std::process::Command::new(&canonical)
                        .args(std::env::args_os().skip(1))
                        .exec();
                    eprintln!(
                        "Failed to re-exec with canonical path (from {} to {}): {err}",
                        exe.display(),
                        canonical.display(),
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    let cli = Cli::parse();

    // Subcommands run to completion here, before any of the single-instance
    // machinery: rendering a page must not be forwarded to a running Arto.
    if let Some(Command::Page(args)) = cli.command {
        if let Err(err) = arto_page::cli::run(&args) {
            // `{:#}` on an anyhow error prints the whole cause chain.
            eprintln!("arto page: {:#}", anyhow::Error::from(err));
            std::process::exit(1);
        }
        return;
    }

    let open_mode = match cli.open {
        Some(OpenModeArg::Screen) => CliOpenMode::CurrentScreen,
        Some(OpenModeArg::New) => CliOpenMode::NewWindow,
        None => CliOpenMode::Config,
    };

    let invocation = CliInvocation {
        paths: cli.paths,
        directory: cli.directory,
        open_mode,
    };

    if let arto::RunResult::SentToExistingInstance = arto::run(invocation) {
        std::process::exit(0);
    }
}
