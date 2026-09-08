use super::super::form_controls::{OptionCardItem, OptionCards};
use crate::bookmarks::{BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::icon::{Icon, IconName};
use crate::config::{Config, StartupBehavior};
use dioxus::prelude::*;
use std::path::PathBuf;

/// The folders kept, and what a window starts with.
///
/// There is no "default directory" here any more: a folder worth starting in
/// is a folder worth keeping, so the two became one list. Places are shared
/// by every window and outlive all of them; a window's temporary roots are
/// the window's own business and are not settings.
#[component]
pub fn DirectoryTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    current_directory: Option<PathBuf>,
) -> Element {
    let on_startup = config.read().directory.on_startup;
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so adding or removing a place redraws; the number says nothing.
    let _ = revision();
    let places = BOOKMARKS.read().places();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Places" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Folders you keep" }
                    p {
                        class: "preference-description",
                        "Every window's file tree starts with these, and they outlive the windows that showed them. Starring a folder anywhere in the app adds it here."
                    }
                }

                div {
                    class: "places-list",

                    if places.is_empty() {
                        p { class: "preference-description", "No places yet." }
                    }

                    for place in places {
                        div {
                            key: "{place.display()}",
                            class: "places-row",
                            title: "{place.display()}",
                            Icon { name: IconName::Folder, size: 16 }
                            span { class: "places-row-path", "{place.display()}" }
                            button {
                                class: "places-row-remove",
                                title: "Remove",
                                "aria-label": "Remove {place.display()}",
                                onclick: {
                                    let place = place.clone();
                                    move |_| {
                                        crate::bookmarks::toggle_bookmark(&place);
                                    }
                                },
                                Icon { name: IconName::Trash, size: 14 }
                            }
                        }
                    }

                    div {
                        class: "places-actions",
                        button {
                            class: "places-add",
                            onclick: move |_| {
                                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                    crate::bookmarks::toggle_bookmark(dir);
                                }
                            },
                            Icon { name: IconName::FolderPlus, size: 16 }
                            span { "Add folder…" }
                        }

                        if let Some(current) = current_directory.clone() {
                            if !BOOKMARKS.read().places().contains(&current) {
                                button {
                                    class: "places-add",
                                    onclick: move |_| {
                                        crate::bookmarks::toggle_bookmark(&current);
                                    },
                                    Icon { name: IconName::Star, size: 16 }
                                    span { "Keep the folder this window is reading" }
                                }
                            }
                        }
                    }
                }
            }

            h3 { class: "preference-section-title", "Behavior" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On Startup" }
                    p { class: "preference-description", "What the first window opens with." }
                }
                OptionCards {
                    name: "dir-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Places only".to_string(),
                            description: Some("Start with the folders you keep".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::LastClosed,
                            title: "Last Closed".to_string(),
                            description: Some("Resume the last window's folders".to_string()),
                        },
                    ],
                    selected: on_startup,
                    on_change: move |new_behavior| {
                        config.write().directory.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
