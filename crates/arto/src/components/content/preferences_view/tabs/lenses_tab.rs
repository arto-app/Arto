use crate::components::icon::{Icon, IconName};
use crate::components::reorder::{drop_side, DragRow};
use crate::config::{
    lens_problems, unused_lens_id, Config, Lens, LensAgent, LensCapability, LensDisplay, LensError,
    MAX_LENS_CONCURRENCY, MAX_LENS_CONTEXT,
};
use dioxus::prelude::*;
use rust_i18n::t;

mod fields;
mod lens_card;
mod recipe_picker;

use lens_card::LensCard;
use recipe_picker::RecipePicker;

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
                {t!("preferences.lenses.lede").to_string()}
            }

            h3 { class: "preference-section-title", {t!("preferences.lenses.lenses_title").to_string()} }

            if lenses.is_empty() {
                p { class: "preference-description lens-empty", {t!("preferences.lenses.empty").to_string()} }
            }

            for (index, (lens, problem)) in lenses.into_iter().zip(problems).enumerate() {
                LensCard {
                    key: "{index}",
                    config,
                    index,
                    lens,
                    problem: problem.map(|problem| problem_text(&problem)),
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
                    span { {t!("preferences.lenses.blank").to_string()} }
                }
            }

            h3 { class: "preference-section-title", {t!("preferences.lenses.recipes.title").to_string()} }
            p {
                class: "preference-description",
                {t!("preferences.lenses.recipes.description").to_string()}
            }
            RecipePicker {
                taken: config.read().lenses.clone(),
                on_add: add,
            }
        }
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
fn display_title(display: LensDisplay) -> String {
    let title = match display {
        LensDisplay::Page => t!("preferences.lenses.display.page.title"),
        LensDisplay::Popover => t!("preferences.lenses.display.popover.title"),
        LensDisplay::Annotate => t!("preferences.lenses.display.annotate.title"),
    };
    title.into_owned()
}

fn display_description(display: LensDisplay) -> String {
    let description = match display {
        LensDisplay::Page => t!("preferences.lenses.display.page.description"),
        LensDisplay::Popover => t!("preferences.lenses.display.popover.description"),
        LensDisplay::Annotate => t!("preferences.lenses.display.annotate.description"),
    };
    description.into_owned()
}

/// How to write the prompt for a lens shown as `display`: how the text
/// reaches the agent, and so what the prompt can call it.
fn prompt_hint(display: LensDisplay) -> String {
    let hint = if display.is_per_block() {
        t!("preferences.lenses.prompt.per_block")
    } else {
        t!("preferences.lenses.prompt.whole")
    };
    hint.into_owned()
}

/// What an agent is called. A product's own name is not translated; a
/// program of the reader's own is.
fn agent_name(agent: Option<LensAgent>) -> String {
    match agent {
        Some(agent) => agent.profile().name.to_string(),
        None => t!("preferences.lenses.agents.command.name").into_owned(),
    }
}

/// What choosing `agent` means, for someone setting a lens up from a recipe.
fn agent_introduction(agent: LensAgent) -> String {
    let introduction = match agent {
        LensAgent::Claude => t!("preferences.lenses.agents.claude.introduction"),
        LensAgent::Codex => t!("preferences.lenses.agents.codex.introduction"),
        LensAgent::Ollama => t!("preferences.lenses.agents.ollama.introduction"),
        LensAgent::Openai => t!("preferences.lenses.agents.openai.introduction"),
    };
    introduction.into_owned()
}

/// What `agent` is, beside the choice in a lens's settings; a program of
/// the reader's own when there is none.
fn agent_description(agent: Option<LensAgent>) -> String {
    let description = match agent {
        Some(LensAgent::Claude) => t!("preferences.lenses.agents.claude.description"),
        Some(LensAgent::Codex) => t!("preferences.lenses.agents.codex.description"),
        Some(LensAgent::Ollama) => t!("preferences.lenses.agents.ollama.description"),
        Some(LensAgent::Openai) => t!("preferences.lenses.agents.openai.description"),
        None => t!("preferences.lenses.agents.command.description"),
    };
    description.into_owned()
}

/// What the endpoint setting of `agent`'s server takes.
fn endpoint_hint(agent: LensAgent) -> String {
    let hint = match agent {
        LensAgent::Ollama => t!("preferences.lenses.agents.ollama.endpoint_hint"),
        LensAgent::Openai => t!("preferences.lenses.agents.openai.endpoint_hint"),
        LensAgent::Claude | LensAgent::Codex => return String::new(),
    };
    hint.into_owned()
}

/// What the preferences call a capability, and what allowing it means.
fn capability_text(capability: LensCapability) -> (String, String) {
    let (label, description) = match capability {
        LensCapability::WebSearch => (
            t!("preferences.lenses.capabilities.web_search.label"),
            t!("preferences.lenses.capabilities.web_search.description"),
        ),
        LensCapability::ReadFiles => (
            t!("preferences.lenses.capabilities.read_files.label"),
            t!("preferences.lenses.capabilities.read_files.description"),
        ),
        LensCapability::Shell => (
            t!("preferences.lenses.capabilities.shell.label"),
            t!("preferences.lenses.capabilities.shell.description"),
        ),
    };
    (label.into_owned(), description.into_owned())
}

/// Why a lens cannot be offered. [`LensError`]'s own message stays English
/// for the CLI; this is the same sentence in the interface's language.
fn problem_text(problem: &LensError) -> String {
    let id = |id: &String| format!("{id:?}");
    let text = match problem {
        LensError::EmptyId => t!("preferences.lenses.problems.empty_id"),
        LensError::DuplicateId(lens) => {
            t!("preferences.lenses.problems.duplicate_id", id = id(lens))
        }
        LensError::NothingToRun(lens) => {
            t!("preferences.lenses.problems.nothing_to_run", id = id(lens))
        }
        LensError::AgentAndCommand(lens) => t!(
            "preferences.lenses.problems.agent_and_command",
            id = id(lens)
        ),
        LensError::MissingModel(lens) => {
            t!("preferences.lenses.problems.missing_model", id = id(lens))
        }
        LensError::NotForAgent(lens, setting) => t!(
            "preferences.lenses.problems.not_for_agent",
            id = id(lens),
            setting = setting
        ),
        LensError::NotAllowed(lens, capability, agent) => t!(
            "preferences.lenses.problems.not_allowed",
            id = id(lens),
            capability = capability,
            agent = agent
        ),
        LensError::UnitWithoutPage(lens) => t!(
            "preferences.lenses.problems.unit_without_page",
            id = id(lens)
        ),
        LensError::PageOnBlock(lens) => {
            t!("preferences.lenses.problems.page_on_block", id = id(lens))
        }
        LensError::Shortcut(lens, reason) => t!(
            "preferences.lenses.problems.shortcut",
            id = id(lens),
            reason = reason
        ),
        LensError::Concurrency(lens) => t!(
            "preferences.lenses.problems.concurrency",
            id = id(lens),
            max = MAX_LENS_CONCURRENCY
        ),
        LensError::Context(lens) => t!(
            "preferences.lenses.problems.context",
            id = id(lens),
            max = MAX_LENS_CONTEXT
        ),
        LensError::Timeout(lens) => t!("preferences.lenses.problems.timeout", id = id(lens)),
    };
    text.into_owned()
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
