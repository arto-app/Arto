//! Tab management module.
//!
//! This module provides types and methods for managing tabs in the application.
//!
//! # Structure
//!
//! - [`TabContent`] - Enum representing the content type of a tab
//! - [`Tab`] - Struct representing a single tab with content and navigation history
//! - [`OpenFileOptions`] - Options for opening files in tabs
//! - `impl AppState` - Extension methods for tab management on AppState

mod content;
mod open_file_options;
mod state_ext;
mod tab;

pub use content::TabContent;
pub use open_file_options::OpenFileOptions;
pub use tab::Tab;
