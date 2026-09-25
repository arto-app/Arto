use super::super::form_controls::{ChoiceItem, ChoiceRow, OptionCardItem, OptionCards, ToggleRow};
use crate::components::icon::{Icon, IconName};
use crate::components::reorder::{drop_side, DragRow};
use crate::config::{
    lens_problems, unused_lens_id, usable_lenses, Config, Lens, LensAgent, LensDisplay, LensRecipe,
    LensTarget, LensUnit, RecipeBlanks, MAX_LENS_CONCURRENCY, MAX_LENS_CONTEXT,
};
use crate::keybindings::lens_shortcut_holder;
use crate::lenses::{
    available_models, forget_key, forget_offered_models, is_stored, key_account, message_shape,
    store_key, ModelSource,
};
use dioxus::prelude::*;
use std::path::PathBuf;
use std::str::FromStr;

/// The lenses the menus offer, in the order they offer them — dragged into
/// another, as the sidebar's saved folders are — each opened in place to be
/// edited.
///
/// A lens that cannot be offered stays in the list with what is wrong with
/// it, rather than vanishing from the menus with no word as to why.
#[component]
pub fn LensesTab(config: Signal<Config>) -> Element {
    let lenses = config.read().lenses.clone();
    let problems = lens_problems(&lenses);
    let mut open = use_signal(|| None::<usize>);
    let mut dragging = use_signal(|| None::<DragRow<()>>);
    let mut drop_target = use_signal(|| None::<DragRow<()>>);
    let mut add = move |lens: Lens| {
        let mut config = config.write();
        config.lenses.push(lens);
        open.set(Some(config.lenses.len() - 1));
    };

    rsx! {
        div {
            class: "preferences-pane",

            p {
                class: "preference-lede",
                "An agent or a command that looks at the document you are reading and shows its answer with it — a translation in the page's places, a summary from the header, a note beside each paragraph. Nothing is sent anywhere until you open one."
            }

            h3 { class: "preference-section-title", "Lenses" }

            if lenses.is_empty() {
                p { class: "preference-description lens-empty", "No lenses yet. Start from a recipe below, or from a blank one." }
            }

            for (index, (lens, problem)) in lenses.into_iter().zip(problems).enumerate() {
                LensCard {
                    key: "{index}",
                    config,
                    index,
                    lens,
                    problem: problem.map(|problem| problem.to_string()),
                    open,
                    is_dragging: dragging.read().as_ref().map(|(at, _)| *at) == Some(index),
                    drop_side: drop_side(&dragging.read(), &drop_target.read(), index),
                    on_drag_start: move |_| dragging.set(Some((index, ()))),
                    on_drag_over: move |_| {
                        if dragging.read().is_some() {
                            drop_target.set(Some((index, ())));
                        }
                    },
                    on_drag_leave: move |_| drop_target.set(None),
                    on_drag_end: move |_| {
                        if let (Some((from, ())), Some((to, ()))) = (dragging.take(), drop_target.take()) {
                            move_lens(config, open, from, to);
                        }
                    },
                }
            }

            div {
                class: "lens-add-line",
                button {
                    class: "lens-button",
                    onclick: move |_| {
                        let id = unused_lens_id(&config.read().lenses, "lens");
                        add(Lens::new(id));
                    },
                    Icon { name: IconName::Add, size: 14 }
                    span { "Blank lens" }
                }
            }

            h3 { class: "preference-section-title", "Start from a recipe" }
            p {
                class: "preference-description",
                "A lens written in advance: pick one, fill in the language and who answers, and add it."
            }
            RecipePicker {
                taken: config.read().lenses.clone(),
                on_add: add,
            }
        }
    }
}

/// The language the reader reads in, named in English for a prompt: the
/// first the system prefers, as the page's own `navigator.language` gives
/// it. English until the page has said.
fn use_reader_language() -> Memo<String> {
    let language = use_resource(|| async {
        let mut eval = document::eval(
            "dioxus.send(new Intl.DisplayNames(['en'], { type: 'language' }).of(navigator.language.split('-')[0]) ?? 'English');",
        );
        eval.recv::<String>().await.ok()
    });
    use_memo(move || {
        language
            .read()
            .clone()
            .flatten()
            .unwrap_or_else(|| "English".to_string())
    })
}

