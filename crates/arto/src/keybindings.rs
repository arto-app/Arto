//! Keybindings for the desktop app.
//!
//! The model (shortcut parsing, binding sets, presets, the matching engine,
//! hint formatting) lives in `arto-keybindings`. This module binds it to the
//! things only the app has: Dioxus keyboard events, native menu accelerators,
//! the user's configuration in `CONFIG`, and the dispatcher that turns matched
//! actions into behavior.

mod accelerator;
pub mod dispatcher;

pub use accelerator::*;
pub use arto_keybindings::*;

use crate::config::{BindingSet, KeyAction, Lens, CONFIG};
use dioxus::events::KeyboardEvent;
use dioxus::prelude::ModifiersInteraction;
use rust_i18n::t;
use std::str::FromStr;

/// Build a chord from a Dioxus keyboard event.
pub fn chord_from_event(event: &KeyboardEvent) -> KeyChord {
    KeyChord::new(event.data().key(), event.data().modifiers())
}

/// The bindings a window's keys are matched against: the configured ones,
/// and each usable lens's own shortcut.
pub fn effective_bindings() -> BindingSet {
    let keybindings = CONFIG.read().keybindings.clone();
    with_lens_shortcuts(keybindings, &crate::lenses::offered_lenses())
}

/// `bindings` with the shortcut of each of `lenses` bound to that lens by
/// its place among them, over the document: a lens looks at the document,
/// and a single letter bound everywhere would fire while typing a search.
///
/// A key something else already has stays with it. The engine lets the
/// last binding of a key win, so a lens appended after the keybindings
/// would otherwise take the key from them without a word.
fn with_lens_shortcuts(bindings: BindingSet, lenses: &[Lens]) -> BindingSet {
    let mut bound = bindings.clone();
    for (place, lens) in lenses.iter().enumerate() {
        let (Some(key), Ok(index)) = (&lens.shortcut, u16::try_from(place)) else {
            continue;
        };
        if let Some(holder) = lens_shortcut_holder(&bindings, lenses, place) {
            tracing::warn!(lens = %lens.id, %key, %holder, "a lens shortcut is already bound");
            continue;
        }
        bound.content.push(KeyAction {
            key: key.clone(),
            action: Action::Lens(index).to_string(),
        });
    }
    bound
}

/// What already has the shortcut of the lens at `place` among `lenses`: an
/// action of `bindings` that is heard over the document, or a lens before
/// it. `None` when the key is free, or when the lens has no shortcut that
/// can be read.
///
/// A binding that is the start of the other counts as having it: the
/// engine runs a sequence the moment it is complete, so of `g` and `g g`
/// the longer could never be finished.
pub fn lens_shortcut_holder(
    bindings: &BindingSet,
    lenses: &[Lens],
    place: usize,
) -> Option<String> {
    let chords = lenses
        .get(place)?
        .shortcut
        .as_deref()
        .and_then(|key| ShortcutSequence::from_str(key).ok())?
        .chords;
    let same = |key: &str| {
        ShortcutSequence::from_str(key).is_ok_and(|other| {
            other.chords.starts_with(&chords) || chords.starts_with(&other.chords)
        })
    };
    let bound = [
        &bindings.menu_shortcuts,
        &bindings.global,
        &bindings.content,
    ]
    .into_iter()
    .flatten()
    .find(|binding| same(&binding.key))
    .map(|binding| binding.action.clone());
    bound.or_else(|| {
        lenses[..place]
            .iter()
            .find(|lens| lens.shortcut.as_deref().is_some_and(same))
            .map(|lens| t!("keybindings.lens_shortcut_holder", label = lens.label).into_owned())
    })
}

