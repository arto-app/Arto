//! Answering `arto/state`: what is open, and where the reader is in it.
//!
//! Every field is already on the main thread — the scroll anchor included,
//! since the reading-position tracker keeps it up to date for the header
//! and the margin trace. So this is a read rather than a round trip into
//! the page, and it is answered in the turn that received the request.

use arto_lsp::{
    CaptureParams, ErrorCode, GotoParams, GotoResult, Heading, Responder, ResponseError,
    ScrollPosition, SelectionParams, StateParams, StateResult, WindowKey, WindowState,
};

use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

use crate::state::AppState;

/// Tell whoever asked to be told how this window reads now.
///
/// Called from the window's own reactive scope, so the reads inside are
/// what subscribe it: anything [`WindowState`] carries re-runs this.
pub fn report_window_state(window_id: WindowId) {
    let Some(key) = crate::window::main::window_key_of(window_id) else {
        return;
    };
    let Some(state) = crate::window::main::get_window_state(window_id) else {
        return;
    };
    // A window hidden rather than destroyed still has its state, and
    // [`describe`] leaves it out. Reporting it here would describe a
    // document nobody can see.
    if !crate::window::main::is_visible_main_window(window_id) {
        return report_window_closed(window_id);
    }
    super::subscribers::publish(key, Some(describe_window(key, &state)));
}

/// Tell them the window is gone. Must run before the key is forgotten.
pub fn report_window_closed(window_id: WindowId) {
    let Some(key) = crate::window::main::window_key_of(window_id) else {
        return;
    };
    super::parked::forget_window(key);
    super::subscribers::publish(key, None);
}

/// Describe the windows a request asked about. A named window that has
/// since closed is left out rather than reported as an error — holding a
/// key across a close is the ordinary case.
pub fn describe(params: &StateParams) -> StateResult {
    // Visible ones only: on macOS the last window closing hides it rather
    // than destroying it, and describing it would report a document and a
    // scroll position for a window the user closed.
    let mut windows: Vec<WindowState> = crate::window::main::list_visible_main_windows()
        .into_iter()
        .filter_map(|context| {
            let window_id = context.window.id();
            let key = crate::window::main::window_key_of(window_id)?;
            if params.window.is_some_and(|asked| asked != key) {
                return None;
            }
            let state = crate::window::main::get_window_state(window_id)?;
            Some(describe_window(key, &state))
        })
        .collect();

    // The order they opened in — the only ordering that means anything to
    // somebody who cannot see the screen.
    windows.sort_by_key(|window| window.window);

    // Only ever one of the windows just described: nothing clears the
    // last-focused record when a window closes, and a request that asked
    // about one window must not be answered with a key for another.
    let focused = crate::window::main::get_last_focused_window()
        .and_then(crate::window::main::window_key_of)
        .filter(|key| windows.iter().any(|window| window.window == *key));

    StateResult { windows, focused }
}

/// Ask a window to move the reader, and say which one was asked. All that
/// is decided here is which window; the move itself happens in that
/// window's own scope.
pub fn goto(params: &GotoParams) -> Result<GotoResult, ResponseError> {
    let (window_id, key) = resolve_window(params.window)?;
    // No receiver means no window is listening, so nothing carried this
    // out. Answering `GotoResult` anyway would tell the client the reader
    // had been moved.
    crate::events::GOTO_IN_WINDOW
        .send((window_id, params.target.clone()))
        .map_err(|_| {
            ResponseError::new(
                ErrorCode::InternalError,
                "no window is listening for a move",
            )
        })?;
    Ok(GotoResult { window: key })
}

/// Ask a window what the reader has selected.
///
/// Answers nothing here: the selection is in the WebView, so the responder
/// is parked and the window answers it a round trip later.
pub fn selection(params: &SelectionParams, responder: Responder) {
    let (window_id, key) = match resolve_window(params.window) {
        Ok(window) => window,
        Err(error) => return responder.err(error),
    };
    let token = super::parked::ask(key, responder);
    if crate::events::READ_SELECTION_IN_WINDOW
        .send((window_id, token))
        .is_err()
    {
        super::parked::abandon(token, "no window is listening for the question");
    }
}

/// Ask a window for a picture of what it draws.
///
/// Answered like [`selection`], a round trip later: the page draws it.
pub fn capture(params: &CaptureParams, responder: Responder) {
    let (window_id, key) = match resolve_window(params.window) {
        Ok(window) => window,
        Err(error) => return responder.err(error),
    };
    let token = super::parked::ask(key, responder);
    if crate::events::CAPTURE_IN_WINDOW
        .send((window_id, token, params.target.clone()))
        .is_err()
    {
        super::parked::abandon(token, "no window is listening for the request");
    }
}

/// The window a request named, or the one a client that named none means.
///
/// Only ever a window [`describe`] would report. The two have to agree: a
/// client picks a window out of what `arto/state` answered, and a window
/// that answer left out is one it has no name for.
fn resolve_window(asked: Option<WindowKey>) -> Result<(WindowId, WindowKey), ResponseError> {
    let visible: Vec<WindowId> = crate::window::main::list_visible_main_windows()
        .iter()
        .map(|context| context.window.id())
        .collect();

    let window_id = match asked {
        Some(key) => crate::window::main::window_for_key(key)
            .filter(|window_id| visible.contains(window_id))
            .ok_or_else(|| {
                ResponseError::new(ErrorCode::InvalidParams, format!("no window {key} is open"))
            })?,
        // The last focused window may have been closed since — nothing
        // clears that record — and then the request means whichever window
        // is still there rather than nothing at all.
        None => crate::window::main::get_last_focused_window()
            .filter(|window_id| visible.contains(window_id))
            .or_else(|| visible.first().copied())
            .ok_or_else(|| ResponseError::new(ErrorCode::InvalidParams, "no window is open"))?,
    };
    let key = crate::window::main::window_key_of(window_id)
        .ok_or_else(|| ResponseError::new(ErrorCode::InvalidParams, "no window is open"))?;
    Ok((window_id, key))
}

fn describe_window(key: WindowKey, state: &AppState) -> WindowState {
    let anchor = *state.current_scroll_anchor.read();
    WindowState {
        window: key,
        document: state.document.read().file().map(|path| path.to_path_buf()),
        theme: *state.current_theme.read(),
        scroll: ScrollPosition {
            line: anchor.line,
            fraction: anchor.fraction,
        },
        headings: state
            .headings
            .read()
            .iter()
            .map(|heading| Heading {
                level: heading.level,
                text: heading.text.clone(),
                id: heading.id.clone(),
            })
            .collect(),
    }
}
