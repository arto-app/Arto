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
                /* Hide body initially to prevent flash of unstyled content (FOUC) */
                body {{
                    opacity: 0;
                    transition: opacity 0.1s ease-in;
                }}
                body.loaded {{
                    opacity: 1;
                }}
            </style>
        </head>
        <body data-theme="{resolved}">
            <div id="main"></div>
            <!-- MODULE LOADER -->
            <script>
                // Show body once CSS is fully loaded and parsed
                // This prevents FOUC (Flash of Unstyled Content) on cold start
                window.addEventListener('DOMContentLoaded', function() {{
                    // Wait for stylesheets to load
                    if (document.styleSheets.length > 0) {{
                        document.body.classList.add('loaded');
                    }} else {{
                        // Fallback: show after a short delay if no stylesheets detected
                        setTimeout(function() {{
                            document.body.classList.add('loaded');
                        }}, 100);
                    }}
                }});
            </script>
        </body>
    </html>
    "#}
}

pub(crate) fn build_mermaid_window_index(theme: Theme) -> String {
    let resolved = resolve_theme(theme);
    indoc::formatdoc! {r#"
    <!DOCTYPE html>
    <html>
        <head>
            <title>Mermaid Viewer - Arto</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <!-- CUSTOM HEAD -->
            <style>
                /* Hide body initially to prevent flash of unstyled content (FOUC) */
                body {{
                    opacity: 0;
                    transition: opacity 0.1s ease-in;
                }}
                body.loaded {{
                    opacity: 1;
                }}
            </style>
        </head>
        <body data-theme="{resolved}" class="mermaid-window-body">
            <div id="main"></div>
            <!-- MODULE LOADER -->
            <script>
                // Show body once CSS is fully loaded and parsed
                window.addEventListener('DOMContentLoaded', function() {{
                    if (document.styleSheets.length > 0) {{
                        document.body.classList.add('loaded');
                    }} else {{
                        setTimeout(function() {{
                            document.body.classList.add('loaded');
                        }}, 100);
                    }}
                }});
            </script>
        </body>
    </html>
    "#}
}
