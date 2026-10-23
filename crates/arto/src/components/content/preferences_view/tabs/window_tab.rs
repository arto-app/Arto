use super::super::form_controls::{
    DimensionInput, DirectoryPicker, OptionCardItem, OptionCards, PreferenceItem, ResetLine,
};
use crate::components::icon::IconName;
use crate::config::{
    Config, FileOpenBehavior, WindowDimension, WindowDimensionUnit, WindowPosition,
    WindowPositionMode, WindowSize,
};
use crate::state::{Position, Size};
use dioxus::prelude::*;
use rust_i18n::t;
use std::path::PathBuf;

/// Windows: how large they open, where they land, and which one a document
/// arrives in.
///
/// The startup folder is here for the same reason the routing is: both answer
/// what a window is holding when it appears, not what the file tree contains.
/// The places the reader keeps are not a setting at all — they are kept by
/// starring a folder, and every window already starts with all of them.
#[component]
pub fn WindowTab(
    config: Signal<Config>,
    current_directory: Option<PathBuf>,
    current_size: Size,
    current_position: Position,
) -> Element {
    let size_cfg = config.read().window_size.clone();
    let position_cfg = config.read().window_position.clone();
    let file_open = config.read().file_open;
    let default_directory = config.read().directory.default_directory.clone();
    let defaults = Config::default();

    let use_current_size = move |_| {
        config.write().window_size.default_size = pixel_size(current_size);
    };

    let use_current_position = move |_| {
        let mut cfg = config.write();
        cfg.window_position.default_position = pixel_position(current_position);
        cfg.window_position.default_position_mode = WindowPositionMode::Coordinates;
    };

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", {t!("preferences.window.size.title").to_string()} }

            PreferenceItem {
                label: t!("preferences.window.size.default_size.label").to_string(),
                description: t!("preferences.window.size.default_size.description").to_string(),
                div {
                    class: "dimension-row",
                    div {
                        class: "dimension-grid",
                        div {
                            class: "dimension-field",
                            label { {t!("preferences.window.size.width").to_string()} }
                            DimensionInput {
                                value: size_cfg.default_size.width,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: false,
                                on_change: move |new_value| {
                                    config.write().window_size.default_size.width = new_value;
                                },
                                shipped: Some(defaults.window_size.default_size.width),
                            }
                        }
                        div {
                            class: "dimension-field",
                            label { {t!("preferences.window.size.height").to_string()} }
                            DimensionInput {
                                value: size_cfg.default_size.height,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: false,
                                on_change: move |new_value| {
                                    config.write().window_size.default_size.height = new_value;
                                },
                                shipped: Some(defaults.window_size.default_size.height),
                            }
                        }
                    }
                    button {
                        class: "use-current-button",
                        onclick: use_current_size,
                        {t!("preferences.controls.use_current").to_string()}
                    }
                }
            }

            h3 { class: "preference-section-title", {t!("preferences.window.position.title").to_string()} }

            PreferenceItem {
                label: t!("preferences.window.position.default_position.label").to_string(),
                description: t!("preferences.window.position.default_position.description").to_string(),
                OptionCards {
                    name: "window-position-mode".to_string(),
                    options: vec![
                        OptionCardItem {
                            value: WindowPositionMode::Coordinates,
                            icon: Some(IconName::Command),
                            title: t!("preferences.window.position.default_position.coordinates.title").to_string(),
                            description: Some(t!("preferences.window.position.default_position.coordinates.description").to_string()),
                        },
                        OptionCardItem {
                            value: WindowPositionMode::Mouse,
                            icon: Some(IconName::Click),
                            title: t!("preferences.window.position.default_position.mouse.title").to_string(),
                            description: Some(t!("preferences.window.position.default_position.mouse.description").to_string()),
                        },
                    ],
                    selected: position_cfg.default_position_mode,
                    on_change: move |new_mode| {
                        config.write().window_position.default_position_mode = new_mode;
                    },
                    shipped: Some(defaults.window_position.default_position_mode),
                }
                div {
                    class: "dimension-row spacing-top-sm",
                    div {
                        class: "dimension-grid",
                        div {
                            class: "dimension-field",
                            label { "X" }
                            DimensionInput {
                                value: position_cfg.default_position.x,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: true,
                                on_change: move |new_value| {
                                    config.write().window_position.default_position.x = new_value;
                                },
                                shipped: Some(defaults.window_position.default_position.x),
                            }
                        }
                        div {
                            class: "dimension-field",
                            label { "Y" }
                            DimensionInput {
                                value: position_cfg.default_position.y,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: true,
                                on_change: move |new_value| {
                                    config.write().window_position.default_position.y = new_value;
                                },
                                shipped: Some(defaults.window_position.default_position.y),
                            }
                        }
                    }
                    button {
                        class: "use-current-button",
                        onclick: use_current_position,
                        {t!("preferences.controls.use_current").to_string()}
                    }
                }
            }

            PreferenceItem {
                label: t!("preferences.window.position.offset.label").to_string(),
                description: t!("preferences.window.position.offset.description").to_string(),
                div {
                    class: "dimension-grid",
                    div {
                        class: "dimension-field",
                        label { "X" }
                        div {
                            class: "dimension-input",
                            input {
                                r#type: "number",
                                inputmode: "decimal",
                                min: "0",
                                step: "1",
                                value: "{position_cfg.position_offset.x}",
                                oninput: move |evt| {
                                    let fallback = config.read().window_position.position_offset.x;
                                    let value = evt.value().parse::<i32>().unwrap_or(fallback);
                                    config.write().window_position.position_offset.x = value.max(0);
                                },
                            }
                            span { class: "dimension-unit", "px" }
                        }
                    }
                    div {
                        class: "dimension-field",
                        label { "Y" }
                        div {
                            class: "dimension-input",
                            input {
                                r#type: "number",
                                inputmode: "decimal",
                                min: "0",
                                step: "1",
                                value: "{position_cfg.position_offset.y}",
                                oninput: move |evt| {
                                    let fallback = config.read().window_position.position_offset.y;
                                    let value = evt.value().parse::<i32>().unwrap_or(fallback);
                                    config.write().window_position.position_offset.y = value.max(0);
                                },
                            }
                            span { class: "dimension-unit", "px" }
                        }
                    }
                }
                // Two raw fields rather than a shared control, so the way back
                // is stated here instead of by the control.
                ResetLine {
                    shipped: (position_cfg.position_offset != defaults.window_position.position_offset)
                        .then(|| {
                            format!(
                                "{} × {}px",
                                defaults.window_position.position_offset.x,
                                defaults.window_position.position_offset.y,
                            )
                        }),
                    on_reset: move |_| {
                        config.write().window_position.position_offset =
                            Config::default().window_position.position_offset;
                    },
                }
            }

            h3 { class: "preference-section-title", {t!("preferences.window.opening.title").to_string()} }

            PreferenceItem {
                label: t!("preferences.window.opening.file_open.label").to_string(),
                description: t!("preferences.window.opening.file_open.description").to_string(),
                OptionCards {
                    name: "window-file-open".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::NewWindow,
                            title: t!("preferences.window.opening.file_open.new_window.title").to_string(),
                            description: Some(t!("preferences.window.opening.file_open.new_window.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::LastFocused,
                            title: t!("preferences.window.opening.file_open.last_focused.title").to_string(),
                            description: Some(t!("preferences.window.opening.file_open.last_focused.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::CurrentScreen,
                            title: t!("preferences.window.opening.file_open.current_screen.title").to_string(),
                            description: Some(t!("preferences.window.opening.file_open.current_screen.description").to_string()),
                        },
                    ],
                    selected: file_open,
                    on_change: move |new_behavior| {
                        config.write().file_open = new_behavior;
                    },
                    shipped: Some(defaults.file_open),
                }
            }

            PreferenceItem {
                label: t!("preferences.window.opening.start_folder.label").to_string(),
                description: t!("preferences.window.opening.start_folder.description").to_string(),
                DirectoryPicker {
                    value: default_directory,
                    placeholder: t!("preferences.window.opening.start_folder.placeholder").to_string(),
                    current_directory: current_directory.clone(),
                    on_change: move |new_directory| {
                        config.write().directory.default_directory = new_directory;
                    },
                    shipped: Some(defaults.directory.default_directory.clone()),
                }
            }
        }
    }
}

fn pixels(value: f64) -> WindowDimension {
    WindowDimension {
        value,
        unit: WindowDimensionUnit::Pixels,
    }
}

/// A window's measured size as a configured default.
fn pixel_size(size: Size) -> WindowSize {
    WindowSize {
        width: pixels(size.width as f64),
        height: pixels(size.height as f64),
    }
}

/// A window's measured position as a configured default.
fn pixel_position(position: Position) -> WindowPosition {
    WindowPosition {
        x: pixels(position.x as f64),
        y: pixels(position.y as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_openers_size_becomes_a_size_in_pixels() {
        let size = pixel_size(Size {
            width: 1280,
            height: 800,
        });

        assert_eq!(
            size,
            WindowSize {
                width: pixels(1280.0),
                height: pixels(800.0),
            }
        );
    }

    #[test]
    fn the_openers_position_becomes_a_position_in_pixels() {
        let position = pixel_position(Position { x: -40, y: 25 });

        assert_eq!(
            position,
            WindowPosition {
                x: pixels(-40.0),
                y: pixels(25.0),
            }
        );
    }
}
