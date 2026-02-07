// State module - manages application state

mod app_state;
pub use app_state::*;

mod persistence;
pub use persistence::{PersistedState, Position, Size};
