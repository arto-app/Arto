use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::icon::{Icon, IconName};
use crate::components::theme_selector::ThemeSelector;
use crate::components::viewer_window_shell::ViewerWindowShell;
use crate::hooks::{
    spawn_viewer_init, use_theme_dispatch, use_window_close_handler, use_zoom_sync, CopyStatus,
};
use crate::theme::Theme;

/// Props for MermaidWindow component
#[derive(Props, Clone, PartialEq)]
pub struct MermaidWindowProps {
    /// Mermaid source code
    pub source: String,
    /// Unique diagram identifier (hash)
    pub diagram_id: String,
    /// Initial theme
    pub theme: Theme,
}

/// Mermaid Window Component
#[component]
pub fn MermaidWindow(props: MermaidWindowProps) -> Element {
    let current_theme = use_signal(|| props.theme);
    let zoom_level = use_signal(|| 100);

    // Setup shared hooks
    use_window_close_handler();
    use_zoom_sync(zoom_level);
    use_theme_dispatch(current_theme);

    use_effect(move || {
        spawn_viewer_init(
            "mermaid",
            "initMermaidWindow",
            vec![
                serde_json::to_string(&props.source).unwrap_or_default(),
                serde_json::to_string(&props.diagram_id).unwrap_or_default(),
            ],
        );
    });

    rsx! {
        ViewerWindowShell {
            kind: "mermaid",
            title: String::new(),
            controls: rsx! {
                MermaidCopyImageButton {}
                ThemeSelector { current_theme }
            },
            status: t!("viewer_window.zoom_status.pan_fit", zoom = zoom_level()).to_string(),
            // The wrapper is translated to pan, the container inside it zoomed.
            div {
                id: "mermaid-diagram-wrapper",
                class: "mermaid-diagram-wrapper",
                div { id: "mermaid-diagram-container", class: "mermaid-diagram-container" }
            }
        }
    }
}

/// Mermaid-specific copy image button that rasterizes SVG inline.
/// Unlike Image/Math windows, Mermaid uses direct SVG→Canvas rasterization
/// rather than a named JS export function.
#[component]
fn MermaidCopyImageButton() -> Element {
    let mut copy_status = use_signal(|| CopyStatus::Idle);

    let handle_click = move |_| {
        spawn(async move {
            copy_status.set(CopyStatus::Copying);

            // Rasterize SVG via Canvas in JS and send PNG data URL back to Rust.
            // NOTE: Cannot use the shared CopyImageButton here because Mermaid
            // uses inline SVG rasterization, not a named JS export function.
            let mut eval = document::eval(indoc::indoc! {r#"
                (async () => {
                    const container = document.getElementById('mermaid-diagram-container');
                    if (!container) { dioxus.send(null); return; }

                    const svg = container.querySelector('svg');
                    if (!svg) { dioxus.send(null); return; }

                    // Get SVG dimensions from viewBox (preferred) or getBBox (fallback)
                    let width, height;
                    const viewBox = svg.getAttribute('viewBox');
                    if (viewBox) {
                        const parts = viewBox.split(/[\s,]+/).map(Number);
                        if (parts.length === 4 && parts[2] > 0 && parts[3] > 0) {
                            width = parts[2];
                            height = parts[3];
                        }
                    }
                    if (!width || !height) {
                        const bbox = svg.getBBox();
                        width = bbox.width;
                        height = bbox.height;
                    }
                    if (!width || !height) { dioxus.send(null); return; }

                    // Serialize SVG with explicit dimensions and resolved font
                    const svgClone = svg.cloneNode(true);
                    svgClone.setAttribute('width', String(width));
                    svgClone.setAttribute('height', String(height));
                    // Resolve inherited font-family for standalone rendering
                    const computedFont = getComputedStyle(svg).fontFamily;
                    if (computedFont) { svgClone.style.fontFamily = computedFont; }
                    const svgString = new XMLSerializer().serializeToString(svgClone);
                    const base64 = btoa(unescape(encodeURIComponent(svgString)));
                    const svgDataUrl = `data:image/svg+xml;base64,${base64}`;

                    // Create 2x canvas with background color
                    const scale = 2;
                    const canvas = document.createElement('canvas');
                    canvas.width = width * scale;
                    canvas.height = height * scale;
                    const ctx = canvas.getContext('2d');
                    if (!ctx) { dioxus.send(null); return; }
                    ctx.scale(scale, scale);
                    const bgColor = getComputedStyle(document.body)
                        .getPropertyValue('--bg-color').trim() || '#ffffff';
                    ctx.fillStyle = bgColor;
                    ctx.fillRect(0, 0, width, height);

                    // Draw SVG onto canvas
                    const img = new Image();
                    const pngDataUrl = await new Promise((resolve) => {
                        img.onload = () => {
                            ctx.drawImage(img, 0, 0);
                            resolve(canvas.toDataURL('image/png'));
                        };
                        img.onerror = () => resolve(null);
                        img.src = svgDataUrl;
                    });
                    dioxus.send(pngDataUrl);
                })();
            "#});

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                eval.recv::<Option<String>>(),
            )
            .await;

            match result {
                Ok(Ok(Some(data_url))) => {
                    std::thread::spawn(move || {
                        crate::utils::clipboard::copy_image_from_data_url(&data_url);
                    });
                    copy_status.set(CopyStatus::Success);
                }
                _ => {
                    tracing::error!("Failed to rasterize Mermaid diagram for clipboard copy");
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
            "aria-label": t!("viewer_window.mermaid.copy").to_string(),
            title: t!("viewer_window.mermaid.copy").to_string(),
            disabled: is_copying,
            onclick: handle_click,
            Icon { name: icon, size: 18 }
        }
    }
}