/// A recipe to start a lens from, and the few blanks it leaves: the
/// language to answer in, who asks, and the model. The lens is added only
/// once it can be offered, and edits like any other afterwards.
#[component]
fn RecipePicker(
    /// The lenses there are, so the new one gets an id of its own.
    taken: Vec<Lens>,
    on_add: EventHandler<Lens>,
) -> Element {
    let language = use_reader_language();
    let mut recipe = use_signal(|| None::<LensRecipe>);
    let mut blanks = use_signal(|| RecipeBlanks {
        language: String::new(),
        languages: ["English".to_string(), "Japanese".to_string()],
        audience: String::new(),
        agent: LensAgent::Claude,
        model: String::new(),
        endpoint: String::new(),
    });
    let mut key_revision = use_signal(|| 0_u64);
    // The reader's language, once the page has said it, unless typed over.
    use_effect(move || {
        let language = language();
        if blanks.peek().language.is_empty() {
            blanks.write().language = language;
        }
    });

    let chosen = recipe();
    let draft = chosen.map(|recipe| {
        let id = unused_lens_id(&taken, recipe.slug());
        recipe.lens(id, &blanks.read())
    });
    let problem = draft
        .as_ref()
        .and_then(|lens| lens_problems(std::slice::from_ref(lens)).remove(0));
    let filled = blanks.read().clone();

    rsx! {
        div {
            class: "lens-recipes",

            div {
                class: "lens-recipe-cards",
                for option in LensRecipe::ALL {
                    button {
                        key: "{option.slug()}",
                        class: "lens-recipe-card",
                        class: if chosen == Some(option) { "selected" },
                        r#type: "button",
                        onclick: move |_| {
                            if chosen == Some(option) {
                                recipe.set(None);
                                return;
                            }
                            recipe.set(Some(option));
                            // The recipe's model for the agent, filled in
                            // where it can be seen and changed.
                            let agent = blanks.peek().agent;
                            blanks.write().model = option.model(agent).unwrap_or_default().to_string();
                        },
                        div {
                            class: "lens-recipe-head",
                            Icon { name: recipe_icon(option), size: 20, class: "lens-recipe-icon" }
                            span { class: "lens-recipe-title", "{option.title()}" }
                        }
                        span { class: "lens-recipe-desc", "{option.description()}" }
                        span { class: "lens-recipe-tag", "{display_title(option.display())}" }
                    }
                }
            }

            if let (Some(chosen), Some(draft)) = (chosen, draft) {
                div {
                    class: "lens-form",
                    if chosen.translates() {
                        div {
                            class: "lens-language-pair",
                            TextField {
                                label: "Between",
                                hint: "Text in this language is translated into the other.",
                                value: filled.languages[0].clone(),
                                on_input: move |text: String| blanks.write().languages[0] = text,
                            }
                            span { class: "lens-language-swap", "aria-hidden": "true", "⇄" }
                            TextField {
                                label: "And",
                                hint: "Text in this language is translated into the other; text in any other language, into this one.",
                                value: filled.languages[1].clone(),
                                on_input: move |text: String| blanks.write().languages[1] = text,
                            }
                        }
                    } else {
                        TextField {
                            label: "Language",
                            hint: "The language the answer is written in, named in English.",
                            value: filled.language.clone(),
                            on_input: move |text: String| blanks.write().language = text,
                        }
                    }
                    if chosen.asks_audience() {
                        TextField {
                            label: "For whom",
                            hint: "Who the explanation is written for — a newcomer to the subject when left empty.",
                            value: filled.audience.clone(),
                            on_input: move |text: String| blanks.write().audience = text,
                        }
                    }
                    ChoiceRow {
                        name: "recipe-agent".to_string(),
                        label: "Asks".to_string(),
                        description: Some(filled.agent.profile().introduction.to_string()),
                        options: LensAgent::ALL
                            .into_iter()
                            .map(|agent| ChoiceItem { value: agent, label: agent_name(Some(agent)).to_string() })
                            .collect::<Vec<_>>(),
                        selected: filled.agent,
                        on_change: move |agent| {
                            let mut blanks = blanks.write();
                            blanks.agent = agent;
                            blanks.model = chosen.model(agent).unwrap_or_default().to_string();
                            blanks.endpoint.clear();
                        },
                    }
                    ModelField {
                        index: usize::MAX,
                        source: ModelSource::of(&draft),
                        key_revision,
                        hint: match chosen.model(filled.agent) {
                            Some(model) => format!("The recipe's choice for {}: {model}.", agent_name(Some(filled.agent))),
                            None if filled.agent.is_server() => "Required: pick one the server offers.".to_string(),
                            None => "Left empty, the agent's own default.".to_string(),
                        },
                        value: filled.model.clone(),
                        on_input: move |text: String| blanks.write().model = text,
                    }
                    if let Some(server) = filled.agent.server().filter(|server| server.remote) {
                        TextField {
                            label: "Endpoint",
                            hint: server.endpoint_hint,
                            value: filled.endpoint.clone(),
                            monospace: true,
                            on_input: move |text: String| blanks.write().endpoint = text,
                        }
                        ApiKeyField {
                            account: key_account(&draft),
                            overridden: false,
                            on_change: move |_| {
                                forget_offered_models();
                                key_revision += 1;
                            },
                        }
                    }
                    if let Some(problem) = &problem {
                        p { class: "lens-card-problem", "Not ready: {problem}" }
                    }
                    div {
                        class: "lens-add-line",
                        if problem.is_none() {
                            button {
                                class: "lens-button",
                                r#type: "button",
                                onclick: {
                                    let draft = draft.clone();
                                    move |_| {
                                        on_add.call(draft.clone());
                                        recipe.set(None);
                                    }
                                },
                                Icon { name: IconName::Add, size: 14 }
                                span { "Add “{draft.label}”" }
                            }
                        }
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: move |_| recipe.set(None),
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}

/// The icon a recipe's card shows.
fn recipe_icon(recipe: LensRecipe) -> IconName {
    match recipe {
        LensRecipe::TranslatePage => IconName::Language,
        LensRecipe::TranslateBeside => IconName::Columns2,
        LensRecipe::Summarize => IconName::ListDetails,
        LensRecipe::ExplainTerms => IconName::Vocabulary,
        LensRecipe::Critique => IconName::MessageReport,
        LensRecipe::FactCheck => IconName::Search,
        LensRecipe::ExplainBlock => IconName::Bulb,
    }
}

/// Move the lens at `from` to `to`, keeping the one being edited open.
fn move_lens(mut config: Signal<Config>, mut open: Signal<Option<usize>>, from: usize, to: usize) {
    let mut config = config.write();
    if from >= config.lenses.len() || to >= config.lenses.len() {
        return;
    }
    let lens = config.lenses.remove(from);
    config.lenses.insert(to, lens);
    let editing = *open.peek();
    open.set(editing.map(|at| moved_index(at, from, to)));
}

/// Where the row at `at` is once the row at `from` has moved to `to`.
fn moved_index(at: usize, from: usize, to: usize) -> usize {
    if at == from {
        to
    } else if from < at && at <= to {
        at - 1
    } else if to <= at && at < from {
        at + 1
    } else {
        at
    }
}

/// Apply `change` to the lens at `index`.
fn edit(mut config: Signal<Config>, index: usize, change: impl FnOnce(&mut Lens)) {
    if let Some(lens) = config.write().lenses.get_mut(index) {
        change(lens);
    }
}

/// `text` as an optional setting: nothing when it is blank.
fn optional(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

/// A program and its arguments, one per line.
fn argv(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// What a display does, in the words the cards and the list use.
fn display_title(display: LensDisplay) -> &'static str {
    match display {
        LensDisplay::Page => "Replaces the page",
        LensDisplay::Popover => "One answer on request",
        LensDisplay::Annotate => "A note beside each block",
    }
}

fn display_description(display: LensDisplay) -> &'static str {
    match display {
        LensDisplay::Page => {
            "The answer takes the document's places, block by block from the top as it is written. For a translation."
        }
        LensDisplay::Popover => {
            "Asked for the whole document from the header, or for one block from its menu, and shown in a popover. For a summary."
        }
        LensDisplay::Annotate => {
            "Every block is asked about on its own; each answer waits behind a mark in the margin. For a gloss or an explanation."
        }
    }
}

/// How to write the prompt for a lens shown as `display`: how the text
/// reaches the agent, and so what the prompt can call it.
fn prompt_hint(display: LensDisplay) -> &'static str {
    if display.is_per_block() {
        "What the agent is asked about each block. The block comes after your prompt between <text> and </text>, and the blocks around it between <before> and </before> and <after> and </after> — so the prompt can say \"explain the terms in <text>\" and tell the agent to use the others as context only. An answer of <nothing/> alone leaves the block unmarked, so a prompt can say \"if there is nothing worth noting, answer with exactly <nothing/>\". Left empty, the agent is handed the block alone."
    } else {
        "What the agent is asked to do with the text, which comes after your prompt and a blank line with nothing around it — call it \"the text\". Left empty, the agent is handed the text alone, which is what a model trained for one task, such as translation, expects."
    }
}

fn agent_name(agent: Option<LensAgent>) -> &'static str {
    agent.map_or("Command", |agent| agent.profile().name)
}

/// One lens: what it is called and does at a glance, dragged by that line
/// to move it, and its settings when it is open.
#[component]
fn LensCard(
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
        _ => agent_name(lens.agent).to_string(),
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
                        title: if is_open { "Close" } else { "Edit" },
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
                            "Remove"
                        }
                    } else {
                        button {
                            class: "lens-icon-button",
                            title: "Remove",
                            onclick: move |_| confirming.set(true),
                            Icon { name: IconName::Trash, size: 16 }
                        }
                    }
                }
            }

            if let Some(problem) = &problem {
                p { class: "lens-card-problem", "Not offered: {problem}" }
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
                label: "Label",
                hint: "What the menus call it.",
                value: lens.label.clone(),
                on_input: move |text: String| edit(config, index, |lens| lens.label = text),
            }
            TextField {
                label: "ID",
                hint: "Unique among the lenses; earlier answers are kept under it.",
                value: lens.id.clone(),
                monospace: true,
                on_input: move |text: String| edit(config, index, |lens| lens.id = text.trim().to_string()),
            }
            TextField {
                label: "Shortcut",
                hint: "Keys that show or hide the lens while reading, written as in Keybindings: Cmd+Shift+t, or g t for one after another.",
                value: lens.shortcut.clone().unwrap_or_default(),
                monospace: true,
                on_input: move |text: String| edit(config, index, |lens| {
                    let text = text.trim();
                    lens.shortcut = (!text.is_empty()).then(|| text.to_string());
                }),
            }
            if let Some(holder) = shortcut_holder {
                p { class: "lens-card-problem", "Not bound: these keys already belong to {holder}." }
            }

            // Cards rather than a row of words: what each display does is
            // the whole of the choice, and a word for it says too little.
            div {
                class: "lens-field",
                span { class: "lens-field-label", "How the answer is shown" }
                OptionCards {
                    name: format!("lens-{index}-display"),
                    options: [LensDisplay::Page, LensDisplay::Popover, LensDisplay::Annotate]
                        .into_iter()
                        .map(|display| OptionCardItem {
                            icon: None,
                            value: display,
                            title: display_title(display).to_string(),
                            description: Some(display_description(display).to_string()),
                        })
                        .collect::<Vec<_>>(),
                    selected: lens.display,
                    on_change: move |display| edit(config, index, |lens| lens.set_display(display)),
                }
            }

            if lens.display == LensDisplay::Page {
                ChoiceRow {
                    name: format!("lens-{index}-unit"),
                    label: "Handed over".to_string(),
                    description: Some("The whole document in one run suits a general model; one block per run suits a small translation model.".to_string()),
                    options: vec![
                        ChoiceItem { value: LensUnit::Document, label: "Whole document".to_string() },
                        ChoiceItem { value: LensUnit::Block, label: "Block by block".to_string() },
                    ],
                    selected: lens.unit,
                    on_change: move |unit| edit(config, index, |lens| lens.unit = unit),
                }
            } else {
                ChoiceRow {
                    name: format!("lens-{index}-on"),
                    label: "Looks at".to_string(),
                    description: Some("Where the right-click menu offers it: Lens on Block, Lens on Document, or both. A shortcut shows a lens that looks at a block only over the block the cursor is on.".to_string()),
                    options: LensTarget::ALL
                        .into_iter()
                        .map(|on| ChoiceItem {
                            value: on,
                            label: match on {
                                LensTarget::Either => "Either",
                                LensTarget::Block => "A block",
                                LensTarget::Document => "The document",
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
                label: "Asks".to_string(),
                description: Some(
                    lens.agent
                        .map_or("A program of your own, handed the request as JSON.", |agent| agent.profile().description)
                        .to_string(),
                ),
                options: LensAgent::ALL
                    .map(Some)
                    .into_iter()
                    .chain([None])
                    .map(|agent| ChoiceItem { value: agent, label: agent_name(agent).to_string() })
                    .collect::<Vec<_>>(),
                selected: lens.agent,
                on_change: move |agent| edit(config, index, |lens| lens.set_agent(agent)),
            }

            if is_command {
                ArgvField {
                    label: "Command",
                    hint: "The program, then each argument on a line of its own. It is run as written, without a shell.",
                    value: lens.command.clone(),
                    rows: 3,
                    on_change: move |args: Vec<String>| edit(config, index, |lens| lens.command = args),
                }
            } else {
                ModelField {
                    index,
                    source: ModelSource::of(&lens),
                    key_revision,
                    hint: if server.is_some() { "Required: the model the server runs." } else { "Left empty, the agent's own default." },
                    value: lens.model.clone().unwrap_or_default(),
                    on_input: move |text: String| edit(config, index, |lens| lens.model = optional(text)),
                }
                TextArea {
                    label: "Prompt",
                    hint: prompt_hint(lens.display),
                    value: lens.prompt.clone().unwrap_or_default(),
                    rows: 5,
                    on_input: move |text: String| edit(config, index, |lens| lens.prompt = optional(text)),
                }
                // The message itself, so that what the hint says the agent
                // is sent can be seen rather than taken on trust.
                details {
                    class: "lens-message",
                    summary { "What the agent is sent" }
                    pre { "{message_shape(lens.display, lens.prompt.as_deref())}" }
                }
            }

            if server.is_some() {
                TextField {
                    label: "Endpoint",
                    hint: server.map_or("", |server| server.endpoint_hint),
                    value: lens.endpoint.clone().unwrap_or_default(),
                    monospace: true,
                    on_input: move |text: String| edit(config, index, |lens| lens.endpoint = optional(text)),
                }
                TextArea {
                    label: "System prompt",
                    hint: "Sent with every request.",
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
                    label: "API key command",
                    hint: "Instead of a stored key: a program that prints it — a password manager such as 1Password's op — then each argument on a line of its own. It is run for every request.",
                    value: lens.api_key_command.clone(),
                    rows: 2,
                    on_change: move |args: Vec<String>| edit(config, index, |lens| lens.api_key_command = args),
                }
            } else if !is_command {
                TextField {
                    label: "Program",
                    hint: "Where the program is, when it is not found on its own.",
                    value: lens.program.as_ref().map(|path| path.display().to_string()).unwrap_or_default(),
                    monospace: true,
                    on_input: move |text: String| edit(config, index, |lens| lens.program = optional(text).map(PathBuf::from)),
                }
            }

            if server.is_some_and(|server| server.context_length) {
                NumberField {
                    label: "Context length",
                    hint: "In tokens. Left empty, sized to each request.",
                    value: lens.context_length.map(|length| length.to_string()).unwrap_or_default(),
                    on_input: move |text: String| {
                        let length = text.trim().parse::<u32>().ok();
                        if text.trim().is_empty() || length.is_some() {
                            edit(config, index, |lens| lens.context_length = length);
                        }
                    },
                }
            }

            // Each off unless the reader turns it on for this lens.
            for capability in lens.agent.map_or(&[][..], |agent| agent.profile().capabilities).iter().copied() {
                ToggleRow {
                    key: "{capability}",
                    label: capability.label().to_string(),
                    description: Some(capability.description().to_string()),
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
                    label: "Context",
                    hint: format!("Blocks on each side handed over with each block, at most {MAX_LENS_CONTEXT}."),
                    value: lens.context.to_string(),
                    on_input: move |text: String| set_number(config, index, &text, |lens, n| lens.context = n),
                }
                NumberField {
                    label: "Runs at once",
                    hint: format!("1 to {MAX_LENS_CONCURRENCY}."),
                    value: lens.concurrency.to_string(),
                    on_input: move |text: String| set_number(config, index, &text, |lens, n| lens.concurrency = n),
                }
            }

            NumberField {
                label: "Timeout",
                hint: "Seconds one run may take before it counts as failed.",
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
    if let Ok(number) = text.trim().parse::<N>() {
        edit(config, index, |lens| change(lens, number));
    }
}

#[component]
fn TextField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    #[props(default)] monospace: bool,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            input {
                class: "lens-input",
                class: if monospace { "monospace" },
                r#type: "text",
                spellcheck: false,
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

#[component]
fn TextArea(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    rows: u32,
    #[props(default)] monospace: bool,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            textarea {
                class: "lens-input",
                class: if monospace { "monospace" },
                rows: "{rows}",
                spellcheck: false,
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

/// The most suggestions shown as buttons; a server with more — a hosted
/// catalog — is left to the field's own completion.
const MAX_MODEL_CHIPS: usize = 12;

/// The model, with the ones the agent offers to pick from: asked of the
/// agent itself when the lens is opened, and again only when where it is
/// asked changes — not as the prompt is typed.
#[component]
fn ModelField(
    index: usize,
    source: ReadSignal<Option<ModelSource>>,
    /// Changes when the stored API key does, to ask again.
    key_revision: ReadSignal<u64>,
    #[props(into)] hint: String,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    let offered = use_resource(move || async move {
        let _ = key_revision();
        match source() {
            Some(source) => available_models(&source).await,
            None => Ok(Vec::new()),
        }
    });
    let list_id = format!("lens-{index}-models");
    let (models, status) = match &*offered.read() {
        None => (
            Vec::new(),
            Some("Asking the agent for its models…".to_string()),
        ),
        Some(Ok(models)) => (models.clone(), None),
        Some(Err(reason)) => (
            Vec::new(),
            Some(format!("The models could not be listed: {reason}")),
        ),
    };

    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "Model" }
            input {
                class: "lens-input",
                r#type: "text",
                spellcheck: false,
                list: "{list_id}",
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            datalist {
                id: "{list_id}",
                for model in models.iter() {
                    option { key: "{model}", value: "{model}" }
                }
            }
            if !models.is_empty() && models.len() <= MAX_MODEL_CHIPS {
                div {
                    class: "lens-model-chips",
                    for model in models.iter().cloned() {
                        button {
                            key: "{model}",
                            class: "lens-model-chip",
                            class: if model == value { "selected" },
                            r#type: "button",
                            onclick: {
                                let model = model.clone();
                                move |_| on_input.call(model.clone())
                            },
                            "{model}"
                        }
                    }
                }
            }
            span { class: "lens-field-hint", "{hint}" }
            if let Some(status) = status {
                span { class: "lens-field-hint lens-field-status", "{status}" }
            }
        }
    }
}

/// The server's API key, kept in the system's credential store under its
/// endpoint rather than in `config.json`: typed once, never shown again,
/// and shared by every lens that asks the same server.
///
/// Saved on a button rather than as it is typed, unlike every other field
/// here: each keystroke would otherwise be a write to the store, and half a
/// key is not a key.
#[component]
fn ApiKeyField(
    account: ReadSignal<Option<String>>,
    /// Whether an API key command takes the stored key's place.
    overridden: bool,
    on_change: EventHandler<()>,
) -> Element {
    let mut revision = use_signal(|| 0_u64);
    let stored = use_resource(move || async move {
        let _ = revision();
        match account() {
            Some(account) => is_stored(account).await.map(Some),
            None => Ok(None),
        }
    });
    let mut draft = use_signal(String::new);
    let mut replacing = use_signal(|| false);
    let mut failure = use_signal(|| None::<String>);

    let mut settle = move |result: Result<(), String>| match result {
        Ok(()) => {
            draft.set(String::new());
            replacing.set(false);
            failure.set(None);
            revision += 1;
            on_change.call(());
        }
        Err(reason) => failure.set(Some(reason)),
    };
    let save = move |_| {
        let Some(account) = account() else { return };
        let key = draft().trim().to_string();
        spawn(async move { settle(store_key(account, key).await) });
    };
    let remove = move |_| {
        let Some(account) = account() else { return };
        spawn(async move { settle(forget_key(account).await) });
    };

    let Some(for_account) = account() else {
        return rsx! {
            div {
                class: "lens-field",
                span { class: "lens-field-label", "API key" }
                span { class: "lens-field-hint", "Set the endpoint first: the key is kept for it." }
            }
        };
    };
    let is_stored = matches!(&*stored.read(), Some(Ok(Some(true))));
    let asking = stored.read().is_none();
    let read_error = match &*stored.read() {
        Some(Err(reason)) => Some(reason.clone()),
        _ => None,
    };

    rsx! {
        div {
            class: "lens-field",
            span { class: "lens-field-label", "API key" }
            if is_stored && !replacing() {
                div {
                    class: "lens-key-line",
                    span { class: "lens-key-stored", "A key is stored for {for_account}." }
                    button {
                        class: "lens-button",
                        r#type: "button",
                        onclick: move |_| replacing.set(true),
                        "Replace"
                    }
                    button {
                        class: "lens-button lens-button-danger",
                        r#type: "button",
                        onclick: remove,
                        "Remove"
                    }
                }
            } else if !asking {
                div {
                    class: "lens-key-line",
                    input {
                        class: "lens-input monospace",
                        r#type: "password",
                        autocomplete: "off",
                        placeholder: "Paste the key",
                        value: "{draft}",
                        oninput: move |event| draft.set(event.value()),
                    }
                    if !draft().trim().is_empty() {
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: save,
                            "Save"
                        }
                    }
                    if replacing() {
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: move |_| {
                                draft.set(String::new());
                                replacing.set(false);
                            },
                            "Cancel"
                        }
                    }
                }
            }
            span {
                class: "lens-field-hint",
                if overridden {
                    "Not used while an API key command is set below."
                } else {
                    "Kept in the system's credential store — the Keychain on macOS — under the endpoint, never in config.json, and used by every lens that asks the same server. A server that wants no key, such as a local Ollama, needs none."
                }
            }
            if let Some(reason) = failure().or(read_error) {
                span { class: "lens-field-hint lens-field-error", "{reason}" }
            }
        }
    }
}

