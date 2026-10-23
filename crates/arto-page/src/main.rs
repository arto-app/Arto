use clap::Parser;

/// Render a Markdown file into a self-contained HTML page with Arto's styling.
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = "Render a Markdown file into a single HTML page that carries Arto's \
        stylesheet and frontend bundle inline, so it can be opened anywhere without \
        the app: in a browser, in a Quick Look preview, or attached to a message. \
        Rendering options and the theme come from the app's config.json unless \
        overridden by flags or disabled with --no-config.",
    after_long_help = "Examples:\n\
        \x20 arto-page README.md > README.html\n\
        \x20 arto-page --output out.html docs/guide.md\n\
        \x20 arto-page --theme dark --no-config notes.md\n\
        \x20 arto-page --no-csp trusted.md"
)]
struct Cli {
    #[command(flatten)]
    args: arto_page::cli::PageArgs,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = arto_page::cli::run(&cli.args) {
        eprintln!("arto-page: {}", cause_chain(&err));
        std::process::exit(1);
    }
}

/// The error and every cause under it on one line, the shape `arto page`
/// prints with anyhow's `{:#}`.
fn cause_chain(err: &dyn std::error::Error) -> String {
    let mut line = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        line.push_str(": ");
        line.push_str(&cause.to_string());
        source = cause.source();
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cause_follows_the_error_on_one_line() {
        let parse = std::io::Error::other("nesting too deep");
        let err = arto_page::PageError::Render(arto_markdown::RenderError::Parse(Box::new(parse)));

        assert_eq!(
            cause_chain(&err),
            "failed to render Markdown: failed to parse Markdown: nesting too deep"
        );
    }
}
