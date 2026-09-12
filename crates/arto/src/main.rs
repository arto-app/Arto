use arto::cli::{parse_position, parse_size, CliInvocation, CliOpenMode};
use arto_ipc::{WindowExtent, WindowOptions, WindowPoint};
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ThemeArg {
    /// Always light, whatever the system is set to
    Light,
    /// Always dark, whatever the system is set to
    Dark,
    /// Follow the system appearance
    System,
}

impl From<ThemeArg> for arto_config::Theme {
    fn from(arg: ThemeArg) -> Self {
        match arg {
            ThemeArg::Light => Self::Light,
            ThemeArg::Dark => Self::Dark,
            ThemeArg::System => Self::Auto,
        }
    }
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
        to the existing process instead of launching a new one.\n\n\
        --position, --size and --theme apply to the window --open selects,\n\
        whether that window is reused or created, and whether or not any\n\
        paths were named; --wait-ready then holds the command until that\n\
        window has drawn what it was given.",
    after_long_help = "Examples:\n\
        \x20 arto                     Launch Arto (shows welcome screen)\n\
        \x20 arto README.md           Open a specific file\n\
        \x20 arto --open=screen README.md\n\
        \x20 arto --open=new README.md\n\
        \x20 arto --behind README.md  Open without taking the focus\n\
        \x20 arto --directory=. README.md\n\
        \x20 arto --position=120,120 --size=1400,920 README.md\n\
        \x20 arto --theme=light --wait-ready README.md\n\
        \x20 arto --open=new --position=80,64 --size=1400,920 --theme=dark --wait-ready\n\
        \x20 arto docs/               Open a directory in the file explorer\n\
        \x20 arto file1.md file2.md   Open each file in its own window\n\
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
    /// Open without activating Arto, leaving the focus where it is
    #[arg(long)]
    behind: bool,
    /// Root directory for the file explorer sidebar
    #[arg(long)]
    directory: Option<PathBuf>,
    /// Place the window at these screen coordinates, as X,Y
    #[arg(long, value_name = "X,Y", value_parser = parse_position, allow_hyphen_values = true)]
    position: Option<WindowPoint>,
    /// Give the window this size, as WIDTH,HEIGHT
    #[arg(long, value_name = "WIDTH,HEIGHT", value_parser = parse_size)]
    size: Option<WindowExtent>,
    /// Theme for this invocation, overriding the configured one
    #[arg(long, value_enum)]
    theme: Option<ThemeArg>,
    /// Return only once the window has drawn the document.
    ///
    /// Applies when Arto is already running and this invocation hands its
    /// request over. A launch that starts Arto itself becomes the app and
    /// runs until it is quit, with or without this flag.
    #[arg(long)]
    wait_ready: bool,
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
        behind: cli.behind,
        window: WindowOptions {
            position: cli.position,
            size: cli.size,
            theme: cli.theme.map(arto_config::Theme::from),
        },
        wait_ready: cli.wait_ready,
    };

    if let arto::RunResult::SentToExistingInstance = arto::run(invocation) {
        std::process::exit(0);
    }
}