/// What an action is called where the reader sees it: the shortcut overlay,
/// and inside its group in the preferences.
///
/// Spelled out per action rather than built from its id, so that each
/// language can name it in its own words.
pub fn action_name(action: Action) -> String {
    let name = match action {
        Action::ScrollDown => t!("keybindings.actions.scroll.down"),
        Action::ScrollUp => t!("keybindings.actions.scroll.up"),
        Action::ScrollPageDown => t!("keybindings.actions.scroll.page_down"),
        Action::ScrollPageUp => t!("keybindings.actions.scroll.page_up"),
        Action::ScrollHalfPageDown => t!("keybindings.actions.scroll.half_page_down"),
        Action::ScrollHalfPageUp => t!("keybindings.actions.scroll.half_page_up"),
        Action::ScrollTop => t!("keybindings.actions.scroll.top"),
        Action::ScrollBottom => t!("keybindings.actions.scroll.bottom"),
        Action::HistoryBack => t!("keybindings.actions.history.back"),
        Action::HistoryForward => t!("keybindings.actions.history.forward"),
        Action::SearchOpen => t!("keybindings.actions.search.open"),
        Action::SearchNext => t!("keybindings.actions.search.next"),
        Action::SearchPrev => t!("keybindings.actions.search.prev"),
        Action::SearchClear => t!("keybindings.actions.search.clear"),
        Action::SearchPinCurrent => t!("keybindings.actions.search.pin_current"),
        Action::HighlightAdd => t!("keybindings.actions.highlight.add"),
        Action::HighlightRemove => t!("keybindings.actions.highlight.remove"),
        Action::HighlightNote => t!("keybindings.actions.highlight.note"),
        Action::ZoomIn => t!("keybindings.actions.zoom.in"),
        Action::ZoomOut => t!("keybindings.actions.zoom.out"),
        Action::ZoomReset => t!("keybindings.actions.zoom.reset"),
        Action::CopyFilePath => t!("keybindings.actions.clipboard.copy_file_path"),
        Action::CopyFilePathWithLine => {
            t!("keybindings.actions.clipboard.copy_file_path_with_line")
        }
        Action::CopyFilePathWithRange => {
            t!("keybindings.actions.clipboard.copy_file_path_with_range")
        }
        Action::CopyAsMarkdown => t!("keybindings.actions.clipboard.copy_as_markdown"),
        Action::CopyCode => t!("keybindings.actions.clipboard.copy_code"),
        Action::CopyCodeAsMarkdown => t!("keybindings.actions.clipboard.copy_code_as_markdown"),
        Action::CopyTableAsTsv => t!("keybindings.actions.clipboard.copy_table_as_tsv"),
        Action::CopyTableAsCsv => t!("keybindings.actions.clipboard.copy_table_as_csv"),
        Action::CopyTableAsMarkdown => t!("keybindings.actions.clipboard.copy_table_as_markdown"),
        Action::CopyImage => t!("keybindings.actions.clipboard.copy_image"),
        Action::CopyImageWithBackground => {
            t!("keybindings.actions.clipboard.copy_image_with_background")
        }
        Action::CopyImagePath => t!("keybindings.actions.clipboard.copy_image_path"),
        Action::CopyImageAsMarkdown => t!("keybindings.actions.clipboard.copy_image_as_markdown"),
        Action::CopyLinkPath => t!("keybindings.actions.clipboard.copy_link_path"),
        Action::WindowNew => t!("keybindings.actions.window.new"),
        Action::WindowDuplicate => t!("keybindings.actions.window.duplicate"),
        Action::WindowNewDocument => t!("keybindings.actions.window.new_document"),
        Action::WindowClose => t!("keybindings.actions.window.close"),
        Action::WindowCloseAllChildWindows => {
            t!("keybindings.actions.window.close_all_child_windows")
        }
        Action::WindowCloseAllWindows => t!("keybindings.actions.window.close_all_windows"),
        Action::WindowToggleSidebar => t!("keybindings.actions.window.toggle_sidebar"),
        Action::WindowToggleFocusMode => t!("keybindings.actions.window.toggle_focus_mode"),
        Action::WindowReload => t!("keybindings.actions.window.reload"),
        Action::FocusPlaces => t!("keybindings.actions.focus.places"),
        Action::FocusStarred => t!("keybindings.actions.focus.starred"),
        Action::FocusRecent => t!("keybindings.actions.focus.recent"),
        Action::FocusLinks => t!("keybindings.actions.focus.links"),
        Action::FocusContent => t!("keybindings.actions.focus.content"),
        Action::FileOpen => t!("keybindings.actions.file.open"),
        Action::FileOpenDirectory => t!("keybindings.actions.file.open_directory"),
        Action::FileSetParentAsRoot => t!("keybindings.actions.file.set_parent_as_root"),
        Action::FileToggleBookmark => t!("keybindings.actions.file.toggle_bookmark"),
        Action::FileOpenLink => t!("keybindings.actions.file.open_link"),
        Action::FileOpenLinkInNewWindow => t!("keybindings.actions.file.open_link_in_new_window"),
        Action::FilePreviewLink => t!("keybindings.actions.file.preview_link"),
        Action::FileSaveImageAs => t!("keybindings.actions.file.save_image_as"),
        Action::FilePreferences => t!("keybindings.actions.file.preferences"),
        Action::FileRevealInFinder => t!("keybindings.actions.file.reveal_in_finder"),
        Action::FilePrint => t!("keybindings.actions.file.print"),
        Action::AppAbout => t!("keybindings.actions.app.about"),
        Action::AppQuit => t!("keybindings.actions.app.quit"),
        Action::AppGoToHomepage => t!("keybindings.actions.app.go_to_homepage"),
        Action::HelpShowKeyboardShortcuts => t!("keybindings.actions.help.show_keyboard_shortcuts"),
        Action::PaletteOpen => t!("keybindings.actions.palette.open"),
        Action::PaletteNext => t!("keybindings.actions.palette.next"),
        Action::PalettePrev => t!("keybindings.actions.palette.prev"),
        Action::PaletteConfirm => t!("keybindings.actions.palette.confirm"),
        Action::PaletteClose => t!("keybindings.actions.palette.close"),
        Action::ContentsToggle => t!("keybindings.actions.contents.toggle"),
        Action::ContentsNext => t!("keybindings.actions.contents.next"),
        Action::ContentsPrev => t!("keybindings.actions.contents.prev"),
        Action::ContentsConfirm => t!("keybindings.actions.contents.confirm"),
        Action::ContentsClose => t!("keybindings.actions.contents.close"),
        Action::ChangesNext => t!("keybindings.actions.changes.next"),
        Action::ChangesPrev => t!("keybindings.actions.changes.prev"),
        Action::ChangesMarkRead => t!("keybindings.actions.changes.mark_read"),
        Action::LensStop => t!("keybindings.actions.lens.stop"),
        Action::LensHide => t!("keybindings.actions.lens.hide"),
        Action::SidebarToggleShowAllFiles => {
            t!("keybindings.actions.sidebar.toggle_show_all_files")
        }
        Action::SidebarFacePlaces => t!("keybindings.actions.sidebar.face_places"),
        Action::SidebarFaceRecent => t!("keybindings.actions.sidebar.face_recent"),
        Action::SidebarFaceStarred => t!("keybindings.actions.sidebar.face_starred"),
        Action::SidebarFaceLinks => t!("keybindings.actions.sidebar.face_links"),
        Action::SidebarFaceNext => t!("keybindings.actions.sidebar.face_next"),
        Action::SidebarFacePrev => t!("keybindings.actions.sidebar.face_prev"),
        Action::ThemeSetLight => t!("keybindings.actions.theme.set_light"),
        Action::ThemeSetDark => t!("keybindings.actions.theme.set_dark"),
        Action::ThemeSetAuto => t!("keybindings.actions.theme.set_auto"),
        Action::CursorDown => t!("keybindings.actions.cursor.down"),
        Action::CursorUp => t!("keybindings.actions.cursor.up"),
        Action::CursorEnter => t!("keybindings.actions.cursor.enter"),
        Action::CursorOpen => t!("keybindings.actions.cursor.open"),
        Action::CursorCollapse => t!("keybindings.actions.cursor.collapse"),
        Action::ContentNext => t!("keybindings.actions.content.next"),
        Action::ContentPrev => t!("keybindings.actions.content.prev"),
        Action::ContentNextHeading => t!("keybindings.actions.content.next_heading"),
        Action::ContentPrevHeading => t!("keybindings.actions.content.prev_heading"),
        Action::ContentOpenViewer => t!("keybindings.actions.content.open_viewer"),
        Action::DirectoryParent => t!("keybindings.actions.directory.parent"),
        Action::Cancel => t!("keybindings.actions.cancel"),
        Action::Lens(index) => t!("keybindings.actions.lens.at", index = index),
    };
    name.into_owned()
}

