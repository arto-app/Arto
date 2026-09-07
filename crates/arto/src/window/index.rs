use crate::theme::{resolve_color_theme, Theme};

pub fn build_custom_index(theme: Theme) -> String {
    let resolved = resolve_color_theme(theme).as_str();
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html data-theme="{resolved}">
        <head>
            <title>Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
            <!-- CUSTOM HEAD -->
        </head>
        <body>
            <div id="main"></div>
            <!-- MODULE LOADER -->
        </body>
    </html>
    "#}
}

fn build_viewer_window_index(title: &str, body_class: &str, theme: Theme) -> String {
    let resolved = resolve_color_theme(theme).as_str();
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html data-theme="{resolved}">
        <head>
            <title>{title} - Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <!-- CUSTOM HEAD -->
        </head>
        <body class="{body_class}">
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
