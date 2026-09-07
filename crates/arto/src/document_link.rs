//! Opening a link to another Markdown document, as written in the source
//! (`./other.md#section`), from any entry point: a click on the rendered
//! link, or the keyboard cursor's open-link action.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use crate::scroll_anchor::ScrollAnchor;
use crate::state::AppState;

/// Where a document link opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkOpen {
    /// Navigate the current tab, remembering `scroll_anchor` in its
    /// history so that going back lands where the reader was.
    CurrentTab { scroll_anchor: ScrollAnchor },
    /// Open a new tab and switch to it.
    NewTab,
}

/// Split a link as written in the document into its path and fragment.
///
/// The fragment is percent-decoded so that `#%E8%A6%8B%E5%87%BA%E3%81%97`
/// finds the heading id it names; an empty fragment counts as none.
pub fn split_link_fragment(link: &str) -> (&str, Option<String>) {
    match link.split_once('#') {
        Some((path, fragment)) if !fragment.is_empty() => (
            path,
            Some(
                percent_encoding::percent_decode_str(fragment)
                    .decode_utf8_lossy()
                    .into_owned(),
            ),
        ),
        Some((path, _)) => (path, None),
        None => (link, None),
    }
}

/// Open `link`, relative to `current_file`, and scroll to its fragment once
/// the target has rendered. Returns `false` when the target cannot be
/// resolved, in which case nothing changes.
pub fn open_document_link(
    state: &mut AppState,
    current_file: &Path,
    link: &str,
    how: LinkOpen,
) -> bool {
    let (path, fragment) = split_link_fragment(link);

    let base_dir = current_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let target_path = base_dir.join(path);
    let Ok(canonical_path) = target_path.canonicalize() else {
        tracing::error!("Failed to resolve path: {:?}", target_path);
        return false;
    };

    // A link into the document that is already open does not reload it,
    // so the scroll happens right away instead of after the next render.
    // The open file may have been recorded through a symlink, so compare
    // canonical paths.
    let current_canonical = current_file
        .canonicalize()
        .unwrap_or_else(|_| current_file.to_path_buf());
    if matches!(how, LinkOpen::CurrentTab { .. }) && canonical_path == current_canonical {
        if let Some(fragment) = fragment {
            let _ = document::eval(&scroll_to_heading_js(&fragment));
        }
        return true;
    }

    tracing::info!("Opening file: {:?}", canonical_path);
    state.pending_scroll_fragment.set(fragment);
    match how {
        LinkOpen::NewTab => {
            state.add_file_tab(canonical_path, true);
        }
        LinkOpen::CurrentTab { scroll_anchor } => {
            state.save_current_scroll_anchor(scroll_anchor);
            state.navigate_to_file(canonical_path);
        }
    }
    true
}

/// JavaScript that scrolls the heading with `id` into view, if it exists.
pub fn scroll_to_heading_js(id: &str) -> String {
    let id_json = serde_json::to_string(id).unwrap_or_else(|_| "null".to_string());
    format!(
        "(() => {{ const el = document.getElementById({id_json}); \
         if (el) {{ el.scrollIntoView({{ block: 'start' }}); }} }})();"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_are_split_off_and_decoded() {
        assert_eq!(split_link_fragment("./doc.md"), ("./doc.md", None));
        assert_eq!(
            split_link_fragment("./doc.md#tables"),
            ("./doc.md", Some("tables".to_string()))
        );
        assert_eq!(split_link_fragment("./doc.md#"), ("./doc.md", None));
        assert_eq!(
            split_link_fragment("doc.md#%E8%A6%8B%E5%87%BA%E3%81%97"),
            ("doc.md", Some("見出し".to_string()))
        );
    }
}
