//! Command-line surface shared by the `arto-page` binary and `arto page`.
//!
//! The arguments are defined once here so both entry points accept exactly
//! the same flags. Errors are reported as [`PageError`]; the binaries decide
//! how to print them.
//!
//! Options are layered: built-in defaults, then the user's `config.json`
//! (the same file the app reads), then the flags. So a page looks like the
//! app would show it unless a flag says otherwise.

use crate::{render_file, Config, PageError, PageOptions, Theme};
use clap::{Args, ValueEnum};
use std::io::Write;
use std::path::PathBuf;

/// The `--theme` values; a clap-side mirror of [`Theme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ThemeArg {
    /// Follow the viewer's system setting
    Auto,
    Light,
    Dark,
}

impl From<ThemeArg> for Theme {
    fn from(theme: ThemeArg) -> Self {
        match theme {
            ThemeArg::Auto => Theme::Auto,
            ThemeArg::Light => Theme::Light,
            ThemeArg::Dark => Theme::Dark,
        }
    }
}

/// Render a Markdown file into a self-contained HTML page.
#[derive(Debug, Clone, Args)]
pub struct PageArgs {
    /// Markdown file to render
    pub input: PathBuf,
    /// Write the page to this file instead of standard output
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Read settings from this file instead of the app's config.json
    #[arg(long, value_name = "FILE", conflicts_with = "no_config")]
    pub config: Option<PathBuf>,
    /// Ignore config.json and start from the built-in defaults
    #[arg(long)]
    pub no_config: bool,
    /// Colour theme of the page
    #[arg(long, value_enum, value_name = "THEME")]
    pub theme: Option<ThemeArg>,
    /// Omit the Content-Security-Policy that blocks scripts embedded in the
    /// Markdown; only for input you trust
    #[arg(long)]
    pub no_csp: bool,
    /// Leave bare URLs as plain text instead of turning them into links
    #[arg(long)]
    pub no_auto_link_urls: bool,
}

impl PageArgs {
    /// The rendering options these arguments describe, on top of the user's
    /// configuration unless `--no-config` was given.
    ///
    /// A missing config.json is not an error; an unreadable or malformed one
    /// is, because silently rendering with other settings than the app uses
    /// would be confusing. `--no-config` is the escape hatch.
    pub fn options(&self) -> Result<PageOptions, PageError> {
        let mut options = match (&self.config, self.no_config) {
            (_, true) => PageOptions::default(),
            (Some(path), false) => PageOptions::from_config(&Config::load_preferences_from(path)?),
            (None, false) => PageOptions::from_config(&Config::load_preferences()?),
        };

        if let Some(theme) = self.theme {
            options.theme = Theme::from(theme);
        }
        if self.no_auto_link_urls {
            options.render.auto_link_urls = false;
        }
        if self.no_csp {
            options.content_security_policy = false;
        }
        Ok(options)
    }
}

/// Render `args.input` and write the page to `args.output` or standard output.
pub fn run(args: &PageArgs) -> Result<(), PageError> {
    let html = render_file(&args.input, &args.options()?)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> PageArgs {
        PageArgs {
            input: PathBuf::from(input),
            output: None,
            config: None,
            no_config: true,
            theme: None,
            no_csp: false,
            no_auto_link_urls: false,
        }
    }

    #[test]
    fn no_config_uses_the_built_in_defaults() {
        let options = args("doc.md").options().unwrap();
        assert!(options.render.auto_link_urls);
        assert_eq!(options.theme, Theme::Auto);
        assert!(options.content_security_policy);
    }

    #[test]
    fn flags_override_the_configuration_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(
            &config_path,
            r#"{"theme":{"defaultTheme":"dark","onStartup":"default","onNewWindow":"default"},"markdown":{"autoLinkUrls":true}}"#,
        )
        .unwrap();

        let from_config = PageArgs {
            config: Some(config_path.clone()),
            no_config: false,
            ..args("doc.md")
        };
        let options = from_config.options().unwrap();
        assert_eq!(options.theme, Theme::Dark);
        assert!(options.render.auto_link_urls);

        let overridden = PageArgs {
            theme: Some(ThemeArg::Light),
            no_auto_link_urls: true,
            no_csp: true,
            ..from_config
        };
        let options = overridden.options().unwrap();
        assert_eq!(options.theme, Theme::Light);
        assert!(!options.render.auto_link_urls);
        assert!(!options.content_security_policy);
    }

    #[test]
    fn a_malformed_configuration_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "{ not json").unwrap();

        let err = PageArgs {
            config: Some(config_path),
            no_config: false,
            ..args("doc.md")
        }
        .options()
        .unwrap_err();
        assert!(matches!(err, PageError::Config(_)));
    }
}
