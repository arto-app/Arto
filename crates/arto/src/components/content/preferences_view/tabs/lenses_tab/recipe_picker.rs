use dioxus::prelude::*;
use rust_i18n::t;

use super::super::super::form_controls::{ChoiceItem, ChoiceRow};
use super::fields::{ApiKeyField, ModelField, TextField};
use super::{agent_introduction, agent_name, display_title, endpoint_hint, problem_text};
use crate::components::icon::{Icon, IconName};
use crate::config::{lens_problems, unused_lens_id, Lens, LensAgent, LensRecipe, RecipeBlanks};
use crate::lenses::{forget_offered_models, key_account, ModelSource};

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
pub(super) fn RecipePicker(
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
        .and_then(|lens| lens_problems(std::slice::from_ref(lens)).remove(0))
        .map(|problem| problem_text(&problem));
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
                            span { class: "lens-recipe-title", "{recipe_title(option)}" }
                        }
                        span { class: "lens-recipe-desc", "{recipe_description(option)}" }
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
                                label: t!("preferences.lenses.recipes.between.label"),
                                hint: t!("preferences.lenses.recipes.between.hint"),
                                value: filled.languages[0].clone(),
                                on_input: move |text: String| blanks.write().languages[0] = text,
                            }
                            span { class: "lens-language-swap", "aria-hidden": "true", "⇄" }
                            TextField {
                                label: t!("preferences.lenses.recipes.and.label"),
                                hint: t!("preferences.lenses.recipes.and.hint"),
                                value: filled.languages[1].clone(),
                                on_input: move |text: String| blanks.write().languages[1] = text,
                            }
                        }
                    } else {
                        TextField {
                            label: t!("preferences.lenses.recipes.language.label"),
                            hint: t!("preferences.lenses.recipes.language.hint"),
                            value: filled.language.clone(),
                            on_input: move |text: String| blanks.write().language = text,
                        }
                    }
                    if chosen.asks_audience() {
                        TextField {
                            label: t!("preferences.lenses.recipes.audience.label"),
                            hint: t!("preferences.lenses.recipes.audience.hint"),
                            value: filled.audience.clone(),
                            on_input: move |text: String| blanks.write().audience = text,
                        }
                    }
                    ChoiceRow {
                        name: "recipe-agent".to_string(),
                        label: t!("preferences.lenses.asks.label").to_string(),
                        description: Some(agent_introduction(filled.agent)),
                        options: LensAgent::ALL
                            .into_iter()
                            .map(|agent| ChoiceItem { value: agent, label: agent_name(Some(agent)) })
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
                            Some(model) => t!("preferences.lenses.model.recipe_choice", agent = agent_name(Some(filled.agent)), model = model).to_string(),
                            None if filled.agent.is_server() => t!("preferences.lenses.model.required_pick").to_string(),
                            None => t!("preferences.lenses.model.default").to_string(),
                        },
                        value: filled.model.clone(),
                        on_input: move |text: String| blanks.write().model = text,
                    }
                    if filled.agent.server().is_some_and(|server| server.remote) {
                        TextField {
                            label: t!("preferences.lenses.endpoint.label"),
                            hint: endpoint_hint(filled.agent),
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
                        p { class: "lens-card-problem", {t!("preferences.lenses.problem.not_ready", problem = problem).to_string()} }
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
                                span { {t!("preferences.lenses.recipes.add", label = draft.label).to_string()} }
                            }
                        }
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: move |_| recipe.set(None),
                            {t!("preferences.lenses.cancel").to_string()}
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

fn recipe_title(recipe: LensRecipe) -> String {
    let title = match recipe {
        LensRecipe::TranslatePage => t!("preferences.lenses.recipes.translate_page.title"),
        LensRecipe::TranslateBeside => t!("preferences.lenses.recipes.translate_beside.title"),
        LensRecipe::Summarize => t!("preferences.lenses.recipes.summarize.title"),
        LensRecipe::ExplainTerms => t!("preferences.lenses.recipes.explain_terms.title"),
        LensRecipe::Critique => t!("preferences.lenses.recipes.critique.title"),
        LensRecipe::FactCheck => t!("preferences.lenses.recipes.fact_check.title"),
        LensRecipe::ExplainBlock => t!("preferences.lenses.recipes.explain_block.title"),
    };
    title.into_owned()
}

fn recipe_description(recipe: LensRecipe) -> String {
    let description = match recipe {
        LensRecipe::TranslatePage => t!("preferences.lenses.recipes.translate_page.description"),
        LensRecipe::TranslateBeside => {
            t!("preferences.lenses.recipes.translate_beside.description")
        }
        LensRecipe::Summarize => t!("preferences.lenses.recipes.summarize.description"),
        LensRecipe::ExplainTerms => t!("preferences.lenses.recipes.explain_terms.description"),
        LensRecipe::Critique => t!("preferences.lenses.recipes.critique.description"),
        LensRecipe::FactCheck => t!("preferences.lenses.recipes.fact_check.description"),
        LensRecipe::ExplainBlock => t!("preferences.lenses.recipes.explain_block.description"),
    };
    description.into_owned()
}
