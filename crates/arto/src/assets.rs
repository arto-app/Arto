use arto_keybindings::BindingSet;
use dioxus::asset_resolver::asset_path;
use dioxus::prelude::*;

pub static MAIN_SCRIPT: Asset = asset!("/assets/frontend/main.js");
pub static MAIN_STYLE: Asset = asset!("/assets/frontend/main.css");

/// The `<head>` markup that loads the main stylesheet, for
/// `Config::with_custom_head` on every window.
///
/// Kept as static head markup on purpose. `document::Stylesheet {}` would be
/// the idiomatic rsx form, but it is inserted by the runtime after the first
/// render, so the window would paint unstyled for a moment on every open.
/// Head markup is parsed with the page and applies before anything shows.
pub fn main_stylesheet_head() -> String {
    format!(r#"<link rel="stylesheet" href="{MAIN_STYLE}">"#)
}

/// The welcome header, in the two flavours the page switches between. The
/// stylesheet shows one and hides the other by theme, so both are always in
/// the document.
static ARTO_HEADER_LIGHT: Asset = asset!("/assets/arto-header-welcome-light.png");
static ARTO_HEADER_DARK: Asset = asset!("/assets/arto-header-welcome-dark.png");
static WELCOME_TEMPLATE: Asset = asset!("/assets/welcome.md");

// Embed and process default markdown content at runtime
pub fn get_default_markdown_content() -> String {
    let template_path = asset_path(WELCOME_TEMPLATE).expect("Failed to resolve WELCOME_TEMPLATE");
    let template = std::fs::read_to_string(template_path).expect("Failed to read WELCOME_TEMPLATE");

    // The template names the images by their source-relative paths so that it
    // is readable (and renderable) on its own; here they become the paths the
    // running app can actually load.
    let template = template
        .replace(
            "../assets/arto-header-welcome-light.png",
            &resolved_asset_path(ARTO_HEADER_LIGHT),
        )
        .replace(
            "../assets/arto-header-welcome-dark.png",
            &resolved_asset_path(ARTO_HEADER_DARK),
        );

    with_current_shortcuts(&template)
}

fn resolved_asset_path(asset: Asset) -> String {
    asset_path(asset)
        .expect("Failed to resolve asset")
        .to_string_lossy()
        .into_owned()
}

/// What a shortcut reads as when the action has none bound.
const UNBOUND_SHORTCUT: &str = "—";

/// Replace every `{{action}}` in the welcome page with the shortcut bound to
/// that action right now.
///
/// The page is the first thing a reader sees, and every shortcut on it is
/// rebindable, so printing the defaults would be printing something untrue
/// for anyone who has changed one — or who uses the Emacs or Vim preset.
fn with_current_shortcuts(template: &str) -> String {
    let config = crate::config::CONFIG.read();
    substitute_shortcuts(template, &config.keybindings)
}

fn substitute_shortcuts(template: &str, bindings: &BindingSet) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        rendered.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            rendered.push_str(&rest[open..]);
            return rendered;
        };
        let action = after[..close].trim();
        let hint = arto_keybindings::hint_for_action(bindings, action, None);
        rendered.push_str(hint.as_deref().unwrap_or(UNBOUND_SHORTCUT));
        rest = &after[close + 2..];
    }
    rendered.push_str(rest);
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_keybindings::KeyAction;

    fn bound(key: &str, action: &str) -> BindingSet {
        BindingSet {
            global: vec![KeyAction {
                key: key.to_string(),
                action: action.to_string(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn shortcuts_come_from_the_bindings_given() {
        let rendered =
            substitute_shortcuts("Open with `{{file.open}}`.", &bound("Cmd+p", "file.open"));

        assert!(!rendered.contains("{{"), "placeholder left unrendered");
        assert!(
            !rendered.contains(UNBOUND_SHORTCUT),
            "a bound action rendered as unbound: {rendered}"
        );
    }

    #[test]
    fn an_action_nobody_bound_says_so() {
        let rendered = substitute_shortcuts("`{{file.open}}`", &BindingSet::default());
        assert_eq!(rendered, format!("`{UNBOUND_SHORTCUT}`"));
    }

    #[test]
    fn text_around_and_between_placeholders_survives() {
        let rendered = substitute_shortcuts(
            "before {{file.open}} between {{file.open}} after {{",
            &bound("Cmd+p", "file.open"),
        );
        assert!(rendered.starts_with("before "));
        assert!(
            rendered.ends_with(" after {{"),
            "unterminated tail lost: {rendered}"
        );
    }
}
