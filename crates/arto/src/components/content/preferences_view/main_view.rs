use super::tabs::{
    about_tab::AboutTab, directory_tab::DirectoryTab, general_tab::GeneralTab,
    keybindings_tab::KeybindingsTab, right_sidebar_tab::RightSidebarTab, sidebar_tab::SidebarTab,
    theme_tab::ThemeTab, window_position_tab::WindowPositionTab, window_size_tab::WindowSizeTab,
};
use crate::components::icon::{Icon, IconName};
use crate::config::{Config, CONFIG, CONFIG_CHANGED_BROADCAST};
use crate::state::AppState;
use dioxus::prelude::*;
use parking_lot::RwLock;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PreferencesTab {
    #[default]
    General,
    Theme,
    Directory,
    WindowSize,
    WindowPosition,
    Sidebar,
    RightSidebar,
    Keybindings,
    About,
}

/// Remember the last selected tab in memory
static LAST_PREFERENCES_TAB: LazyLock<RwLock<PreferencesTab>> =
    LazyLock::new(|| RwLock::new(PreferencesTab::default()));

/// Set the preferences tab to About (called from menu)
pub fn set_preferences_tab_to_about() {
    *LAST_PREFERENCES_TAB.write() = PreferencesTab::About;
}

/// Save status for the preferences page
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SaveStatus {
    #[default]
    Idle,
    Saving,
    Saved,
}

#[component]
pub fn PreferencesView() -> Element {
    let state = use_context::<AppState>();
    let mut config = use_signal(Config::default);
    let mut has_changes = use_signal(|| false);
    let mut active_tab = use_signal(|| *LAST_PREFERENCES_TAB.read());
    let mut save_status = use_signal(|| SaveStatus::Idle);

    // Load initial config on mount (use_hook runs only once)
    use_hook(|| {
        let cfg = CONFIG.read().clone();
        config.set(cfg);
        has_changes.set(false);
    });

    let handle_save = move |_| {
        let cfg = config().clone();
        save_status.set(SaveStatus::Saving);
        spawn(async move {
            if let Err(e) = cfg.save() {
                tracing::error!("Failed to save configuration: {:?}", e);
                save_status.set(SaveStatus::Idle);
            } else {
                *CONFIG.write() = cfg.clone();
                CONFIG_CHANGED_BROADCAST.send(()).ok();
                has_changes.set(false);
                save_status.set(SaveStatus::Saved);
                // Reset to idle after showing success
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                save_status.set(SaveStatus::Idle);
            }
        });
    };

    let current_tab = active_tab();
    let current_save_status = *save_status.read();

    rsx! {
        div {
            class: "preferences-page",

            // Navigation and settings
            div {
                class: "preferences-page-body",

                // Left navigation sidebar
                nav {
                    class: "preferences-nav",
                    button {
                        class: if current_tab == PreferencesTab::General { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::General);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::General;
                        },
                        Icon { name: IconName::Gear, size: 18 }
                        span { "General" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::Theme { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::Theme);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::Theme;
                        },
                        Icon { name: IconName::SunMoon, size: 18 }
                        span { "Theme" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::Directory { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::Directory);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::Directory;
                        },
                        Icon { name: IconName::Folder, size: 18 }
                        span { "Directory" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::WindowSize { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::WindowSize);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::WindowSize;
                        },
                        Icon { name: IconName::ArrowsDiagonal, size: 18 }
                        span { "Window Size" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::WindowPosition { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::WindowPosition);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::WindowPosition;
                        },
                        Icon { name: IconName::ArrowsMove, size: 18 }
                        span { "Window Position" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::Sidebar { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::Sidebar);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::Sidebar;
                        },
                        Icon { name: IconName::Sidebar, size: 18 }
                        span { "Sidebar" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::RightSidebar { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::RightSidebar);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::RightSidebar;
                        },
                        Icon { name: IconName::List, size: 18 }
                        span { "Right Sidebar" }
                    }
                    button {
                        class: if current_tab == PreferencesTab::Keybindings { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::Keybindings);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::Keybindings;
                        },
                        Icon { name: IconName::Command, size: 18 }
                        span { "Keybindings" }
                    }

                    // Spacer to push About to bottom
                    div { class: "nav-spacer" }

                    button {
                        class: if current_tab == PreferencesTab::About { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(PreferencesTab::About);
                            *LAST_PREFERENCES_TAB.write() = PreferencesTab::About;
                        },
                        Icon { name: IconName::InfoCircle, size: 18 }
                        span { "About" }
                    }
                }

                // Settings content area
                div {
                    class: "preferences-settings",

                    // Header with save status
                    div {
                        class: "preferences-settings-header",
                        div {
                            class: "save-status",
                            match current_save_status {
                                SaveStatus::Idle if has_changes() => rsx! {
                                    button {
                                        class: "save-button",
                                        onclick: handle_save,
                                        "Save Changes"
                                    }
                                },
                                SaveStatus::Saving => rsx! {
                                    span { class: "saving", "Saving..." }
                                },
                                SaveStatus::Saved => rsx! {
                                    span { class: "saved", "Saved!" }
                                },
                                _ => rsx! {},
                            }
                        }
                    }

                    // Tab content
                    match current_tab {
                        PreferencesTab::General => rsx! {
                            GeneralTab {
                                config,
                                has_changes,
                            }
                        },
                        PreferencesTab::Theme => rsx! {
                            ThemeTab {
                                config,
                                has_changes,
                            }
                        },
                        PreferencesTab::Directory => rsx! {
                            DirectoryTab {
                                config,
                                has_changes,
                                current_directory: state.sidebar.read().root_directory.clone(),
                            }
                        },
                        PreferencesTab::WindowSize => rsx! {
                            WindowSizeTab {
                                config,
                                has_changes,
                            }
                        },
                        PreferencesTab::WindowPosition => rsx! {
                            WindowPositionTab {
                                config,
                                has_changes,
                            }
                        },
                        PreferencesTab::Sidebar => rsx! {
                            SidebarTab {
                                config,
                                has_changes,
                                state,
                            }
                        },
                        PreferencesTab::RightSidebar => rsx! {
                            RightSidebarTab {
                                config,
                                has_changes,
                                state,
                            }
                        },
                        PreferencesTab::Keybindings => rsx! {
                            KeybindingsTab {
                                config,
                                has_changes,
                            }
                        },
                        PreferencesTab::About => rsx! {
                            AboutTab {}
                        },
                    }
                }
            }
        }
    }
}
