use crate::keybindings::KeyContext;

/// What the keyboard is in.
///
/// The panel is one place, whichever of its faces is showing: the keys that
/// walk a list of documents are the same keys whether the list is a tree, a
/// history or a set of stars, so they are bound once and the face decides what
/// they walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    Content,
    Panel,
}

impl FocusedPanel {
    /// Map panel to keybinding context for engine matching.
    pub fn key_context(&self) -> KeyContext {
        match self {
            Self::Content => KeyContext::Content,
            Self::Panel => KeyContext::Sidebar,
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
}