/// What the palette calls an action that answers to a name, and what a
/// typed query is matched against: [`Action::command_label`] in the
/// interface's language, so a reader finds a command in the words they read.
pub fn command_name(action: Action) -> Option<String> {
    let name = match action {
        Action::HistoryBack => t!("commands.history.back"),
        Action::HistoryForward => t!("commands.history.forward"),
        Action::SearchOpen => t!("commands.search.open"),
        Action::HighlightNote => t!("commands.highlight.note"),
        Action::ZoomIn => t!("commands.zoom.in"),
        Action::ZoomOut => t!("commands.zoom.out"),
        Action::ZoomReset => t!("commands.zoom.reset"),
        Action::CopyFilePath => t!("commands.clipboard.copy_file_path"),
        Action::CopyAsMarkdown => t!("commands.clipboard.copy_as_markdown"),
        Action::WindowNew => t!("commands.window.new"),
        Action::WindowDuplicate => t!("commands.window.duplicate"),
        Action::WindowNewDocument => t!("commands.window.new_document"),
        Action::WindowClose => t!("commands.window.close"),
        Action::WindowCloseAllChildWindows => t!("commands.window.close_all_child_windows"),
        Action::WindowCloseAllWindows => t!("commands.window.close_all_windows"),
        Action::WindowToggleSidebar => t!("commands.window.toggle_sidebar"),
        Action::WindowToggleFocusMode => t!("commands.window.toggle_focus_mode"),
        Action::WindowReload => t!("commands.window.reload"),
        Action::FileOpen => t!("commands.file.open"),
        Action::FileOpenDirectory => t!("commands.file.open_directory"),
        Action::FileSetParentAsRoot => t!("commands.file.set_parent_as_root"),
        Action::FileToggleBookmark => t!("commands.file.toggle_bookmark"),
        Action::FilePreferences => t!("commands.file.preferences"),
        Action::FileRevealInFinder => t!("commands.file.reveal_in_finder"),
        Action::FilePrint => t!("commands.file.print"),
        Action::AppAbout => t!("commands.app.about"),
        Action::AppQuit => t!("commands.app.quit"),
        Action::AppGoToHomepage => t!("commands.app.go_to_homepage"),
        Action::ContentsToggle => t!("commands.contents.toggle"),
        Action::ChangesMarkRead => t!("commands.changes.mark_read"),
        Action::LensStop => t!("commands.lens.stop"),
        Action::LensHide => t!("commands.lens.hide"),
        Action::SidebarToggleShowAllFiles => t!("commands.sidebar.toggle_show_all_files"),
        Action::SidebarFacePlaces => t!("commands.sidebar.face_places"),
        Action::SidebarFaceRecent => t!("commands.sidebar.face_recent"),
        Action::SidebarFaceStarred => t!("commands.sidebar.face_starred"),
        Action::SidebarFaceLinks => t!("commands.sidebar.face_links"),
        Action::SidebarFaceNext => t!("commands.sidebar.face_next"),
        Action::SidebarFacePrev => t!("commands.sidebar.face_prev"),
        Action::ThemeSetLight => t!("commands.theme.set_light"),
        Action::ThemeSetDark => t!("commands.theme.set_dark"),
        Action::ThemeSetAuto => t!("commands.theme.set_auto"),
        _ => return None,
    };
    Some(name.into_owned())
}

