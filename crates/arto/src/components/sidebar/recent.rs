use chrono::Local;
use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::context_menu::{open_row_context_menu, SidebarItemKind};
use crate::components::sidebar::row_actions::RowActions;
use crate::state::{AppState, FocusedPanel};
use crate::visits::{Bucket, Visit, VISITS, VISITS_CHANGED};

/// The whole reading history, newest first.
///
/// It answers "everything, while I keep looking at what I am reading", which
/// is why it is a face of the panel and not a screen of its own.
///
/// Groups coarsen with age, and everything older than last week arrives
/// collapsed to its heading and a count, so several years still fit in a dozen
/// rows.
#[component]
pub fn RecentFace() -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);
    // Everything past last week arrives folded, so several years of history
    // still opens as about a dozen rows. It is held in the panel's state, not
    // here: what is folded decides which rows are drawn, and the keyboard
    // cursor has to walk the rows that are drawn.
    use_hook(move || {
        let mut sidebar = state.sidebar.write();
        if sidebar.recent_collapsed.is_empty() {
            let visits = VISITS.read();
            sidebar.recent_collapsed = default_folds(
                visits
                    .grouped(Local::now())
                    .iter()
                    .map(|(bucket, _)| bucket),
            )
            .into_iter()
            .collect();
        }
    });

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit redraws the list; the numbers say nothing.
    let _ = revision();
    let _ = state.visits_revision.read();
    let current = state.current_file();
    let cursor = if *state.focused_panel.read() == FocusedPanel::Panel {
        state.panel_cursor.read().clone()
    } else {
        None
    };
    let now = Local::now();
    let folded_groups = state.sidebar.read().recent_collapsed.clone();
    let visits = VISITS.read();
    let groups = visits.grouped(now);

    rsx! {
        div {
            class: "left-sidebar-face",

            div {
                class: "left-sidebar-face-list",

            if groups.is_empty() {
                div { class: "left-sidebar-explorer-empty", "Nothing read yet" }
            }

            for (bucket, entries) in groups {
                {
                    let heading = bucket.heading();
                    let matching: Vec<&Visit> = entries.into_iter().collect();
                    let folded = folded_groups.contains(&heading);
                    let count = matching.len();

                    if matching.is_empty() {
                        rsx! {}
                    } else {
                        rsx! {
                            div {
                                class: "left-sidebar-group",
                                class: if folded { "folded" },
                                onclick: {
                                    let heading = heading.clone();
                                    move |_| state.sidebar.write().toggle_group(&heading)
                                },
                                span { "{heading}" }
                                span { class: "left-sidebar-group-count", "{count}" }
                            }
                            if !folded {
                                for visit in matching {
                                    div {
                                        class: "left-sidebar-tree-node-content left-sidebar-recent-row",
                                        // The document on screen is in this
                                        // list like any other, and saying so
                                        // is what makes the rest of the list
                                        // read as "before this one".
                                        class: if current.as_deref() == Some(visit.path.as_path()) { "active" },
                                        class: if cursor.as_ref().is_some_and(|(_, at)| *at == visit.path) { "keyboard-focused" },
                                        onclick: {
                                            let path = visit.path.clone();
                                            move |_| state.open_from_panel(&path)
                                        },
                                        oncontextmenu: {
                                            let path = visit.path.clone();
                                            move |evt: Event<MouseData>| {
                                                open_row_context_menu(
                                                    state,
                                                    &path,
                                                    SidebarItemKind::File,
                                                    &evt,
                                                );
                                            }
                                        },
                                        Icon {
                                            name: IconName::File,
                                            size: 16,
                                            class: "left-sidebar-tree-icon",
                                        }
                                        span {
                                            class: "left-sidebar-tree-label",
                                            DocumentName { path: visit.path.clone() }
                                        }
                                        span {
                                            class: "left-sidebar-row-when",
                                            "{crate::visits::short_when(visit.at, now)}"
                                        }
                                        // What a row can do, drawn only while
                                        // the pointer is on it and standing
                                        // where the clock was: the row keeps
                                        // its width, and at rest it is a name.
                                        RowActions {
                                            path: visit.path.clone(),
                                            forgettable: true,
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            }
        }
    }
}

/// Everything older than last week collapses by default, so the list stays
/// about a dozen rows however long the history is.
fn folds_by_default(bucket: Bucket) -> bool {
    !matches!(bucket, Bucket::Today | Bucket::Yesterday | Bucket::ThisWeek)
}

/// The headings that start folded, for a freshly opened face.
pub fn default_folds<'a>(buckets: impl Iterator<Item = &'a Bucket>) -> Vec<String> {
    buckets
        .filter(|bucket| folds_by_default(**bucket))
        .map(|bucket| bucket.heading())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_few_days_stay_open() {
        assert!(!folds_by_default(Bucket::Today));
        assert!(!folds_by_default(Bucket::Yesterday));
        assert!(!folds_by_default(Bucket::ThisWeek));
        assert!(folds_by_default(Bucket::LastWeek));
        assert!(folds_by_default(Bucket::Month(2026, 3)));
        assert!(folds_by_default(Bucket::Year(2024)));
    }

    #[test]
    fn default_folds_names_only_the_older_groups() {
        let buckets = [Bucket::Today, Bucket::LastWeek, Bucket::Year(2024)];
        assert_eq!(
            default_folds(buckets.iter()),
            vec!["Last week".to_string(), "2024".to_string()]
        );
    }
}
