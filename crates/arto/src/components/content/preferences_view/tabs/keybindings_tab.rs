use std::str::FromStr;

use dioxus::events::KeyboardEvent;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::config::{BindingSet, Config, KeyAction};
use crate::keybindings::{
    accelerator_for_key, presets, resolve_bindings, KeyContext, ResolvedBinding, ACTION_GROUPS,
    MENU_ACTIONS,
};
use crate::keybindings::{KeyChord, ShortcutSequence};

/// Which shortcut table a section edits.
///
/// Menu shortcuts become native OS menu accelerators (single chord, shown in the
/// menu bar); engine bindings are handled in-window and support chord sequences
/// and per-context behavior.
#[derive(Clone, Copy, PartialEq)]
enum BindingScope {
    Menu,
    Engine(Option<KeyContext>),
}

#[component]
pub fn KeybindingsTab(config: Signal<Config>, has_changes: Signal<bool>) -> Element {
    let keybindings = config.read().keybindings.clone();
    let mut filter_text = use_signal(String::new);

    rsx! {
        div {
            class: "preferences-pane preferences-pane--keybindings",

            // Preset cards
            h3 { class: "preference-section-title", "Presets" }
            div {
                class: "preset-cards",
                button {
                    class: "preset-card",
                    onclick: move |_| {
                        config.write().keybindings = presets::default::bindings();
                        has_changes.set(true);
                    },
                    span { class: "preset-card-name", "Default" }
                    span { class: "preset-card-desc", "Arrow keys, Cmd+Key, Ctrl+Tab" }
                }
                button {
                    class: "preset-card",
                    onclick: move |_| {
                        config.write().keybindings = presets::vim::bindings();
                        has_changes.set(true);
                    },
                    span { class: "preset-card-name", "Vim" }
                    span { class: "preset-card-desc", "j/k scroll, g g, chord sequences" }
                }
                button {
                    class: "preset-card",
                    onclick: move |_| {
                        config.write().keybindings = presets::emacs::bindings();
                        has_changes.set(true);
                    },
                    span { class: "preset-card-name", "Emacs" }
                    span { class: "preset-card-desc", "Ctrl+n/p, Ctrl+x combos" }
                }
                button {
                    class: "preset-card",
                    onclick: move |_| {
                        config.write().keybindings = BindingSet::default();
                        has_changes.set(true);
                    },
                    span { class: "preset-card-name", "Clear" }
                    span { class: "preset-card-desc", "Remove all keybindings" }
                }
            }

            // Binding sections grouped by context
            h3 { class: "preference-section-title", "Bindings" }
            input {
                r#type: "text",
                class: "binding-filter-input",
                placeholder: "Filter by key or action...",
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
                title: "Menu Shortcuts",
                scope: BindingScope::Menu,
                bindings: keybindings.menu_shortcuts.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Global",
                scope: BindingScope::Engine(None),
                bindings: keybindings.global.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Content",
                scope: BindingScope::Engine(Some(KeyContext::Content)),
                bindings: keybindings.content.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Sidebar",
                scope: BindingScope::Engine(Some(KeyContext::Sidebar)),
                bindings: keybindings.sidebar.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Quick Access",
                scope: BindingScope::Engine(Some(KeyContext::QuickAccess)),
                bindings: keybindings.quick_access.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Right Sidebar",
                scope: BindingScope::Engine(Some(KeyContext::RightSidebar)),
                bindings: keybindings.right_sidebar.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
            }
            BindingSection {
                title: "Search",
                scope: BindingScope::Engine(Some(KeyContext::Search)),
                bindings: keybindings.search.clone(),
                filter_query: filter_text(),
                config,
                has_changes,
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
    title: &'static str,
    scope: BindingScope,
    bindings: Vec<KeyAction>,
    filter_query: String,
    config: Signal<Config>,
    has_changes: Signal<bool>,
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
                p { class: "binding-empty", "No bindings defined yet." }
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
                                    "Key"
                                    if let Some(icon) = sort_icon(SortColumn::Key) {
                                        Icon { name: icon, size: 12 }
                                    }
                                }
                                th {
                                    class: "binding-header-action",
                                    onclick: move |_| toggle_sort(SortColumn::Action),
                                    "Action"
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
                                                has_changes,
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
                    p { class: "binding-empty", "No matching bindings." }
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
                    has_changes,
                    on_close: move |_| show_add_form.set(false),
                }
            } else {
                button {
                    class: "binding-add-btn",
                    onclick: move |_| show_add_form.set(true),
                    "+ Add binding"
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
    has_changes: Signal<bool>,
    on_close: EventHandler<()>,
) -> Element {
    let is_edit = edit_index.is_some();
    let orig_key = initial_key.clone().unwrap_or_default();
    let orig_action = initial_action.clone().unwrap_or_default();

    let mut key_input = use_signal({
        let k = initial_key.unwrap_or_default();
        move || k
    });
    let mut selected_action = use_signal({
        let a = initial_action.unwrap_or_default();
        move || a
    });
    let mut recording = use_signal(|| false);
    let mut recorded_chords: Signal<Vec<KeyChord>> = use_signal(Vec::new);
    // Increments on each recorded key input; used to detect idle timeout.
    let mut recording_input_epoch = use_signal(|| 0_u64);
    // Snapshot key_input before recording starts, so Cancel can revert.
    let mut pre_record_value = use_signal(String::new);

    // Focus key input on mount so outside click can be detected via focusout.
    use_hook(|| {
        spawn(async move {
            let _ = document::eval(
                "document.querySelector('.binding-form .key-recorder-text')?.focus()",
            )
            .await;
        });
    });

    // Close form when clicking outside `.binding-form`.
    // This uses pointer events (not focus/timer based) so button clicks inside
    // the form don't accidentally close it on platforms where buttons don't take focus.
    use_hook(move || {
        spawn(async move {
            let mut eval = document::eval(
                r#"
                (() => {
                    const prev = window.__artoBindingFormOutsideClickHandler;
                    if (prev) {
                        document.removeEventListener('pointerdown', prev, true);
                    }
                    const handler = (e) => {
                        const inside = e.target instanceof Element
                            && e.target.closest('.binding-form');
                        if (!inside) {
                            dioxus.send(true);
                        }
                    };
                    window.__artoBindingFormOutsideClickHandler = handler;
                    document.addEventListener('pointerdown', handler, true);
                })();
                "#,
            );
            while let Ok(outside) = eval.recv::<bool>().await {
                if outside && !*recording.read() {
                    on_close.call(());
                    break;
                }
            }
        });
    });

    // Close form on Escape regardless of focused element.
    use_hook(move || {
        spawn(async move {
            let mut eval = document::eval(
                r#"
                (() => {
                    const prev = window.__artoBindingFormEscapeHandler;
                    if (prev) {
                        document.removeEventListener('keydown', prev, true);
                    }
                    const handler = (e) => {
                        if (e.key === 'Escape') {
                            dioxus.send(true);
                        }
                    };
                    window.__artoBindingFormEscapeHandler = handler;
                    document.addEventListener('keydown', handler, true);
                })();
                "#,
            );
            while let Ok(pressed) = eval.recv::<bool>().await {
                if pressed && !*recording.read() {
                    on_close.call(());
                    break;
                }
            }
        });
    });

    // Pause/resume keyboard interceptor when recording state changes.
    use_effect(move || {
        if *recording.read() {
            spawn(async move {
                let _ = document::eval("window.Arto?.keyboard?.pause?.()").await;
                let _ =
                    document::eval("document.querySelector('.key-recorder-text')?.focus()").await;
            });
        } else {
            spawn(async move {
                let _ = document::eval("window.Arto?.keyboard?.resume?.()").await;
            });
        }
    });

    // Auto-complete recording when idle for 2 seconds after the last key input.
    use_effect(move || {
        let is_recording = *recording.read();
        let epoch = *recording_input_epoch.read();
        if !is_recording || epoch == 0 {
            return;
        }
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if *recording.read() && *recording_input_epoch.read() == epoch {
                recording.set(false);
            }
        });
    });

    // Resume interceptor unconditionally on unmount.
    // Calling resume when not paused is a safe no-op, and this avoids
    // depending on Signal state during teardown.
    use_drop(move || {
        let _ = document::eval("window.Arto?.keyboard?.resume?.()");
        let _ = document::eval(
            r#"
            (() => {
                const handler = window.__artoBindingFormOutsideClickHandler;
                if (handler) {
                    document.removeEventListener('pointerdown', handler, true);
                    delete window.__artoBindingFormOutsideClickHandler;
                }
            })();
            "#,
        );
        let _ = document::eval(
            r#"
            (() => {
                const handler = window.__artoBindingFormEscapeHandler;
                if (handler) {
                    document.removeEventListener('keydown', handler, true);
                    delete window.__artoBindingFormEscapeHandler;
                }
            })();
            "#,
        );
    });

    // Memoize resolved bindings: only recompute when config changes,
    // not on every keystroke in the key input field.
    let resolved = use_memo(move || {
        let keybindings = config.read().keybindings.clone();
        resolve_bindings(&keybindings)
    });

    // Conflict / overwrite detection
    let notice = {
        let key_str = key_input.read().clone();
        if key_str.is_empty() {
            None
        } else {
            match scope {
                BindingScope::Engine(context) => {
                    let same_scope = {
                        let resolved = resolved.read();
                        if is_edit {
                            check_conflict_excluding(
                                &resolved,
                                &key_str,
                                context,
                                &orig_key,
                                &orig_action,
                            )
                        } else {
                            check_conflict(&resolved, &key_str, context)
                        }
                    };
                    // A chord owned by a native menu accelerator is consumed
                    // before the engine, so also warn on menu-scope overlap.
                    same_scope.or_else(|| {
                        let menu_shortcuts = config.read().keybindings.menu_shortcuts.clone();
                        cross_scope_conflict(&menu_shortcuts, &key_str, "Menu Shortcuts")
                    })
                }
                BindingScope::Menu => {
                    let keybindings = config.read().keybindings.clone();
                    check_menu_conflict(&keybindings.menu_shortcuts, &key_str, is_edit, &orig_key)
                        .or_else(|| cross_scope_conflict(&keybindings.global, &key_str, "Global"))
                }
            }
        }
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
                // Key input with recorder
                div {
                    class: "key-recorder-input",
                    input {
                        r#type: "text",
                        class: if *recording.read() { "key-recorder-text recording" } else { "key-recorder-text" },
                        placeholder: if *recording.read() { "Press a key..." } else { "e.g. Cmd+k, g g" },
                        value: "{key_input}",
                        readonly: *recording.read(),
                        oninput: move |evt| {
                            if !*recording.read() {
                                key_input.set(evt.value());
                            }
                        },
                        onkeydown: move |evt: KeyboardEvent| {
                            if !*recording.read() {
                                return;
                            }
                            evt.prevent_default();
                            evt.stop_propagation();
                            let chord = crate::keybindings::chord_from_event(&evt);
                            if chord.is_modifier_only() {
                                return;
                            }
                            // Backspace removes last chord from sequence
                            if chord.to_string() == "Backspace" {
                                let mut chords = recorded_chords.write();
                                chords.pop();
                                let display: Vec<String> = chords.iter().map(|c| c.to_string()).collect();
                                key_input.set(display.join(" "));
                                let next = *recording_input_epoch.read() + 1;
                                recording_input_epoch.set(next);
                                return;
                            }
                            recorded_chords.write().push(chord);
                            let display: Vec<String> = recorded_chords.read().iter().map(|c| c.to_string()).collect();
                            key_input.set(display.join(" "));
                            let next = *recording_input_epoch.read() + 1;
                            recording_input_epoch.set(next);
                        },
                    }
                    if *recording.read() {
                        button {
                            class: "key-record-btn recording",
                            onclick: move |_| {
                                // Done: keep recorded value, stop recording
                                recording.set(false);
                            },
                            "Done"
                        }
                        button {
                            class: "key-record-btn",
                            onclick: move |_| {
                                // Cancel: revert to pre-record value
                                key_input.set(pre_record_value.read().clone());
                                recorded_chords.write().clear();
                                recording_input_epoch.set(0);
                                recording.set(false);
                            },
                            "Cancel"
                        }
                    } else {
                        button {
                            class: "key-record-btn",
                            onclick: move |_| {
                                // Start recording: snapshot current value, clear chords
                                pre_record_value.set(key_input.read().clone());
                                recorded_chords.write().clear();
                                key_input.set(String::new());
                                recording_input_epoch.set(0);
                                recording.set(true);
                            },
                            "Record"
                        }
                    }
                }

                // Action dropdown (grouped by category)
                select {
                    class: "action-select",
                    value: "{selected_action_value}",
                    onchange: move |evt: FormEvent| selected_action.set(evt.value()),
                    option {
                        value: "",
                        disabled: true,
                        selected: selected_action_value.is_empty(),
                        "Select action..."
                    }
                    // Menu shortcuts may only target menu-backed actions; engine
                    // bindings can target any action (grouped by category).
                    if scope == BindingScope::Menu {
                        for action in MENU_ACTIONS {
                            {
                                let action_value = action.to_string();
                                let is_selected = selected_action_value == action_value;
                                rsx! {
                                    option {
                                        key: "{action_value}",
                                        value: "{action_value}",
                                        selected: is_selected,
                                        "{action_label(&action_value)}"
                                    }
                                }
                            }
                        }
                    } else {
                        for (group_label, actions) in ACTION_GROUPS {
                            optgroup {
                                label: *group_label,
                                for action in *actions {
                                    {
                                        let action_value = action.to_string();
                                        let is_selected = selected_action_value == action_value;
                                        rsx! {
                                    option {
                                                key: "{action_value}",
                                                value: "{action_value}",
                                                selected: is_selected,
                                                "{action_label(&action_value)}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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
                        has_changes.set(true);
                        on_close.call(());
                    },
                    if is_edit { "Save" } else { "Add" }
                }
                button {
                    class: "binding-form-cancel",
                    onclick: move |_| {
                        recording.set(false);
                        on_close.call(());
                    },
                    "Cancel"
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
                                has_changes.set(true);
                                on_close.call(());
                            }
                        },
                        "Delete"
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get mutable reference to the bindings for a given scope.
fn bindings_mut(set: &mut crate::config::BindingSet, scope: BindingScope) -> &mut Vec<KeyAction> {
    match scope {
        BindingScope::Menu => &mut set.menu_shortcuts,
        BindingScope::Engine(None) => &mut set.global,
        BindingScope::Engine(Some(KeyContext::Content)) => &mut set.content,
        BindingScope::Engine(Some(KeyContext::Sidebar)) => &mut set.sidebar,
        BindingScope::Engine(Some(KeyContext::QuickAccess)) => &mut set.quick_access,
        BindingScope::Engine(Some(KeyContext::RightSidebar)) => &mut set.right_sidebar,
        BindingScope::Engine(Some(KeyContext::Search)) => &mut set.search,
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
            "Menu shortcuts must be a single chord (e.g. Cmd+K)".to_string(),
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
            return Some(BindingNotice::Conflict(format!(
                "Conflicts with \"{}\" (Menu Shortcuts)",
                action_label(&ka.action)
            )));
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
            return Some(BindingNotice::Conflict(format!(
                "Also bound as \"{}\" ({other_label}) — one will shadow the other",
                action_label(&ka.action)
            )));
        }
    }
    None
}

/// Convert an action string like "scroll.down" to a human-readable label "Scroll Down".
fn action_label(action_str: &str) -> String {
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
fn context_label(context: Option<KeyContext>) -> &'static str {
    match context {
        None => "Global",
        Some(KeyContext::Content) => "Content",
        Some(KeyContext::Sidebar) => "Sidebar",
        Some(KeyContext::QuickAccess) => "Quick Access",
        Some(KeyContext::RightSidebar) => "Right Sidebar",
        Some(KeyContext::Search) => "Search",
    }
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

        let action_desc = format!(
            "\"{}\" ({})",
            action_label(&binding.action.to_string()),
            context_label(binding.context),
        );

        match (binding.context, context) {
            // Same context (including both Global) → true conflict
            (None, None) | (Some(_), Some(_)) if binding.context == context => {
                return Some(BindingNotice::Conflict(format!(
                    "Conflicts with {action_desc}"
                )))
            }
            // New context binding overrides existing global
            (None, Some(_)) => {
                overrides_global = overrides_global.or(Some(action_desc));
            }
            // New global binding will be overridden by existing context binding
            (Some(_), None) => {
                overridden_by.push(context_label(binding.context).to_string());
            }
            // Different specific contexts → no overlap
            _ => {}
        }
    }

    if let Some(desc) = overrides_global {
        return Some(BindingNotice::Overwrite(format!(
            "Overrides {desc} in this context"
        )));
    }

    if !overridden_by.is_empty() {
        overridden_by.sort();
        overridden_by.dedup();
        return Some(BindingNotice::Overwrite(format!(
            "Will be overridden in: {}",
            overridden_by.join(", ")
        )));
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
        assert_eq!(action_label("tab.close_all"), "Tab Close All");
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
        assert_eq!(
            context_label(Some(KeyContext::RightSidebar)),
            "Right Sidebar"
        );
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
        // Adding "j" to Global — overridden by both Sidebar and Content
        let result = check_conflict(&resolved, "j", None);
        match result {
            Some(BindingNotice::Overwrite(msg)) => {
                assert!(msg.contains("Content"), "should list Content: {msg}");
                assert!(msg.contains("Sidebar"), "should list Sidebar: {msg}");
            }
            other => panic!("Expected Overwrite, got {other:?}"),
        }
    }
}
