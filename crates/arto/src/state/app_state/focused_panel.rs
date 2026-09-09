use crate::keybindings::KeyContext;

/// Which panel currently has keyboard focus.
///
/// When a panel is focused, the keybinding engine uses the associated
/// `KeyContext` to match context-specific bindings (e.g., `j` → `cursor.down`
/// in the panel vs `j` → `scroll.down` globally in Content).
///
/// The panel is one entry however many faces it has: the keys that walk a
/// list of documents are the same keys whether the list is a tree, a history
/// or a set of stars, and having one per face was three chances to bind them
/// differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    Content,
    Panel,
    RightSidebar,
}

impl FocusedPanel {
    /// Map panel to keybinding context for engine matching.
    pub fn key_context(&self) -> KeyContext {
        match self {
            Self::Content => KeyContext::Content,
            Self::Panel => KeyContext::Sidebar,
            Self::RightSidebar => KeyContext::RightSidebar,
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
    fn the_panel_maps_to_the_sidebar_context() {
        assert_eq!(FocusedPanel::Panel.key_context(), KeyContext::Sidebar);
    }

    #[test]
    fn right_sidebar_maps_to_right_sidebar_context() {
        assert_eq!(
            FocusedPanel::RightSidebar.key_context(),
            KeyContext::RightSidebar
        );
    }
}
