use dioxus::prelude::*;

/// The frame a viewer window shows its content in: a header with the title
/// and the controls, the canvas the content is panned and zoomed on, and a
/// status line under it.
///
/// `kind` names the canvas, which the frontend's controller looks up by
/// its id.
#[component]
pub fn ViewerWindowShell(
    kind: &'static str,
    title: String,
    controls: Element,
    status: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "viewer-window-container",
            div {
                class: "viewer-window-header",
                div { class: "viewer-window-title", "{title}" }
                div { class: "viewer-window-controls", {controls} }
            }
            div {
                id: "{kind}-window-canvas",
                class: "viewer-window-canvas",
                {children}
            }
            div { class: "viewer-window-status", "{status}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn the_content_sits_on_the_canvas_between_the_header_and_the_status() {
        let html = render(|| {
            rsx! {
                ViewerWindowShell {
                    kind: "math",
                    title: "E = mc^2".to_string(),
                    controls: rsx! { button { "copy" } },
                    status: "100%".to_string(),
                    div { class: "math-wrapper" }
                }
            }
        });

        assert_eq!(
            html,
            concat!(
                r#"<div class="viewer-window-container">"#,
                r#"<div class="viewer-window-header">"#,
                r#"<div class="viewer-window-title">E = mc^2</div>"#,
                r#"<div class="viewer-window-controls"><button>copy</button></div>"#,
                r#"</div>"#,
                r#"<div id="math-window-canvas" class="viewer-window-canvas">"#,
                r#"<div class="math-wrapper"></div>"#,
                r#"</div>"#,
                r#"<div class="viewer-window-status">100%</div>"#,
                r#"</div>"#,
            )
        );
    }
}
