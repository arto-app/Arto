use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::theme_selector::ThemeSelector;
use crate::components::viewer_window_shell::ViewerWindowShell;
use crate::hooks::{
    spawn_viewer_init, use_clipboard_image_handler, use_theme_dispatch, use_window_close_handler,
    use_zoom_sync, CopyImageButton,
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

    use_effect(move || {
        // Resolve "auto" and the configured choice before passing to JS
        let theme = crate::theme::resolve_color_theme(*current_theme.read()).as_str();
        spawn_viewer_init(
            "math",
            "initMathWindow",
            vec![
                serde_json::to_string(&props.source).unwrap_or_default(),
                serde_json::to_string(&props.math_id).unwrap_or_default(),
                serde_json::to_string(theme).unwrap_or_default(),
            ],
        );
    });

    rsx! {
        ViewerWindowShell {
            kind: "math",
            title,
            controls: rsx! {
                CopyImageButton {
                    js_function: "copyMathAsImage",
                    label: t!("viewer_window.math.copy").to_string(),
                }
                ThemeSelector { current_theme }
            },
            status: t!("viewer_window.zoom_status.pan_fit", zoom = zoom_level()).to_string(),
            // The wrapper is translated to pan, the container inside it zoomed.
            div {
                id: "math-wrapper",
                class: "math-wrapper",
                div { id: "math-container", class: "math-container" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
