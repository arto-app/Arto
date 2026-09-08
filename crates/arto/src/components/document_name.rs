//! A document's name, as every list of documents draws it.

use dioxus::prelude::*;
use std::path::PathBuf;

/// `samples/README.md`, with the folder set a step quieter than the file.
///
/// The folder is only there to tell two documents of one name apart; at the
/// same weight as the name it reads as part of it, and the eye has to take the
/// whole string apart to find what it came for.
#[component]
pub fn DocumentName(path: PathBuf) -> Element {
    let (folder, name) = crate::utils::paths::split_name(&path);

    rsx! {
        if !folder.is_empty() {
            span { class: "document-folder", "{folder}" }
        }
        "{name}"
    }
}
