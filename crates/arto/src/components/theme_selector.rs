use dioxus::document;
use dioxus::prelude::*;
use dioxus_sdk_window::theme::use_system_theme;

use crate::components::icon::{Icon, IconName};
use crate::theme::{DioxusTheme, Theme};

#[component]
pub fn ThemeSelector(current_theme: Signal<Theme>) -> Element {
    let system_theme = use_system_theme();
    let resolved_theme = use_memo(move || match current_theme() {
        Theme::Auto => system_theme().unwrap_or(DioxusTheme::Light),
        Theme::Light => DioxusTheme::Light,
        Theme::Dark => DioxusTheme::Dark,
    });

    // Dispatch custom event when resolved theme changes
    use_effect(move || {
        let theme = resolved_theme();
        let theme_str = match theme {
            DioxusTheme::Light => "light",
            DioxusTheme::Dark => "dark",
        };
        tracing::info!("Theme changed to: {}", theme_str);
        let theme_str_owned = theme_str.to_string();
        spawn(async move {
            tracing::info!("Dispatching theme-changed event: {}", theme_str_owned);
            let _ = document::eval(&format!(
                "document.dispatchEvent(new CustomEvent('arto:theme-changed', {{ detail: '{}' }}))",
                theme_str_owned
            ))
            .await;
        });
    });

    // Expansion state for dropdown menu
    let mut is_expanded = use_signal(|| false);

    // Listen for clicks outside the theme selector
    use_hook(|| {
        spawn(async move {
            loop {
                let _ = document::eval(
                    r#"
                    await new Promise((resolve) => {
                        const handler = (e) => {
                            const selector = e.target.closest('.theme-selector');
                            if (!selector) {
                                // Outside click, resolve to close expanded.
                                resolve();
                            } else {
                                // Inside click, re-listen mousedown event.
                                document.addEventListener('mousedown', handler, { once: true });
                            }
                        };
                        document.addEventListener('mousedown', handler, { once: true });
                    })
                    "#,
                )
                .await;

                // Only close if actually expanded
                if is_expanded() {
                    is_expanded.set(false);
                }
            }
        });
    });

    // Get current theme icon and title
    let (current_icon, current_title) = match current_theme() {
        Theme::Light => (IconName::Sun, "Light theme"),
        Theme::Dark => (IconName::Moon, "Dark theme"),
        Theme::Auto => (IconName::SunMoon, "Auto theme (follows system)"),
    };

    // Get other theme options (remaining 2 themes)
    let other_themes = match current_theme() {
        Theme::Light => [
            (Theme::Dark, IconName::Moon, "Dark theme"),
            (
                Theme::Auto,
                IconName::SunMoon,
                "Auto theme (follows system)",
            ),
        ],
        Theme::Dark => [
            (Theme::Light, IconName::Sun, "Light theme"),
            (
                Theme::Auto,
                IconName::SunMoon,
                "Auto theme (follows system)",
            ),
        ],
        Theme::Auto => [
            (Theme::Light, IconName::Sun, "Light theme"),
            (Theme::Dark, IconName::Moon, "Dark theme"),
        ],
    };

    rsx! {
        div {
            class: "theme-selector",

            // Main button (current theme)
            button {
                class: "theme-selector-main",
                "aria-expanded": if is_expanded() { "true" } else { "false" },
                "aria-haspopup": "menu",
                title: current_title,
                onmousedown: move |evt| {
                    evt.stop_propagation();
                },
                onclick: move |evt| {
                    evt.stop_propagation();
                    is_expanded.set(!is_expanded());
                },
                Icon { name: current_icon, size: 18 }
            }

            // Dropdown menu (remaining 2 themes)
            div {
                class: "theme-selector-dropdown",
                class: if is_expanded() { "theme-selector-dropdown--expanded" },
                role: "menu",
                onmousedown: move |evt| {
                    evt.stop_propagation();
                },

                for (theme, icon, title) in other_themes {
                    button {
                        class: "theme-option",
                        role: "menuitem",
                        title: title,
                        onmousedown: move |evt| {
                            evt.stop_propagation();
                        },
                        onclick: move |evt| {
                            evt.stop_propagation();
                            let mut current_theme = current_theme;
                            current_theme.set(theme);
                            is_expanded.set(false);
                        },
                        Icon { name: icon, size: 18 }
                    }
                }
            }
        }
    }
}
