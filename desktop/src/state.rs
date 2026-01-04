// State module - manages application state

mod app_state;
pub use app_state::{AppState, Sidebar, Tab, TabContent};

mod persistence;
pub use persistence::{PersistedState, Position, Size, LAST_FOCUSED_STATE};
