use clap::Parser;

/// Render a Markdown file into a self-contained HTML page with Arto's styling.
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = "Render a Markdown file into a single HTML page that carries Arto's \
        stylesheet and frontend bundle inline, so it can be opened anywhere without \
        the app: in a browser, in a Quick Look preview, or attached to a message.",
    after_long_help = "Examples:\n\
        \x20 arto-page README.md > README.html\n\
        \x20 arto-page --output out.html docs/guide.md\n\
        \x20 arto-page --no-csp trusted.md"
)]
struct Cli {
    #[command(flatten)]
    args: arto_page::cli::PageArgs,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = arto_page::cli::run(&cli.args) {
        // Same shape as `arto page`: `{:#}` on an anyhow error prints the
        // whole cause chain on one line.
        eprintln!("arto-page: {:#}", anyhow::Error::from(err));
        std::process::exit(1);
    }
}
