//! A binding set seen action by action: which keys run an action, and where
//! a key added to it is bound.

use keyboard_types::Modifiers;

use crate::action::{Action, MENU_ACTIONS};
use crate::bindings::{BindingSet, KeyAction};
use crate::codes::single_chord_code;
use crate::context::KeyContext;
use crate::keymap::{normalize, Section};

impl Action {
    /// The section where this action does what it does.
    ///
    /// Most actions mean the same wherever the keys are and belong in
    /// `global`; the ones that move through a list or a field only make sense
    /// while that list or field has the keys.
    pub fn home(&self) -> Section {
        let context = match self {
            Self::ContentNext
            | Self::ContentPrev
            | Self::ContentNextHeading
            | Self::ContentPrevHeading
            | Self::ContentOpenViewer
            | Self::HighlightAdd
            | Self::HighlightRemove
            | Self::HighlightNote
            | Self::ChangesNext
            | Self::ChangesPrev => KeyContext::Content,
            Self::CursorDown
            | Self::CursorUp
            | Self::CursorEnter
            | Self::CursorOpen
            | Self::CursorCollapse
            | Self::DirectoryParent
            | Self::FocusContent
            | Self::SidebarFaceNext
            | Self::SidebarFacePrev
            | Self::SidebarToggleShowAllFiles => KeyContext::Sidebar,
            Self::SearchClear | Self::SearchPinCurrent => KeyContext::Search,
            Self::PaletteNext | Self::PalettePrev | Self::PaletteConfirm | Self::PaletteClose => {
                KeyContext::Palette
            }
            Self::ContentsNext
            | Self::ContentsPrev
            | Self::ContentsConfirm
            | Self::ContentsClose => KeyContext::Contents,
            _ => return Section::Global,
        };
        Section::Context(context)
    }
}

/// Keys that run an action, and the sections they run it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundKey {
    /// As the first binding of these keys writes them.
    pub key: String,
    pub sections: Vec<Section>,
}

impl BindingSet {
    /// The keys that run `action`, each once however many sections bind it,
    /// in the order the sections are written.
    pub fn keys_for(&self, action: impl AsRef<str>) -> Vec<BoundKey> {
        let action = action.as_ref();
        let mut keys: Vec<(String, BoundKey)> = Vec::new();
        for section in Section::ALL {
            for binding in self.section(section) {
                if binding.action != action {
                    continue;
                }
                let normalized = normalize(&binding.key);
                match keys.iter_mut().find(|(key, _)| *key == normalized) {
                    Some((_, bound)) if !bound.sections.contains(&section) => {
                        bound.sections.push(section)
                    }
                    Some(_) => {}
                    None => keys.push((
                        normalized,
                        BoundKey {
                            key: binding.key.clone(),
                            sections: vec![section],
                        },
                    )),
                }
            }
        }
        keys.into_iter().map(|(_, bound)| bound).collect()
    }

    /// Where `key` goes when it is added to `action`.
    ///
    /// A menu item carries one shortcut, a single chord held with Cmd or
    /// Ctrl: the first such key of an action the menu offers becomes the
    /// item's. A bare key is never a menu shortcut, because the menu would
    /// take it from every text field. Everything else goes to the action's
    /// [home](Action::home).
    pub fn section_for_new_key(&self, action: Action, key: impl AsRef<str>) -> Section {
        let held = single_chord_code(key.as_ref()).is_some_and(|(modifiers, _)| {
            modifiers.intersects(Modifiers::META | Modifiers::CONTROL)
        });
        let action_name = action.to_string();
        let has_menu_shortcut = self
            .menu_shortcuts
            .iter()
            .any(|binding| binding.action == action_name);
        if held && MENU_ACTIONS.contains(&action) && !has_menu_shortcut {
            Section::Menu
        } else {
            action.home()
        }
    }

