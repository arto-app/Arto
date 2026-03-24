/// Common hooks for specialized viewer windows (Mermaid, Math, Image).
/// Provides reusable functionality for window lifecycle and zoom synchronization.
use dioxus::desktop::{use_muda_event_handler, window};
use dioxus::prelude::*;

use crate::assets::get_main_script_path;
use crate::components::icon::{Icon, IconName};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewerCloseKeyEventData {
    key: String,
    meta_key: bool,
    ctrl_key: bool,
    alt_key: bool,
}

fn is_close_shortcut(key: &str, meta_key: bool, ctrl_key: bool, alt_key: bool) -> bool {
    if alt_key || !(meta_key || ctrl_key) {
        return false;
    }
    key.eq_ignore_ascii_case("w")
}

/// Handle Cmd+W and Cmd+Shift+W to close the viewer window.
/// Call this in viewer window components to enable standard window close shortcuts.
pub fn use_window_close_handler() {
    use_muda_event_handler(move |event| {
        if !window().is_focused() {
            return;
        }
        if crate::menu::is_close_action(event) {
            window().close();
        }
    });

    // Fallback for child windows without menu accelerators.
    // Captures Cmd/Ctrl+W (with optional Shift) directly from keydown events.
    use_effect(move || {
        spawn(async move {
            let mut eval = dioxus::document::eval(indoc::indoc! {r#"
                (() => {
                    const handler = (event) => {
                        const target = event.target;
                        const isEditable =
                            target instanceof HTMLInputElement ||
                            target instanceof HTMLTextAreaElement ||
                            target instanceof HTMLSelectElement ||
                            (target instanceof HTMLElement && target.isContentEditable);
                        if (isEditable) return;

                        if (event.altKey) return;
                        if (!(event.metaKey || event.ctrlKey)) return;
                        if (event.key.toLowerCase() !== "w") return;

                        event.preventDefault();
                        event.stopPropagation();
                        dioxus.send({
                            key: event.key,
                            metaKey: event.metaKey,
                            ctrlKey: event.ctrlKey,
                            altKey: event.altKey,
                        });
                    };

                    document.addEventListener("keydown", handler, { capture: true });
                })();
            "#});

            while let Ok(data) = eval.recv::<ViewerCloseKeyEventData>().await {
                if is_close_shortcut(&data.key, data.meta_key, data.ctrl_key, data.alt_key) {
                    window().close();
                }
            }
        });
    });
}

/// Synchronize zoom level between JavaScript and Rust.
/// Establishes a bidirectional channel for zoom updates.
/// The JavaScript side should call `window.updateZoomLevel(zoomPercent)`.
pub fn use_zoom_sync(mut zoom_level: Signal<i32>) {
    use_effect(move || {
        spawn(async move {
            let mut eval_provider = dioxus::document::eval(indoc::indoc! {r#"
                window.updateZoomLevel = (zoom) => {
                    dioxus.send({ zoom: Math.round(zoom) });
                };
            "#});

            while let Ok(data) = eval_provider.recv::<serde_json::Value>().await {
                if let Some(zoom) = data.get("zoom").and_then(|v| v.as_i64()) {
                    zoom_level.set(zoom as i32);
                }
            }
        });
    });
}

/// Setup clipboard handler for viewer windows.
/// Registers `window.rustCopyImage(dataUrl)` to bridge JS clipboard requests to Rust.
pub fn use_clipboard_image_handler() {
    use_effect(move || {
        spawn(async move {
            let mut eval = dioxus::document::eval(indoc::indoc! {r#"
                window.rustCopyImage = (dataUrl) => {
                    dioxus.send({ type: "image", data: dataUrl });
                };
            "#});

            while let Ok(msg) = eval.recv::<serde_json::Value>().await {
                if let Some(data_url) = msg.get("data").and_then(|v| v.as_str()) {
                    let data_url = data_url.to_string();
                    std::thread::spawn(move || {
                        crate::utils::clipboard::copy_image_from_data_url(&data_url);
                    });
                }
            }
        });
    });
}

/// Copy status for visual feedback on copy buttons.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum CopyStatus {
    #[default]
    Idle,
    Copying,
    Success,
    Error,
}

/// Props for the CopyImageButton component.
#[derive(Props, Clone, PartialEq)]
pub struct CopyImageButtonProps {
    /// JavaScript function to call for copy (e.g., "copyImageToClipboard", "copyMathAsImage")
    pub js_function: String,
    /// Aria label for accessibility
    pub label: String,
}

/// Reusable copy image button that calls a JS function and shows feedback.
#[component]
pub fn CopyImageButton(props: CopyImageButtonProps) -> Element {
    let mut copy_status = use_signal(|| CopyStatus::Idle);
    let js_function = props.js_function.clone();

    let handle_click = move |_| {
        let js_function = js_function.clone();
        spawn(async move {
            copy_status.set(CopyStatus::Copying);

            let mut eval = document::eval(&indoc::formatdoc!(
                r#"
                (async () => {{
                    const {{ {js_function} }} = await import("{}");
                    try {{
                        await {js_function}();
                        dioxus.send(true);
                    }} catch (error) {{
                        console.error("Failed to copy:", error);
                        dioxus.send(false);
                    }}
                }})();
                "#,
                get_main_script_path()
            ));

            let result =
                tokio::time::timeout(std::time::Duration::from_secs(5), eval.recv::<bool>()).await;

            match result {
                Ok(Ok(true)) => {
                    copy_status.set(CopyStatus::Success);
                }
                _ => {
                    tracing::error!("Failed to copy as image");
                    copy_status.set(CopyStatus::Error);
                }
            }

            // Reset after 2 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            copy_status.set(CopyStatus::Idle);
        });
    };

    let (icon, extra_class) = match *copy_status.read() {
        CopyStatus::Idle => (IconName::Photo, ""),
        CopyStatus::Copying => (IconName::Photo, "copying"),
        CopyStatus::Success => (IconName::Check, "copied"),
        CopyStatus::Error => (IconName::Close, "error"),
    };

    let is_copying = matches!(*copy_status.read(), CopyStatus::Copying);

    rsx! {
        button {
            class: "viewer-control-btn {extra_class}",
            "aria-label": "{props.label}",
            title: "{props.label}",
            disabled: is_copying,
            onclick: handle_click,
            Icon { name: icon, size: 18 }
        }
    }
}

/// Dispatch a theme-changed event to JavaScript.
/// Call from a `use_effect` that watches the current_theme signal.
pub fn use_theme_dispatch(current_theme: Signal<crate::theme::Theme>) {
    use_effect(move || {
        // Resolve "auto" to actual light/dark before dispatching to JS,
        // since the renderer theme system only supports "light" and "dark".
        let theme_str = match crate::theme::resolve_theme(*current_theme.read()) {
            crate::theme::DioxusTheme::Light => "light",
            crate::theme::DioxusTheme::Dark => "dark",
        };

        spawn(async move {
            if let Err(e) = document::eval(&format!(
                "document.dispatchEvent(new CustomEvent('arto:theme-changed', {{ detail: '{}' }}))",
                theme_str
            ))
            .await
            {
                tracing::debug!("Failed to dispatch theme change event: {}", e);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::is_close_shortcut;

    #[test]
    fn matches_cmd_w() {
        assert!(is_close_shortcut("w", true, false, false));
    }

    #[test]
    fn matches_cmd_shift_w() {
        assert!(is_close_shortcut("W", true, false, false));
    }

    #[test]
    fn matches_ctrl_w_for_non_macos() {
        assert!(is_close_shortcut("w", false, true, false));
    }

    #[test]
    fn ignores_non_w_key() {
        assert!(!is_close_shortcut("q", true, false, false));
    }

    #[test]
    fn ignores_alt_modified_w() {
        assert!(!is_close_shortcut("w", true, false, true));
    }
}
