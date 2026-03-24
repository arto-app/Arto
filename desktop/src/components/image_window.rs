use dioxus::prelude::*;
use sha2::{Digest, Sha256};

use crate::assets::get_main_script_path;
use crate::components::theme_selector::ThemeSelector;
use crate::hooks::{
    use_clipboard_image_handler, use_theme_dispatch, use_window_close_handler, use_zoom_sync,
    CopyImageButton,
};
use crate::theme::Theme;

/// Props for ImageWindow component
#[derive(Props, Clone, PartialEq)]
pub struct ImageWindowProps {
    /// Image URL (data URL or HTTP)
    pub src: String,
    /// Alt text / filename
    pub alt: Option<String>,
    /// Unique image identifier (hash)
    pub image_id: String,
    /// Initial theme
    pub theme: Theme,
}

/// Generate unique ID from image source
pub fn generate_image_id(src: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(src.as_bytes());
    let result = hasher.finalize();
    // Use first 16 characters of hex hash
    format!("{:x}", result)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_image_id_deterministic() {
        let id1 = generate_image_id("data:image/png;base64,abc123");
        let id2 = generate_image_id("data:image/png;base64,abc123");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_generate_image_id_different_sources() {
        let id1 = generate_image_id("data:image/png;base64,abc123");
        let id2 = generate_image_id("https://example.com/photo.jpg");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_image_id_length() {
        let id = generate_image_id("https://example.com/image.png");
        assert_eq!(id.len(), 16);
    }
}

/// Image Window Component
#[component]
pub fn ImageWindow(props: ImageWindowProps) -> Element {
    let current_theme = use_signal(|| props.theme);
    let zoom_level = use_signal(|| 100);

    let title = props.alt.as_deref().unwrap_or("Image");

    // Setup shared hooks
    use_window_close_handler();
    use_zoom_sync(zoom_level);
    use_clipboard_image_handler();
    use_theme_dispatch(current_theme);

    // Load viewer script on mount
    use_effect(move || {
        let src_json = serde_json::to_string(&props.src).unwrap_or_default();
        let image_id_json = serde_json::to_string(&props.image_id).unwrap_or_default();

        spawn(async move {
            let eval_result = document::eval(&indoc::formatdoc!(
                r#"
                (async () => {{
                    try {{
                        const {{ initImageWindow }} = await import("{}");
                        await initImageWindow({src_json}, {image_id_json});
                    }} catch (error) {{
                        console.error("Failed to load image window module:", error);
                    }}
                }})();
                "#,
                get_main_script_path()
            ));

            if let Err(e) = eval_result.await {
                tracing::error!("Failed to initialize image window: {}", e);
            }
        });
    });

    rsx! {
        div {
            class: "image-window-container",

            // Header with controls
            div {
                class: "image-window-header",
                div { class: "image-window-title", "{title}" }
                div {
                    class: "image-window-controls",
                    CopyImageButton {
                        js_function: "copyImageToClipboard",
                        label: "Copy image to clipboard",
                    }
                    ThemeSelector { current_theme }
                }
            }

            // Canvas container for image
            div {
                id: "image-window-canvas",
                class: "image-window-canvas",

                // Wrapper for positioning (translate)
                div {
                    id: "image-wrapper",
                    class: "image-wrapper",

                    // Inner container for zoom
                    div {
                        id: "image-container",
                        class: "image-container",
                        img {
                            alt: "{title}",
                        }
                    }
                }
            }

            // Status bar
            div {
                class: "image-window-status",
                "Zoom: {zoom_level}% | Scroll to zoom, drag to pan"
            }
        }
    }
}
