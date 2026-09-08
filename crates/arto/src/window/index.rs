use crate::assets::icon_sprite;
use crate::theme::{resolve_color_theme, Theme};

/// The class the header's macOS clearance keys off, carried by the first frame
/// so the window buttons never land on a glyph before Rust has had a chance to
/// say so. `window::titlebar::sync_traffic_light_clearance` owns it afterwards
/// and takes it away in full screen. Empty everywhere the OS still draws a
/// title bar of its own.
const TITLEBAR_CLASS: &str = if cfg!(target_os = "macos") {
    " class=\"has-traffic-lights\""
} else {
    ""
};

pub fn build_custom_index(theme: Theme) -> String {
    let resolved = resolve_color_theme(theme).as_str();
    let sprite = icon_sprite();
    let titlebar = TITLEBAR_CLASS;
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html data-theme="{resolved}"{titlebar}>
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
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html data-theme="{resolved}">
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
    build_viewer_window_index("Mermaid Viewer", "mermaid-window-body", theme)
}

pub(crate) fn build_math_window_index(theme: Theme) -> String {
    build_viewer_window_index("Math Viewer", "math-window-body", theme)
}

pub(crate) fn build_image_window_index(theme: Theme) -> String {
    build_viewer_window_index("Image Viewer", "image-window-body", theme)
}

pub(crate) fn build_preferences_window_index(theme: Theme) -> String {
    build_viewer_window_index("Preferences", "preferences-window-body", theme)
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
}
