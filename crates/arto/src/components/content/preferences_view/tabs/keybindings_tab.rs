use std::str::FromStr;

use dioxus::events::KeyboardEvent;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::config::{BindingSet, Config, KeyAction};
use crate::hooks::use_dismiss_on_outside;
use crate::keybindings::{
    accelerator_for_key, action_group_name, action_title, context_name, presets, resolve_bindings,
    Action, KeyContext, ResolvedBinding, ACTION_GROUPS, MENU_ACTIONS,
};
use crate::keybindings::{KeyChord, ShortcutSequence};
use rust_i18n::t;

/// Which shortcut table a section edits.
///
/// Menu shortcuts are single chords, the ones a menu item can carry. On macOS
/// they become native accelerators and the OS dispatches them; everywhere else
/// the engine does, since the menu is drawn in the header and has no
/// accelerator table behind it. Engine bindings are always handled in-window
/// and support chord sequences and per-context behavior.
#[derive(Clone, Copy, PartialEq)]
enum BindingScope {
    Menu,
    Engine(Option<KeyContext>),
}

#[component]
pub fn KeybindingsTab(config: Signal<Config>) -> Element {
    let keybindings = config.read().keybindings.clone();
    let mut filter_text = use_signal(String::new);

    rsx! {
        div {
            class: "preferences-pane",

            // Preset cards
            h3 { class: "preference-section-title", {t!("preferences.keybindings.presets.title").to_string()} }
            PresetCards { on_choose: move |bindings| config.write().keybindings = bindings }

            // Binding sections grouped by context
            h3 { class: "preference-section-title", {t!("preferences.keybindings.bindings.title").to_string()} }
            input {
                r#type: "text",
                class: "binding-filter-input",
                placeholder: t!("preferences.keybindings.bindings.filter_placeholder").to_string(),
                value: "{filter_text}",
                oninput: move |evt| filter_text.set(evt.value()),
                onkeydown: move |evt: KeyboardEvent| {
                    if evt.key() == Key::Escape {
                        spawn(async move {
                            let _ = document::eval("document.activeElement?.blur()").await;
                        });
                    }
                },
            }

            BindingSection {
                title: menu_shortcuts_title(),
                scope: BindingScope::Menu,
                bindings: keybindings.menu_shortcuts.clone(),
                filter_query: filter_text(),
                config,
            }
            BindingSection {
                title: context_label(None),
                scope: BindingScope::Engine(None),
                bindings: keybindings.global.clone(),
                filter_query: filter_text(),
                config,
            }
            for context in KeyContext::ALL {
                BindingSection {
                    key: "{context}",
                    title: context_name(context),
                    scope: BindingScope::Engine(Some(context)),
                    bindings: keybindings.of(context).clone(),
                    filter_query: filter_text(),
                    config,
                }
            }
        }
    }
}

/// A set of bindings the reader can start from, replacing every binding.
struct Preset {
    name: fn() -> String,
    description: fn() -> String,
    bindings: fn() -> BindingSet,
}

const PRESETS: &[Preset] = &[
    Preset {
        name: || t!("preferences.keybindings.presets.default.name").into_owned(),
        description: || t!("preferences.keybindings.presets.default.description").into_owned(),
        bindings: presets::default::bindings,
    },
    Preset {
        name: || "Vim".to_string(),
        description: || t!("preferences.keybindings.presets.vim.description").into_owned(),
        bindings: presets::vim::bindings,
    },
    Preset {
        name: || "Emacs".to_string(),
        description: || t!("preferences.keybindings.presets.emacs.description").into_owned(),
        bindings: presets::emacs::bindings,
    },
    Preset {
        name: || t!("preferences.keybindings.presets.clear.name").into_owned(),
        description: || t!("preferences.keybindings.presets.clear.description").into_owned(),
        bindings: BindingSet::default,
    },
];

#[component]
fn PresetCards(on_choose: EventHandler<BindingSet>) -> Element {
    rsx! {
        div {
            class: "preset-cards",
            for preset in PRESETS.iter() {
                button {
                    class: "preset-card",
                    onclick: move |_| on_choose.call((preset.bindings)()),
                    span { class: "preset-card-name", {(preset.name)()} }
                    span { class: "preset-card-desc", {(preset.description)()} }
                }
            }
        }
    }
}

/// Sort column for binding table.
#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Key,
    Action,
}

