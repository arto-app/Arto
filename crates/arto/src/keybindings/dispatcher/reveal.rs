//! Bringing something into view, and the page's own scrolling.

use dioxus::document;

use super::*;

/// Scroll the keyboard-focused element into view using JS.
pub(super) fn scroll_cursor_into_view() {
    scroll_into_view(".keyboard-focused");
}

/// Bring the row a cursor just moved to into view, with room around it.
///
/// On the next frame, because the row it is looking for is the one the move
/// has yet to draw.
///
/// Not `scrollIntoView({ block: 'nearest' })`: that is satisfied by a row
/// with one pixel showing, so the cursor spends the whole list pinned to an
/// edge with nothing ahead of it. The row is kept a couple of rows clear of
/// both ends instead — what is coming next is as much of an answer as where
/// the cursor is.
pub(super) fn scroll_into_view(selector: &'static str) {
    spawn_detached(async move {
        let js = format!(
            r#"
            requestAnimationFrame(() => {{
                const row = document.querySelector({selector:?});
                if (!row) return;
                let box = row.parentElement;
                while (box && box.scrollHeight <= box.clientHeight) {{
                    box = box.parentElement;
                }}
                if (!box) {{
                    row.scrollIntoView({{ block: 'nearest' }});
                    return;
                }}
                const rowBox = row.getBoundingClientRect();
                const view = box.getBoundingClientRect();
                // Two rows of room, or a third of the list where two rows is
                // most of it.
                const margin = Math.min(rowBox.height * 2, view.height / 3);
                const above = rowBox.top - view.top;
                const below = view.bottom - rowBox.bottom;
                if (above < margin) {{
                    box.scrollTop += above - margin;
                }} else if (below < margin) {{
                    box.scrollTop -= below - margin;
                }}
            }});
            "#
        );
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(selector, "Failed to scroll cursor into view: {e}");
        }
    });
}

pub(super) fn scroll_eval(method: &'static str) {
    spawn_detached(async move {
        let js = format!("window.Arto.scroll.{method}();");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Scroll eval failed: {e}");
        }
    });
}

pub(crate) fn content_cursor_eval(method: &'static str) {
    spawn_detached(async move {
        let js = format!("window.Arto.contentCursor.{method}()");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Content cursor eval failed: {e}");
        }
    });
}
