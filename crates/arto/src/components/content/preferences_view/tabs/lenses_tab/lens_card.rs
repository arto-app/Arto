use std::path::PathBuf;
use std::str::FromStr;

use dioxus::prelude::*;
use rust_i18n::t;

use super::super::super::form_controls::{
    ChoiceItem, ChoiceRow, OptionCardItem, OptionCards, ToggleRow,
};
use super::fields::{ApiKeyField, ArgvField, ModelField, NumberField, TextArea, TextField};
use super::{
    agent_description, agent_name, capability_text, display_description, display_title, edit,
    endpoint_hint, optional, prompt_hint,
};
use crate::components::icon::{Icon, IconName};
use crate::config::{
    usable_lenses, Config, Lens, LensAgent, LensDisplay, LensTarget, LensUnit,
    MAX_LENS_CONCURRENCY, MAX_LENS_CONTEXT,
};
use crate::keybindings::lens_shortcut_holder;
use crate::lenses::{forget_offered_models, key_account, message_shape, ModelSource};

/// One lens: what it is called and does at a glance, dragged by that line
/// to move it, and its settings when it is open.
#[component]
pub(super) fn LensCard(
    config: Signal<Config>,
    index: usize,
    lens: Lens,
    problem: Option<String>,
    open: Signal<Option<usize>>,
    is_dragging: bool,
    /// Which side of this lens a dragged one would land on, when it is the
    /// one being rested on.
    drop_side: Option<bool>,
    on_drag_start: EventHandler<()>,
    on_drag_over: EventHandler<()>,
    on_drag_leave: EventHandler<()>,
    on_drag_end: EventHandler<()>,
) -> Element {
    let is_open = open() == Some(index);
    // Removing takes a second click, since the prompt goes with it and
    // there is no undo.
    let mut confirming = use_signal(|| false);
    let runner = match (&lens.agent, &lens.model) {
        (Some(_), Some(model)) => format!("{} · {model}", agent_name(lens.agent)),
        _ => agent_name(lens.agent),
    };
    let drop_class = match drop_side {
        Some(true) => "lens-drop-after",
        Some(false) => "lens-drop-before",
        None => "",
    };

    rsx! {
        div {
            class: "lens-card {drop_class}",
            class: if is_open { "open" },
            class: if is_dragging { "dragging" },
            class: if problem.is_some() { "has-problem" },
            ondragover: move |event: Event<DragData>| {
                event.prevent_default();
                on_drag_over.call(());
            },
            ondragleave: move |_| on_drag_leave.call(()),

            // Only this line is dragged: the fields below it are for
            // selecting text in.
            div {
                class: "lens-card-header",
                draggable: true,
                ondragstart: move |_| on_drag_start.call(()),
                ondragend: move |_| on_drag_end.call(()),
                button {
                    class: "lens-card-summary",
                    "aria-expanded": is_open,
                    onclick: move |_| open.set(if is_open { None } else { Some(index) }),
                    span { class: "lens-card-label", "{lens.label}" }
                    span { class: "lens-card-meta", "{display_title(lens.display)} · {runner}" }
                }
                div {
                    class: "lens-card-actions",
                    button {
                        class: "lens-icon-button",
                        class: if is_open { "active" },
                        title: if is_open { t!("preferences.lenses.card.close").to_string() } else { t!("preferences.lenses.card.edit").to_string() },
                        onclick: move |_| open.set(if is_open { None } else { Some(index) }),
                        Icon { name: IconName::Edit, size: 16 }
                    }
                    if confirming() {
                        button {
                            class: "lens-button lens-button-danger",
                            onclick: move |_| {
                                config.write().lenses.remove(index);
                                confirming.set(false);
                                open.set(None);
                            },
                            onmouseleave: move |_| confirming.set(false),
                            {t!("preferences.lenses.card.remove").to_string()}
                        }
                    } else {
                        button {
                            class: "lens-icon-button",
                            title: t!("preferences.lenses.card.remove").to_string(),
                            onclick: move |_| confirming.set(true),
                            Icon { name: IconName::Trash, size: 16 }
                        }
                    }
                }
            }

            if let Some(problem) = &problem {
                p { class: "lens-card-problem", {t!("preferences.lenses.problem.not_offered", problem = problem).to_string()} }
            }

            if is_open {
                LensForm { config, index, lens: lens.clone() }
            }
        }
    }
}

