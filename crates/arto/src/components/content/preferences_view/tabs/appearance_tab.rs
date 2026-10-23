use super::super::form_controls::{
    ChoiceItem, OptionCardItem, OptionCards, PreferenceItem, SelectRow, ThemePicker,
};
use crate::components::icon::IconName;
use crate::config::{Config, Language};
use crate::theme::{preview_theme, Theme};
use dioxus::prelude::*;
use rust_i18n::t;

/// The language of Arto's own words, and which of GitHub's themes the
/// window paints.
///
/// What the window does with the theme on the next startup, or in the next
/// window, is asked once for everything in [`super::startup_tab`] rather than
/// again here.
#[component]
pub fn AppearanceTab(config: Signal<Config>) -> Element {
    let language = config.read().language;
    let theme = config.read().theme.clone();
    let defaults = Config::default().theme;

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", {t!("preferences.appearance.language.title").to_string()} }

            LanguageRow {
                selected: language,
                on_change: move |new_language| {
                    config.write().language = new_language;
                },
            }

            h3 { class: "preference-section-title", {t!("preferences.appearance.mode.title").to_string()} }

            PreferenceItem {
                label: t!("preferences.appearance.mode.default_theme.label").to_string(),
                description: t!("preferences.appearance.mode.default_theme.description").to_string(),
                OptionCards {
                    name: "theme-default".to_string(),
                    options: vec![
                        OptionCardItem {
                            value: Theme::Auto,
                            icon: Some(IconName::SunMoon),
                            title: t!("preferences.appearance.mode.default_theme.auto").to_string(),
                            description: None,
                        },
                        OptionCardItem {
                            value: Theme::Light,
                            icon: Some(IconName::Sun),
                            title: t!("preferences.appearance.mode.default_theme.light").to_string(),
                            description: None,
                        },
                        OptionCardItem {
                            value: Theme::Dark,
                            icon: Some(IconName::Moon),
                            title: t!("preferences.appearance.mode.default_theme.dark").to_string(),
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

            h3 { class: "preference-section-title", {t!("preferences.appearance.themes.title").to_string()} }

            PreferenceItem {
                label: t!("preferences.appearance.themes.light.label").to_string(),
                description: t!("preferences.appearance.themes.light.description").to_string(),
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

            PreferenceItem {
                label: t!("preferences.appearance.themes.dark.label").to_string(),
                description: t!("preferences.appearance.themes.dark.description").to_string(),
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

/// The interface language, as a list that grows with the languages Arto has.
///
/// The language this launch shows cannot change under the running app (see
/// `crate::i18n`), so a choice that would show another one offers the restart
/// that brings it about rather than leaving the reader to find it.
#[component]
fn LanguageRow(selected: Language, on_change: EventHandler<Language>) -> Element {
    let label = |language: Language| match crate::i18n::native_name(language) {
        Some(name) => name.to_string(),
        None => t!("preferences.appearance.language.auto").to_string(),
    };
    let options = crate::i18n::CHOICES
        .iter()
        .map(|&language| ChoiceItem {
            value: language,
            label: label(language),
        })
        .collect::<Vec<_>>();
    let needs_restart = crate::i18n::locale_at_launch(selected) != crate::i18n::locale();

    rsx! {
        SelectRow {
            label: t!("preferences.appearance.language.label").to_string(),
            description: t!("preferences.appearance.language.description").to_string(),
            options,
            selected,
            on_change,
            shipped: Config::default().language,
            if needs_restart {
                div {
                    class: "preference-restart",
                    span { {t!("preferences.appearance.language.restart_note").to_string()} }
                    button {
                        class: "use-current-button",
                        onclick: move |_| crate::relaunch::request(),
                        {t!("preferences.appearance.language.restart").to_string()}
                    }
                }
            }
        }
    }
}
