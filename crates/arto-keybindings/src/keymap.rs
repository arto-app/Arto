//! `keys.toml`: a preset to start from and the changes made to it.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use toml_edit::{DocumentMut, Item, Table, Value};

use crate::action::Action;
use crate::bindings::{BindingSet, KeyAction};
use crate::context::KeyContext;
use crate::presets;
use crate::shortcut::ShortcutSequence;

/// What a change writes in place of an action to take the keys away.
const UNBOUND: &str = "none";

/// A binding set Arto ships, which a keymap starts from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Preset {
    #[default]
    Default,
    Vim,
    Emacs,
    /// Nothing bound at all.
    None,
}

impl Preset {
    pub const ALL: [Self; 4] = [Self::Default, Self::Vim, Self::Emacs, Self::None];

    /// The bindings this preset holds.
    pub fn bindings(self) -> BindingSet {
        match self {
            Self::Default => presets::default::bindings(),
            Self::Vim => presets::vim::bindings(),
            Self::Emacs => presets::emacs::bindings(),
            Self::None => BindingSet::default(),
        }
    }

    /// The preset that `bindings` differs least from.
    ///
    /// A binding set that came from somewhere other than a keymap — an older
    /// file that held every binding — is written back as changes to this
    /// one, so the changes are as few as they can be.
    pub fn closest_to(bindings: &BindingSet) -> Self {
        Self::ALL
            .into_iter()
            .min_by_key(|preset| Keymap::between(*preset, bindings).len())
            .unwrap_or_default()
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Default => "default",
            Self::Vim => "vim",
            Self::Emacs => "emacs",
            Self::None => "none",
        })
    }
}

impl FromStr for Preset {
    type Err = KeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.to_string() == s)
            .ok_or_else(|| KeymapError::UnknownPreset(s.to_string()))
    }
}

/// A section of a binding set: the menu, `global`, or a context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Menu,
    Global,
    Context(KeyContext),
}

impl Section {
    /// Every section, in the order a keymap writes them.
    pub const ALL: [Self; 7] = [
        Self::Menu,
        Self::Global,
        Self::Context(KeyContext::Content),
        Self::Context(KeyContext::Sidebar),
        Self::Context(KeyContext::Search),
        Self::Context(KeyContext::Palette),
        Self::Context(KeyContext::Contents),
    ];

    /// The table this section is written as.
    pub fn name(&self) -> String {
        match self {
            Self::Menu => "menu".to_string(),
            Self::Global => "global".to_string(),
            Self::Context(context) => context.to_string(),
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.name() == name)
    }
}

impl BindingSet {
    /// The bindings a section holds.
    pub fn section(&self, section: Section) -> &Vec<KeyAction> {
        match section {
            Section::Menu => &self.menu_shortcuts,
            Section::Global => &self.global,
            Section::Context(context) => self.of(context),
        }
    }

    /// The same, to be edited.
    pub fn section_mut(&mut self, section: Section) -> &mut Vec<KeyAction> {
        match section {
            Section::Menu => &mut self.menu_shortcuts,
            Section::Global => &mut self.global,
            Section::Context(context) => self.of_mut(context),
        }
    }
}

/// One entry of a keymap: keys, and the action they now run, or `None` when
/// the keys are taken away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub key: String,
    pub action: Option<String>,
}

/// Why a `keys.toml` cannot be read.
///
/// An entry whose keys or action Arto does not know is not one of these: it
/// is read, and skipped when the bindings are resolved, as any other binding
/// would be.
#[derive(Debug, thiserror::Error)]
pub enum KeymapError {
    #[error(transparent)]
    Syntax(#[from] toml_edit::TomlError),
    #[error("unknown preset {0:?}")]
    UnknownPreset(String),
    #[error("unknown section {0:?}")]
    UnknownSection(String),
    #[error("{0:?} is not a string")]
    NotAString(String),
    #[error("section {0:?} is not a table")]
    NotATable(String),
}

/// A preset and the changes made to it, as `keys.toml` holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keymap {
    pub preset: Preset,
    changes: HashMap<Section, Vec<Change>>,
}

