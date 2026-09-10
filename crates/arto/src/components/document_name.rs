//! A document's name, as every list of documents draws it.

use crate::components::matched::Spans;
use dioxus::prelude::*;
use std::path::PathBuf;

/// `samples/README.md`, with the folder set a step quieter than the file.
///
/// The folder is only there to tell two documents of one name apart; at the
/// same weight as the name it reads as part of it, and the eye has to take the
/// whole string apart to find what it came for.
///
/// `query` marks the characters that found this row, for the lists that are
/// typed into. It is asked of the whole name — folder and file — because that
/// is what the reader sees, and the answer is then cut where the drawing is.
///
/// Nothing typed is the common case by far: every row of the tree is a name
/// drawn this way and none of them is being searched. So an empty query is
/// not "a match with no marks" but no matching at all, and the name is the
/// plain text it always was.
#[component]
pub fn DocumentName(path: PathBuf, #[props(default)] query: String) -> Element {
    let (folder, name) = crate::utils::paths::split_name(&path);

    let query = crate::fuzzy::Query::new(&query);
    if query.is_empty() {
        return rsx! {
            if !folder.is_empty() {
                span { class: "document-folder", "{folder}" }
            }
            "{name}"
        };
    }

    let spans = query.highlight(&format!("{folder}{name}"));
    let (folder_spans, name_spans) = crate::fuzzy::split_spans(spans, folder.chars().count());

    rsx! {
        if !folder.is_empty() {
            span {
                class: "document-folder",
                Spans { spans: folder_spans }
            }
        }
        Spans { spans: name_spans }
    }
}
