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
            <meta charset="utf-8">
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
            <meta charset="utf-8">
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
        "viewer-window-body",
        theme,
    )
}

pub(crate) fn build_math_window_index(theme: Theme) -> String {
    build_viewer_window_index(&t!("app.viewer_window.math"), "viewer-window-body", theme)
}

pub(crate) fn build_image_window_index(theme: Theme) -> String {
    build_viewer_window_index(&t!("app.viewer_window.image"), "viewer-window-body", theme)
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

    /// The protocol serves the page as `text/html` with no charset, so
    /// without a declaration WebKit decodes it in the system's legacy
    /// encoding — Shift_JIS on a Japanese Mac — and the translated title and
    /// the frontend's catalog in the head come out garbled. The declaration
    /// counts only within the first 1024 bytes, ahead of any of that text.
    #[test]
    fn every_window_declares_utf8_before_any_text() {
        let charset = r#"<meta charset="utf-8">"#;
        for index in [
            build_custom_index(Theme::Light),
            build_mermaid_window_index(Theme::Light),
            build_math_window_index(Theme::Light),
            build_image_window_index(Theme::Light),
            build_preferences_window_index(Theme::Light),
        ] {
            let at = index.find(charset);
            assert!(
                at.is_some_and(|at| at < 1024 && at < index.find("<title>").unwrap()),
                "no leading {charset}: {index:.200}"
            );
        }
    }
}
