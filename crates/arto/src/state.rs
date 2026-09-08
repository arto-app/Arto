// State module - manages application state

mod app_state;
pub(crate) use app_state::sidebar_cursor;
pub use app_state::{AppState, Document, DocumentContent, Face, FocusedPanel, SearchMatch};

mod persistence;
pub use persistence::{PersistedState, Position, Size};