impl Keymap {
    /// Read a keymap from the text of a `keys.toml`.
    pub fn parse(text: impl AsRef<str>) -> Result<Self, KeymapError> {
        let document: DocumentMut = text.as_ref().parse()?;
        let mut keymap = Self::default();
        for (name, item) in document.iter() {
            if name == "preset" {
                let preset = item
                    .as_str()
                    .ok_or_else(|| KeymapError::NotAString(name.to_string()))?;
                keymap.preset = preset.parse()?;
                continue;
            }
            let section = Section::from_name(name)
                .ok_or_else(|| KeymapError::UnknownSection(name.to_string()))?;
            let table = item
                .as_table_like()
                .ok_or_else(|| KeymapError::NotATable(name.to_string()))?;
            let mut changes = Vec::new();
            for (key, value) in table.iter() {
                let action = value
                    .as_str()
                    .ok_or_else(|| KeymapError::NotAString(key.to_string()))?;
                changes.push(Change {
                    key: key.to_string(),
                    action: (action != UNBOUND).then(|| action.to_string()),
                });
            }
            if !changes.is_empty() {
                keymap.changes.insert(section, changes);
            }
        }
        Ok(keymap)
    }

    /// The keymap that turns `preset` into `bindings`.
    ///
    /// Keys bound more than once in a section are taken as their last
    /// binding, which is the one the engine runs.
    // The order of keys the preset already binds is not kept: a change
    // replaces the preset's binding in place. Only the menu reads order (its
    // first key for an action), and two menu keys for one action, out of the
    // preset's order, come only from an old `mappings.json`; recording order
    // in the file was not worth the format it would take.
    pub fn between(preset: Preset, bindings: &BindingSet) -> Self {
        let base = preset.bindings();
        let mut changes = HashMap::new();
        for section in Section::ALL {
            let before = base.section(section);
            let after = last_of_each_key(bindings.section(section));
            let mut section_changes = Vec::new();
            for binding in before {
                let normalized = normalize(&binding.key);
                match after.iter().find(|(key, _)| *key == normalized) {
                    None => section_changes.push(Change {
                        key: binding.key.clone(),
                        action: None,
                    }),
                    Some((_, now)) if now.action != binding.action => {
                        section_changes.push(Change {
                            key: now.key.clone(),
                            action: Some(now.action.clone()),
                        })
                    }
                    Some(_) => {}
                }
            }
            let known: Vec<String> = before.iter().map(|b| normalize(&b.key)).collect();
            for (normalized, now) in &after {
                if !known.contains(normalized) {
                    section_changes.push(Change {
                        key: now.key.clone(),
                        action: Some(now.action.clone()),
                    });
                }
            }
            if !section_changes.is_empty() {
                changes.insert(section, section_changes);
            }
        }
        Self { preset, changes }
    }

    /// The changes a section carries, in the order they apply.
    pub fn changes(&self, section: Section) -> &[Change] {
        self.changes.get(&section).map_or(&[], Vec::as_slice)
    }

    /// How many changes the keymap carries.
    pub fn len(&self) -> usize {
        self.changes.values().map(Vec::len).sum()
    }

    /// Whether the keymap leaves its preset as it is.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The bindings in force: the preset with the changes applied.
    pub fn resolve(&self) -> BindingSet {
        self.apply_to(self.preset.bindings())
    }

    /// `base` with the changes applied, whatever the preset.
    pub(crate) fn apply_to(&self, mut base: BindingSet) -> BindingSet {
        for (section, changes) in &self.changes {
            let bindings = base.section_mut(*section);
            for change in changes {
                let normalized = normalize(&change.key);
                let at = bindings
                    .iter()
                    .position(|binding| normalize(&binding.key) == normalized);
                match (at, &change.action) {
                    (Some(at), Some(action)) => {
                        bindings[at] = KeyAction {
                            key: change.key.clone(),
                            action: action.clone(),
                        }
                    }
                    (Some(at), None) => {
                        bindings.remove(at);
                    }
                    (None, Some(action)) => bindings.push(KeyAction {
                        key: change.key.clone(),
                        action: action.clone(),
                    }),
                    (None, None) => {}
                }
            }
        }
        base
    }