    /// Bind `key` to `action` where [`BindingSet::section_for_new_key`] says,
    /// taking the keys from whatever they ran there. Returns that section.
    ///
    /// Keys the action already has stay where they are, so the menu shortcut
    /// that works with every window closed is not moved into a window.
    pub fn bind(&mut self, action: Action, key: impl AsRef<str>) -> Section {
        let key = key.as_ref();
        let (normalized, id) = (normalize(key), action.to_string());
        let held = Section::ALL.into_iter().find(|section| {
            self.section(*section)
                .iter()
                .any(|binding| binding.action == id && normalize(&binding.key) == normalized)
        });
        if let Some(section) = held {
            return section;
        }
        let section = self.section_for_new_key(action, key);
        put(self, section, key, &action.to_string());
        section
    }

    /// Take `key` away from `action` in every section it runs it in.
    pub fn unbind(&mut self, action: impl AsRef<str>, key: impl AsRef<str>) {
        let action = action.as_ref();
        let normalized = normalize(key.as_ref());
        for section in Section::ALL {
            self.section_mut(section).retain(|binding| {
                binding.action != action || normalize(&binding.key) != normalized
            });
        }
    }

    /// Give `action` the keys `preset` gives it, and no others. Keys the
    /// preset gives it are taken from whatever they run here.
    pub fn restore(&mut self, action: impl AsRef<str>, preset: &BindingSet) {
        let action = action.as_ref();
        for section in Section::ALL {
            self.section_mut(section)
                .retain(|binding| binding.action != action);
        }
        for section in Section::ALL {
            for binding in preset.section(section) {
                if binding.action == action {
                    put(self, section, &binding.key, action);
                }
            }
        }
    }

    /// Whether `action` runs from the same keys in the same sections here as
    /// in `other`.
    pub fn binds_alike(&self, action: impl AsRef<str>, other: &BindingSet) -> bool {
        let mut mine = placed_keys(self, action.as_ref());
        let mut theirs = placed_keys(other, action.as_ref());
        mine.sort();
        theirs.sort();
        mine == theirs
    }
}

impl Section {
    /// The sections keys bound here are taken from.
    ///
    /// The menu takes the keys from `global` and the contexts as well, and
    /// they from it: on macOS the menu sees a chord before the window does,
    /// so a chord in both would leave the window's binding unreachable.
    /// `global` and a context keep the same keys apart, since the context's
    /// own binding wins where it applies.
    pub fn rivals(self) -> Vec<Section> {
        match self {
            Section::Menu => Section::ALL.to_vec(),
            Section::Global => vec![Section::Global, Section::Menu],
            context => vec![context, Section::Menu],
        }
    }
}

/// Bind `key` to `action` in `section`, in place of what it ran in its
/// [rivals](Section::rivals).
fn put(set: &mut BindingSet, section: Section, key: &str, action: &str) {
    let normalized = normalize(key);
    for rival in section.rivals() {
        set.section_mut(rival)
            .retain(|binding| normalize(&binding.key) != normalized);
    }
    set.section_mut(section).push(KeyAction {
        key: key.to_string(),
        action: action.to_string(),
    });
}