/// A program and its arguments, one per line.
///
/// The text is kept as typed, apart from what it means: the arguments are
/// trimmed and blank lines dropped, and writing that back into the field on
/// every keystroke would take away the space or the new line being typed.
#[component]
fn ArgvField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: ReadSignal<Vec<String>>,
    rows: u32,
    on_change: EventHandler<Vec<String>>,
) -> Element {
    let mut draft = use_signal(|| value.peek().join("\n"));
    // A value that is not what the draft means came from elsewhere — the
    // file edited by hand while this is open — and replaces the draft;
    // typing on over a stale draft would write that edit away.
    use_effect(move || {
        let value = value();
        if argv(&draft.peek()) != value {
            draft.set(value.join("\n"));
        }
    });

    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            textarea {
                class: "lens-input monospace",
                rows: "{rows}",
                spellcheck: false,
                value: "{draft}",
                oninput: move |event| {
                    let text = event.value();
                    on_change.call(argv(&text));
                    draft.set(text);
                },
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

#[component]
fn NumberField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field lens-field-number",
            span { class: "lens-field-label", "{label}" }
            input {
                class: "lens-input",
                r#type: "number",
                min: "0",
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_read_one_per_line() {
        assert_eq!(
            argv("security\n  find-generic-password \n\n-w\n"),
            ["security", "find-generic-password", "-w"]
        );
    }

    #[test]
    fn the_lens_being_edited_stays_open_wherever_a_move_takes_it() {
        // The dragged lens itself goes where it was dropped.
        assert_eq!(moved_index(1, 1, 3), 3);
        // A lens it passed on the way down moves up one, and on the way up,
        // down one.
        assert_eq!(moved_index(2, 1, 3), 1);
        assert_eq!(moved_index(1, 3, 0), 2);
        // One outside the stretch it crossed stays.
        assert_eq!(moved_index(0, 1, 3), 0);
        assert_eq!(moved_index(4, 1, 3), 4);
    }

    #[test]
    fn a_blank_setting_is_no_setting() {
        assert_eq!(optional("  ".to_string()), None);
        assert_eq!(optional("sonnet".to_string()), Some("sonnet".to_string()));
    }
}