    /// The text of a `keys.toml` holding this keymap.
    ///
    /// `existing` is the file as it is; what still holds of it is kept as it
    /// was written, comments included, and only what changed is rewritten.
    pub fn write(&self, existing: Option<&str>) -> Result<String, KeymapError> {
        let existing = existing.unwrap_or_default();
        Self::parse(existing)?;
        let mut document: DocumentMut = existing.parse()?;

        let written = document
            .get("preset")
            .and_then(Item::as_str)
            .and_then(|name| name.parse::<Preset>().ok());
        if written != Some(self.preset) {
            if self.preset == Preset::default() {
                document.remove("preset");
            } else {
                document["preset"] = toml_edit::value(self.preset.to_string());
            }
        }

        for section in Section::ALL {
            let name = section.name();
            let changes = self.changes(section);
            if changes.is_empty() {
                document.remove(&name);
                continue;
            }
            if !document.get(&name).is_some_and(Item::is_table_like) {
                document.insert(&name, Item::Table(Table::new()));
            }
            let table = document[&name]
                .as_table_like_mut()
                .expect("the section was made a table above");
            let stale: Vec<String> = table
                .iter()
                .map(|(key, _)| key.to_string())
                .filter(|key| {
                    let normalized = normalize(key);
                    !changes.iter().any(|c| normalize(&c.key) == normalized)
                })
                .collect();
            for key in stale {
                table.remove(&key);
            }
            for change in changes {
                let action = change.action.as_deref().unwrap_or(UNBOUND);
                let normalized = normalize(&change.key);
                let mut written_keys = table
                    .iter()
                    .map(|(key, _)| key.to_string())
                    .filter(|key| normalize(key) == normalized)
                    .collect::<Vec<_>>()
                    .into_iter();
                let written_key = written_keys.next();
                // A later spelling of the same keys would win when the file
                // is read again, undoing the change written to the first.
                for duplicate in written_keys {
                    table.remove(&duplicate);
                }
                match written_key {
                    Some(key) => {
                        let item = table.get_mut(&key).expect("the key was just found");
                        if item.as_str() != Some(action) {
                            let mut value = Value::from(action);
                            if let Some(old) = item.as_value() {
                                *value.decor_mut() = old.decor().clone();
                            }
                            *item = Item::Value(value);
                        }
                    }
                    None => {
                        table.insert(&change.key, toml_edit::value(action));
                    }
                }
            }
        }

        Ok(document.to_string())
    }
}

/// The form two spellings of the same keys share, so `Command+O` finds
/// `Cmd+o`. Keys that cannot be read are compared as written.
pub(crate) fn normalize(key: &str) -> String {
    ShortcutSequence::from_str(key).map_or_else(|_| key.to_string(), |keys| keys.to_string())
}

