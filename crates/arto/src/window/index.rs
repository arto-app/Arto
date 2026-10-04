use crate::assets::icon_sprite;
use crate::theme::{resolve_color_theme, Theme};
use rust_i18n::t;

pub fn build_custom_index(theme: Theme) -> String {
    let resolved = resolve_color_theme(theme).as_str();
    let sprite = icon_sprite();
    let lang = crate::i18n::locale();
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html lang="{lang}" data-theme="{resolved}">
        <head>
            <title>Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
            <!-- CUSTOM HEAD -->
        </head>
        <body>
            {sprite}
            <div id="main"></div>
            <!-- MODULE LOADER -->
        </body>
    </html>
    "#}
}

fn build_viewer_window_index(title: &str, body_class: &str, theme: Theme) -> String {
    let resolved = resolve_color_theme(theme).as_str();
    let sprite = icon_sprite();
    let lang = crate::i18n::locale();
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html lang="{lang}" data-theme="{resolved}">
        <head>
            <title>{title} - Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <!-- CUSTOM HEAD -->
        </head>
        <body class="{body_class}">
            {sprite}
            <div id="main"></div>
            <!-- MODULE LOADER -->
        </body>
    </html>
    "#}
}

pub(crate) fn build_mermaid_window_index(theme: Theme) -> String {
    build_viewer_window_index(
        &t!("app.viewer_window.mermaid"),
        "mermaid-window-body",
        theme,
    )
}

pub(crate) fn build_math_window_index(theme: Theme) -> String {
    build_viewer_window_index(&t!("app.viewer_window.math"), "math-window-body", theme)
}

pub(crate) fn build_image_window_index(theme: Theme) -> String {
    build_viewer_window_index(&t!("app.viewer_window.image"), "image-window-body", theme)
}

pub(crate) fn build_preferences_window_index(theme: Theme) -> String {
    build_viewer_window_index(
        &t!("app.viewer_window.preferences"),
        "preferences-window-body",
        theme,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The icons are `<use href="#tabler-…">`, which resolves in the document
    /// it is written in and nowhere else, so every window has to carry the
    /// sprite. Without it the interface renders with every icon blank and
    /// nothing reported anywhere.
    #[test]
    fn every_window_carries_the_icon_sprite() {
        for index in [
            build_custom_index(Theme::Light),
            build_mermaid_window_index(Theme::Light),
            build_math_window_index(Theme::Light),
            build_image_window_index(Theme::Light),
            build_preferences_window_index(Theme::Light),
        ] {
            assert!(
                index.contains("id=\"tabler-"),
                "sprite missing: {index:.200}"
            );
        }
    }

    /// A screen reader reads the interface by the language on the root, so
    /// a Japanese interface without one is read with English rules.
    #[test]
    fn every_window_states_the_interface_language() {
        let lang = format!(r#"<html lang="{}""#, crate::i18n::locale());
        for index in [
            build_custom_index(Theme::Light),
            build_mermaid_window_index(Theme::Light),
            build_math_window_index(Theme::Light),
            build_image_window_index(Theme::Light),
            build_preferences_window_index(Theme::Light),
        ] {
            assert!(index.contains(&lang), "no {lang}: {index:.200}");
        }
    }
}
