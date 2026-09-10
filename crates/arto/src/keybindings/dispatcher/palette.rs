//! Moving through the palette and taking what it is on.
//!
//! The palette's query and cursor are the window's state, which is what lets
//! a binding move them from out here.

use super::*;

/// Move the palette's cursor by one row.
pub(super) fn step_palette(state: &mut AppState, forward: bool) {
    let len = *state.palette_rows.read();
    let at = crate::components::palette::cursor_row(state, len);
    state
        .palette_cursor
        .set(Some(crate::components::palette::step_cursor(
            at, len, forward,
        )));
    scroll_into_view(".palette-row.keyboard-focused");
}

/// Take the row the palette is on: read the document, work in the folder, or
/// do the thing.
///
/// The palette closes first. A command can put a modal file dialog or a new
/// window on screen, and the list it was picked from has no business still
/// floating over that.
pub fn activate_palette_row(mut state: AppState, index: usize) {
    let row = crate::components::palette::current_rows(&state)
        .get(index)
        .cloned();
    state.close_palette();
    match row {
        Some(crate::components::palette::Row::Document(visit)) => state.open_file(&visit.path),
        Some(crate::components::palette::Row::Starred(path)) => state.open_file(&path),
        Some(crate::components::palette::Row::File(path)) => state.open_file(&path),
        Some(crate::components::palette::Row::Place(path)) => state.add_root(&path),
        Some(crate::components::palette::Row::Command(action)) => dispatch_action(&action, state),
        None => {}
    }
}