/// Every setting of the lens at `index`, showing only those its display and
/// runner read.
#[component]
fn LensForm(config: Signal<Config>, index: usize, lens: Lens) -> Element {
    let server = lens.agent.and_then(LensAgent::server);
    let is_command = lens.agent.is_none();
    // Bumped when the stored key changes, so the models are asked for again
    // with it: a server that refused to list them without one may now.
    let mut key_revision = use_signal(|| 0_u64);
    let shortcut_holder = {
        let config = config.read();
        let offered: Vec<Lens> = usable_lenses(&config.lenses)
            .0
            .into_iter()
            .cloned()
            .collect();
        offered
            .iter()
            .position(|offered| offered.id == lens.id)
            .and_then(|place| lens_shortcut_holder(&config.keybindings, &offered, place))
    };

    rsx! {
        div {
            class: "lens-form",

            TextField {
                label: t!("preferences.lenses.form.label.label"),
                hint: t!("preferences.lenses.form.label.hint"),
                value: lens.label.clone(),
                on_input: move |text: String| edit(config, index, |lens| lens.label = text),
            }
            TextField {
                label: t!("preferences.lenses.form.id.label"),
                hint: t!("preferences.lenses.form.id.hint"),
                value: lens.id.clone(),
                monospace: true,
                on_input: move |text: String| edit(config, index, |lens| lens.id = text.trim().to_string()),
            }
            TextField {
                label: t!("preferences.lenses.form.shortcut.label"),
                hint: t!("preferences.lenses.form.shortcut.hint"),
                value: lens.shortcut.clone().unwrap_or_default(),
                monospace: true,
                on_input: move |text: String| edit(config, index, |lens| {
                    let text = text.trim();
                    lens.shortcut = (!text.is_empty()).then(|| text.to_string());
                }),
            }
            if let Some(holder) = shortcut_holder {
                p { class: "lens-card-problem", {t!("preferences.lenses.form.shortcut.taken", holder = holder).to_string()} }
            }

            // Cards rather than a row of words: what each display does is
            // the whole of the choice, and a word for it says too little.
            div {
                class: "lens-field",
                span { class: "lens-field-label", {t!("preferences.lenses.form.display").to_string()} }
                OptionCards {
                    name: format!("lens-{index}-display"),
                    options: [LensDisplay::Page, LensDisplay::Popover, LensDisplay::Annotate]
                        .into_iter()
                        .map(|display| OptionCardItem {
                            icon: None,
                            value: display,
                            title: display_title(display),
                            description: Some(display_description(display)),
                        })
                        .collect::<Vec<_>>(),
                    selected: lens.display,
                    on_change: move |display| edit(config, index, |lens| lens.set_display(display)),
                }
            }

            if lens.display == LensDisplay::Page {
                ChoiceRow {
                    name: format!("lens-{index}-unit"),
                    label: t!("preferences.lenses.form.unit.label").to_string(),
                    description: Some(t!("preferences.lenses.form.unit.description").to_string()),
                    options: vec![
                        ChoiceItem { value: LensUnit::Document, label: t!("preferences.lenses.form.unit.document").to_string() },
                        ChoiceItem { value: LensUnit::Block, label: t!("preferences.lenses.form.unit.block").to_string() },
                    ],
                    selected: lens.unit,
                    on_change: move |unit| edit(config, index, |lens| lens.unit = unit),
                }
            } else {
                ChoiceRow {
                    name: format!("lens-{index}-on"),
                    label: t!("preferences.lenses.form.on.label").to_string(),
                    description: Some(t!("preferences.lenses.form.on.description").to_string()),
                    options: LensTarget::ALL
                        .into_iter()
                        .map(|on| ChoiceItem {
                            value: on,
                            label: match on {
                                LensTarget::Either => t!("preferences.lenses.form.on.either"),
                                LensTarget::Block => t!("preferences.lenses.form.on.block"),
                                LensTarget::Document => t!("preferences.lenses.form.on.document"),
                            }
                            .to_string(),
                        })
                        .collect::<Vec<_>>(),
                    selected: lens.on,
                    on_change: move |on| edit(config, index, |lens| lens.on = on),
                }
            }

            ChoiceRow {
                name: format!("lens-{index}-agent"),
                label: t!("preferences.lenses.asks.label").to_string(),
                description: Some(agent_description(lens.agent)),
                options: LensAgent::ALL
                    .map(Some)
                    .into_iter()
                    .chain([None])
                    .map(|agent| ChoiceItem { value: agent, label: agent_name(agent) })
                    .collect::<Vec<_>>(),
                selected: lens.agent,
                on_change: move |agent| edit(config, index, |lens| lens.set_agent(agent)),
            }

            if is_command {
                ArgvField {
                    label: t!("preferences.lenses.form.command.label"),
                    hint: t!("preferences.lenses.form.command.hint"),
                    value: lens.command.clone(),
                    rows: 3,
                    on_change: move |args: Vec<String>| edit(config, index, |lens| lens.command = args),
                }
            } else {
                ModelField {
                    index,
                    source: ModelSource::of(&lens),
                    key_revision,
                    hint: if server.is_some() { t!("preferences.lenses.model.required") } else { t!("preferences.lenses.model.default") },
                    value: lens.model.clone().unwrap_or_default(),
                    on_input: move |text: String| edit(config, index, |lens| lens.model = optional(text)),
                }
                TextArea {
                    label: t!("preferences.lenses.form.prompt"),
                    hint: prompt_hint(lens.display),
                    value: lens.prompt.clone().unwrap_or_default(),
                    rows: 5,
                    on_input: move |text: String| edit(config, index, |lens| lens.prompt = optional(text)),
                }
                // The message itself, so that what the hint says the agent
                // is sent can be seen rather than taken on trust.
                details {
                    class: "lens-message",
                    summary { {t!("preferences.lenses.form.message").to_string()} }
                    pre { "{message_shape(lens.display, lens.prompt.as_deref())}" }
                }
            }

            if server.is_some() {
                TextField {
                    label: t!("preferences.lenses.endpoint.label"),
                    hint: lens.agent.map(endpoint_hint).unwrap_or_default(),
                    value: lens.endpoint.clone().unwrap_or_default(),
                    monospace: true,
                    on_input: move |text: String| edit(config, index, |lens| lens.endpoint = optional(text)),
                }
                TextArea {
                    label: t!("preferences.lenses.form.system.label"),
                    hint: t!("preferences.lenses.form.system.hint"),
                    value: lens.system.clone().unwrap_or_default(),
                    rows: 2,
                    on_input: move |text: String| edit(config, index, |lens| lens.system = optional(text)),
                }
                ApiKeyField {
                    account: key_account(&lens),
                    overridden: !lens.api_key_command.is_empty(),
                    on_change: move |_| {
                        forget_offered_models();
                        key_revision += 1;
                    },
                }
                ArgvField {
                    label: t!("preferences.lenses.form.api_key_command.label"),
                    hint: t!("preferences.lenses.form.api_key_command.hint"),
                    value: lens.api_key_command.clone(),
                    rows: 2,
                    on_change: move |args: Vec<String>| edit(config, index, |lens| lens.api_key_command = args),
                }
            } else if !is_command {
                TextField {
                    label: t!("preferences.lenses.form.program.label"),
                    hint: t!("preferences.lenses.form.program.hint"),
                    value: lens.program.as_ref().map(|path| path.display().to_string()).unwrap_or_default(),
                    monospace: true,
                    on_input: move |text: String| edit(config, index, |lens| lens.program = optional(text).map(PathBuf::from)),
                }
            }

            if server.is_some_and(|server| server.context_length) {
                NumberField {
                    label: t!("preferences.lenses.form.context_length.label"),
                    hint: t!("preferences.lenses.form.context_length.hint"),
                    value: lens.context_length.map(|length| length.to_string()).unwrap_or_default(),
                    on_input: move |text: String| set_optional_number(config, index, &text, |lens, n| lens.context_length = n),
                }
            }

            // Each off unless the reader turns it on for this lens.
            for capability in lens.agent.map_or(&[][..], |agent| agent.profile().capabilities).iter().copied() {
                ToggleRow {
                    key: "{capability}",
                    label: capability_text(capability).0,
                    description: Some(capability_text(capability).1),
                    checked: lens.allow.contains(&capability),
                    on_change: move |on| edit(config, index, |lens| {
                        lens.allow.retain(|allowed| *allowed != capability);
                        if on {
                            lens.allow.push(capability);
                        }
                    }),
                    shipped: Some(false),
                }
            }

            if lens.display == LensDisplay::Annotate {
                NumberField {
                    label: t!("preferences.lenses.form.context.label"),
                    hint: t!("preferences.lenses.form.context.hint", max = MAX_LENS_CONTEXT),
                    value: lens.context.to_string(),
                    on_input: move |text: String| set_number(config, index, &text, |lens, n| lens.context = n),
                }
                NumberField {
                    label: t!("preferences.lenses.form.concurrency.label"),
                    hint: t!("preferences.lenses.form.concurrency.hint", max = MAX_LENS_CONCURRENCY),
                    value: lens.concurrency.to_string(),
                    on_input: move |text: String| set_number(config, index, &text, |lens, n| lens.concurrency = n),
                }
            }

            NumberField {
                label: t!("preferences.lenses.form.timeout.label"),
                hint: t!("preferences.lenses.form.timeout.hint"),
                value: lens.timeout_seconds.to_string(),
                on_input: move |text: String| set_number(config, index, &text, |lens, n| lens.timeout_seconds = n),
            }
        }
    }
}