/// [`action_name`] with its group in front where the bare name does not say
/// what it acts on: what a binding's row in the preferences shows.
pub fn action_title(action: Action) -> String {
    let name = action_name(action);
    let title = match action.to_string().split_once('.') {
        Some(("clipboard", _)) => t!("keybindings.grouped.clipboard", action = name),
        Some(("help", _)) => t!("keybindings.grouped.help", action = name),
        _ => return name,
    };
    title.into_owned()
}

/// What a group of [`ACTION_GROUPS`] is called, from the English label the
/// keybindings crate gives it.
pub fn action_group_name(label: &str) -> String {
    let name = match label {
        "Scroll" => t!("keybindings.groups.scroll"),
        "History" => t!("keybindings.groups.history"),
        "Search" => t!("keybindings.groups.search"),
        "Highlights" => t!("keybindings.groups.highlights"),
        "Zoom" => t!("keybindings.groups.zoom"),
        "Clipboard" => t!("keybindings.groups.clipboard"),
        "Window" => t!("keybindings.groups.window"),
        "Focus" => t!("keybindings.groups.focus"),
        "File" => t!("keybindings.groups.file"),
        "App" => t!("keybindings.groups.app"),
        "Palette" => t!("keybindings.groups.palette"),
        "Contents" => t!("keybindings.groups.contents"),
        "Changes" => t!("keybindings.groups.changes"),
        "Lenses" => t!("keybindings.groups.lenses"),
        "Sidebar" => t!("keybindings.groups.sidebar"),
        "Theme" => t!("keybindings.groups.theme"),
        "Cursor" => t!("keybindings.groups.cursor"),
        "Content" => t!("keybindings.groups.content"),
        "Directory" => t!("keybindings.groups.directory"),
        "Cancel" => t!("keybindings.groups.cancel"),
        other => return other.to_string(),
    };
    name.into_owned()
}

