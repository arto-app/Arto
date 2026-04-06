use crate::theme::{resolve_theme, Theme};

/// Inline CSS to hide body until stylesheets are loaded (FOUC prevention).
const FOUC_STYLE: &str = r#"<style>
    body { opacity: 0; transition: opacity 0.1s ease-in; }
    body.loaded { opacity: 1; }
</style>"#;

/// Inline JS to detect external stylesheet readiness and reveal the body.
/// Only checks stylesheets with an `href` (external) to avoid matching
/// the inline FOUC style itself. Falls back after 200ms.
const FOUC_SCRIPT: &str = r#"<script>
    (function() {
        function showBody() { document.body.classList.add('loaded'); }
        function hasLoadedExternalStylesheet() {
            for (var i = 0; i < document.styleSheets.length; i++) {
                try {
                    var sheet = document.styleSheets[i];
                    if (sheet.href && sheet.cssRules && sheet.cssRules.length > 0) return true;
                } catch (e) {}
            }
            return false;
        }
        window.addEventListener('DOMContentLoaded', function() {
            if (hasLoadedExternalStylesheet()) { showBody(); return; }
            var elapsed = 0;
            var interval = setInterval(function() {
                elapsed += 10;
                if (hasLoadedExternalStylesheet() || elapsed >= 200) { clearInterval(interval); showBody(); }
            }, 10);
        });
    })();
</script>"#;

pub fn build_custom_index(theme: Theme) -> String {
    let resolved = resolve_theme(theme);
    format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <title>Arto</title>
        <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
        <!-- CUSTOM HEAD -->
        {FOUC_STYLE}
    </head>
    <body data-theme="{resolved}">
        <div id="main"></div>
        <!-- MODULE LOADER -->
        {FOUC_SCRIPT}
    </body>
</html>
"#
    )
}

fn build_viewer_window_index(title: &str, body_class: &str, theme: Theme) -> String {
    let resolved = resolve_theme(theme);
    format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <title>{title} - Arto</title>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <!-- CUSTOM HEAD -->
        {FOUC_STYLE}
    </head>
    <body data-theme="{resolved}" class="{body_class}">
        <div id="main"></div>
        <!-- MODULE LOADER -->
        {FOUC_SCRIPT}
    </body>
</html>
"#
    )
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
