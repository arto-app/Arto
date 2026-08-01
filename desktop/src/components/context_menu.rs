//! Shared building blocks for every menu in the app.
//!
//! Each menu owns its own contents, but they all render the same
//! item/separator/submenu markup and clamp themselves into the viewport the
//! same way. Consumers include the tab bar, the sidebar file tree, the markdown
//! content area, and the Windows hamburger menu — the last of which is
//! `cfg`-gated, so a grep or build on another platform will not surface it.
//! Check it too before assuming a menu change is complete.

mod menu_item;
mod position;

pub use menu_item::*;
pub use position::*;
