use chrono::Local;
use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::row_actions::RowActions;
use crate::state::AppState;
use crate::visits::{Bucket, Visit, VISITS, VISITS_CHANGED};

/// The whole reading history, newest first.
///
/// The palette answers "back to the last one" and the welcome page answers "what
/// shall I read"; this is the one that answers "everything, while I keep
/// looking at what I am reading" — which is why it is a face of the panel and
/// not another screen.
///
/// Groups coarsen with age, and everything older than last week arrives
/// collapsed to its heading and a count, so several years still fit in a dozen
/// rows.
#[component]
pub fn RecentFace() -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);
    let mut filter = use_signal(String::new);
    // Everything past last week arrives folded, so several years of history
    // still opens as about a dozen rows.
    let mut collapsed = use_signal(|| {
        let visits = VISITS.read();
        default_folds(
            visits
                .grouped(Local::now())
                .iter()
                .map(|(bucket, _)| bucket),
        )
        .into_iter()
        .collect::<std::collections::HashSet<String>>()
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
    let needle = filter();
    let current = state.current_file();
    let now = Local::now();
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
                    let matching: Vec<&Visit> = entries
                        .into_iter()
                        .filter(|visit| crate::visits::matches(visit, &needle))
                        .collect();
                    // Filtering opens what it matched: a hit hidden inside a
                    // collapsed year would look like no hit at all.
                    let folded = collapsed.read().contains(&heading) && needle.is_empty();
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
                                    move |_| {
                                        let mut set = collapsed.write();
                                        if !set.remove(&heading) {
                                            set.insert(heading.clone());
                                        }
                                    }
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
                                        onclick: {
                                            let path = visit.path.clone();
                                            move |_| state.open_file(&path)
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

            // At the foot: the list is what the face is for, and a field
            // above it would be the first thing between the reader and it.
            div {
                class: "left-sidebar-filter",
                Icon { name: IconName::Search, size: 12 }
                input {
                    class: "left-sidebar-filter-input",
                    r#type: "text",
                    placeholder: "Filter",
                    value: "{filter}",
                    oninput: move |evt| filter.set(evt.value()),
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
