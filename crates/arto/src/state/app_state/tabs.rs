//! Tab management module.
//!
//! This module provides types and methods for managing tabs in the application.
//!
//! # Structure
//!
//! - [`TabContent`] - Enum representing the content type of a tab
//! - [`Tab`] - Struct representing a single tab with content and navigation history
//! - `impl AppState` - Extension methods for tab management on AppState
//!
//! # Submodule Guide
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | `crud` | Single-tab create/read/update/delete |
//! | `bulk_ops` | Multi-tab batch operations |
//! | `file_ops` | File-oriented tab actions |
//! | `navigation` | Tab switching and reordering |
//! | `scroll_history` | Per-tab scroll position tracking |

mod bulk_ops;
mod content;
mod crud;
mod file_ops;
mod navigation;
mod scroll_history;
mod tab;

pub use content::TabContent;
pub use tab::Tab;
