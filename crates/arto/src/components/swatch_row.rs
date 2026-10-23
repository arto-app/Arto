use dioxus::prelude::*;

use crate::highlight_color::HighlightColor;

/// A swatch for every colour a mark can take, the current one marked.
///
/// Rendered without a container of its own, so it lines up with whatever
/// else shares the bar it is placed in.
#[component]
pub fn SwatchRow(
    selected: Option<HighlightColor>,
    on_pick: EventHandler<HighlightColor>,
) -> Element {
    rsx! {
        for choice in HighlightColor::ALL {
            button {
                key: "{choice.to_js_name()}",
                class: if Some(choice) == selected {
                    format!("color-palette-swatch selected {}", choice.css_class())
                } else {
                    format!("color-palette-swatch {}", choice.css_class())
                },
                onclick: move |_| on_pick.call(choice),
            }
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
    fn every_colour_is_a_swatch_and_the_chosen_one_is_marked() {
        let html = render(|| {
            rsx! {
                SwatchRow { selected: HighlightColor::Pink, on_pick: |_| {} }
            }
        });

        assert_eq!(
            html,
            concat!(
                r#"<button class="color-palette-swatch highlight-green"></button>"#,
                r#"<button class="color-palette-swatch highlight-blue"></button>"#,
                r#"<button class="color-palette-swatch selected highlight-pink"></button>"#,
                r#"<button class="color-palette-swatch highlight-orange"></button>"#,
                r#"<button class="color-palette-swatch highlight-purple"></button>"#,
            )
        );
    }

    #[test]
    fn with_no_colour_chosen_none_is_marked() {
        let html = render(|| {
            rsx! {
                SwatchRow { selected: None, on_pick: |_| {} }
            }
        });

        assert!(!html.contains("selected"), "{html}");
    }
}
