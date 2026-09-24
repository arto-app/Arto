//! Recipes: lenses written in advance, whose prompt and display are chosen,
//! leaving the reader only the blanks that differ from reader to reader —
//! the language to answer in, who asks, and on which model.
//!
//! Writing a lens from nothing means knowing how the text reaches the
//! agent, which display suits what it answers, and how to ask a model for
//! nothing when there is nothing to say. A recipe carries that, and the
//! lens it makes is an ordinary one, to be edited like any other.

use crate::{Lens, LensAgent, LensDisplay, NOTHING_TO_ADD};

/// A lens written in advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LensRecipe {
    /// The document translated, in the page's places.
    TranslatePage,
    /// Each block's translation behind a mark beside it, the original left
    /// in place.
    TranslateBeside,
    /// A summary of the document, from the header.
    Summarize,
    /// The terms a block uses that a reader outside its field may not know.
    ExplainTerms,
    /// Claims without support, vague wording and leaps in a block.
    Critique,
    /// One block explained for a reader who is new to it.
    ExplainBlock,
}

/// What a reader fills in to make a lens from a recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeBlanks {
    /// The language the answer is written in, named in English; the
    /// recipes that translate take `languages` instead.
    pub language: String,
    /// The two languages a translation goes between, either way: text in
    /// the first is translated into the second, text in the second into the
    /// first, and text in any other language into the second. English and
    /// Japanese when left empty.
    pub languages: [String; 2],
    /// Who the explanation is for; only [`LensRecipe::ExplainBlock`] asks.
    pub audience: String,
    pub agent: LensAgent,
    /// The model; the recipe's choice for the agent when empty.
    pub model: String,
    /// The server's address; the agent's default when empty.
    pub endpoint: String,
}

