// State module - manages application state

mod app_state;
pub(crate) use app_state::sidebar_cursor;
pub use app_state::{AppState, FocusedPanel, SearchMatch, Sidebar, SidebarPanel, Tab, TabContent};

mod persistence;
pub use persistence::{PersistedFileView, PersistedState, Position, Size};