/// Set a number from `text`, leaving the setting alone while `text` is not
/// one yet — half-way through being typed.
fn set_number<N: FromStr>(
    config: Signal<Config>,
    index: usize,
    text: &str,
    change: impl FnOnce(&mut Lens, N),
) {
    if let Some(number) = typed_number(text) {
        edit(config, index, |lens| change(lens, number));
    }
}

/// [`set_number`] for a setting that may be left unset, which a blank
/// `text` does.
fn set_optional_number<N: FromStr>(
    config: Signal<Config>,
    index: usize,
    text: &str,
    change: impl FnOnce(&mut Lens, Option<N>),
) {
    if let Some(number) = typed_optional_number(text) {
        edit(config, index, |lens| change(lens, number));
    }
}

fn typed_number<N: FromStr>(text: &str) -> Option<N> {
    text.trim().parse().ok()
}

/// `Some(None)` for a blank `text`, and `None` while it is neither blank
/// nor a number.
fn typed_optional_number<N: FromStr>(text: &str) -> Option<Option<N>> {
    if text.trim().is_empty() {
        Some(None)
    } else {
        typed_number(text).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_read_once_it_is_one() {
        assert_eq!(typed_number::<u32>(" 30 "), Some(30));
        assert_eq!(typed_number::<u32>(""), None);
        assert_eq!(typed_number::<u32>("3x"), None);
    }

    #[test]
    fn an_optional_number_is_cleared_by_a_blank() {
        assert_eq!(typed_optional_number::<u32>(" 8192 "), Some(Some(8192)));
        assert_eq!(typed_optional_number::<u32>("  "), Some(None));
        assert_eq!(typed_optional_number::<u32>("8k"), None);
    }
}
