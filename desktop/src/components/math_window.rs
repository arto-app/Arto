use dioxus::prelude::*;
use sha2::{Digest, Sha256};

use crate::assets::get_main_script_path;
use crate::components::theme_selector::ThemeSelector;
use crate::hooks::{
    use_clipboard_image_handler, use_theme_dispatch, use_window_close_handler, use_zoom_sync,
    CopyImageButton,
};
use crate::theme::Theme;

/// Props for MathWindow component
#[derive(Props, Clone, PartialEq)]
pub struct MathWindowProps {
    /// LaTeX source code
    pub source: String,
    /// Unique math identifier (hash)
    pub math_id: String,
    /// Initial theme
    pub theme: Theme,
}

/// Generate unique ID from LaTeX source
pub fn generate_math_id(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let result = hasher.finalize();
    // Use first 16 characters of hex hash
    format!("{:x}", result)[..16].to_string()
}

/// Truncate LaTeX source for window title
fn truncate_latex(source: &str, max_len: usize) -> String {
    if source.chars().count() <= max_len {
        source.to_string()
    } else {
        let truncated: String = source.chars().take(max_len).collect();
        format!("{truncated}…")
    }
}

/// Math Window Component
#[component]
pub fn MathWindow(props: MathWindowProps) -> Element {
    let current_theme = use_signal(|| props.theme);
    let zoom_level = use_signal(|| 100);
    let title = truncate_latex(&props.source, 50);

    // Setup shared hooks
    use_window_close_handler();
    use_zoom_sync(zoom_level);
    use_clipboard_image_handler();
    use_theme_dispatch(current_theme);

    // Load viewer script on mount
    use_effect(move || {
        let source_json = serde_json::to_string(&props.source).unwrap_or_default();
        let math_id_json = serde_json::to_string(&props.math_id).unwrap_or_default();
        // Resolve "auto" to actual light/dark before passing to JS
        let theme_str = match crate::theme::resolve_theme(*current_theme.read()) {
            crate::theme::DioxusTheme::Light => "light",
            crate::theme::DioxusTheme::Dark => "dark",
        };

        spawn(async move {
            let eval_result = document::eval(&indoc::formatdoc!(
                r#"
                (async () => {{
                    try {{
                        const {{ initMathWindow }} = await import("{}");
                        await initMathWindow({source_json}, {math_id_json}, "{theme_str}");
                    }} catch (error) {{
                        console.error("Failed to load math window module:", error);
                    }}
                }})();
                "#,
                get_main_script_path()
            ));

            if let Err(e) = eval_result.await {
                tracing::error!("Failed to initialize math window: {}", e);
            }
        });
    });

    rsx! {
        div {
            class: "math-window-container",

            // Header with controls
            div {
                class: "math-window-header",
                div { class: "math-window-title", "{title}" }
                div {
                    class: "math-window-controls",
                    CopyImageButton {
                        js_function: "copyMathAsImage",
                        label: "Copy Math as image",
                    }
                    ThemeSelector { current_theme }
                }
            }

            // Canvas container for math
            div {
                id: "math-window-canvas",
                class: "math-window-canvas",

                // Wrapper for positioning (translate)
                div {
                    id: "math-wrapper",
                    class: "math-wrapper",

                    // Inner container for zoom
                    div {
                        id: "math-container",
                        class: "math-container",
                        // Placeholder for Math content
                    }
                }
            }

            // Status bar
            div {
                class: "math-window-status",
                "Zoom: {zoom_level}% | Scroll to zoom, drag to pan, double-click to fit"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_math_id_deterministic() {
        let id1 = generate_math_id("E = mc^2");
        let id2 = generate_math_id("E = mc^2");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_generate_math_id_different_sources() {
        let id1 = generate_math_id("E = mc^2");
        let id2 = generate_math_id("a^2 + b^2 = c^2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_math_id_length() {
        let id = generate_math_id("\\int_0^\\infty e^{-x} dx");
        assert_eq!(id.len(), 16);
    }

    #[test]
    fn test_truncate_latex_short() {
        assert_eq!(truncate_latex("E = mc^2", 50), "E = mc^2");
    }

    #[test]
    fn test_truncate_latex_exact() {
        let source = "a".repeat(50);
        assert_eq!(truncate_latex(&source, 50), source);
    }

    #[test]
    fn test_truncate_latex_long() {
        let source = "a".repeat(60);
        let result = truncate_latex(&source, 50);
        assert!(result.starts_with(&"a".repeat(50)));
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_latex_multibyte() {
        // Ensure no panic on multi-byte UTF-8 characters
        let source = "あ".repeat(60);
        let result = truncate_latex(&source, 50);
        assert_eq!(result.chars().count(), 51); // 50 chars + '…'
        assert!(result.ends_with('…'));
    }
}
