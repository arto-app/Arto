use crate::keybindings::KeyContext;

/// Which panel currently holds keyboard focus.
///
/// This is runtime-only state (not persisted). It determines the key context
/// passed to the keybinding engine, allowing context-specific bindings
/// (e.g., `j` → cursor.down in sidebar vs scroll.down in content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    Content,
    LeftSidebar,
    RightSidebar,
    QuickAccess,
}

impl FocusedPanel {
    /// Map this panel to a keybinding context.
    ///
    /// Returns `None` for Content (global/default context),
    /// `Some(KeyContext)` for panels with context-specific bindings.
    pub fn key_context(&self) -> Option<KeyContext> {
        match self {
            Self::Content => None,
            Self::LeftSidebar => Some(KeyContext::Sidebar),
            Self::RightSidebar => Some(KeyContext::RightSidebar),
            Self::QuickAccess => Some(KeyContext::QuickAccess),
        }
    }
}
