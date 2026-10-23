use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::theme_selector::ThemeSelector;
use crate::components::viewer_window_shell::ViewerWindowShell;
use crate::hooks::{
    spawn_viewer_init, use_clipboard_image_handler, use_theme_dispatch, use_window_close_handler,
    use_zoom_sync, CopyImageButton,
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

/// Image Window Component
#[component]
pub fn ImageWindow(props: ImageWindowProps) -> Element {
    let current_theme = use_signal(|| props.theme);
    let zoom_level = use_signal(|| 100);

    let title = props
        .alt
        .clone()
        .unwrap_or_else(|| t!("viewer_window.image.untitled").to_string());

    // Setup shared hooks
    use_window_close_handler();
    use_zoom_sync(zoom_level);
    use_clipboard_image_handler();
    use_theme_dispatch(current_theme);

    use_effect(move || {
        // The copy button rasterizes what this window shows, and a canvas that
        // drew an image from the app's own origin is tainted — reading it back
        // throws. An image the app serves therefore reaches the window as its
        // bytes instead of as its URL.
        let src =
            crate::assets::images::data_url_for(&props.src).unwrap_or_else(|| props.src.clone());
        spawn_viewer_init(
            "image",
            "initImageWindow",
            vec![
                serde_json::to_string(&src).unwrap_or_default(),
                serde_json::to_string(&props.image_id).unwrap_or_default(),
            ],
        );
    });

    rsx! {
        ViewerWindowShell {
            kind: "image",
            title: title.clone(),
            controls: rsx! {
                CopyImageButton {
                    js_function: "copyImageToClipboard",
                    label: t!("viewer_window.image.copy").to_string(),
                }
                ThemeSelector { current_theme }
            },
            status: t!("viewer_window.zoom_status.pan", zoom = zoom_level()).to_string(),
            // The wrapper is translated to pan, the container inside it zoomed.
            div {
                id: "image-wrapper",
                class: "image-wrapper",
                div {
                    id: "image-container",
                    class: "image-container",
                    img { alt: "{title}" }
                }
            }
        }
    }
}
