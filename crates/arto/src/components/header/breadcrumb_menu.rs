use std::path::Path;

use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, Face};
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many documents the breadcrumb drops.
///
/// Enough to cover a morning's reading; the last row goes to the face that
/// holds the rest, so this list never has to grow into a browser.
const MAX_ROWS: usize = 12;

/// The name of what is being read, and the way back to what was read before.
///
/// The header used to state the file name and stop there. The name is also
/// the natural place to ask "what else have I had open", so it became the
/// control that answers: clicking it drops the recent documents, and the last
/// row opens the panel's Recent face with all of them.
#[component]
pub fn Breadcrumb(label: String) -> Element {
    let mut state = use_context::<AppState>();
    let mut is_open = use_signal(|| false);
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit redraws the list; the numbers say nothing.
    let _ = revision();
    let _ = state.visits_revision.read();

    let now = chrono::Local::now();
    let current = state.current_file();
    // The folders between the root the document was reached through and the
    // document itself. It truncates from the left, so what survives at any
    // width is the part nearest the name.
    let prefix = current
        .as_deref()
        .and_then(|file| {
            let sidebar = state.sidebar.read();
            let root = sidebar.roots.covering(file)?;
            let parent = file.parent()?;
            Some(trail(root, parent))
        })
        .unwrap_or_default();
    let rows: Vec<Visit> = {
        let visits = VISITS.read();
        visits
            .items
            .iter()
            // What is on screen is not somewhere to go back to.
            .filter(|visit| current.as_deref() != Some(visit.path.as_path()))
            .take(MAX_ROWS)
            .cloned()
            .collect()
    };

    rsx! {
        div {
            class: "breadcrumb",

            button {
                class: "breadcrumb-label",
                class: if is_open() { "open" },
                // No `title`: it would land on the list this opens. See the
                // app menu's glyph in `components::header`.
                "aria-label": "Recently read",
                onclick: move |_| is_open.toggle(),
                if !prefix.is_empty() {
                    span { class: "breadcrumb-prefix", "{prefix}" }
                }
                span { class: "file-name", "{label}" }
                Icon { name: IconName::ChevronDown, size: 12 }
            }

            if is_open() {
                // Catches the click that closes the menu, so the rows below
                // do not have to guess where the pointer went.
                div {
                    class: "breadcrumb-backdrop",
                    onclick: move |_| is_open.set(false),
                }

                div {
                    class: "breadcrumb-menu",

                    if rows.is_empty() {
                        div { class: "breadcrumb-empty", "Nothing else read yet" }
                    }

                    // Grouped by day, like every other window on this history:
                    // "before this one" is a question about time, and the
                    // heading is what makes the clock beside each row mean
                    // something.
                    for (bucket, entries) in crate::visits::group(&rows, now) {
                        div { class: "breadcrumb-group", "{bucket.heading()}" }
                        for visit in entries {
                            div {
                                key: "{visit.path.display()}",
                                class: "breadcrumb-row",
                                title: "{visit.path.display()}",
                                onclick: {
                                    let path = visit.path.clone();
                                    move |_| {
                                        state.open_file(&path);
                                        is_open.set(false);
                                    }
                                },
                                Icon { name: IconName::File, size: 14 }
                                span {
                                    class: "breadcrumb-row-name",
                                    DocumentName { path: visit.path.clone() }
                                }
                                span {
                                    class: "breadcrumb-row-when",
                                    "{crate::visits::short_when(visit.at, now)}"
                                }
                            }
                        }
                    }

                    div {
                        class: "breadcrumb-row breadcrumb-row-all",
                        onclick: move |_| {
                            state.show_face(Face::Recent);
                            is_open.set(false);
                        },
                        Icon { name: IconName::History, size: 14 }
                        span { class: "breadcrumb-row-name", "All history…" }
                    }
                }
            }
        }
    }
}

/// The folders from `root` down to `parent`, as `arto / docs /`.
///
/// The root's own name leads, because knowing which of several roots the
/// document came from is the question the trail actually answers.
fn trail(root: &Path, parent: &Path) -> String {
    let root_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let mut names = vec![root_name];
    if let Ok(rest) = parent.strip_prefix(root) {
        names.extend(
            rest.components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned()),
        );
    }

    // The trailing separator ends the string with neutral characters, and the
    // element they land in is `direction: rtl` so that it truncates from the
    // left. Bidi resolves trailing neutrals to the paragraph's direction,
    // which put the separator at the visual *start* — "/ demo README.md". A
    // left-to-right mark closes the run with a strong character, so the whole
    // trail is one LTR run and reads in the order it was written.
    format!("{} / \u{200e}", names.join(" / "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_trail_starts_at_the_root_it_came_from() {
        let root = PathBuf::from("/home/reader/arto");
        let parent = PathBuf::from("/home/reader/arto/docs/design");
        assert_eq!(trail(&root, &parent), "arto / docs / design / \u{200e}");
    }

    #[test]
    fn a_document_in_the_root_names_only_the_root() {
        let root = PathBuf::from("/home/reader/arto");
        assert_eq!(trail(&root, &root), "arto / \u{200e}");
    }

    #[test]
    fn a_parent_outside_the_root_still_names_the_root() {
        // `covering` only hands over a root the file is under, so this is a
        // defensive case rather than one the app reaches.
        let root = PathBuf::from("/home/reader/arto");
        let parent = PathBuf::from("/elsewhere");
        assert_eq!(trail(&root, &parent), "arto / \u{200e}");
    }
}