fn placed_keys(bindings: &BindingSet, action: &str) -> Vec<(usize, String)> {
    Section::ALL
        .into_iter()
        .enumerate()
        .flat_map(|(at, section)| {
            bindings
                .section(section)
                .iter()
                .filter(move |binding| binding.action == action)
                .map(move |binding| (at, normalize(&binding.key)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Preset;

    fn binding(key: &str, action: &str) -> KeyAction {
        KeyAction {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    #[test]
    fn an_action_that_moves_through_a_list_lives_with_the_list() {
        assert_eq!(
            Action::PaletteNext.home(),
            Section::Context(KeyContext::Palette)
        );
        assert_eq!(
            Action::CursorDown.home(),
            Section::Context(KeyContext::Sidebar)
        );
        assert_eq!(
            Action::ContentsConfirm.home(),
            Section::Context(KeyContext::Contents)
        );
        assert_eq!(
            Action::SearchClear.home(),
            Section::Context(KeyContext::Search)
        );
        assert_eq!(
            Action::HighlightAdd.home(),
            Section::Context(KeyContext::Content)
        );
    }

    #[test]
    fn an_action_that_means_the_same_everywhere_lives_in_global() {
        for action in [
            Action::ScrollDown,
            Action::PaletteOpen,
            Action::ContentsToggle,
            Action::SearchNext,
            Action::FileOpen,
        ] {
            assert_eq!(action.home(), Section::Global, "{action}");
        }
    }

    #[test]
    fn every_preset_binds_each_action_at_home_or_in_the_menu() {
        for preset in Preset::ALL {
            let bindings = preset.bindings();
            for section in Section::ALL {
                for bound in bindings.section(section) {
                    let action: Action = bound.action.parse().unwrap();
                    // A few actions are bound in more than one place on
                    // purpose: moving to the places from wherever the keys
                    // are, and walking matches from inside the find field.
                    let elsewhere_on_purpose = matches!(
                        action,
                        Action::FocusPlaces | Action::SearchNext | Action::SearchPrev
                    );
                    assert!(
                        section == action.home()
                            || section == Section::Menu
                            || elsewhere_on_purpose,
                        "{preset} binds {action} in {}",
                        section.name()
                    );
                }
            }
        }
    }

    #[test]
    fn keys_for_lists_each_key_once_with_every_section_it_runs_the_action_in() {
        let bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+g", "search.next")],
            global: vec![binding("n", "search.next"), binding("j", "scroll.down")],
            search: vec![
                binding("Ctrl+n", "search.next"),
                binding("N", "search.next"),
            ],
            ..Default::default()
        };
        assert_eq!(
            bindings.keys_for("search.next"),
            [
                BoundKey {
                    key: "Cmd+g".to_string(),
                    sections: vec![Section::Menu]
                },
                BoundKey {
                    key: "n".to_string(),
                    sections: vec![Section::Global, Section::Context(KeyContext::Search)]
                },
                BoundKey {
                    key: "Ctrl+n".to_string(),
                    sections: vec![Section::Context(KeyContext::Search)]
                },
            ]
        );
    }

    #[test]
    fn the_first_held_chord_of_a_menu_action_becomes_its_menu_shortcut() {
        let bindings = BindingSet::default();
        assert_eq!(
            bindings.section_for_new_key(Action::FileOpen, "Cmd+o"),
            Section::Menu
        );
        assert_eq!(
            bindings.section_for_new_key(Action::FileOpen, "Ctrl+o"),
            Section::Menu
        );
    }

    #[test]
    fn a_menu_action_with_a_shortcut_already_takes_more_keys_at_home() {
        let bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+o", "file.open")],
            ..Default::default()
        };
        assert_eq!(
            bindings.section_for_new_key(Action::FileOpen, "Ctrl+o"),
            Section::Global
        );
    }

    #[test]
    fn a_bare_key_or_a_sequence_is_never_a_menu_shortcut() {
        let bindings = BindingSet::default();
        assert_eq!(
            bindings.section_for_new_key(Action::SearchOpen, "/"),
            Section::Global
        );
        assert_eq!(
            bindings.section_for_new_key(Action::SearchOpen, "Shift+s"),
            Section::Global
        );
        assert_eq!(
            bindings.section_for_new_key(Action::ZoomIn, "Cmd+z Cmd+i"),
            Section::Global
        );
    }

    #[test]
    fn an_action_the_menu_does_not_offer_takes_held_chords_at_home() {
        let bindings = BindingSet::default();
        assert_eq!(
            bindings.section_for_new_key(Action::PaletteNext, "Ctrl+j"),
            Section::Context(KeyContext::Palette)
        );
    }

    #[test]
    fn binding_takes_the_keys_from_what_they_ran_in_that_section() {
        let mut bindings = BindingSet {
            sidebar: vec![binding("Ctrl+n", "cursor.down")],
            global: vec![binding("Ctrl+n", "scroll.down")],
            ..Default::default()
        };
        let section = bindings.bind(Action::CursorUp, "Ctrl+N");
        assert_eq!(section, Section::Context(KeyContext::Sidebar));
        assert_eq!(bindings.sidebar, [binding("Ctrl+N", "cursor.up")]);
        assert_eq!(bindings.global, [binding("Ctrl+n", "scroll.down")]);
    }

    #[test]
    fn binding_keys_the_action_already_has_leaves_them_where_they_are() {
        let mut bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+n", "window.new")],
            ..Default::default()
        };
        let section = bindings.bind(Action::WindowNew, "Command+N");
        assert_eq!(section, Section::Menu);
        assert_eq!(bindings.menu_shortcuts, [binding("Cmd+n", "window.new")]);
        assert!(bindings.global.is_empty());
    }

    #[test]
    fn binding_in_a_context_takes_the_keys_from_the_menu() {
        let mut bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+o", "file.open")],
            ..Default::default()
        };
        let section = bindings.bind(Action::CursorDown, "Cmd+o");
        assert_eq!(section, Section::Context(KeyContext::Sidebar));
        assert_eq!(bindings.sidebar, [binding("Cmd+o", "cursor.down")]);
        assert!(bindings.menu_shortcuts.is_empty());

        let section = bindings.bind(Action::FileOpen, "Cmd+o");
        assert_eq!(section, Section::Menu);
        assert_eq!(bindings.menu_shortcuts, [binding("Cmd+o", "file.open")]);
        assert!(bindings.sidebar.is_empty());
    }

    #[test]
    fn unbinding_takes_the_keys_away_from_the_action_everywhere_and_nothing_else() {
        let mut bindings = BindingSet {
            global: vec![binding("n", "search.next"), binding("j", "scroll.down")],
            search: vec![binding("n", "search.next")],
            content: vec![binding("n", "content.next")],
            ..Default::default()
        };
        bindings.unbind("search.next", "n");
        assert_eq!(bindings.global, [binding("j", "scroll.down")]);
        assert!(bindings.search.is_empty());
        assert_eq!(bindings.content, [binding("n", "content.next")]);
    }

    #[test]
    fn restoring_gives_the_action_the_preset_keys_alone() {
        let preset = Preset::Default.bindings();
        let mut bindings = preset.clone();
        bindings.unbind("file.open", "Cmd+o");
        bindings.bind(Action::FileOpen, "Cmd+Shift+y");
        bindings.global.push(binding("Ctrl+o", "file.open"));
        assert!(!bindings.binds_alike("file.open", &preset));

        bindings.restore("file.open", &preset);
        assert!(bindings.binds_alike("file.open", &preset));
        assert_eq!(bindings.keys_for("file.open"), preset.keys_for("file.open"));
    }

    #[test]
    fn restoring_takes_the_preset_keys_from_whatever_ran_them() {
        let preset = Preset::Default.bindings();
        let mut bindings = preset.clone();
        bindings.unbind("file.open", "Cmd+o");
        bindings.bind(Action::FileOpenDirectory, "Cmd+o");
        bindings.restore("file.open", &preset);
        assert!(bindings
            .keys_for("file.open_directory")
            .iter()
            .all(|bound| bound.key != "Cmd+o"));
    }

    #[test]
    fn binds_alike_ignores_order_and_spelling() {
        let one = BindingSet {
            global: vec![
                binding("j", "scroll.down"),
                binding("Ctrl+n", "scroll.down"),
            ],
            ..Default::default()
        };
        let other = BindingSet {
            global: vec![
                binding("Control+N", "scroll.down"),
                binding("J", "scroll.down"),
            ],
            ..Default::default()
        };
        assert!(one.binds_alike("scroll.down", &other));
    }
}