/// Per-context binding section with sort and filter support.
#[component]
fn BindingSection(
    title: String,
    scope: BindingScope,
    bindings: Vec<KeyAction>,
    filter_query: String,
    config: Signal<Config>,
) -> Element {
    let mut show_add_form = use_signal(|| false);
    // Track which binding index is being edited (None = no edit in progress)
    let mut editing_index = use_signal(|| None::<usize>);
    let mut sort_column = use_signal(|| None::<SortColumn>);
    let mut sort_ascending = use_signal(|| true);

    // Build display list: (real_index, key, action_label) with filter and sort applied
    let query_lower = filter_query.to_lowercase();
    let mut display_items: Vec<(usize, &KeyAction)> = bindings
        .iter()
        .enumerate()
        .filter(|(_, ka)| {
            if query_lower.is_empty() {
                return true;
            }
            ka.key.to_lowercase().contains(&query_lower)
                || action_label(&ka.action)
                    .to_lowercase()
                    .contains(&query_lower)
        })
        .collect();

    if let Some(col) = *sort_column.read() {
        let asc = *sort_ascending.read();
        display_items.sort_by(|(_, a), (_, b)| {
            let cmp = match col {
                SortColumn::Key => a.key.to_lowercase().cmp(&b.key.to_lowercase()),
                SortColumn::Action => action_label(&a.action)
                    .to_lowercase()
                    .cmp(&action_label(&b.action).to_lowercase()),
            };
            if asc {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }

    let mut toggle_sort = move |col: SortColumn| {
        if *sort_column.read() == Some(col) {
            if *sort_ascending.read() {
                sort_ascending.set(false);
            } else {
                // Third click: reset
                sort_column.set(None);
                sort_ascending.set(true);
            }
        } else {
            sort_column.set(Some(col));
            sort_ascending.set(true);
        }
    };

    let sort_icon = |col: SortColumn| -> Option<IconName> {
        if *sort_column.read() == Some(col) {
            if *sort_ascending.read() {
                Some(IconName::ChevronUp)
            } else {
                Some(IconName::ChevronDown)
            }
        } else {
            None
        }
    };

    let has_items = !display_items.is_empty();

    rsx! {
        div {
            class: "binding-section",
            h4 { class: "binding-section-title", "{title}" }

            if bindings.is_empty() && !*show_add_form.read() {
                p { class: "binding-empty", {t!("preferences.keybindings.bindings.empty").to_string()} }
            }

            if !bindings.is_empty() {
                if has_items {
                    table {
                        class: "binding-table",
                        thead {
                            tr {
                                th {
                                    class: "binding-header-key",
                                    onclick: move |_| toggle_sort(SortColumn::Key),
                                    {t!("preferences.keybindings.bindings.key").to_string()}
                                    if let Some(icon) = sort_icon(SortColumn::Key) {
                                        Icon { name: icon, size: 12 }
                                    }
                                }
                                th {
                                    class: "binding-header-action",
                                    onclick: move |_| toggle_sort(SortColumn::Action),
                                    {t!("preferences.keybindings.bindings.action").to_string()}
                                    if let Some(icon) = sort_icon(SortColumn::Action) {
                                        Icon { name: icon, size: 12 }
                                    }
                                }
                            }
                        }
                        tbody {
                            for (real_idx, ka) in display_items.iter() {
                                if *editing_index.read() == Some(*real_idx) {
                                    tr {
                                        key: "edit-{real_idx}",
                                        td {
                                            colspan: "2",
                                            BindingForm {
                                                key: "edit-form-{real_idx}-{ka.key}-{ka.action}",
                                                scope,
                                                edit_index: Some(*real_idx),
                                                initial_key: Some(ka.key.clone()),
                                                initial_action: Some(ka.action.clone()),
                                                config,
                                                on_close: move |_| editing_index.set(None),
                                            }
                                        }
                                    }
                                } else {
                                    {
                                        let idx = *real_idx;
                                        rsx! {
                                            tr {
                                                key: "row-{real_idx}",
                                                class: "binding-row",
                                                onclick: move |_| editing_index.set(Some(idx)),
                                                td {
                                                    class: "binding-key-cell",
                                                    span {
                                                        class: "key-badge",
                                                        "{ka.key}"
                                                    }
                                                }
                                                td {
                                                    class: "binding-action",
                                                    "{action_label(&ka.action)}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if !query_lower.is_empty() {
                    p { class: "binding-empty", {t!("preferences.keybindings.bindings.no_match").to_string()} }
                }
            }

            // Add button / inline add form (below bindings)
            if *show_add_form.read() {
                BindingForm {
                    scope,
                    edit_index: None::<usize>,
                    initial_key: None::<String>,
                    initial_action: None::<String>,
                    config,
                    on_close: move |_| show_add_form.set(false),
                }
            } else {
                button {
                    class: "binding-add-btn",
                    onclick: move |_| show_add_form.set(true),
                    {t!("preferences.keybindings.bindings.add").to_string()}
                }
            }
        }
    }
}

/// Unified binding form for both adding and editing.
///
/// - `edit_index: None` → Add mode (empty fields, push new binding)
/// - `edit_index: Some(idx)` → Edit mode (pre-filled fields, update in place, show delete)
#[component]
fn BindingForm(
    scope: BindingScope,
    edit_index: Option<usize>,
    initial_key: Option<String>,
    initial_action: Option<String>,
    config: Signal<Config>,
    on_close: EventHandler<()>,
) -> Element {
    let is_edit = edit_index.is_some();
    let orig_key = initial_key.clone().unwrap_or_default();
    let orig_action = initial_action.clone().unwrap_or_default();

    let key_input = use_signal({
        let k = initial_key.unwrap_or_default();
        move || k
    });
    let mut selected_action = use_signal({
        let a = initial_action.unwrap_or_default();
        move || a
    });
    let mut recording = use_signal(|| false);

    use_dismiss_on_outside(".binding-form", move || {
        if !*recording.peek() {
            on_close.call(());
        }
    });

    // Memoize resolved bindings: only recompute when config changes,
    // not on every keystroke in the key input field.
    let resolved = use_memo(move || {
        let keybindings = config.read().keybindings.clone();
        resolve_bindings(&keybindings)
    });

    let notice = {
        let original = is_edit.then(|| KeyAction {
            key: orig_key.clone(),
            action: orig_action.clone(),
        });
        binding_notice(
            scope,
            &key_input.read(),
            original.as_ref(),
            &config.read().keybindings,
            &resolved.read(),
        )
    };

    // Menu shortcuts must be a single chord representable as a native accelerator;
    // engine bindings accept any valid chord sequence.
    let key_valid = {
        let key = key_input.read();
        !key.is_empty()
            && match scope {
                BindingScope::Menu => accelerator_for_key(&key).is_some(),
                BindingScope::Engine(_) => ShortcutSequence::from_str(&key).is_ok(),
            }
    };
    let action_valid = !selected_action.read().is_empty();

    let can_submit = if is_edit {
        // Edit: must have valid key+action AND something must have changed
        let key = key_input.read().clone();
        let action = selected_action.read().clone();
        key_valid && action_valid && (key != orig_key || action != orig_action)
    } else {
        key_valid && action_valid
    };
    let selected_action_value = selected_action.read().clone();

    rsx! {
        div {
            class: "binding-form",
            div {
                class: "binding-form-row",
                KeyRecorder { value: key_input, recording }

                ActionSelect {
                    scope,
                    selected: selected_action_value,
                    on_change: move |action| selected_action.set(action),
                }
            }

            // Conflict / overwrite notice
            match notice {
                Some(BindingNotice::Conflict(ref msg)) => rsx! {
                    p { class: "binding-notice binding-notice--conflict", "{msg}" }
                },
                Some(BindingNotice::Overwrite(ref msg)) => rsx! {
                    p { class: "binding-notice binding-notice--overwrite", "{msg}" }
                },
                None => rsx! {},
            }

            // Action buttons
            div {
                class: "binding-form-buttons",
                button {
                    class: "binding-form-confirm",
                    disabled: !can_submit,
                    onclick: move |_| {
                        let key = key_input.read().clone();
                        let action = selected_action.read().clone();
                        if key.is_empty() || action.is_empty() {
                            return;
                        }
                        let mut cfg = config.write();
                        let bindings = bindings_mut(&mut cfg.keybindings, scope);
                        if let Some(idx) = edit_index {
                            if let Some(binding) = bindings.get_mut(idx) {
                                binding.key = key;
                                binding.action = action;
                            }
                        } else {
                            bindings.push(KeyAction { key, action });
                        }
                        drop(cfg);
                        on_close.call(());
                    },
                    if is_edit { {t!("preferences.keybindings.form.save").to_string()} } else { {t!("preferences.keybindings.form.add").to_string()} }
                }
                button {
                    class: "binding-form-cancel",
                    onclick: move |_| {
                        recording.set(false);
                        on_close.call(());
                    },
                    {t!("preferences.keybindings.form.cancel").to_string()}
                }
                // Delete button (edit mode only), pushed to the right
                if is_edit {
                    div { class: "binding-form-spacer" }
                    button {
                        class: "binding-form-delete",
                        onclick: {
                            let key_to_remove = orig_key.clone();
                            move |_| {
                                bindings_mut(
                                    &mut config.write().keybindings,
                                    scope,
                                ).retain(|b| b.key != key_to_remove);
                                on_close.call(());
                            }
                        },
                        {t!("preferences.keybindings.form.delete").to_string()}
                    }
                }
            }
        }
    }
}

/// The key a binding answers to, typed or recorded as it is pressed.
///
/// While `recording`, the keyboard interceptor is paused so the keys reach
/// the field instead of running their bindings, and recording ends by itself
/// two seconds after the last key.
#[component]
fn KeyRecorder(value: Signal<String>, recording: Signal<bool>) -> Element {
    let mut chords: Signal<Vec<KeyChord>> = use_signal(Vec::new);
    // Bumped on every recorded key, so a pending idle timeout can tell that
    // another key came after the one that started it.
    let mut epoch = use_signal(|| 0_u64);
    let mut before_recording = use_signal(String::new);
    let mut field = use_signal(|| None::<std::rc::Rc<MountedData>>);

    use_effect(move || {
        if *recording.read() {
            spawn(async move {
                let _ = document::eval("window.Arto?.keyboard?.pause?.()").await;
                let field = field.peek().clone();
                if let Some(field) = field {
                    let _ = field.set_focus(true).await;
                }
            });
        } else {
            spawn(async move {
                let _ = document::eval("window.Arto?.keyboard?.resume?.()").await;
            });
        }
    });

    use_effect(move || {
        let is_recording = *recording.read();
        let started = *epoch.read();
        if !is_recording || started == 0 {
            return;
        }
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if *recording.read() && *epoch.read() == started {
                recording.set(false);
            }
        });
    });

    // Resumed unconditionally rather than by reading `recording`: resuming
    // when not paused is a no-op, and a signal may already be gone here.
    use_drop(move || {
        let _ = document::eval("window.Arto?.keyboard?.resume?.()");
    });

    let is_recording = *recording.read();
    rsx! {
        div {
            class: "key-recorder-input",
            input {
                r#type: "text",
                class: if is_recording { "key-recorder-text recording" } else { "key-recorder-text" },
                placeholder: if is_recording { t!("preferences.keybindings.form.recording_placeholder").to_string() } else { t!("preferences.keybindings.form.key_placeholder").to_string() },
                value: "{value}",
                readonly: is_recording,
                onmounted: move |evt: MountedEvent| async move {
                    let mounted = evt.data();
                    field.set(Some(mounted.clone()));
                    let _ = mounted.set_focus(true).await;
                },
                oninput: move |evt| {
                    if !*recording.read() {
                        value.set(evt.value());
                    }
                },
                onkeydown: move |evt: KeyboardEvent| {
                    if !*recording.read() {
                        return;
                    }
                    evt.prevent_default();
                    evt.stop_propagation();
                    let chord = crate::keybindings::chord_from_event(&evt);
                    if let Some(sequence) = record_chord(&mut chords.write(), chord) {
                        value.set(sequence);
                        let next = *epoch.read() + 1;
                        epoch.set(next);
                    }
                },
            }
            if is_recording {
                button {
                    class: "key-record-btn recording",
                    onclick: move |_| recording.set(false),
                    {t!("preferences.keybindings.form.done").to_string()}
                }
                button {
                    class: "key-record-btn",
                    onclick: move |_| {
                        value.set(before_recording.read().clone());
                        chords.write().clear();
                        epoch.set(0);
                        recording.set(false);
                    },
                    {t!("preferences.keybindings.form.cancel").to_string()}
                }
            } else {
                button {
                    class: "key-record-btn",
                    onclick: move |_| {
                        before_recording.set(value.read().clone());
                        chords.write().clear();
                        value.set(String::new());
                        epoch.set(0);
                        recording.set(true);
                    },
                    {t!("preferences.keybindings.form.record").to_string()}
                }
            }
        }
    }
}

/// Add a pressed chord to the sequence being recorded and return the
/// sequence as it now reads, or `None` when the chord is a modifier on its
/// own. Backspace takes the last chord back instead of being recorded.
fn record_chord(chords: &mut Vec<KeyChord>, chord: KeyChord) -> Option<String> {
    if chord.is_modifier_only() {
        return None;
    }
    if chord.to_string() == "Backspace" {
        chords.pop();
    } else {
        chords.push(chord);
    }
    Some(
        chords
            .iter()
            .map(|chord| chord.to_string())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The action a binding runs.
///
/// Menu shortcuts may only target menu-backed actions; engine bindings can
/// target any action, grouped by category.
#[component]
fn ActionSelect(scope: BindingScope, selected: String, on_change: EventHandler<String>) -> Element {
    rsx! {
        select {
            class: "action-select",
            value: "{selected}",
            onchange: move |evt: FormEvent| on_change.call(evt.value()),
            option {
                value: "",
                disabled: true,
                selected: selected.is_empty(),
                {t!("preferences.keybindings.form.select_action").to_string()}
            }
            if scope == BindingScope::Menu {
                for action in MENU_ACTIONS {
                    ActionOption { key: "{action}", action: *action, selected: selected.clone() }
                }
            } else {
                for (group_label, actions) in ACTION_GROUPS {
                    optgroup {
                        label: action_group_name(group_label),
                        for action in *actions {
                            ActionOption { key: "{action}", action: *action, selected: selected.clone() }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ActionOption(action: Action, selected: String) -> Element {
    let value = action.to_string();
    rsx! {
        option {
            value: "{value}",
            selected: selected == value,
            "{action_label(&value)}"
        }
    }
}

/// Get mutable reference to the bindings for a given scope.
fn bindings_mut(set: &mut crate::config::BindingSet, scope: BindingScope) -> &mut Vec<KeyAction> {
    match scope {
        BindingScope::Menu => &mut set.menu_shortcuts,
        BindingScope::Engine(None) => &mut set.global,
        BindingScope::Engine(Some(context)) => set.of_mut(context),
    }
}

/// What the form says about `key` before it is saved into `scope`.
///
/// `original` is the binding being edited, which `key` may keep without
/// conflicting with itself. A clash inside the scope is reported before an
/// overlap with the other scope.
fn binding_notice(
    scope: BindingScope,
    key: &str,
    original: Option<&KeyAction>,
    bindings: &BindingSet,
    resolved: &[ResolvedBinding],
) -> Option<BindingNotice> {
    if key.is_empty() {
        return None;
    }
    match scope {
        BindingScope::Engine(context) => {
            let same_scope = match original {
                Some(original) => check_conflict_excluding(
                    resolved,
                    key,
                    context,
                    &original.key,
                    &original.action,
                ),
                None => check_conflict(resolved, key, context),
            };
            // A chord owned by a native menu accelerator is consumed before
            // the engine, so an engine binding can collide with the menu too.
            same_scope.or_else(|| {
                cross_scope_conflict(&bindings.menu_shortcuts, key, &menu_shortcuts_title())
            })
        }
        BindingScope::Menu => check_menu_conflict(
            &bindings.menu_shortcuts,
            key,
            original.is_some(),
            original.map_or("", |original| &original.key),
        )
        .or_else(|| cross_scope_conflict(&bindings.global, key, &context_label(None))),
    }
}

/// Check a menu-shortcut key for validity and duplicates.
///
/// Menu shortcuts must be a single chord representable as a native accelerator,
/// and each chord may be bound only once (one chord = one owner).
fn check_menu_conflict(
    menu_shortcuts: &[KeyAction],
    new_key: &str,
    is_edit: bool,
    exclude_key: &str,
) -> Option<BindingNotice> {
    if accelerator_for_key(new_key).is_none() {
        return Some(BindingNotice::Conflict(
            t!("preferences.keybindings.notice.single_chord").into_owned(),
        ));
    }

    let new_seq = ShortcutSequence::from_str(new_key).ok()?.to_string();
    for ka in menu_shortcuts {
        if is_edit && ka.key == exclude_key {
            continue;
        }
        let existing = ShortcutSequence::from_str(&ka.key)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if existing == new_seq {
            return Some(BindingNotice::Conflict(
                t!(
                    "preferences.keybindings.notice.conflicts_with",
                    binding = binding_desc(&ka.action, &menu_shortcuts_title())
                )
                .into_owned(),
            ));
        }
    }
    None
}

/// Detect a same-chord collision against a flat binding list from the OTHER
/// shortcut scope.
///
/// A chord owned by both a native menu accelerator and an engine binding leaves
/// one permanently unreachable (the OS menu / interceptor consumes the chord
/// before the engine sees it), so surface it as a conflict in both editors.
fn cross_scope_conflict(
    other: &[KeyAction],
    new_key: &str,
    other_label: &str,
) -> Option<BindingNotice> {
    let new_seq = ShortcutSequence::from_str(new_key).ok()?.to_string();
    for ka in other {
        let existing = ShortcutSequence::from_str(&ka.key)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if existing == new_seq {
            return Some(BindingNotice::Conflict(
                t!(
                    "preferences.keybindings.notice.also_bound",
                    binding = binding_desc(&ka.action, other_label)
                )
                .into_owned(),
            ));
        }
    }
    None
}

/// What a binding's action is called: "scroll.down" is "Scroll Down".
///
/// An action this version does not know — a hand-edited mapping — has no
/// name of its own, and is shown as its id spelled out.
fn action_label(action_str: &str) -> String {
    if let Ok(action) = Action::from_str(action_str) {
        return action_title(action);
    }
    action_str
        .split('.')
        .flat_map(|part| part.split('_'))
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Human-readable label for a context.
fn context_label(context: Option<KeyContext>) -> String {
    match context {
        None => t!("preferences.keybindings.contexts.global").into_owned(),
        Some(context) => context_name(context),
    }
}

/// What the section of menu shortcuts is called.
fn menu_shortcuts_title() -> String {
    t!("preferences.keybindings.contexts.menu_shortcuts").into_owned()
}

/// An action and where it is bound, as a notice names an existing binding.
fn binding_desc(action: &str, context: &str) -> String {
    t!(
        "preferences.keybindings.notice.binding",
        action = action_label(action),
        context = context
    )
    .into_owned()
}

/// Result of checking a binding for conflicts or overrides.
#[derive(Debug)]
enum BindingNotice {
    /// Same context collision — true conflict (warning).
    Conflict(String),
    /// Cross-context override (context shadows global or vice versa) — informational.
    Overwrite(String),
}

/// Check whether a new binding conflicts with or overrides an existing binding.
fn check_conflict(
    resolved: &[ResolvedBinding],
    new_key: &str,
    context: Option<KeyContext>,
) -> Option<BindingNotice> {
    check_conflict_inner(resolved, new_key, context, None, None)
}

/// Check conflict/override while excluding a specific binding (used during editing).
fn check_conflict_excluding(
    resolved: &[ResolvedBinding],
    new_key: &str,
    context: Option<KeyContext>,
    exclude_key: &str,
    exclude_action: &str,
) -> Option<BindingNotice> {
    check_conflict_inner(
        resolved,
        new_key,
        context,
        Some(exclude_key),
        Some(exclude_action),
    )
}

fn check_conflict_inner(
    resolved: &[ResolvedBinding],
    new_key: &str,
    context: Option<KeyContext>,
    exclude_key: Option<&str>,
    exclude_action: Option<&str>,
) -> Option<BindingNotice> {
    let new_seq = match ShortcutSequence::from_str(new_key) {
        Ok(seq) => seq,
        Err(_) => return None,
    };

    let exclude_seq = exclude_key.and_then(|ek| ShortcutSequence::from_str(ek).ok());

    let mut overrides_global: Option<String> = None;
    let mut overridden_by: Vec<String> = Vec::new();

    for binding in resolved {
        // Skip the binding being edited
        if let (Some(ref ex_seq), Some(ea)) = (&exclude_seq, exclude_action) {
            if binding.sequence == *ex_seq
                && binding.action.to_string() == ea
                && binding.context == context
            {
                continue;
            }
        }

        if binding.sequence != new_seq {
            continue;
        }

        let action_desc =
            binding_desc(&binding.action.to_string(), &context_label(binding.context));

        match (binding.context, context) {
            // Same context (including both Global) → true conflict
            (None, None) | (Some(_), Some(_)) if binding.context == context => {
                return Some(BindingNotice::Conflict(
                    t!(
                        "preferences.keybindings.notice.conflicts_with",
                        binding = action_desc
                    )
                    .into_owned(),
                ))
            }
            // New context binding overrides existing global
            (None, Some(_)) => {
                overrides_global = overrides_global.or(Some(action_desc));
            }
            // New global binding will be overridden by existing context binding
            (Some(_), None) => {
                overridden_by.push(context_label(binding.context));
            }
            // Different specific contexts → no overlap
            _ => {}
        }
    }

    if let Some(desc) = overrides_global {
        return Some(BindingNotice::Overwrite(
            t!("preferences.keybindings.notice.overrides", binding = desc).into_owned(),
        ));
    }

    if !overridden_by.is_empty() {
        overridden_by.sort();
        overridden_by.dedup();
        let separator = t!("preferences.keybindings.notice.separator");
        return Some(BindingNotice::Overwrite(
            t!(
                "preferences.keybindings.notice.overridden_in",
                contexts = overridden_by.join(&*separator)
            )
            .into_owned(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    // Disambiguate from dioxus::prelude::Action
    use crate::keybindings::Action as KbAction;

    #[test]
    fn action_label_converts_dot_and_underscore() {
        assert_eq!(action_label("scroll.down"), "Scroll Down");
        assert_eq!(action_label("window.new_document"), "Window New Document");
        assert_eq!(action_label("cancel"), "Cancel");
        assert_eq!(
            action_label("clipboard.copy_file_path"),
            "Clipboard Copy File Path"
        );
    }

    #[test]
    fn context_label_display() {
        assert_eq!(context_label(None), "Global");
        assert_eq!(context_label(Some(KeyContext::Content)), "Content");
    }

    #[test]
    fn same_key_global_is_conflict() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::ScrollDown,
            context: None,
        }];
        let result = check_conflict(&resolved, "j", None);
        assert!(matches!(result, Some(BindingNotice::Conflict(_))));
    }

    #[test]
    fn context_overriding_global_is_overwrite() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::ScrollDown,
            context: None,
        }];
        // Adding "j" to Content context overrides global "j" — informational, not conflict
        let result = check_conflict(&resolved, "j", Some(KeyContext::Content));
        assert!(matches!(result, Some(BindingNotice::Overwrite(_))));
    }

    #[test]
    fn same_context_is_conflict() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::CursorDown,
            context: Some(KeyContext::Sidebar),
        }];
        let result = check_conflict(&resolved, "j", Some(KeyContext::Sidebar));
        assert!(matches!(result, Some(BindingNotice::Conflict(_))));
    }

    #[test]
    fn different_contexts_no_overlap() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::CursorDown,
            context: Some(KeyContext::Sidebar),
        }];
        // Adding "j" to Content doesn't conflict with Sidebar "j"
        let result = check_conflict(&resolved, "j", Some(KeyContext::Content));
        assert!(result.is_none());
    }

    #[test]
    fn invalid_key_returns_none() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::ScrollDown,
            context: None,
        }];
        let result = check_conflict(&resolved, "", None);
        assert!(result.is_none());
    }

    #[test]
    fn no_match_returns_none() {
        let resolved = vec![ResolvedBinding {
            sequence: ShortcutSequence::from_str("j").unwrap(),
            action: KbAction::ScrollDown,
            context: None,
        }];
        let result = check_conflict(&resolved, "k", None);
        assert!(result.is_none());
    }

    #[test]
    fn menu_conflict_rejects_multi_chord() {
        let result = check_menu_conflict(&[], "g g", false, "");
        assert!(matches!(result, Some(BindingNotice::Conflict(_))));
    }

    #[test]
    fn menu_conflict_detects_duplicate_chord() {
        let existing = vec![KeyAction {
            key: "Cmd+o".to_string(),
            action: "file.open".to_string(),
        }];
        let result = check_menu_conflict(&existing, "Cmd+o", false, "");
        assert!(matches!(result, Some(BindingNotice::Conflict(_))));
    }

    #[test]
    fn menu_conflict_allows_unique_single_chord() {
        let existing = vec![KeyAction {
            key: "Cmd+o".to_string(),
            action: "file.open".to_string(),
        }];
        assert!(check_menu_conflict(&existing, "Cmd+k", false, "").is_none());
    }

    #[test]
    fn cross_scope_conflict_detects_overlap() {
        let global = vec![KeyAction {
            key: "Cmd+r".to_string(),
            action: "window.reload".to_string(),
        }];
        // Adding Cmd+r as a menu shortcut collides with the global engine binding.
        let result = cross_scope_conflict(&global, "Cmd+r", "Global");
        assert!(matches!(result, Some(BindingNotice::Conflict(_))));
    }

    #[test]
    fn cross_scope_conflict_ignores_distinct_chords() {
        let menu = vec![KeyAction {
            key: "Cmd+o".to_string(),
            action: "file.open".to_string(),
        }];
        assert!(cross_scope_conflict(&menu, "Cmd+k", "Menu Shortcuts").is_none());
    }

    #[test]
    fn menu_conflict_excludes_edited_entry() {
        let existing = vec![KeyAction {
            key: "Cmd+o".to_string(),
            action: "file.open".to_string(),
        }];
        // Editing the same entry (orig_key = "Cmd+o") should not self-conflict.
        assert!(check_menu_conflict(&existing, "Cmd+o", true, "Cmd+o").is_none());
    }

    #[test]
    fn the_preset_cards_are_drawn_in_order_with_their_descriptions() {
        let mut dom = VirtualDom::new(|| rsx! { PresetCards { on_choose: |_| {} } });
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);

        assert_eq!(
            html,
            concat!(
                r#"<div class="preset-cards">"#,
                r#"<button class="preset-card">"#,
                r#"<span class="preset-card-name">Default</span>"#,
                r#"<span class="preset-card-desc">Arrow keys, Cmd+Key, Ctrl+Tab</span>"#,
                r#"</button>"#,
                r#"<button class="preset-card">"#,
                r#"<span class="preset-card-name">Vim</span>"#,
                r#"<span class="preset-card-desc">j/k scroll, g g, chord sequences</span>"#,
                r#"</button>"#,
                r#"<button class="preset-card">"#,
                r#"<span class="preset-card-name">Emacs</span>"#,
                r#"<span class="preset-card-desc">Ctrl+n/p, Ctrl+x combos</span>"#,
                r#"</button>"#,
                r#"<button class="preset-card">"#,
                r#"<span class="preset-card-name">Clear</span>"#,
                r#"<span class="preset-card-desc">Remove all keybindings</span>"#,
                r#"</button>"#,
                r#"</div>"#,
            )
        );
    }

    #[test]
    fn each_preset_applies_its_own_bindings() {
        let applied: Vec<BindingSet> = PRESETS.iter().map(|preset| (preset.bindings)()).collect();
        assert_eq!(
            applied,
            vec![
                presets::default::bindings(),
                presets::vim::bindings(),
                presets::emacs::bindings(),
                BindingSet::default(),
            ]
        );
    }

    #[test]
    fn recording_a_chord_appends_it_to_the_sequence() {
        let mut chords = Vec::new();
        assert_eq!(
            record_chord(&mut chords, KeyChord::from_js_event("g", 0)).as_deref(),
            Some("g")
        );
        assert_eq!(
            record_chord(&mut chords, KeyChord::from_js_event("G", 0)).as_deref(),
            Some("g Shift+g")
        );
    }

    #[test]
    fn recording_backspace_takes_the_last_chord_back() {
        let mut chords = vec![
            KeyChord::from_js_event("g", 0),
            KeyChord::from_js_event("g", 0),
        ];
        assert_eq!(
            record_chord(&mut chords, KeyChord::from_js_event("Backspace", 0)).as_deref(),
            Some("g")
        );
        assert_eq!(
            record_chord(&mut chords, KeyChord::from_js_event("Backspace", 0)).as_deref(),
            Some("")
        );
    }

    #[test]
    fn recording_ignores_a_modifier_pressed_on_its_own() {
        let mut chords = vec![KeyChord::from_js_event("g", 0)];
        assert_eq!(
            record_chord(&mut chords, KeyChord::from_js_event("Shift", 8)),
            None
        );
        assert_eq!(chords.len(), 1);
    }

    #[test]
    fn the_key_recorder_offers_to_record_while_idle() {
        let html = render(|| {
            let value = use_signal(|| "Cmd+o".to_string());
            let recording = use_signal(|| false);
            rsx! { KeyRecorder { value, recording } }
        });

        assert_eq!(
            html,
            concat!(
                r#"<div class="key-recorder-input">"#,
                r#"<input type="text" class="key-recorder-text" placeholder="e.g. Cmd+k, g g" value="Cmd+o"/>"#,
                r#"<button class="key-record-btn">Record</button>"#,
                r#"</div>"#,
            )
        );
    }

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn a_menu_shortcut_chooses_among_menu_actions_only() {
        let html = render(|| {
            rsx! {
                ActionSelect {
                    scope: BindingScope::Menu,
                    selected: KbAction::FileOpen.to_string(),
                    on_change: |_| {},
                }
            }
        });

        assert!(!html.contains("<optgroup"), "{html}");
        assert_eq!(html.matches("<option ").count(), MENU_ACTIONS.len() + 1);
        assert!(
            html.contains(&format!(
                r#"<option value="file.open" selected=true>{}</option>"#,
                action_label("file.open")
            )),
            "{html}"
        );
    }

    #[test]
    fn an_engine_binding_chooses_among_every_action_by_group() {
        let html = render(|| {
            rsx! {
                ActionSelect {
                    scope: BindingScope::Engine(None),
                    selected: String::new(),
                    on_change: |_| {},
                }
            }
        });

        let every_action: usize = ACTION_GROUPS.iter().map(|(_, actions)| actions.len()).sum();
        assert_eq!(html.matches("<optgroup").count(), ACTION_GROUPS.len());
        assert_eq!(html.matches("<option ").count(), every_action + 1);
        assert!(
            html.contains(r#"<option value="" disabled=true selected=true>"#),
            "the placeholder is chosen until an action is: {html}"
        );
    }

    fn binding(key: &str, action: &str) -> KeyAction {
        KeyAction {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    fn notice_for(
        scope: BindingScope,
        key: &str,
        original: Option<&KeyAction>,
        bindings: &BindingSet,
    ) -> Option<BindingNotice> {
        binding_notice(scope, key, original, bindings, &resolve_bindings(bindings))
    }

    #[test]
    fn an_empty_key_has_no_notice() {
        let bindings = BindingSet {
            global: vec![binding("j", "scroll.down")],
            ..BindingSet::default()
        };
        assert!(notice_for(BindingScope::Engine(None), "", None, &bindings).is_none());
        assert!(notice_for(BindingScope::Menu, "", None, &bindings).is_none());
    }

    #[test]
    fn an_engine_binding_being_edited_does_not_conflict_with_itself() {
        let bindings = BindingSet {
            global: vec![binding("j", "scroll.down")],
            ..BindingSet::default()
        };
        let original = binding("j", "scroll.down");
        assert!(matches!(
            notice_for(BindingScope::Engine(None), "j", None, &bindings),
            Some(BindingNotice::Conflict(_))
        ));
        assert!(notice_for(BindingScope::Engine(None), "j", Some(&original), &bindings).is_none());
    }

    #[test]
    fn an_engine_binding_on_a_menu_chord_conflicts() {
        let bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+o", "file.open")],
            ..BindingSet::default()
        };
        let notice = notice_for(
            BindingScope::Engine(Some(KeyContext::Content)),
            "Cmd+o",
            None,
            &bindings,
        );
        // On macOS the OS dispatches menu shortcuts, so the engine binding
        // would never see the chord. Elsewhere the engine owns them as global
        // bindings, which a context binding overrides.
        match notice {
            #[cfg(target_os = "macos")]
            Some(BindingNotice::Conflict(msg)) => {
                assert!(msg.contains("Menu Shortcuts"), "names the menu: {msg}")
            }
            #[cfg(not(target_os = "macos"))]
            Some(BindingNotice::Overwrite(msg)) => {
                assert!(msg.contains("File Open"), "names the menu action: {msg}")
            }
            other => panic!("Expected this platform's notice, got {other:?}"),
        }
    }

    #[test]
    fn a_same_scope_conflict_wins_over_a_menu_overlap() {
        let bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+o", "file.open")],
            global: vec![binding("Cmd+o", "scroll.down")],
            ..BindingSet::default()
        };
        match notice_for(BindingScope::Engine(None), "Cmd+o", None, &bindings) {
            Some(BindingNotice::Conflict(msg)) => {
                assert!(msg.contains("Global"), "names the global binding: {msg}")
            }
            other => panic!("Expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_shortcut_on_a_global_chord_conflicts() {
        let bindings = BindingSet {
            global: vec![binding("Cmd+r", "window.reload")],
            ..BindingSet::default()
        };
        match notice_for(BindingScope::Menu, "Cmd+r", None, &bindings) {
            Some(BindingNotice::Conflict(msg)) => {
                assert!(msg.contains("Global"), "names the global binding: {msg}")
            }
            other => panic!("Expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_shortcut_being_edited_does_not_conflict_with_itself() {
        let bindings = BindingSet {
            menu_shortcuts: vec![binding("Cmd+o", "file.open")],
            ..BindingSet::default()
        };
        let original = binding("Cmd+o", "file.open");
        assert!(notice_for(BindingScope::Menu, "Cmd+o", Some(&original), &bindings).is_none());
    }

    #[test]
    fn global_overridden_by_multiple_contexts_lists_all() {
        let resolved = vec![
            ResolvedBinding {
                sequence: ShortcutSequence::from_str("j").unwrap(),
                action: KbAction::CursorDown,
                context: Some(KeyContext::Sidebar),
            },
            ResolvedBinding {
                sequence: ShortcutSequence::from_str("j").unwrap(),
                action: KbAction::CursorDown,
                context: Some(KeyContext::Content),
            },
        ];
        // Adding "j" to Global — overridden by both the panel and the content
        let result = check_conflict(&resolved, "j", None);
        match result {
            Some(BindingNotice::Overwrite(msg)) => {
                assert!(msg.contains("Content"), "should list Content: {msg}");
                assert!(msg.contains("Panel"), "should list Panel: {msg}");
            }
            other => panic!("Expected Overwrite, got {other:?}"),
        }
    }
}
