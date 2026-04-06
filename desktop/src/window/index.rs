use crate::theme::{resolve_theme, Theme};

pub fn build_custom_index(theme: Theme) -> String {
    let resolved = resolve_theme(theme);
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html>
        <head>
            <title>Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
            <!-- CUSTOM HEAD -->
            <style>
                /* Hide body until CSS is loaded to prevent FOUC on cold start.
                   WebView2 on Windows may take longer to load stylesheets. */
                body {{ opacity: 0; transition: opacity 0.1s ease-in; }}
                body.loaded {{ opacity: 1; }}
            </style>
        </head>
        <body data-theme="{resolved}">
            <div id="main"></div>
            <!-- MODULE LOADER -->
            <script>
                (function() {{
                    function showBody() {{ document.body.classList.add('loaded'); }}
                    function hasLoadedStylesheets() {{
                        for (var i = 0; i < document.styleSheets.length; i++) {{
                            try {{
                                if (document.styleSheets[i].cssRules && document.styleSheets[i].cssRules.length > 0) return true;
                            }} catch (e) {{}}
                        }}
                        return false;
                    }}
                    window.addEventListener('DOMContentLoaded', function() {{
                        if (hasLoadedStylesheets()) {{ showBody(); return; }}
                        var elapsed = 0;
                        var interval = setInterval(function() {{
                            elapsed += 10;
                            if (hasLoadedStylesheets() || elapsed >= 200) {{ clearInterval(interval); showBody(); }}
                        }}, 10);
                    }});
                }})();
            </script>
        </body>
    </html>
    "#}
}

fn build_viewer_window_index(title: &str, body_class: &str, theme: Theme) -> String {
    let resolved = resolve_theme(theme);
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html>
        <head>
            <title>{title} - Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <!-- CUSTOM HEAD -->
            <style>
                body {{ opacity: 0; transition: opacity 0.1s ease-in; }}
                body.loaded {{ opacity: 1; }}
            </style>
        </head>
        <body data-theme="{resolved}" class="{body_class}">
            <div id="main"></div>
            <!-- MODULE LOADER -->
            <script>
                (function() {{
                    function showBody() {{ document.body.classList.add('loaded'); }}
                    function hasLoadedStylesheets() {{
                        for (var i = 0; i < document.styleSheets.length; i++) {{
                            try {{
                                if (document.styleSheets[i].cssRules && document.styleSheets[i].cssRules.length > 0) return true;
                            }} catch (e) {{}}
                        }}
                        return false;
                    }}
                    window.addEventListener('DOMContentLoaded', function() {{
                        if (hasLoadedStylesheets()) {{ showBody(); return; }}
                        var elapsed = 0;
                        var interval = setInterval(function() {{
                            elapsed += 10;
                            if (hasLoadedStylesheets() || elapsed >= 200) {{ clearInterval(interval); showBody(); }}
                        }}, 10);
                    }});
                }})();
            </script>
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