/// What a context is listed under. [`KeyContext::label`] stays English,
/// since the CLI and the schema read it too.
pub fn context_name(context: KeyContext) -> String {
    let name = match context {
        KeyContext::Content => t!("keybindings.contexts.content"),
        KeyContext::Sidebar => t!("keybindings.contexts.sidebar"),
        KeyContext::Search => t!("keybindings.contexts.search"),
        KeyContext::Palette => t!("keybindings.contexts.palette"),
        KeyContext::Contents => t!("keybindings.contexts.contents"),
    };
    name.into_owned()
}

/// Build the matching engine for `bindings`, reporting the entries it had to
/// skip. Invalid keys or unknown actions come from a hand-edited
/// `mappings.json`; they are warned about here, once per engine build,
/// rather than silently dropped.
pub fn engine_for(bindings: &BindingSet) -> KeybindingEngine {
    let (engine, errors) = KeybindingEngine::build(bindings);
    for error in errors {
        tracing::warn!(%error, "Skipping keybinding");
    }
    engine
}

/// Return a formatted shortcut hint for the given action from the user's
/// current bindings.
///
/// Lookup order: context binding → global keybinding → menu shortcut.
pub fn shortcut_hint_for_action(action: &str, context: Option<KeyContext>) -> Option<String> {
    hint_for_action(&CONFIG.read().keybindings, action, context)
}

/// Return a formatted shortcut hint from global (and menu) keybindings.
///
/// This is the hint for an action reached by name rather than by key — from
/// the palette, or from the in-app menu — so it asks for no context.
pub fn shortcut_hint_for_global_action(action: &str) -> Option<String> {
    shortcut_hint_for_action(action, None)
}

