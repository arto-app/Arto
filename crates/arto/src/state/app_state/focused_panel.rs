use crate::keybindings::KeyContext;

/// Which panel currently has keyboard focus.
///
/// When a panel is focused, the keybinding engine uses the associated
/// `KeyContext` to match context-specific bindings (e.g., `j` → `cursor.down`
/// in Sidebar vs `j` → `scroll.down` globally in Content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    Content,
    LeftSidebar,
    RightSidebar,
    QuickAccess,
}

impl FocusedPanel {
    /// Map panel to keybinding context for engine matching.
    pub fn key_context(&self) -> KeyContext {
        match self {
            Self::Content => KeyContext::Content,
            Self::LeftSidebar => KeyContext::Sidebar,
            Self::RightSidebar => KeyContext::RightSidebar,
            Self::QuickAccess => KeyContext::QuickAccess,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_content() {
        assert_eq!(FocusedPanel::default(), FocusedPanel::Content);
    }

    #[test]
    fn content_maps_to_content_context() {
        assert_eq!(FocusedPanel::Content.key_context(), KeyContext::Content);
    }

    #[test]
    fn left_sidebar_maps_to_sidebar_context() {
        assert_eq!(FocusedPanel::LeftSidebar.key_context(), KeyContext::Sidebar);
    }

    #[test]
    fn right_sidebar_maps_to_right_sidebar_context() {
        assert_eq!(
            FocusedPanel::RightSidebar.key_context(),
            KeyContext::RightSidebar
        );
    }

    #[test]
    fn quick_access_maps_to_quick_access_context() {
        assert_eq!(
            FocusedPanel::QuickAccess.key_context(),
            KeyContext::QuickAccess
        );
    }
}
