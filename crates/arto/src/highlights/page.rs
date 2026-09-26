//! Keeping the page's highlights in step with the ones kept on disk.
//!
//! The page draws them (`frontend/src/user-highlights.ts`) and is the only
//! thing that can find their words again, so the two talk both ways: the
//! app hands it the document's highlights whenever the document is drawn or
//! they change, and the page answers with where it found each one, which is
//! kept so that a highlight follows its words through an edit.

use dioxus::document;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};
use tokio::sync::broadcast::error::RecvError;

use super::{load, rebase, rebase_all, same_document, Highlight, PageReport, HIGHLIGHTS_CHANGED};
use crate::state::AppState;

/// How the page names the document it is asked to draw highlights on, and
/// names it back in what it reports.
pub(crate) fn doc_name(document: &Path) -> String {
    document.to_string_lossy().into_owned()
}

/// What the page says: that it is ready to draw highlights, or what it
/// found when it drew them.
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FromPage {
    Ready,
    Report(PageReport),
}

/// The script that draws `highlights` on the page holding `document`, the
/// render `generation` of it when it is a rendered one.
///
/// The page draws them only while it still holds that render: between a
/// document being chosen and its page arriving, the page holds the one
/// before, and highlights drawn there would be found in the wrong words.
///
/// Guarded: the page's runtime arrives with the renderer bundle, which on a
/// cold start can be behind the first document; it says when it is ready,
/// and is handed them again then.
pub fn show_js(document: &Path, highlights: &[Highlight], generation: Option<u64>) -> String {
    let doc = serde_json::to_string(&doc_name(document)).unwrap_or_else(|_| "\"\"".into());
    let list = serde_json::to_string(highlights).unwrap_or_else(|_| "[]".into());
    let generation = generation.map_or_else(|| "null".to_string(), |g| g.to_string());
    format!("window.Arto?.highlights?.show?.({doc}, {list}, {generation});")
}

/// [`show_js`] for the render on screen, if it is `document`'s.
fn shown_js(state: AppState, document: &Path, highlights: &[Highlight]) -> Option<String> {
    let rendered = state.rendered_source.peek();
    let rendered = rendered
        .as_ref()
        .filter(|rendered| rendered.path == document)?;
    Some(show_js(document, highlights, Some(rendered.generation)))
}

/// Load the highlights on `document` into the window's state, forgetting
/// where the page found the previous ones, and return them for the page.
pub fn take_up(mut state: AppState, document: Option<&Path>) -> Vec<Highlight> {
    let highlights = document.map(load).unwrap_or_default();
    state.highlights.set(highlights.clone());
    state.highlight_places.set(Default::default());
    highlights
}

/// Listen to the page and to other windows for as long as the viewer shows
/// `file`.
pub fn use_page_highlights(file: ReadSignal<PathBuf>, mut state: AppState) {
    // What the page found, each time it draws them.
    use_future(move || async move {
        let mut eval = document::eval(indoc::indoc! {r#"
            (async () => {
                while (!window.Arto?.highlights?.setup) {
                    await new Promise(resolve => setTimeout(resolve, 10));
                }
                window.Arto.highlights.setup((report) => dioxus.send({ type: "report", ...report }));
                dioxus.send({ type: "ready" });
            })();
        "#});
        while let Ok(message) = eval.recv::<FromPage>().await {
            let report = match message {
                FromPage::Report(report) => report,
                // On a cold start the document can be drawn before the
                // page's runtime is there to draw its highlights, which were
                // then handed to nothing: hand them over again.
                FromPage::Ready => {
                    let current = file.peek().clone();
                    let js = shown_js(state, &current, &state.highlights.peek());
                    if let Some(js) = js {
                        let _ = document::eval(&js).await;
                    }
                    continue;
                }
            };
            let current = file.peek().clone();
            // A report on a document the viewer has since left is about
            // highlights no longer in the state.
            if report.doc != doc_name(&current) {
                continue;
            }
            state.highlight_places.set(report.places());
            let moves = report.moves(&state.highlights.peek());
            if moves.is_empty() {
                continue;
            }
            rebase_all(&current, &moves);
            let mut highlights = state.highlights.write();
            for (id, start, line) in &moves {
                rebase(&mut highlights, id, *start, *line);
            }
        }
    });

    // A highlight added, removed or recoloured, here or in another window
    // showing the same document.
    use_future(move || async move {
        let mut rx = HIGHLIGHTS_CHANGED.subscribe();
        loop {
            let changed = match rx.recv().await {
                Ok(changed) => Some(changed),
                // Changes were missed; whether one was to this document is
                // not known, so it is read again.
                Err(RecvError::Lagged(_)) => None,
                Err(RecvError::Closed) => break,
            };
            let current = file.peek().clone();
            if changed.is_some_and(|changed| !same_document(&changed, &current)) {
                continue;
            }
            let highlights = take_up(state, Some(&current));
            if let Some(js) = shown_js(state, &current, &highlights) {
                let _ = document::eval(&js).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlights::{HighlightColor, TextAnchor};

    #[test]
    fn the_page_says_it_is_ready_or_what_it_found() {
        let ready: FromPage = serde_json::from_str(r#"{"type":"ready"}"#).unwrap();
        assert!(matches!(ready, FromPage::Ready));

        let report: FromPage = serde_json::from_str(
            r#"{"type":"report","doc":"/a.md","placed":[],"orphans":["hl_1"]}"#,
        )
        .unwrap();
        assert!(matches!(report, FromPage::Report(report) if report.doc == "/a.md"));

        // A report the page got wrong is not taken for the page being ready.
        assert!(serde_json::from_str::<FromPage>(r#"{"type":"report","doc":1}"#).is_err());
    }

    #[test]
    fn the_page_is_handed_the_document_and_its_highlights_as_json() {
        let highlight = Highlight::new(
            TextAnchor {
                exact: "it's \"quoted\"".to_string(),
                prefix: String::new(),
                suffix: String::new(),
                start: 0,
                line: 1,
            },
            HighlightColor::Orange,
        );

        let js = show_js(
            Path::new("/a \"b\".md"),
            std::slice::from_ref(&highlight),
            Some(7),
        );

        let args = js
            .strip_prefix("window.Arto?.highlights?.show?.(")
            .and_then(|rest| rest.strip_suffix(");"))
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&format!("[{args}]")).unwrap();
        assert_eq!(parsed[0], "/a \"b\".md");
        assert_eq!(parsed[1][0]["id"], highlight.id.to_string());
        assert_eq!(parsed[1][0]["color"], "orange");
        assert_eq!(parsed[1][0]["anchor"]["exact"], "it's \"quoted\"");
        assert_eq!(parsed[2], 7);
    }
}
