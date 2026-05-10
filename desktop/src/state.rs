// State module - manages application state

mod app_state;
pub(crate) use app_state::sidebar_cursor;
pub use app_state::{
    AiOverlay, AiSession, AppState, ChatTurn, ChatTurnRole, FocusedPanel, SearchMatch, Sidebar,
    Tab, TabContent, TabId,
};

mod persistence;
pub use persistence::{PersistedState, Position, Size};
