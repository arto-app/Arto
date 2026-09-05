//! Global context menu state to avoid re-rendering content when menu state changes.
//!
//! By keeping the context menu state separate from FileViewer:
//! - FileViewer only writes to the state (doesn't read)
//! - App reads and renders the context menu
//! - Changes to menu state don't trigger FileViewer re-renders
//! - This preserves text selection in the content

use dioxus::prelude::*;
use std::path::PathBuf;
use std::sync::LazyLock;

use super::context_menu::ContextMenuData;

/// Complete state for rendering the content context menu
#[derive(Debug, Clone)]
pub struct ContentContextMenuState {
    pub data: ContextMenuData,
    pub current_file: Option<PathBuf>,
    pub base_dir: PathBuf,
}

/// Global signal for content context menu state.
///
/// This is intentionally separate from AppState to:
/// 1. Avoid circular dependencies with ContextMenuData
/// 2. Allow FileViewer to write without subscribing (no re-render)
/// 3. Allow App to read and render without affecting content
pub static CONTENT_CONTEXT_MENU: LazyLock<GlobalSignal<Option<ContentContextMenuState>>> =
    LazyLock::new(|| Signal::global(|| None));

/// Open the content context menu
pub fn open_context_menu(state: ContentContextMenuState) {
    *CONTENT_CONTEXT_MENU.write() = Some(state);
}

/// Close the content context menu.
///
/// Element references are NOT cleaned up here because async rasterization
/// tasks (Copy Image / Save Image) may still need them after menu close.
/// Cleanup happens in `rasterize_special_block` after rasterization completes.
pub fn close_context_menu() {
    *CONTENT_CONTEXT_MENU.write() = None;
}