/// Each key's last binding in `bindings` that the engine would run, in the
/// order the keys first appear.
///
/// The engine skips an action it does not know, so a later unknown binding
/// does not replace a known one; a key bound only to unknown actions keeps
/// its last, so the file still names it.
fn last_of_each_key(bindings: &[KeyAction]) -> Vec<(String, &KeyAction)> {
    let runs = |binding: &KeyAction| Action::from_str(&binding.action).is_ok();
    let mut last: Vec<(String, &KeyAction)> = Vec::new();
    for binding in bindings {
        let normalized = normalize(&binding.key);
        match last.iter_mut().find(|(key, _)| *key == normalized) {
            Some(entry) if runs(binding) || !runs(entry.1) => entry.1 = binding,
            Some(_) => {}
            None => last.push((normalized, binding)),
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    fn binding(key: &str, action: &str) -> KeyAction {
        KeyAction {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    fn bound<'a>(bindings: &'a BindingSet, section: Section, key: &str) -> Option<&'a str> {
        bindings
            .section(section)
            .iter()
            .rfind(|binding| binding.key == key)
            .map(|binding| binding.action.as_str())
    }

    #[test]
    fn an_empty_file_is_the_default_preset() {
        let keymap = Keymap::parse("").unwrap();
        assert_eq!(keymap.preset, Preset::Default);
        assert!(keymap.is_empty());
        assert_eq!(keymap.resolve(), Preset::Default.bindings());
    }

    #[test]
    fn the_preset_names_the_bindings_the_file_starts_from() {
        for preset in Preset::ALL {
            let keymap = Keymap::parse(format!("preset = \"{preset}\"")).unwrap();
            assert_eq!(keymap.resolve(), preset.bindings(), "{preset}");
        }
    }

    #[test]
    fn a_binding_for_keys_the_preset_binds_replaces_it_in_place() {
        let keymap = Keymap::parse(indoc! {r#"
            [menu]
            "Cmd+o" = "file.open_directory"
        "#})
        .unwrap();
        let base = Preset::Default.bindings();
        let resolved = keymap.resolve();
        let position = |set: &BindingSet| {
            set.menu_shortcuts
                .iter()
                .position(|binding| binding.key == "Cmd+o")
        };
        assert_eq!(
            bound(&resolved, Section::Menu, "Cmd+o"),
            Some("file.open_directory")
        );
        assert_eq!(position(&resolved), position(&base));
        assert_eq!(resolved.menu_shortcuts.len(), base.menu_shortcuts.len());
    }

    #[test]
    fn keys_written_another_way_are_the_same_keys() {
        let keymap = Keymap::parse(indoc! {r#"
            [menu]
            "Command+O" = "none"
        "#})
        .unwrap();
        assert_eq!(bound(&keymap.resolve(), Section::Menu, "Cmd+o"), None);
    }

    #[test]
    fn none_takes_the_keys_away() {
        let keymap = Keymap::parse(indoc! {r#"
            [menu]
            "Cmd+o" = "none"
        "#})
        .unwrap();
        let base = Preset::Default.bindings();
        let resolved = keymap.resolve();
        assert_eq!(bound(&resolved, Section::Menu, "Cmd+o"), None);
        assert_eq!(resolved.menu_shortcuts.len(), base.menu_shortcuts.len() - 1);
    }

    #[test]
    fn a_binding_for_new_keys_is_added_after_the_preset() {
        let keymap = Keymap::parse(indoc! {r#"
            [sidebar]
            "Ctrl+n" = "cursor.down"
            "Ctrl+p" = "cursor.up"
        "#})
        .unwrap();
        let resolved = keymap.resolve();
        let tail: Vec<_> = resolved.sidebar.iter().rev().take(2).rev().collect();
        assert_eq!(
            tail,
            [
                &binding("Ctrl+n", "cursor.down"),
                &binding("Ctrl+p", "cursor.up")
            ]
        );
    }

    #[test]
    fn the_none_preset_holds_only_what_the_file_binds() {
        let keymap = Keymap::parse(indoc! {r#"
            preset = "none"

            [global]
            "j" = "scroll.down"
        "#})
        .unwrap();
        assert_eq!(
            keymap.resolve(),
            BindingSet {
                global: vec![binding("j", "scroll.down")],
                ..Default::default()
            }
        );
    }

    #[test]
    fn keys_and_actions_arto_does_not_know_are_left_for_resolving_to_skip() {
        let keymap = Keymap::parse(indoc! {r#"
            preset = "none"

            [global]
            "Hyper+j" = "scroll.down"
            "j" = "scroll.sideways"
        "#})
        .unwrap();
        assert_eq!(keymap.resolve().binding_errors().len(), 2);
    }

    #[test]
    fn a_file_that_is_not_toml_is_not_read() {
        assert!(matches!(
            Keymap::parse("[global"),
            Err(KeymapError::Syntax(_))
        ));
    }

    #[test]
    fn a_preset_arto_does_not_ship_is_not_read() {
        assert!(matches!(
            Keymap::parse(r#"preset = "helix""#),
            Err(KeymapError::UnknownPreset(name)) if name == "helix"
        ));
    }

    #[test]
    fn a_section_arto_does_not_have_is_not_read() {
        assert!(matches!(
            Keymap::parse("[sidbar]"),
            Err(KeymapError::UnknownSection(name)) if name == "sidbar"
        ));
    }

    #[test]
    fn a_binding_that_is_not_a_string_is_not_read() {
        assert!(matches!(
            Keymap::parse("[global]\nj = 1"),
            Err(KeymapError::NotAString(key)) if key == "j"
        ));
    }

    #[test]
    fn a_section_that_is_not_a_table_is_not_read() {
        assert!(matches!(
            Keymap::parse(r#"global = "j""#),
            Err(KeymapError::NotATable(name)) if name == "global"
        ));
    }

    #[test]
    fn between_holds_only_what_differs_from_the_preset() {
        let mut bindings = Preset::Default.bindings();
        bindings
            .menu_shortcuts
            .retain(|binding| binding.key != "Cmd+p");
        bindings.global.push(binding("Ctrl+n", "scroll.down"));
        let keymap = Keymap::between(Preset::Default, &bindings);
        assert_eq!(
            keymap.changes(Section::Menu),
            [Change {
                key: "Cmd+p".to_string(),
                action: None
            }]
        );
        assert_eq!(
            keymap.changes(Section::Global),
            [Change {
                key: "Ctrl+n".to_string(),
                action: Some("scroll.down".to_string())
            }]
        );
        assert_eq!(keymap.len(), 2);
    }

    #[test]
    fn between_keeps_the_binding_the_engine_runs_over_a_later_unknown_one() {
        let mut bindings = Preset::Vim.bindings();
        bindings.global.push(binding("j", "scroll.sideways"));
        let keymap = Keymap::between(Preset::Vim, &bindings);
        assert!(keymap.is_empty(), "{:?}", keymap.changes(Section::Global));
    }

    #[test]
    fn between_keeps_an_unknown_action_bound_alone() {
        let mut bindings = Preset::Vim.bindings();
        bindings.global.push(binding("Ctrl+F12", "scroll.sideways"));
        let keymap = Keymap::between(Preset::Vim, &bindings);
        assert_eq!(
            keymap.changes(Section::Global),
            [Change {
                key: "Ctrl+F12".to_string(),
                action: Some("scroll.sideways".to_string())
            }]
        );
    }

    #[test]
    fn between_a_preset_and_itself_is_empty() {
        for preset in Preset::ALL {
            assert!(Keymap::between(preset, &preset.bindings()).is_empty());
        }
    }

    fn edited_vim_bindings() -> BindingSet {
        let mut bindings = Preset::Vim.bindings();
        bindings.global.retain(|binding| binding.key != "j");
        bindings.global[0].action = "zoom.in".to_string();
        bindings.sidebar.push(binding("Ctrl+o", "cursor.open"));
        bindings
    }

    #[test]
    fn between_and_resolve_lead_back_to_the_same_bindings_in_the_same_order() {
        let bindings = edited_vim_bindings();
        assert_eq!(Keymap::between(Preset::Vim, &bindings).resolve(), bindings);
    }

    #[test]
    fn between_another_preset_and_resolve_lead_back_to_the_same_bindings() {
        let sorted = |mut set: BindingSet| {
            for section in Section::ALL {
                set.section_mut(section).sort_by(|a, b| a.key.cmp(&b.key));
            }
            set
        };
        let bindings = edited_vim_bindings();
        for preset in Preset::ALL {
            assert_eq!(
                sorted(Keymap::between(preset, &bindings).resolve()),
                sorted(bindings.clone()),
                "from {preset}"
            );
        }
    }

    #[test]
    fn the_closest_preset_is_the_one_with_fewest_changes() {
        let mut bindings = Preset::Emacs.bindings();
        bindings.global.push(binding("Ctrl+z", "zoom.in"));
        assert_eq!(Preset::closest_to(&bindings), Preset::Emacs);
        assert_eq!(Preset::closest_to(&BindingSet::default()), Preset::None);
    }

    #[test]
    fn a_keymap_with_nothing_to_say_writes_nothing() {
        assert_eq!(Keymap::default().write(None).unwrap(), "");
    }

    #[test]
    fn writing_a_new_file_writes_the_preset_then_each_section_in_order() {
        let keymap = Keymap::parse(indoc! {r#"
            [sidebar]
            "Ctrl+n" = "cursor.down"

            [menu]
            "Cmd+p" = "none"
        "#})
        .unwrap();
        let keymap = Keymap {
            preset: Preset::Vim,
            ..keymap
        };
        assert_eq!(
            keymap.write(None).unwrap(),
            indoc! {r#"
                preset = "vim"

                [menu]
                "Cmd+p" = "none"

                [sidebar]
                "Ctrl+n" = "cursor.down"
            "#}
        );
    }

    #[test]
    fn writing_keeps_what_still_holds_as_it_was_written() {
        let existing = indoc! {r#"
            # My keys.
            preset = "default"

            [global]
            # Emacs habits.
            "Ctrl+n" = "scroll.down" # down
            "Ctrl+p" = "scroll.up"
            "Ctrl+v" = "scroll.page_down"
        "#};
        let mut bindings = Keymap::parse(existing).unwrap().resolve();
        bindings.global.retain(|binding| binding.key != "Ctrl+v");
        bindings
            .global
            .iter_mut()
            .filter(|binding| binding.key == "Ctrl+p")
            .for_each(|binding| binding.action = "scroll.top".to_string());
        let written = Keymap::between(Preset::Default, &bindings)
            .write(Some(existing))
            .unwrap();
        assert_eq!(
            written,
            indoc! {r#"
                # My keys.
                preset = "default"

                [global]
                # Emacs habits.
                "Ctrl+n" = "scroll.down" # down
                "Ctrl+p" = "scroll.top"
            "#}
        );
    }

    #[test]
    fn writing_changes_the_preset_and_drops_sections_left_empty() {
        let existing = indoc! {r#"
            preset = "vim"

            [global]
            "Ctrl+n" = "scroll.down"
        "#};
        assert_eq!(Keymap::default().write(Some(existing)).unwrap(), "");
        let emacs = Keymap {
            preset: Preset::Emacs,
            ..Keymap::default()
        };
        assert_eq!(emacs.write(Some(existing)).unwrap(), "preset = \"emacs\"\n");
    }

    #[test]
    fn writing_over_a_file_that_is_not_toml_is_refused() {
        assert!(matches!(
            Keymap::default().write(Some("[global")),
            Err(KeymapError::Syntax(_))
        ));
    }

    #[test]
    fn writing_over_a_file_that_is_not_a_keymap_is_refused() {
        for existing in [
            "preset = \"helix\"\n",
            "[global]\n\"Ctrl+n\" = 1\n",
            "[nowhere]\n\"Ctrl+n\" = \"scroll.down\"\n",
        ] {
            assert!(
                Keymap::default().write(Some(existing)).is_err(),
                "{existing}"
            );
        }
    }

    #[test]
    fn writing_keys_spelled_twice_leaves_one_spelling() {
        let existing = indoc! {r#"
            [global]
            "Ctrl+n" = "scroll.down"
            "Control+N" = "scroll.up"
        "#};
        let mut bindings = Keymap::parse(existing).unwrap().resolve();
        bindings
            .global
            .iter_mut()
            .filter(|binding| normalize(&binding.key) == normalize("Ctrl+n"))
            .for_each(|binding| binding.action = "scroll.bottom".to_string());
        let written = Keymap::between(Preset::Default, &bindings)
            .write(Some(existing))
            .unwrap();
        let reread = Keymap::parse(&written).unwrap().resolve();
        let ctrl_n: Vec<&str> = reread
            .global
            .iter()
            .filter(|binding| normalize(&binding.key) == normalize("Ctrl+n"))
            .map(|binding| binding.action.as_str())
            .collect();
        assert_eq!(ctrl_n, ["scroll.bottom"], "{written}");
        assert_eq!(written.matches("scroll.").count(), 1, "{written}");
    }
}
