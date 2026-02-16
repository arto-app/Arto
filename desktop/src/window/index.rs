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
                // when the asset server might not be fully ready yet
                (function() {{
                    function showBody() {{
                        document.body.classList.add('loaded');
                    }}

                    // Check if stylesheets are loaded
                    function checkStylesheets() {{
                        // Count loaded stylesheets (excluding inline styles)
                        var loadedCount = 0;
                        for (var i = 0; i < document.styleSheets.length; i++) {{
                            try {{
                                // Try to access cssRules to verify stylesheet is loaded
                                // This will throw for unloaded or cross-origin stylesheets
                                var rules = document.styleSheets[i].cssRules;
                                if (rules && rules.length > 0) {{
                                    loadedCount++;
                                }}
                            }} catch (e) {{
                                // Stylesheet not loaded or cross-origin
                            }}
                        }}
                        return loadedCount > 0;
                    }}

                    window.addEventListener('DOMContentLoaded', function() {{
                        // If stylesheets are already loaded, show immediately
                        if (checkStylesheets()) {{
                            showBody();
                        }} else {{
                            // Otherwise wait a bit for asset server to respond
                            // Most stylesheets should load within 50ms on cold start
                            var maxWait = 200;
                            var checkInterval = 10;
                            var elapsed = 0;
                            
                            var interval = setInterval(function() {{
                                elapsed += checkInterval;
                                if (checkStylesheets() || elapsed >= maxWait) {{
                                    clearInterval(interval);
                                    showBody();
                                }}
                            }}, checkInterval);
                        }}
                    }});
                }})();
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
                (function() {{
                    function showBody() {{
                        document.body.classList.add('loaded');
                    }}

                    function checkStylesheets() {{
                        var loadedCount = 0;
                        for (var i = 0; i < document.styleSheets.length; i++) {{
                            try {{
                                var rules = document.styleSheets[i].cssRules;
                                if (rules && rules.length > 0) {{
                                    loadedCount++;
                                }}
                            }} catch (e) {{
                                // Stylesheet not loaded or cross-origin
                            }}
                        }}
                        return loadedCount > 0;
                    }}

                    window.addEventListener('DOMContentLoaded', function() {{
                        if (checkStylesheets()) {{
                            showBody();
                        }} else {{
                            var maxWait = 200;
                            var checkInterval = 10;
                            var elapsed = 0;
                            
                            var interval = setInterval(function() {{
                                elapsed += checkInterval;
                                if (checkStylesheets() || elapsed >= maxWait) {{
                                    clearInterval(interval);
                                    showBody();
                                }}
                            }}, checkInterval);
                        }}
                    }});
                }})();
            </script>
        </body>
    </html>
    "#}
}
