use dioxus::prelude::*;

/// A panel over the page that a click anywhere else puts away.
///
/// The backdrop is drawn beside the panel rather than around it, so each
/// is placed by its own class and the panel's position never depends on the
/// backdrop's box.
#[component]
pub fn Popover(
    backdrop_class: &'static str,
    #[props(into)] class: String,
    #[props(default)] style: Option<String>,
    #[props(default)] role: Option<&'static str>,
    /// Keep a press on the popover from clearing the page's text selection,
    /// which a menu acting on that selection needs.
    #[props(default)]
    keep_selection: bool,
    /// Stop the click that puts the popover away at the backdrop, for a
    /// popover inside a row that would otherwise take the click as its own.
    #[props(default)]
    contain_dismiss: bool,
    /// Stop clicks inside the panel at the panel.
    #[props(default)]
    contain_clicks: bool,
    on_close: EventHandler<()>,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: backdrop_class,
            onmousedown: move |evt| {
                if keep_selection {
                    evt.prevent_default();
                }
            },
            onclick: move |evt| {
                if contain_dismiss {
                    evt.stop_propagation();
                }
                on_close.call(());
            },
        }
        div {
            class,
            style,
            role,
            onmousedown: move |evt| {
                if keep_selection {
                    evt.prevent_default();
                }
            },
            onclick: move |evt| {
                if contain_clicks {
                    evt.stop_propagation();
                }
            },
            {children}
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
    fn the_backdrop_is_drawn_beside_the_panel() {
        let html = render(|| {
            rsx! {
                Popover {
                    backdrop_class: "menu-backdrop",
                    class: "menu",
                    on_close: |_| {},
                    span { "row" }
                }
            }
        });

        assert_eq!(
            html,
            r#"<div class="menu-backdrop"></div><div class="menu"><span>row</span></div>"#
        );
    }

    #[test]
    fn the_panel_carries_its_place_and_role() {
        let html = render(|| {
            rsx! {
                Popover {
                    backdrop_class: "menu-backdrop",
                    class: "menu",
                    style: "left: 1px;".to_string(),
                    role: "menu",
                    on_close: |_| {},
                }
            }
        });

        assert_eq!(
            html,
            r#"<div class="menu-backdrop"></div><div class="menu" style="left: 1px;" role="menu"></div>"#
        );
    }
}
