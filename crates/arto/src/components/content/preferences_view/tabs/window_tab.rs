use super::super::form_controls::{
    DimensionInput, DirectoryPicker, OptionCardItem, OptionCards, ResetLine,
};
use crate::components::icon::IconName;
use crate::config::{
    Config, FileOpenBehavior, WindowDimension, WindowDimensionUnit, WindowPosition,
    WindowPositionMode, WindowSize,
};
use dioxus::prelude::*;
use dioxus_desktop::window;
use std::path::PathBuf;

/// Windows: how large they open, where they land, and which one a document
/// arrives in.
///
/// The startup folder is here for the same reason the routing is: both answer
/// what a window is holding when it appears, not what the file tree contains.
/// The places the reader keeps are not a setting at all — they are kept by
/// starring a folder, and every window already starts with all of them.
#[component]
pub fn WindowTab(config: Signal<Config>, current_directory: Option<PathBuf>) -> Element {
    let size_cfg = config.read().window_size.clone();
    let position_cfg = config.read().window_position.clone();
    let file_open = config.read().file_open;
    let default_directory = config.read().directory.default_directory.clone();
    let defaults = Config::default();

    let use_current_size = move |_| {
        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        config.write().window_size.default_size = WindowSize {
            width: WindowDimension {
                value: metrics.size.width as f64,
                unit: WindowDimensionUnit::Pixels,
            },
            height: WindowDimension {
                value: metrics.size.height as f64,
                unit: WindowDimensionUnit::Pixels,
            },
        };
    };

    let use_current_position = move |_| {
        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        let mut cfg = config.write();
        cfg.window_position.default_position = WindowPosition {
            x: WindowDimension {
                value: metrics.position.x as f64,
                unit: WindowDimensionUnit::Pixels,
            },
            y: WindowDimension {
                value: metrics.position.y as f64,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        cfg.window_position.default_position_mode = WindowPositionMode::Coordinates;
    };

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Size" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Size" }
                    p { class: "preference-description", "How large a window opens. Percent values are relative to the current screen." }
                }
                div {
                    class: "dimension-row",
                    div {
                        class: "dimension-grid",
                        div {
                            class: "dimension-field",
                            label { "Width" }
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
                            label { "Height" }
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
                        "Use Current"
                    }
                }
            }

            h3 { class: "preference-section-title", "Position" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Position" }
                    p { class: "preference-description", "Where a window lands. Percent values place it within the available screen area (0% = top/left, 100% = bottom/right)." }
                }
                OptionCards {
                    name: "window-position-mode".to_string(),
                    options: vec![
                        OptionCardItem {
                            value: WindowPositionMode::Coordinates,
                            icon: Some(IconName::Command),
                            title: "Coordinates".to_string(),
                            description: Some("Use the X/Y values below".to_string()),
                        },
                        OptionCardItem {
                            value: WindowPositionMode::Mouse,
                            icon: Some(IconName::Click),
                            title: "Mouse Position".to_string(),
                            description: Some("Open at the current mouse location".to_string()),
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
                        "Use Current"
                    }
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Position Offset" }
                    p { class: "preference-description", "If another window already uses a nearby position, shift the new window by this offset (pixels only)." }
                }
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

            h3 { class: "preference-section-title", "Opening" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Where a File Opens" }
                    p {
                        class: "preference-description",
                        "Which window receives a file or folder opened from Finder, the command line, or another instance of Arto."
                    }
                }
                OptionCards {
                    name: "window-file-open".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::NewWindow,
                            title: "New Window".to_string(),
                            description: Some("Always create a new window".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::LastFocused,
                            title: "Last Focused".to_string(),
                            description: Some("Open in the last focused visible window".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FileOpenBehavior::CurrentScreen,
                            title: "Current Screen".to_string(),
                            description: Some("Open in a visible window on the cursor screen".to_string()),
                        },
                    ],
                    selected: file_open,
                    on_change: move |new_behavior| {
                        config.write().file_open = new_behavior;
                    },
                    shipped: Some(defaults.file_open),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Folder to Start In" }
                    p {
                        class: "preference-description",
                        "The folder the first window works in, beside the places you keep. Leave it empty to start with the places alone."
                    }
                }
                DirectoryPicker {
                    value: default_directory,
                    placeholder: "No folder — the places alone".to_string(),
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
