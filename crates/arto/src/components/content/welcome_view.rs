use chrono::Local;
use dioxus::prelude::*;
use rust_i18n::t;
use std::path::PathBuf;

use crate::bookmarks::{BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, Face};
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many documents the welcome page offers before deferring to the Recent face.
///
/// A landing screen is for picking up where reading left off, not for
/// browsing years; the panel's face is the browser.
const MAX_ROWS: usize = 24;

/// What a window shows when it is not showing a document.
///
/// A window with nothing open used to explain that nothing was open. This
/// says instead what there is to read: the documents last read, and the
/// places kept. It is the same history the palette and the Recent face draw,
/// narrowed by the same [`crate::visits::matches`], so the filter field here
/// behaves exactly like the palette's.
#[component]
pub fn WelcomeView() -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);

    // Anything this page lists, changing anywhere: a document read in another
    // window, a folder starred in this one.
    use_future(move || async move {
        let mut visited = VISITS_CHANGED.subscribe();
        let mut bookmarked = BOOKMARKS_CHANGED.subscribe();
        loop {
            let changed = tokio::select! {
                result = visited.recv() => result.is_ok(),
                result = bookmarked.recv() => result.is_ok(),
            };
            if !changed {
                break;
            }
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit or a new place redraws; the numbers say nothing.
    let _ = revision();
    let _ = state.visits_revision.read();

    let now = Local::now();
    let palette_hint = crate::keybindings::shortcut_hint_for_global_action("palette.open")
        .map(|hint| around_key(t!("welcome.hint.with_key").to_string(), hint));

    let visits = VISITS.read();
    let recent: Vec<Visit> = visits.items.iter().take(MAX_ROWS).cloned().collect();
    let more_recent = visits.items.len() > recent.len();
    let groups = crate::visits::group(&recent, now);
    let bookmarks = BOOKMARKS.read();
    let all_places = bookmarks.places();
    let places: Vec<PathBuf> = all_places.iter().take(MAX_ROWS).cloned().collect();
    let more_places = all_places.len() > places.len();
    // A starred folder is a place, and the places are listed as places.
    let all_starred: Vec<PathBuf> = bookmarks
        .items
        .iter()
        .filter(|bookmark| !bookmark.is_dir())
        .map(|bookmark| bookmark.path.clone())
        .collect();
    let starred: Vec<PathBuf> = all_starred.iter().take(MAX_ROWS).cloned().collect();
    let more_starred = all_starred.len() > starred.len();
    drop(bookmarks);

    rsx! {
        div {
            class: "welcome",

            div {
                class: "welcome-content",

                // The app's mark. A window with no document in it is the one
                // screen with nothing for it to take attention away from.
                div {
                    class: "welcome-mark",
                    img {
                        class: "welcome-mark-image light",
                        src: crate::assets::welcome_mark_data_url(false),
                        alt: "Arto",
                    }
                    img {
                        class: "welcome-mark-image dark",
                        src: crate::assets::welcome_mark_data_url(true),
                        alt: "Arto",
                    }
                    p { class: "welcome-tagline", "The Art of Reading Markdown" }
                }

                // The way in is a keystroke, said once. A field here would be
                // a second one to aim at, and it would answer with the same
                // list the palette already floats over whatever is on screen.
                p {
                    class: "welcome-hint",
                    if let Some((before, hint, after)) = palette_hint {
                        "{before}"
                        kbd { class: "welcome-key", "{hint}" }
                        "{after}"
                    } else {
                        {t!("welcome.hint.without_key").to_string()}
                    }
                }

                div {
                    class: "welcome-columns",

                    // What is kept and what is worked in, on the left: two
                    // short lists that answer "where do I go" rather than
                    // "what was I doing".
                    div {
                        class: "welcome-column welcome-column-side",

                        div {
                            h2 {
                                class: "welcome-heading",
                                Icon { name: IconName::Folder, size: 12 }
                                {t!("welcome.places.heading").to_string()}
                            }

                            if places.is_empty() {
                                p {
                                    class: "welcome-empty",
                                    {t!("welcome.places.empty").to_string()}
                                }
                            }

                            for place in places {
                                WelcomeRow {
                                    key: "{place.display()}",
                                    path: place.clone(),
                                    icon: IconName::Folder,
                                    when: when_read(crate::visits::last_read_under(&place), now),
                                    on_pick: move |place: PathBuf| state.add_root(&place),
                                }
                            }

                            if more_places {
                                button {
                                    class: "welcome-more",
                                    onclick: move |_| state.show_face(Face::Places),
                                    {t!("welcome.places.all").to_string()}
                                }
                            }
                        }

                        if !starred.is_empty() {
                            div {
                                h2 {
                                    class: "welcome-heading",
                                    Icon { name: IconName::Star, size: 12 }
                                    {t!("welcome.starred.heading").to_string()}
                                }

                                for path in starred {
                                    WelcomeRow {
                                        key: "{path.display()}",
                                        path: path.clone(),
                                        icon: IconName::File,
                                        when: when_read(crate::visits::last_read(&path), now),
                                        on_pick: move |path: PathBuf| state.open_file(&path),
                                    }
                                }

                                if more_starred {
                                    button {
                                        class: "welcome-more",
                                        onclick: move |_| state.show_face(Face::Starred),
                                        {t!("welcome.starred.all").to_string()}
                                    }
                                }
                            }
                        }
                    }

                    // What has been read, on the right: the longest list, and
                    // the one a reader scans rather than picks from.
                    div {
                        class: "welcome-column",
                        h2 {
                            class: "welcome-heading",
                            Icon { name: IconName::History, size: 12 }
                            {t!("welcome.recent.heading").to_string()}
                        }

                        if groups.is_empty() {
                            p {
                                class: "welcome-empty",
                                {t!("welcome.recent.empty").to_string()}
                            }
                        }

                        for (bucket, entries) in groups {
                            div { class: "welcome-group", "{bucket.heading()}" }
                            for visit in entries {
                                WelcomeRow {
                                    // The day is part of the key: a document
                                    // read on two days is a row under each,
                                    // and two siblings with one key are one
                                    // row to the renderer.
                                    key: "{visit.at}:{visit.path.display()}",
                                    path: visit.path.clone(),
                                    icon: IconName::File,
                                    when: crate::visits::short_when(visit.at, now),
                                    on_pick: move |path: PathBuf| state.open_file(&path),
                                }
                            }
                        }

                        // The list is a landing, not a browser: what does not
                        // fit is one row away, in the face that holds it all.
                        if more_recent {
                            button {
                                class: "welcome-more",
                                onclick: move |_| state.show_face(Face::Recent),
                                {t!("welcome.recent.all").to_string()}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `sentence` split where its `%{key}` stands, with `key` between the halves.
///
/// The shortcut is drawn as a key cap rather than as text, so it cannot be
/// passed to the translation as an argument; and where it sits in the
/// sentence is the translation's to decide, since a language that puts the
/// verb last puts the key first.
fn around_key(sentence: String, key: String) -> (String, String, String) {
    match sentence.split_once("%{key}") {
        Some((before, after)) => (before.to_string(), key, after.to_string()),
        None => (String::new(), key, format!(" {sentence}")),
    }
}

/// When something was last read, worded for a row, or nothing at all.
fn when_read(at: Option<chrono::DateTime<Local>>, now: chrono::DateTime<Local>) -> String {
    at.map(|at| crate::visits::short_when(at, now))
        .unwrap_or_default()
}

/// One offer on the landing page: a document to read, or a folder to work in.
///
/// Which of the two it is shows in its glyph and in what picking it does; the
/// row itself is the same, because all three columns are lists of one thing to
/// go to next.
#[component]
fn WelcomeRow(
    path: PathBuf,
    icon: IconName,
    when: String,
    on_pick: EventHandler<PathBuf>,
) -> Element {
    rsx! {
        div {
            class: "welcome-row",
            title: "{path.display()}",
            onclick: {
                let path = path.clone();
                move |_| on_pick.call(path.clone())
            },
            Icon { name: icon, size: 14 }
            span {
                class: "welcome-row-name",
                DocumentName { path: path.clone() }
            }
            if !when.is_empty() {
                span { class: "welcome-row-when", "{when}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_sits_where_the_sentence_places_it() {
        let split = |sentence: &str| around_key(sentence.to_string(), "⌘K".to_string());
        assert_eq!(
            split("Press %{key} to find it."),
            (
                "Press ".to_string(),
                "⌘K".to_string(),
                " to find it.".to_string()
            )
        );
        assert_eq!(
            split("%{key} で探せます。"),
            (String::new(), "⌘K".to_string(), " で探せます。".to_string())
        );
    }

    #[test]
    fn a_sentence_without_a_place_for_the_key_follows_it() {
        assert_eq!(
            around_key("Find it.".to_string(), "⌘K".to_string()),
            (String::new(), "⌘K".to_string(), " Find it.".to_string())
        );
    }
}
