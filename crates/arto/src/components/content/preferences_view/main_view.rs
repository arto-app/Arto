use super::tabs::{
    about_tab::AboutTab, appearance_tab::AppearanceTab, keybindings_tab::KeybindingsTab,
    markdown_tab::MarkdownTab, panel_tab::PanelTab, reading_tab::ReadingTab,
    startup_tab::StartupTab, window_tab::WindowTab,
};
use crate::components::icon::{Icon, IconName};
use crate::config::{Config, CONFIG, CONFIG_CHANGED_BROADCAST};
use crate::window::preferences::PreferencesSnapshot;
use dioxus::prelude::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PreferencesTab {
    #[default]
    Appearance,
    Markdown,
    Reading,
    Panel,
    Window,
    Startup,
    Keybindings,
    About,
}

impl PreferencesTab {
    /// The panes in the order the navigation lists them.
    const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Markdown,
        Self::Reading,
        Self::Panel,
        Self::Window,
        Self::Startup,
        Self::Keybindings,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Markdown => "Markdown",
            Self::Reading => "Reading",
            Self::Panel => "Panel",
            Self::Window => "Window",
            Self::Startup => "Startup",
            Self::Keybindings => "Keybindings",
            Self::About => "About",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Appearance => IconName::SunMoon,
            Self::Markdown => IconName::Markdown,
            Self::Reading => IconName::Book,
            Self::Panel => IconName::Sidebar,
            Self::Window => IconName::AppWindow,
            Self::Startup => IconName::Power,
            Self::Keybindings => IconName::Command,
            Self::About => IconName::InfoCircle,
        }
    }
}

/// Remember the last selected tab in memory
static LAST_PREFERENCES_TAB: LazyLock<RwLock<PreferencesTab>> =
    LazyLock::new(|| RwLock::new(PreferencesTab::default()));

/// Set the preferences tab to About (called from menu)
pub fn set_preferences_tab_to_about() {
    *LAST_PREFERENCES_TAB.write() = PreferencesTab::About;
}

/// How long an edit is left alone before it is applied.
///
/// A slider fires all through a drag, so the writes are coalesced; short
/// enough that a click still looks immediate.
const SETTLE: Duration = Duration::from_millis(200);

/// An edit waiting out [`SETTLE`], held outside the component.
///
/// Not a signal: the window can be closed while an edit is still settling, and
/// what flushes it then runs as the page is being torn down, where the page's
/// own signals are no longer safe to read.
#[derive(Default)]
struct PendingEdit {
    config: parking_lot::Mutex<Option<Config>>,
    /// Which edit the settling tasks are waiting for. A task that wakes to
    /// find the number moved on has been overtaken and does nothing.
    ticket: AtomicU64,
}

impl PendingEdit {
    /// Write the edit out and tell every window about it.
    fn flush(&self) {
        let Some(edited) = self.config.lock().take() else {
            return;
        };
        if let Err(error) = edited.save() {
            tracing::error!(?error, "Failed to save configuration");
            return;
        }
        *CONFIG.write() = edited;
        CONFIG_CHANGED_BROADCAST.send(()).ok();
    }
}

/// Apply every edit as it is made.
///
/// There is no Save button. A setting the reader changed is a decision, not a
/// draft: it goes into the live configuration, out to the windows reading it,
/// and on to disk. Nothing is lost by closing the window, and nothing has to
/// be confirmed — which is also what lets the header say which pane you are in
/// instead of holding a button and a status.
fn use_auto_save(config: Signal<Config>) {
    let pending = use_hook(|| Arc::new(PendingEdit::default()));

    use_effect({
        let pending = pending.clone();
        move || {
            // Reading the whole configuration subscribes this to every field.
            let edited = config();
            if edited == *CONFIG.read() {
                // The configuration as loaded, or an edit already applied.
                return;
            }

            *pending.config.lock() = Some(edited);
            let ticket = pending.ticket.fetch_add(1, Ordering::Relaxed) + 1;

            // Detached, because the write has to happen even if the window
            // that started it is gone by the time it comes due.
            let pending = pending.clone();
            crate::utils::task::spawn_detached(async move {
                tokio::time::sleep(SETTLE).await;
                if pending.ticket.load(Ordering::Relaxed) == ticket {
                    pending.flush();
                }
            });
        }
    });

    // Closing the window is not a way to take an edit back, so one that has
    // not settled yet is written on the way out rather than waited for.
    use_drop(move || {
        pending.ticket.fetch_add(1, Ordering::Relaxed);
        pending.flush();
    });
}

