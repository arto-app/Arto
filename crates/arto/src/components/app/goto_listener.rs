use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;

use crate::events::{GotoTarget, GOTO_IN_WINDOW};

/// Carry out `arto/goto` for this window.
///
/// A listener here rather than a call from `ipc.rs` because `document::eval`
/// only reaches the WebView from inside the window's own scope.
pub(super) fn setup_goto_listener() {
    use_future(move || async move {
        let window_id = window().id();
        let mut rx = GOTO_IN_WINDOW.subscribe();

        loop {
            match rx.recv().await {
                Ok((target_window, target)) if target_window == window_id => {
                    let _ = document::eval(&goto_js(&target));
                }
                Ok(_) => {}
                // Losing a move the client was told about beats a window
                // that never moves again.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::debug!(?window_id, missed, "Missed goto requests");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn goto_js(target: &GotoTarget) -> String {
    match target {
        GotoTarget::Top => "window.Arto.scroll.toTop();".to_string(),
        GotoTarget::Bottom => "window.Arto.scroll.toBottom();".to_string(),
        GotoTarget::Heading(id) => format!(
            "window.Arto.scroll.toHeading({});",
            serde_json::to_string(id).unwrap_or_else(|_| "\"\"".to_string())
        ),
        GotoTarget::Scroll(position) => format!(
            "window.Arto.scroll.toAnchor({{line: {}, fraction: {}}});",
            position.line, position.fraction
        ),
        // The panel's own find, so the hits stay highlighted where the
        // reader can see them — a jump that left no trace would put them
        // somewhere they could not account for.
        GotoTarget::Search { query, index } => format!(
            "window.Arto.search.find({}); window.Arto.search.navigateTo({index});",
            serde_json::to_string(query).unwrap_or_else(|_| "\"\"".to_string())
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_lsp::ScrollPosition;

    #[test]
    fn each_destination_goes_through_the_pages_own_scroller() {
        // Not `scrollTo`: these destinations wait for the document to stop
        // growing, so a move from outside lands where one from inside does.
        assert!(goto_js(&GotoTarget::Top).starts_with("window.Arto.scroll."));
        assert!(goto_js(&GotoTarget::Bottom).starts_with("window.Arto.scroll."));
        assert!(goto_js(&GotoTarget::Heading("install".into())).starts_with("window.Arto.scroll."));
        assert!(goto_js(&GotoTarget::Scroll(ScrollPosition::default()))
            .starts_with("window.Arto.scroll."));
    }

    #[test]
    fn a_search_runs_the_find_before_it_moves_to_a_hit() {
        // `navigateTo` indexes into the hits the preceding `find` left
        // behind, so the order is the whole of it.
        let js = goto_js(&GotoTarget::Search {
            query: "install".into(),
            index: 2,
        });
        let find = js.find("window.Arto.search.find(").unwrap();
        let navigate = js.find("window.Arto.search.navigateTo(2)").unwrap();
        assert!(find < navigate, "{js}");
    }

    #[test]
    fn a_query_is_encoded_rather_than_interpolated() {
        let js = goto_js(&GotoTarget::Search {
            query: r#"a"b\c"#.into(),
            index: 0,
        });
        assert!(js.contains(r#""a\"b\\c""#), "{js}");
    }

    #[test]
    fn a_heading_id_is_encoded_rather_than_interpolated() {
        // Anchors come from heading text, so they can carry quotes and
        // backslashes; pasting one into a string literal would end it.
        let js = goto_js(&GotoTarget::Heading(r#"a"b\c"#.into()));
        assert!(js.contains(r#""a\"b\\c""#), "{js}");
    }

    #[test]
    fn an_anchor_travels_as_the_pair_the_page_reads() {
        let js = goto_js(&GotoTarget::Scroll(ScrollPosition {
            line: 42,
            fraction: 0.25,
        }));
        assert!(js.contains("line: 42"), "{js}");
        assert!(js.contains("fraction: 0.25"), "{js}");
    }
}
