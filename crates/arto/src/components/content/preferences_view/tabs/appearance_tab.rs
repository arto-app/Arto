use super::super::form_controls::{OptionCardItem, OptionCards, ThemePicker};
use crate::components::icon::IconName;
use crate::config::Config;
use crate::theme::{preview_theme, Theme};
use dioxus::prelude::*;

/// Which of GitHub's themes the window paints.
///
/// What the window does with the theme on the next startup, or in the next
/// window, is asked once for everything in [`super::startup_tab`] rather than
/// again here.
#[component]
pub fn AppearanceTab(config: Signal<Config>) -> Element {
    let theme = config.read().theme.clone();
    let defaults = Config::default().theme;

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Mode" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Theme" }
                    p { class: "preference-description", "Whether to paint the light theme, the dark theme, or the one the system asks for." }
                }
                OptionCards {
                    name: "theme-default".to_string(),
                    options: vec![
                        OptionCardItem {
                            value: Theme::Auto,
                            icon: Some(IconName::SunMoon),
                            title: "Auto".to_string(),
                            description: None,
                        },
                        OptionCardItem {
                            value: Theme::Light,
                            icon: Some(IconName::Sun),
                            title: "Light".to_string(),
                            description: None,
                        },
                        OptionCardItem {
                            value: Theme::Dark,
                            icon: Some(IconName::Moon),
                            title: "Dark".to_string(),
                            description: None,
                        },
                    ],
                    selected: theme.default_theme,
                    on_change: move |new_theme| {
                        config.write().theme.default_theme = new_theme;
                    },
                    shipped: Some(defaults.default_theme),
                }
            }

            h3 { class: "preference-section-title", "Themes" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Light Theme" }
                    p { class: "preference-description", "Which of GitHub's themes to paint in light mode." }
                }
                ThemePicker {
                    name: "theme-light".to_string(),
                    dark_mode: false,
                    selected: theme.light_theme,
                    on_change: move |new_theme| {
                        config.write().theme.light_theme = new_theme;
                        preview_theme(new_theme);
                    },
                    shipped: Some(defaults.light_theme),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Dark Theme" }
                    p { class: "preference-description", "Which of GitHub's themes to paint in dark mode. A light theme is a valid choice here." }
                }
                ThemePicker {
                    name: "theme-dark".to_string(),
                    dark_mode: true,
                    selected: theme.dark_theme,
                    on_change: move |new_theme| {
                        config.write().theme.dark_theme = new_theme;
                        preview_theme(new_theme);
                    },
                    shipped: Some(defaults.dark_theme),
                }
            }
        }
    }
}