#[component]
pub fn PreferencesView(snapshot: PreferencesSnapshot) -> Element {
    let mut config = use_signal(Config::default);
    let mut active_tab = use_signal(|| *LAST_PREFERENCES_TAB.read());

    // What the "Current Settings" sliders are showing. The snapshot says what
    // the window had when preferences opened; after that these sliders are the
    // only thing moving it, so they keep the value here as well as sending it
    // — a slider that reported the snapshot forever would sit still while the
    // window it acts on zoomed.
    let sidebar_zoom = use_signal(|| snapshot.sidebar_zoom_level);
    let content_zoom = use_signal(|| snapshot.content_zoom_level);

    // Load initial config on mount (use_hook runs only once)
    use_hook(|| {
        let cfg = CONFIG.read().clone();
        config.set(cfg);
    });

    use_auto_save(config);

    // A theme picked here is worn by the window while the page is open, so a
    // dark theme can be judged from light mode. It is dropped with the page,
    // which is what lets the mode decide again.
    use_drop(crate::theme::clear_theme_preview);

    let current_tab = active_tab();
    let mut select = move |tab: PreferencesTab| {
        active_tab.set(tab);
        *LAST_PREFERENCES_TAB.write() = tab;
    };

    rsx! {
        div {
            class: "preferences-page",

            div {
                class: "preferences-page-body",

                nav {
                    class: "preferences-nav",
                    role: "tablist",
                    "aria-label": "Preferences sections",

                    for tab in PreferencesTab::ALL {
                        button {
                            key: "{tab.title()}",
                            class: if current_tab == tab { "nav-tab active" } else { "nav-tab" },
                            role: "tab",
                            "aria-selected": current_tab == tab,
                            onclick: move |_| select(tab),
                            Icon { name: tab.icon(), size: 18 }
                            span { "{tab.title()}" }
                        }
                    }

                    // Spacer to push About to bottom
                    div { class: "nav-spacer" }

                    button {
                        class: if current_tab == PreferencesTab::About { "nav-tab active" } else { "nav-tab" },
                        role: "tab",
                        "aria-selected": current_tab == PreferencesTab::About,
                        onclick: move |_| select(PreferencesTab::About),
                        Icon { name: PreferencesTab::About.icon(), size: 18 }
                        span { "{PreferencesTab::About.title()}" }
                    }
                }

                div {
                    class: "preferences-settings",
                    role: "tabpanel",

                    if current_tab != PreferencesTab::About {
                        div {
                            class: "preferences-settings-header",
                            h2 { class: "preferences-pane-title", "{current_tab.title()}" }
                        }
                    }

                    match current_tab {
                        PreferencesTab::Appearance => rsx! {
                            AppearanceTab { config }
                        },
                        PreferencesTab::Markdown => rsx! {
                            MarkdownTab { config }
                        },
                        PreferencesTab::Reading => rsx! {
                            ReadingTab {
                                config,
                                window_id: snapshot.window_id,
                                current_zoom: content_zoom,
                            }
                        },
                        PreferencesTab::Panel => rsx! {
                            PanelTab {
                                config,
                                window_id: snapshot.window_id,
                                current_width: snapshot.sidebar_width,
                                current_zoom: sidebar_zoom,
                            }
                        },
                        PreferencesTab::Window => rsx! {
                            WindowTab {
                                config,
                                current_directory: snapshot.directory.clone(),
                            }
                        },
                        PreferencesTab::Startup => rsx! {
                            StartupTab { config }
                        },
                        PreferencesTab::Keybindings => rsx! {
                            KeybindingsTab { config }
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
