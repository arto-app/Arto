//! The panel's keyboard: which face is showing, where the cursor is in it,
//! and what happens to the row it is on.

use super::*;

/// Clone sidebar data needed for cursor navigation, releasing the read guard.
///
/// The rows the panel is drawing, in the order it draws them.
///
/// One list per face, and the cursor walks whichever is on screen — the same
/// keys, over a tree, a history or a set of stars. What is folded away counts:
/// a cursor on a row nobody can see is a cursor nobody can follow.
pub(super) fn panel_items(state: &AppState) -> Vec<crate::state::PanelRow> {
    match state.sidebar.peek().face {
        crate::state::Face::Places => match extract_sidebar_data(state) {
            Some(tree) => sidebar_cursor::visible_items_in_roots(
                &tree.roots,
                &tree.expanded,
                tree.show_all_files,
            ),
            None => Vec::new(),
        },
        crate::state::Face::Recent => {
            let folded = state.sidebar.peek().recent_collapsed.clone();
            crate::visits::VISITS
                .read()
                .grouped(chrono::Local::now())
                .into_iter()
                .filter(|(bucket, _)| !folded.contains(&bucket.heading()))
                .flat_map(|(_, visits)| {
                    visits
                        .into_iter()
                        .map(|visit| (crate::state::Group::Flat, visit.path.clone()))
                })
                .collect()
        }
        crate::state::Face::Starred => crate::bookmarks::BOOKMARKS
            .read()
            .items
            .iter()
            .filter(|bookmark| !bookmark.is_dir())
            .map(|bookmark| (crate::state::Group::Flat, bookmark.path.clone()))
            .collect(),
    }
}

/// Put the cursor on the first row, if it is not on one already.
pub(super) fn rest_cursor(state: &mut AppState) {
    let items = panel_items(state);
    let resting = state
        .panel_cursor
        .peek()
        .as_ref()
        .is_some_and(|at| items.contains(at));
    if !resting {
        state.panel_cursor.set(items.first().cloned());
    }
}

/// Show a face and take the keyboard to it, cursor and all — or put the panel
/// away, if that is where the keyboard already is.
///
/// Asking for the face you are already in is asking to leave: one key opens
/// the list and closes it again, which is what a reader means by pressing it
/// twice. Asking for a different face while in the panel only changes faces.
pub(super) fn face_to(state: &mut AppState, face: crate::state::Face) {
    let already_here =
        *state.focused_panel.read() == FocusedPanel::Panel && state.sidebar.peek().face == face;
    if already_here {
        state.hide_panel();
        return;
    }
    state.focus_face(face);
    rest_cursor(state);
    scroll_cursor_into_view();
}

/// Step along the rail to the next face.
pub(super) fn step_face(state: &mut AppState, forward: bool) {
    state.step_face(forward);
    rest_cursor(state);
    scroll_cursor_into_view();
}

/// Which root of its own group the cursor's row descends from.
fn root_of(state: &AppState, row: &crate::state::PanelRow) -> std::path::PathBuf {
    let (group, path) = row;
    let roots = extract_sidebar_data(state)
        .map(|tree| tree.roots)
        .unwrap_or_default();
    roots
        .into_iter()
        .find(|(row_group, root)| row_group == group && path.starts_with(root))
        .map(|(_, root)| root)
        .unwrap_or_else(|| path.to_path_buf())
}

/// What the tree is drawing, as the cursor needs to see it.
pub(super) struct TreeShape {
    /// Every root the tree draws, in the order it draws them, each with the
    /// group it is drawn in.
    roots: Vec<(crate::state::Group, std::path::PathBuf)>,
    /// The rows that are open — see [`crate::state::Sidebar::expanded_dirs`].
    expanded: std::collections::HashSet<crate::state::TreeRow>,
    show_all_files: bool,
}

/// The tree's shape, if it has any root at all.
pub(super) fn extract_sidebar_data(state: &AppState) -> Option<TreeShape> {
    use crate::state::Group;

    let sidebar = state.sidebar.read();
    // The window's own folder first, which is the order the tree draws them.
    let roots: Vec<_> = sidebar
        .roots
        .temps()
        .iter()
        .map(|root| (Group::Current, root.clone()))
        .chain(
            sidebar
                .roots
                .places()
                .iter()
                .map(|root| (Group::Bookmark, root.clone())),
        )
        .collect();
    (!roots.is_empty()).then(|| TreeShape {
        roots,
        expanded: sidebar.expanded_dirs.clone(),
        show_all_files: sidebar.show_all_files,
    })
}