impl LensRecipe {
    /// Every recipe, in the order they are offered.
    pub const ALL: [Self; 6] = [
        Self::TranslatePage,
        Self::TranslateBeside,
        Self::Summarize,
        Self::ExplainTerms,
        Self::Critique,
        Self::ExplainBlock,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::TranslatePage => "Translate the page",
            Self::TranslateBeside => "Translation beside each block",
            Self::Summarize => "Summarize",
            Self::ExplainTerms => "Explain the terms",
            Self::Critique => "Critique",
            Self::ExplainBlock => "Explain a block",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::TranslatePage => {
                "The whole document translated either way between two languages, taking the page's places block by block from the top."
            }
            Self::TranslateBeside => {
                "Each block translated either way between two languages, behind a mark in its margin, so the original stays as you read it."
            }
            Self::Summarize => "The gist and the main points of the document, opened from the header.",
            Self::ExplainTerms => {
                "A note beside each block that uses terms, abbreviations or names a newcomer may not know."
            }
            Self::Critique => {
                "A note beside each block with a claim that lacks support, vague wording or a leap in logic."
            }
            Self::ExplainBlock => {
                "The block you pick from the menu, explained in plain words for the reader you name."
            }
        }
    }

    /// The id its lenses are named after.
    pub fn slug(self) -> &'static str {
        match self {
            Self::TranslatePage => "translate",
            Self::TranslateBeside => "translate-beside",
            Self::Summarize => "summarize",
            Self::ExplainTerms => "explain-terms",
            Self::Critique => "critique",
            Self::ExplainBlock => "explain-block",
        }
    }

    pub fn display(self) -> LensDisplay {
        match self {
            Self::TranslatePage => LensDisplay::Page,
            Self::Summarize | Self::ExplainBlock => LensDisplay::Popover,
            Self::TranslateBeside | Self::ExplainTerms | Self::Critique => LensDisplay::Annotate,
        }
    }

    /// Whether it translates, and so asks for two languages rather than one.
    pub fn translates(self) -> bool {
        matches!(self, Self::TranslatePage | Self::TranslateBeside)
    }

    /// Whether it asks who the answer is for.
    pub fn asks_audience(self) -> bool {
        self == Self::ExplainBlock
    }

    /// The model the recipe chooses for `agent`, when it can: `claude`'s
    /// aliases, the stronger one for a whole document; `codex` knows its
    /// own. A server's models are the reader's to name.
    pub fn model(self, agent: LensAgent) -> Option<&'static str> {
        let models = agent.profile().recipe_models?;
        Some(match self {
            Self::TranslatePage => models.document,
            _ => models.block,
        })
    }

    /// The lens `blanks` make of the recipe, called `id`.
    pub fn lens(self, id: impl Into<String>, blanks: &RecipeBlanks) -> Lens {
        let language = blanks.language.trim();
        let language = if language.is_empty() {
            "English"
        } else {
            language
        };
        let audience = blanks.audience.trim();
        let audience = if audience.is_empty() {
            "a newcomer to the subject"
        } else {
            audience
        };
        let [first, second] = &blanks.languages;
        let or = |language: &str, default: &'static str| {
            let language = language.trim();
            if language.is_empty() {
                default.to_string()
            } else {
                language.to_string()
            }
        };
        let (a, b) = (or(first, "English"), or(second, "Japanese"));
        // Each part by the language it is in, so a document that mixes the
        // two comes out in the other throughout.
        let directions = format!("{a} into {b}, {b} into {a}, and any other language into {b}");
        let (label, prompt) = match self {
            Self::TranslatePage => (
                format!("Translate {a} ⇄ {b}"),
                format!(
                    "Translate the text between {a} and {b}, deciding by the language each \
                     part is written in: {directions}. Keep its structure exactly — \
                     frontmatter, headings, lists, tables, alerts, code blocks, math, links, and \
                     the blank lines between blocks — and translate the prose only: leave code, \
                     URLs, identifiers and frontmatter keys as they are. Write the translated \
                     document and nothing else."
                ),
            ),
            Self::TranslateBeside => (
                format!("{a} ⇄ {b} beside each block"),
                format!(
                    "Translate <text> between {a} and {b}, by the language it is written in: \
                     {directions}. Use <before> and <after> only as context and do not \
                     translate them. If <text> is code, answer with exactly {NOTHING_TO_ADD}. \
                     Otherwise write the translation and nothing else."
                ),
            ),
            Self::Summarize => (
                "Summarize".to_string(),
                format!(
                    "Summarize the text in {language}: a one-sentence gist first, then its main \
                     points as a short bulleted list. Write Markdown only."
                ),
            ),
            Self::ExplainTerms => (
                "Explain the terms".to_string(),
                format!(
                    "Find the technical terms, abbreviations and proper nouns in <text> that a \
                     reader outside its field may not know, and explain each in one short \
                     sentence in {language}. Write a Markdown bullet list: each term on its own \
                     line, starting with `- `, in the form `- **term**: explanation` — never \
                     several terms run together on one line. Use <before> and <after> only to \
                     understand the context. Skip terms that are \
                     common knowledge or explained in <text> itself. If no term needs \
                     explaining, answer with exactly {NOTHING_TO_ADD} and nothing else — not a \
                     sentence saying so."
                ),
            ),
            Self::Critique => (
                "Critique".to_string(),
                format!(
                    "Read <text> as a careful reviewer. Point out, in {language} and in at most \
                     three short bullet points, claims without support, vague wording, leaps in \
                     logic or likely mistakes in <text>. Use <before> and <after> only as \
                     context. If <text> has no real problem, answer with exactly \
                     {NOTHING_TO_ADD} and nothing else — do not invent a problem, and do not \
                     write a sentence saying there is none."
                ),
            ),
            Self::ExplainBlock => (
                format!("Explain for {audience}"),
                format!(
                    "Explain the text in {language} for {audience}: what it says, and what it \
                     assumes the reader already knows. Keep it short and plain. Write Markdown \
                     only."
                ),
            ),
        };

        let mut lens = Lens::new(id);
        lens.label = label;
        lens.set_display(self.display());
        lens.set_agent(Some(blanks.agent));
        lens.prompt = Some(prompt);
        let model = blanks.model.trim();
        lens.model = if model.is_empty() {
            self.model(blanks.agent).map(str::to_string)
        } else {
            Some(model.to_string())
        };
        let endpoint = blanks.endpoint.trim();
        if blanks.agent.is_server() && !endpoint.is_empty() {
            lens.endpoint = Some(endpoint.to_string());
        }
        if self == Self::TranslateBeside {
            // The neighbours only help a translation along; a small local
            // model is quicker and steadier with fewer of them.
            lens.context = 1;
        }
        lens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lens_problems;

    fn blanks(agent: LensAgent, model: &str) -> RecipeBlanks {
        RecipeBlanks {
            language: "Japanese".to_string(),
            languages: ["French".to_string(), "German".to_string()],
            audience: String::new(),
            agent,
            model: model.to_string(),
            endpoint: String::new(),
        }
    }

    #[test]
    fn every_recipe_makes_a_lens_ready_to_offer_with_every_agent() {
        for recipe in LensRecipe::ALL {
            for (agent, model) in [
                (LensAgent::Claude, ""),
                (LensAgent::Codex, ""),
                (LensAgent::Ollama, "qwen3:4b-instruct"),
                (LensAgent::Openai, "gpt-5-mini"),
            ] {
                let lens = recipe.lens(recipe.slug(), &blanks(agent, model));

                assert_eq!(
                    lens_problems(std::slice::from_ref(&lens)),
                    [None],
                    "{lens:?}"
                );
                assert_eq!(lens.display, recipe.display());
            }
        }
    }

    #[test]
    fn the_answer_is_asked_for_in_the_language_filled_in() {
        for recipe in LensRecipe::ALL
            .into_iter()
            .filter(|recipe| !recipe.translates())
        {
            let lens = recipe.lens("l", &blanks(LensAgent::Claude, ""));

            assert!(lens.prompt.unwrap().contains("Japanese"), "{recipe:?}");
        }
    }

    #[test]
    fn every_note_beside_a_block_is_asked_for_the_nothing_mark_when_there_is_none() {
        for recipe in LensRecipe::ALL
            .into_iter()
            .filter(|recipe| recipe.display() == LensDisplay::Annotate)
        {
            let lens = recipe.lens("l", &blanks(LensAgent::Claude, ""));

            assert!(lens.prompt.unwrap().contains(NOTHING_TO_ADD), "{recipe:?}");
        }
    }

    #[test]
    fn terms_are_asked_for_as_a_markdown_list_a_line_each() {
        let prompt = LensRecipe::ExplainTerms
            .lens("l", &blanks(LensAgent::Claude, ""))
            .prompt
            .unwrap();

        assert!(prompt.contains("- **term**: explanation"), "{prompt}");
        assert!(prompt.contains("its own line"), "{prompt}");
    }

    #[test]
    fn a_translation_goes_both_ways_between_the_two_languages() {
        for recipe in [LensRecipe::TranslatePage, LensRecipe::TranslateBeside] {
            assert!(recipe.translates());
            let lens = recipe.lens("l", &blanks(LensAgent::Claude, ""));
            let prompt = lens.prompt.unwrap();

            assert!(prompt.contains("French into German"), "{prompt}");
            assert!(prompt.contains("German into French"), "{prompt}");
            assert!(
                prompt.contains("any other language into German"),
                "{prompt}"
            );
            assert!(!prompt.contains("Japanese"), "{prompt}");
            assert!(lens.label.contains("French ⇄ German"), "{}", lens.label);
        }
    }

    #[test]
    fn a_translation_left_blank_is_between_english_and_japanese() {
        let lens = LensRecipe::TranslatePage.lens(
            "l",
            &RecipeBlanks {
                languages: [String::new(), " ".to_string()],
                ..blanks(LensAgent::Claude, "")
            },
        );

        assert_eq!(lens.label, "Translate English ⇄ Japanese");
    }

    #[test]
    fn a_model_filled_in_wins_over_the_recipes_own() {
        let recipe = LensRecipe::TranslatePage;

        assert_eq!(
            recipe
                .lens("l", &blanks(LensAgent::Claude, ""))
                .model
                .as_deref(),
            Some("sonnet")
        );
        assert_eq!(
            recipe
                .lens("l", &blanks(LensAgent::Claude, "opus"))
                .model
                .as_deref(),
            Some("opus")
        );
        assert_eq!(recipe.lens("l", &blanks(LensAgent::Codex, "")).model, None);
    }

    #[test]
    fn a_server_recipe_is_left_without_a_model_until_one_is_filled_in() {
        let lens = LensRecipe::Summarize.lens("l", &blanks(LensAgent::Ollama, ""));

        assert!(lens_problems(&[lens])[0].is_some());
    }

    #[test]
    fn an_explanation_is_written_for_the_audience_named() {
        let lens = LensRecipe::ExplainBlock.lens(
            "l",
            &RecipeBlanks {
                audience: "a product manager".to_string(),
                ..blanks(LensAgent::Claude, "")
            },
        );

        assert_eq!(lens.label, "Explain for a product manager");
        assert!(lens.prompt.unwrap().contains("for a product manager"));
    }

    #[test]
    fn an_endpoint_is_kept_only_for_a_server() {
        let mut filled = blanks(LensAgent::Openai, "m");
        filled.endpoint = "http://localhost:1234/v1".to_string();
        let server = LensRecipe::Summarize.lens("l", &filled);
        filled.agent = LensAgent::Claude;
        let program = LensRecipe::Summarize.lens("l", &filled);

        assert_eq!(server.endpoint.as_deref(), Some("http://localhost:1234/v1"));
        assert_eq!(program.endpoint, None);
    }
}
