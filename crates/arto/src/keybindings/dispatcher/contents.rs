//! Moving through the contents while they are held open.
//!
//! The list is the gutter's own — the same headings, in the same place — so
//! these keys move a cursor through what is already on screen rather than
//! opening a second list of their own.

use super::*;

/// Move the cursor by one heading, from wherever it is.
pub(super) fn step_contents(state: &mut AppState, forward: bool) {
    let next = step(
        *state.contents_cursor.read(),
        state.headings.read().len(),
        forward,
    );
    if next.is_none() {
        return;
    }
    state.contents_cursor.set(next);
    scroll_into_view(".contents-toc-row.keyboard-focused");
}

/// Where one step lands, or `None` when there is nothing to step through.
///
/// From nowhere it starts at the end it is walking from: the first step down
/// lands on the first heading, the first step up on the last. Both ends stop
/// rather than wrap — this list is the document's shape, and a cursor that
/// reappeared at the top of it would lose where the reader was in it. (The
/// palette wraps, because there the list is short and its order is not the
/// document's.)
fn step(at: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    let last = len.checked_sub(1)?;
    Some(match (at, forward) {
        (None, true) => 0,
        (None, false) => last,
        (Some(at), true) => (at + 1).min(last),
        (Some(at), false) => at.saturating_sub(1),
    })
}

/// Go to the heading the cursor is on, and let the list go.
///
/// The list was opened to get somewhere; once it has, keeping it over the
/// page would leave the reader looking at the map instead of the place.
pub(super) fn confirm_contents(state: &mut AppState) {
    let Some(at) = *state.contents_cursor.read() else {
        return;
    };
    let Some(id) = state.headings.read().get(at).map(|h| h.id.clone()) else {
        return;
    };
    state.close_contents();
    spawn_detached(async move {
        let _ = document::eval(&crate::document_link::scroll_to_heading_js(&id)).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_step_starts_at_the_end_it_is_walking_from() {
        assert_eq!(step(None, 4, true), Some(0));
        assert_eq!(step(None, 4, false), Some(3));
    }

    #[test]
    fn both_ends_stop_rather_than_wrap() {
        assert_eq!(step(Some(3), 4, true), Some(3));
        assert_eq!(step(Some(0), 4, false), Some(0));
    }

    #[test]
    fn a_document_with_no_headings_has_nowhere_to_step() {
        assert_eq!(step(None, 0, true), None);
        assert_eq!(step(Some(0), 0, false), None);
    }
}