pub(super) enum CursorDirection {
    Down,
    Up,
}

pub(super) fn dispatch_cursor_move(state: &mut AppState, direction: CursorDirection) {
    if *state.focused_panel.read() != FocusedPanel::Panel {
        return;
    }
    let items = panel_items(state);
    if items.is_empty() {
        return;
    }
    let current = state.panel_cursor.read().clone();
    let next = match direction {
        CursorDirection::Down => sidebar_cursor::move_down(&current, &items),
        CursorDirection::Up => sidebar_cursor::move_up(&current, &items),
    };
    state.panel_cursor.set(next);
    scroll_cursor_into_view();
}

/// cursor.enter — "Enter into": directory → set as root, file → open, heading → scroll to.
pub(super) fn dispatch_cursor_enter(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::Panel => {
            let Some((_, path)) = state.panel_cursor.read().clone() else {
                return;
            };
            if path.is_dir() {
                state.add_root(&path);
            } else if path.exists() {
                state.open_from_panel(&path);
            }
        }
        FocusedPanel::Content => {}
    }
}

/// cursor.open — "Open/expand": directory → expand tree, file → open, heading → scroll to.
pub(super) fn dispatch_cursor_open(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::Panel => open_panel_row(state),
        FocusedPanel::Content => {}
    }
}

/// Open what the cursor is on: a document is read, a folder opens onto its
/// contents with the cursor on the first of them.
pub(super) fn open_panel_row(state: &mut AppState) {
    let Some(row) = state.panel_cursor.read().clone() else {
        return;
    };
    let (group, path) = row.clone();

    if !path.is_dir() {
        if path.exists() {
            state.open_from_panel(&path);
        }
        return;
    }

    let root = root_of(state, &row);
    if !state.sidebar.peek().is_expanded(group, &root, &path) {
        state.toggle_directory_expansion(group, &root, &path);
    }
    let items = panel_items(state);
    let next = items
        .iter()
        .position(|item| item == &row)
        .and_then(|at| items.get(at + 1).cloned());
    if let Some(next) = next {
        state.panel_cursor.set(Some(next));
        scroll_cursor_into_view();
    }
}

pub(super) fn dispatch_cursor_collapse(state: &mut AppState) {
    if *state.focused_panel.read() != FocusedPanel::Panel {
        return;
    }
    let Some(row) = state.panel_cursor.read().clone() else {
        return;
    };
    let (group, path) = row.clone();
    let root = root_of(state, &row);
    if path.is_dir() && state.sidebar.peek().is_expanded(group, &root, &path) {
        state.toggle_directory_expansion(group, &root, &path);
        return;
    }
    // Out of a folder is up to the one holding it, where the list has one.
    let items = panel_items(state);
    if let Some(parent) = sidebar_cursor::find_parent_dir(&row, &items) {
        state.panel_cursor.set(Some(parent));
        scroll_cursor_into_view();
    }
}

pub(super) fn toggle_bookmark_on_cursor_or_current(state: &mut AppState) {
    let target_path = get_bookmark_target_path(state).or_else(|| get_current_file(state));
    let Some(path) = target_path else { return };

    let is_bookmarked = crate::bookmarks::toggle_bookmark(&path);
    if is_bookmarked {
        show_action_feedback("Bookmarked");
    } else {
        show_action_feedback("Bookmark removed");
    }
}

pub(super) fn get_bookmark_target_path(state: &AppState) -> Option<std::path::PathBuf> {
    match *state.focused_panel.read() {
        FocusedPanel::Panel => state.panel_cursor.read().clone().map(|(_, path)| path),
        FocusedPanel::Content => None,
    }
}

pub(super) fn set_parent_of_current_file_as_root(state: &mut AppState) {
    let Some(file) = get_current_file(state) else {
        return;
    };
    let Some(parent) = file.parent() else {
        return;
    };
    state.add_root(parent);
}