/// Return a formatted shortcut hint in the given context.
pub fn shortcut_hint_for_context_action(context: KeyContext, action: &str) -> Option<String> {
    shortcut_hint_for_action(action, Some(context))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lens_shortcut_is_bound_over_the_document_to_the_lens_s_place() {
        let lenses = [
            Lens {
                shortcut: Some("Cmd+Shift+t".to_string()),
                ..Lens::new("translate")
            },
            Lens::new("summary"),
            Lens {
                shortcut: Some("g e".to_string()),
                ..Lens::new("explain")
            },
        ];

        let bindings = with_lens_shortcuts(BindingSet::default(), &lenses);

        let bound: Vec<(&str, &str)> = bindings
            .content
            .iter()
            .map(|binding| (binding.key.as_str(), binding.action.as_str()))
            .collect();
        assert_eq!(bound, [("Cmd+Shift+t", "lens.0"), ("g e", "lens.2")]);
        assert!(bindings.global.is_empty());

        let (engine_bindings, errors) = bindings.resolve();
        assert!(errors.is_empty(), "{errors:?}");
        assert!(engine_bindings
            .iter()
            .any(|binding| binding.action == Action::Lens(2)));
    }

    #[test]
    fn a_lens_shortcut_already_bound_is_left_to_what_has_it() {
        let bindings = BindingSet {
            global: vec![KeyAction {
                key: "Cmd+w".to_string(),
                action: "window.close".to_string(),
            }],
            ..Default::default()
        };
        let lenses = [
            Lens {
                shortcut: Some(" Cmd+w ".to_string()),
                ..Lens::new("steals")
            },
            Lens {
                shortcut: Some("g t".to_string()),
                ..Lens::new("first")
            },
            Lens {
                shortcut: Some("g  t".to_string()),
                label: "Second".to_string(),
                ..Lens::new("second")
            },
        ];

        assert_eq!(
            lens_shortcut_holder(&bindings, &lenses, 0).as_deref(),
            Some("window.close")
        );
        assert_eq!(lens_shortcut_holder(&bindings, &lenses, 1), None);
        assert!(lens_shortcut_holder(&bindings, &lenses, 2).is_some());

        // One binding the start of the other takes the other's keys too: the
        // shorter fires before the longer can be finished.
        let vim = BindingSet {
            content: vec![KeyAction {
                key: "g g".to_string(),
                action: "scroll.top".to_string(),
            }],
            ..Default::default()
        };
        let prefixed = |key: &str| {
            [Lens {
                shortcut: Some(key.to_string()),
                ..Lens::new("l")
            }]
        };
        assert!(lens_shortcut_holder(&vim, &prefixed("g"), 0).is_some());
        assert!(lens_shortcut_holder(&vim, &prefixed("g g t"), 0).is_some());
        assert_eq!(lens_shortcut_holder(&vim, &prefixed("g t"), 0), None);

        let bound = with_lens_shortcuts(bindings, &lenses);
        let lens_keys: Vec<&str> = bound
            .content
            .iter()
            .map(|binding| binding.action.as_str())
            .collect();
        assert_eq!(lens_keys, ["lens.1"]);
    }

    /// The English names are the ids spelled as words, which is what the
    /// preferences and the overlay showed before they could be translated.
    #[test]
    fn an_english_action_title_spells_out_its_id() {
        let spelled = |id: &str| {
            id.split(['.', '_'])
                .map(|word| {
                    let mut chars = word.chars();
                    chars.next().map_or(String::new(), |first| {
                        first.to_uppercase().collect::<String>() + chars.as_str()
                    })
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let actions = ACTION_GROUPS
            .iter()
            .flat_map(|(_, actions)| actions.iter().copied())
            .chain([Action::Lens(3)]);
        for action in actions {
            let id = action.to_string();
            assert_eq!(action_title(action), spelled(&id), "{id}");
        }
        assert_eq!(action_name(Action::CopyFilePath), "Copy File Path");
        assert_eq!(
            action_name(Action::HelpShowKeyboardShortcuts),
            "Show Keyboard Shortcuts"
        );
    }

    #[test]
    fn an_english_command_name_is_the_library_label() {
        let actions = ACTION_GROUPS
            .iter()
            .flat_map(|(_, actions)| actions.iter().copied());
        for action in actions {
            assert_eq!(
                command_name(action).as_deref(),
                action.command_label(),
                "{action}"
            );
        }
    }

    #[test]
    fn english_group_and_context_names_are_the_library_labels() {
        for (label, _) in ACTION_GROUPS {
            assert_eq!(action_group_name(label), *label);
        }
        for context in KeyContext::ALL {
            assert_eq!(context_name(context), context.label());
        }
    }
}
