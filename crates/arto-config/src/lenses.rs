use crate::{LensAgent, LensCapability};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;

/// How much of the document a lens hands its command, and where the answer
/// is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LensDisplay {
    /// One run per block, with its neighbours as context; each answer is
    /// kept beside its block and shown on hover — an explanation, a gloss.
    Annotate,
    /// One run over the document or the block, and one answer, shown on
    /// request — a summary.
    Popover,
    /// One run over the whole document, whose answer is a document of its
    /// own, shown in place of the page as it arrives — a translation.
    Page,
}

/// How much of the document one run of a `page` lens is given.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LensUnit {
    /// The whole document in one run, which a general model translates
    /// best: it sees every part of what it is translating.
    #[default]
    Document,
    /// Each of the document's top-level blocks in a run of its own, which
    /// is what a model trained on sentences and paragraphs — a small
    /// translation model — handles: handed a whole document, it drops and
    /// merges blocks.
    Block,
}

impl LensUnit {
    fn is_document(&self) -> bool {
        *self == Self::Document
    }
}

/// What a lens is offered to look at: some questions are about one block —
/// explain this paragraph — some only make sense of a whole — summarize it —
/// and some fit either.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LensTarget {
    /// The block a right-click marks, or the keyboard cursor is on.
    Block,
    /// The whole document.
    Document,
    #[default]
    Either,
}

impl LensTarget {
    /// Every target, in the order the preferences offer them.
    pub const ALL: [LensTarget; 3] = [Self::Either, Self::Block, Self::Document];

    fn is_either(&self) -> bool {
        *self == Self::Either
    }
}

impl LensDisplay {
    /// Whether the command runs once per block rather than once for all.
    pub fn is_per_block(self) -> bool {
        self == Self::Annotate
    }
}

/// What an `annotate` lens answers when it has nothing to add about a block.
///
/// A model asked to "write nothing" rarely writes nothing: it says so in a
/// sentence, and that sentence would mark the block like any other note. A
/// mark of its own is easy for a model to write exactly, and written in the
/// same shape as `<text>` it reads as part of the protocol, not as prose.
pub const NOTHING_TO_ADD: &str = "<nothing/>";

/// Whether `answer` says there is nothing to add: it is empty, or it is
/// [`NOTHING_TO_ADD`] alone, allowing for the spacing, case and code quotes
/// a model may wrap it in.
pub fn says_nothing(answer: &str) -> bool {
    let answer = answer.trim().trim_matches('`').trim();
    let mark: String = answer.chars().filter(|c| !c.is_whitespace()).collect();
    answer.is_empty() || mark.eq_ignore_ascii_case(NOTHING_TO_ADD)
}

/// The most neighbours on each side a per-block run is handed.
pub const MAX_LENS_CONTEXT: usize = 20;
/// The most runs of a per-block lens in flight at once.
pub const MAX_LENS_CONCURRENCY: usize = 16;

fn default_context() -> usize {
    2
}

fn default_concurrency() -> usize {
    4
}

fn default_timeout_seconds() -> u64 {
    300
}

/// A command the reader configured to look at a document through: its
/// answer is shown with the document, which itself is never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lens {
    /// Unique among the lenses; also what cached answers are filed under.
    pub id: String,
    /// What the menus call it.
    pub label: String,
    pub display: LensDisplay,
    /// The agent to run, which Arto knows how to call and to read; without
    /// one, `command` is run instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<LensAgent>,
    /// The model the agent uses; its own default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// What the agent is asked to do with the text it is handed. Without one
    /// the agent is handed the text alone, which is what a model trained for
    /// a single task — a translation model — expects: it reads any
    /// instruction as more text to work on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Where the agent's program is, when it is not found on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<PathBuf>,
    /// The system prompt a server is given with every request. A model
    /// trained for a single task may read its settings from it — the language
    /// to translate into, say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The server's base URL. Ollama: its address, `http://127.0.0.1:11434`
    /// when absent. OpenAI: the part before `/chat/completions`, such as
    /// `https://api.openai.com/v1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Ollama: the context the model is loaded with, in tokens; sized to
    /// each request when absent. Ollama otherwise loads a model with the
    /// longest context it takes, which for some models means tens of
    /// gigabytes of memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    /// A program and its arguments that print the server's API key — a
    /// password manager — in place of the key Arto keeps in the system's
    /// credential store for the endpoint. Neither is written into the
    /// configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_key_command: Vec<String>,
    /// What the agent may do beyond reading the text it is handed; nothing
    /// when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<LensCapability>,
    /// Program and arguments of a command that is not a known agent, run as
    /// they are, without a shell.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// How many blocks on each side a per-block run is handed as context.
    #[serde(default = "default_context")]
    pub context: usize,
    /// How many per-block runs may be in flight at once.
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    /// How long one run may take before it counts as failed.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    /// How a `page` lens hands the document over.
    #[serde(default, skip_serializing_if = "LensUnit::is_document")]
    pub unit: LensUnit,
    /// What the lens is offered to look at. A `page` lens takes the
    /// document's places, so it looks at a whole document whatever this
    /// says, and cannot be set to a block.
    #[serde(default, skip_serializing_if = "LensTarget::is_either")]
    pub on: LensTarget,
    /// The keys that show or hide the lens over the document, written as the
    /// keybindings are: `Cmd+Shift+t`, or `g t` for one chord after another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

/// Why a configured lens cannot be offered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LensError {
    #[error("a lens has an empty id")]
    EmptyId,
    #[error("lens {0:?} is defined more than once")]
    DuplicateId(String),
    #[error("lens {0:?} has neither an agent nor a command to run")]
    NothingToRun(String),
    #[error("lens {0:?} has both an agent and a command; it can run one")]
    AgentAndCommand(String),
    #[error("lens {0:?} asks a server, which needs a model")]
    MissingModel(String),
    #[error("lens {0:?} sets {1}, which only a server agent uses")]
    NotForAgent(String, &'static str),
    #[error("lens {0:?} allows {1}, which {2} cannot be given")]
    NotAllowed(String, LensCapability, &'static str),
    #[error("lens {0:?} hands the document over by block, which only a page lens does")]
    UnitWithoutPage(String),
    #[error("lens {0:?} is a page lens, which looks at a whole document, not a block")]
    PageOnBlock(String),
    #[error("lens {0:?} has a shortcut that cannot be read: {1}")]
    Shortcut(String, String),
    #[error("lens {0:?} must allow between 1 and {MAX_LENS_CONCURRENCY} runs at a time")]
    Concurrency(String),
    #[error("lens {0:?} may hand at most {MAX_LENS_CONTEXT} neighbours on each side")]
    Context(String),
    #[error("lens {0:?} must allow its command at least a second")]
    Timeout(String),
}

/// The lenses that can be offered, and what is wrong with the others.
///
/// A lens with a problem is left out on its own, so one mistake in the list
/// does not take the rest away. Every definition of a repeated id is left
/// out, since which one was meant cannot be told.
pub fn usable_lenses(lenses: &[Lens]) -> (Vec<&Lens>, Vec<LensError>) {
    let repeated = repeated_ids(lenses);
    let mut usable = Vec::new();
    let mut errors: Vec<LensError> = repeated
        .iter()
        .filter(|id| !id.is_empty())
        .map(|id| LensError::DuplicateId(id.to_string()))
        .collect();
    errors.sort_by_key(|error| error.to_string());
    for lens in lenses {
        if !lens.id.is_empty() && repeated.contains(lens.id.as_str()) {
            continue;
        }
        match own_problem(lens) {
            Some(error) => errors.push(error),
            None => usable.push(lens),
        }
    }
    (usable, errors)
}

/// What keeps each of `lenses` from being offered, lens by lens: the same
/// judgement as [`usable_lenses`], told to the lens it is about, so that an
/// editor can show it beside that lens.
pub fn lens_problems(lenses: &[Lens]) -> Vec<Option<LensError>> {
    let repeated = repeated_ids(lenses);
    lenses
        .iter()
        .map(|lens| {
            if !lens.id.is_empty() && repeated.contains(lens.id.as_str()) {
                Some(LensError::DuplicateId(lens.id.clone()))
            } else {
                own_problem(lens)
            }
        })
        .collect()
}

/// An id no lens in `lenses` has: `base`, or `base-2`, `base-3` and on.
pub fn unused_lens_id(lenses: &[Lens], base: &str) -> String {
    let taken: HashSet<&str> = lenses.iter().map(|lens| lens.id.as_str()).collect();
    std::iter::once(base.to_string())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|id| !taken.contains(id.as_str()))
        .expect("an unbounded sequence has an unused id")
}

fn repeated_ids(lenses: &[Lens]) -> HashSet<&str> {
    let mut seen = HashSet::new();
    lenses
        .iter()
        .filter(|lens| !seen.insert(lens.id.as_str()))
        .map(|lens| lens.id.as_str())
        .collect()
}

/// What is wrong with `lens` on its own, apart from sharing its id.
fn own_problem(lens: &Lens) -> Option<LensError> {
    let id = || lens.id.clone();
    if lens.id.is_empty() {
        Some(LensError::EmptyId)
    } else if let Some(error) = runner_error(lens) {
        Some(error)
    } else if !(1..=MAX_LENS_CONCURRENCY).contains(&lens.concurrency) {
        Some(LensError::Concurrency(id()))
    } else if lens.context > MAX_LENS_CONTEXT {
        Some(LensError::Context(id()))
    } else if lens.timeout_seconds == 0 {
        Some(LensError::Timeout(id()))
    } else if lens.unit == LensUnit::Block && lens.display != LensDisplay::Page {
        Some(LensError::UnitWithoutPage(id()))
    } else if lens.on == LensTarget::Block && lens.display == LensDisplay::Page {
        Some(LensError::PageOnBlock(id()))
    } else if let Some(Err(error)) = lens
        .shortcut
        .as_deref()
        .map(arto_keybindings::ShortcutSequence::from_str)
    {
        Some(LensError::Shortcut(id(), error.to_string()))
    } else {
        None
    }
}

impl Lens {
    /// A lens with nothing but its id chosen: a summary by `claude`, asked
    /// for from the header — usable as it is, and a start to edit from.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: "New lens".to_string(),
            display: LensDisplay::Popover,
            agent: Some(LensAgent::Claude),
            model: None,
            prompt: Some("Summarize the text in a few sentences.".to_string()),
            program: None,
            system: None,
            endpoint: None,
            context_length: None,
            api_key_command: Vec::new(),
            allow: Vec::new(),
            command: Vec::new(),
            context: default_context(),
            concurrency: default_concurrency(),
            timeout_seconds: default_timeout_seconds(),
            unit: LensUnit::default(),
            on: LensTarget::default(),
            shortcut: None,
        }
    }

    /// Run `agent`, or the `command` when it is `None`, dropping what only
    /// the one it ran before read — so switching does not leave the lens
    /// with a setting its new runner would refuse it over.
    pub fn set_agent(&mut self, agent: Option<LensAgent>) {
        let changed = self.agent != agent;
        self.agent = agent;
        let server = agent.is_some_and(LensAgent::is_server);
        if agent.is_some() {
            self.command.clear();
        } else {
            self.model = None;
            self.prompt = None;
        }
        // Where one agent's program is says nothing of another's.
        if server || agent.is_none() || changed {
            self.program = None;
        }
        if !server {
            self.system = None;
            self.endpoint = None;
            self.api_key_command.clear();
        }
        if !agent
            .and_then(LensAgent::server)
            .is_some_and(|server| server.context_length)
        {
            self.context_length = None;
        }
        // What was allowed one agent is not carried to another: an agent's
        // tools reach differently, and the reader allowed the one they saw.
        if changed {
            self.allow.clear();
        }
    }

    /// Whether the agent may reach what is on this machine beyond the
    /// document.
    pub fn reaches_local(&self) -> bool {
        self.allow.iter().any(|capability| capability.is_local())
    }

    /// Show the answer as `display` says, dropping the unit when the lens
    /// no longer takes the page's places, and a block to look at when it
    /// now does.
    pub fn set_display(&mut self, display: LensDisplay) {
        self.display = display;
        if display != LensDisplay::Page {
            self.unit = LensUnit::Document;
        } else if self.on == LensTarget::Block {
            self.on = LensTarget::Either;
        }
    }

    /// Whether the lens is offered over one block.
    pub fn on_block(&self) -> bool {
        self.display != LensDisplay::Page && self.on != LensTarget::Document
    }

    /// Whether the lens is offered over the whole document.
    pub fn on_document(&self) -> bool {
        self.on != LensTarget::Block
    }
}

/// What is wrong with how `lens` says to run its command, if anything.
fn runner_error(lens: &Lens) -> Option<LensError> {
    let id = || lens.id.clone();
    let has_command = lens
        .command
        .first()
        .is_some_and(|program| !program.is_empty());
    let server_only = [
        ("system", lens.system.is_some()),
        ("endpoint", lens.endpoint.is_some()),
        ("apiKeyCommand", !lens.api_key_command.is_empty()),
        ("contextLength", lens.context_length.is_some()),
    ];
    let misplaced = server_only
        .iter()
        .find(|(_, set)| *set)
        .map(|(field, _)| *field);
    let offered = lens
        .agent
        .map_or(&[][..], |agent| agent.profile().capabilities);
    if let Some(&capability) = lens
        .allow
        .iter()
        .find(|capability| !offered.contains(capability))
    {
        let runner = lens.agent.map_or("a command", |agent| agent.profile().name);
        return Some(LensError::NotAllowed(id(), capability, runner));
    }
    let server = lens.agent.and_then(LensAgent::server);
    match (lens.agent, server) {
        (None, _) if !has_command => Some(LensError::NothingToRun(id())),
        (Some(_), _) if !lens.command.is_empty() => Some(LensError::AgentAndCommand(id())),
        (_, Some(_)) if lens.model.as_deref().is_none_or(str::is_empty) => {
            Some(LensError::MissingModel(id()))
        }
        (_, Some(server)) if !server.context_length && lens.context_length.is_some() => {
            Some(LensError::NotForAgent(id(), "contextLength"))
        }
        (_, Some(_)) => None,
        _ => misplaced.map(|field| LensError::NotForAgent(id(), field)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lens(id: &str, command: &[&str]) -> Lens {
        Lens {
            id: id.to_string(),
            label: id.to_string(),
            display: LensDisplay::Page,
            agent: None,
            model: None,
            prompt: None,
            program: None,
            system: None,
            endpoint: None,
            api_key_command: Vec::new(),
            allow: Vec::new(),
            context_length: None,
            command: command.iter().map(|arg| arg.to_string()).collect(),
            context: 2,
            concurrency: 4,
            timeout_seconds: 300,
            unit: Default::default(),
            on: Default::default(),
            shortcut: None,
        }
    }

    fn agent(id: &str, agent: LensAgent, model: Option<&str>, prompt: Option<&str>) -> Lens {
        Lens {
            agent: Some(agent),
            model: model.map(str::to_string),
            prompt: prompt.map(str::to_string),
            ..lens(id, &[])
        }
    }

    #[test]
    fn a_command_lens_reads_with_its_defaults() {
        let parsed: Lens = serde_json::from_str(
            r#"{"id": "summarize", "label": "Summarize", "display": "popover", "command": ["llm", "-s", "Summarize"]}"#,
        )
        .unwrap();

        assert_eq!(parsed.display, LensDisplay::Popover);
        assert_eq!(parsed.command, ["llm", "-s", "Summarize"]);
        assert_eq!(parsed.agent, None);
        assert_eq!(parsed.context, 2);
        assert_eq!(parsed.concurrency, 4);
        assert_eq!(parsed.timeout_seconds, 300);
    }

    #[test]
    fn an_agent_lens_reads_without_a_command() {
        let parsed: Lens = serde_json::from_str(
            r#"{"id": "t", "label": "T", "display": "page", "agent": "claude", "model": "sonnet", "prompt": "Translate."}"#,
        )
        .unwrap();

        assert_eq!(parsed.agent, Some(LensAgent::Claude));
        assert_eq!(parsed.model.as_deref(), Some("sonnet"));
        assert!(parsed.command.is_empty());
        let json = serde_json::to_value(&parsed).unwrap();
        assert!(json.get("command").is_none(), "{json}");
        assert!(json.get("program").is_none(), "{json}");
    }

    #[test]
    fn a_lens_round_trips_in_camel_case() {
        let json = serde_json::to_value(lens("translate", &["claude", "-p"])).unwrap();

        assert_eq!(json["timeoutSeconds"], 300);
        assert_eq!(json["display"], "page");
        assert_eq!(
            serde_json::from_value::<Lens>(json).unwrap(),
            lens("translate", &["claude", "-p"])
        );
    }

    #[test]
    fn every_display_reads_and_only_annotate_runs_per_block() {
        for (name, display) in [
            ("annotate", LensDisplay::Annotate),
            ("popover", LensDisplay::Popover),
            ("page", LensDisplay::Page),
        ] {
            let parsed: LensDisplay = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(parsed, display);
            assert_eq!(parsed.is_per_block(), name == "annotate");
        }
        assert!(serde_json::from_str::<LensDisplay>(r#""replace""#).is_err());
        assert!(serde_json::from_str::<LensAgent>(r#""gpt""#).is_err());
    }

    #[test]
    fn a_broken_lens_is_left_out_on_its_own() {
        let with = |mut lens: Lens, change: fn(&mut Lens)| {
            change(&mut lens);
            lens
        };
        let lenses = [
            lens("good", &["x"]),
            agent("agent", LensAgent::Claude, None, Some("Translate.")),
            lens("", &["x"]),
            lens("empty", &[]),
            lens("blank", &[""]),
            with(agent("both", LensAgent::Codex, None, Some("Go.")), |lens| {
                lens.command = vec!["x".to_string()]
            }),
            with(
                agent("server", LensAgent::Openai, Some("m"), None),
                |lens| lens.endpoint = Some("http://localhost:11434/v1".to_string()),
            ),
            agent("endless", LensAgent::Openai, Some("m"), None),
            agent("local", LensAgent::Ollama, Some("qwen3:4b-instruct"), None),
            agent("unnamed", LensAgent::Ollama, None, None),
            with(agent("sized", LensAgent::Openai, Some("m"), None), |lens| {
                lens.endpoint = Some("http://localhost:1234/v1".to_string());
                lens.context_length = Some(8192);
            }),
            with(agent("modelless", LensAgent::Openai, None, None), |lens| {
                lens.endpoint = Some("http://localhost:11434/v1".to_string())
            }),
            with(agent("stray", LensAgent::Claude, None, None), |lens| {
                lens.endpoint = Some("http://localhost".to_string())
            }),
            with(lens("idle", &["x"]), |lens| lens.concurrency = 0),
            with(lens("swarm", &["x"]), |lens| lens.concurrency = 10_000),
            with(lens("nosy", &["x"]), |lens| lens.context = usize::MAX),
            with(lens("hasty", &["x"]), |lens| lens.timeout_seconds = 0),
        ];

        let (usable, errors) = usable_lenses(&lenses);

        assert_eq!(
            usable
                .iter()
                .map(|lens| lens.id.as_str())
                .collect::<Vec<_>>(),
            ["good", "agent", "server", "endless", "local"]
        );
        assert_eq!(
            errors,
            [
                LensError::EmptyId,
                LensError::NothingToRun("empty".to_string()),
                LensError::NothingToRun("blank".to_string()),
                LensError::AgentAndCommand("both".to_string()),
                LensError::MissingModel("unnamed".to_string()),
                LensError::NotForAgent("sized".to_string(), "contextLength"),
                LensError::MissingModel("modelless".to_string()),
                LensError::NotForAgent("stray".to_string(), "endpoint"),
                LensError::Concurrency("idle".to_string()),
                LensError::Concurrency("swarm".to_string()),
                LensError::Context("nosy".to_string()),
                LensError::Timeout("hasty".to_string()),
            ]
        );
    }

    #[test]
    fn a_lens_allows_only_what_its_agent_can_be_given() {
        let allowing = |mut lens: Lens, allow: &[LensCapability]| {
            lens.allow = allow.to_vec();
            lens
        };
        let lenses = [
            allowing(
                agent("checker", LensAgent::Claude, None, Some("Check.")),
                &[LensCapability::WebSearch],
            ),
            allowing(
                agent("digger", LensAgent::Codex, None, Some("Look.")),
                &[LensCapability::ReadFiles, LensCapability::Shell],
            ),
            allowing(
                agent("local", LensAgent::Ollama, Some("m"), None),
                &[LensCapability::WebSearch],
            ),
            allowing(lens("own", &["x"]), &[LensCapability::Shell]),
        ];

        let (usable, errors) = usable_lenses(&lenses);

        assert_eq!(
            usable
                .iter()
                .map(|lens| lens.id.as_str())
                .collect::<Vec<_>>(),
            ["checker", "digger"]
        );
        assert_eq!(
            errors,
            [
                LensError::NotAllowed("local".to_string(), LensCapability::WebSearch, "Ollama"),
                LensError::NotAllowed("own".to_string(), LensCapability::Shell, "a command"),
            ]
        );
        assert!(!lenses[0].reaches_local());
        assert!(lenses[1].reaches_local());
    }

    #[test]
    fn what_a_lens_allows_is_read_and_written_by_name_and_left_out_when_empty() {
        let parsed: Lens = serde_json::from_str(
            r#"{"id": "t", "label": "T", "display": "annotate", "agent": "claude", "allow": ["webSearch", "readFiles", "shell"]}"#,
        )
        .unwrap();
        assert_eq!(parsed.allow, LensCapability::ALL);
        let written = serde_json::to_value(&parsed).unwrap();
        assert_eq!(
            written["allow"],
            serde_json::json!(["webSearch", "readFiles", "shell"])
        );

        let bare = serde_json::to_value(Lens::new("b")).unwrap();
        assert!(bare.get("allow").is_none(), "{bare}");
    }

    #[test]
    fn what_was_allowed_is_not_carried_to_another_agent() {
        let mut lens = Lens::new("a");
        lens.allow = vec![LensCapability::WebSearch];
        lens.set_agent(Some(LensAgent::Claude));
        assert_eq!(lens.allow, [LensCapability::WebSearch], "the same agent");
        lens.set_agent(Some(LensAgent::Codex));
        assert!(lens.allow.is_empty());
    }

    #[test]
    fn a_lens_is_offered_over_what_it_is_on() {
        let mut lens = Lens::new("a");
        assert!(lens.on_block() && lens.on_document(), "either by default");

        lens.on = LensTarget::Block;
        assert!(lens.on_block() && !lens.on_document());
        lens.on = LensTarget::Document;
        assert!(!lens.on_block() && lens.on_document());

        lens.on = LensTarget::Either;
        lens.set_display(LensDisplay::Page);
        assert!(
            !lens.on_block() && lens.on_document(),
            "a page lens looks at a whole"
        );
    }

    #[test]
    fn a_page_lens_on_a_block_is_left_out_and_becoming_one_lets_go_of_the_block() {
        let mut lens = Lens::new("page");
        lens.on = LensTarget::Block;
        lens.display = LensDisplay::Page;
        assert_eq!(
            lens_problems(std::slice::from_ref(&lens)),
            [Some(LensError::PageOnBlock("page".to_string()))]
        );

        let mut lens = Lens::new("popover");
        lens.on = LensTarget::Block;
        lens.set_display(LensDisplay::Page);
        assert_eq!(lens.on, LensTarget::Either);
        assert_eq!(lens_problems(std::slice::from_ref(&lens)), [None]);
    }

    #[test]
    fn what_a_lens_is_on_is_read_by_name_and_written_only_when_it_is_not_either() {
        let parsed: Lens = serde_json::from_str(
            r#"{"id": "t", "label": "T", "display": "popover", "agent": "claude", "on": "block"}"#,
        )
        .unwrap();
        assert_eq!(parsed.on, LensTarget::Block);
        assert_eq!(serde_json::to_value(&parsed).unwrap()["on"], "block");

        let bare = serde_json::to_value(Lens::new("b")).unwrap();
        assert!(bare.get("on").is_none(), "{bare}");
    }

    #[test]
    fn a_page_lens_reads_its_unit_and_writes_it_only_when_it_is_not_the_default() {
        let parsed: Lens = serde_json::from_str(
            r#"{"id": "t", "label": "T", "display": "page", "agent": "openai", "endpoint": "http://localhost:11434/v1", "model": "m", "unit": "block"}"#,
        )
        .unwrap();
        assert_eq!(parsed.unit, LensUnit::Block);
        assert_eq!(serde_json::to_value(&parsed).unwrap()["unit"], "block");

        let document = lens("d", &["x"]);
        assert_eq!(document.unit, LensUnit::Document);
        assert!(serde_json::to_value(&document)
            .unwrap()
            .get("unit")
            .is_none());
    }

    #[test]
    fn only_a_page_lens_goes_by_block() {
        let mut popover = lens("popover", &["x"]);
        popover.display = LensDisplay::Popover;
        popover.unit = LensUnit::Block;
        let mut page = lens("page", &["x"]);
        page.unit = LensUnit::Block;

        let lenses = [popover, page];
        let (usable, errors) = usable_lenses(&lenses);

        assert_eq!(
            usable
                .iter()
                .map(|lens| lens.id.as_str())
                .collect::<Vec<_>>(),
            ["page"]
        );
        assert_eq!(errors, [LensError::UnitWithoutPage("popover".to_string())]);
    }

    #[test]
    fn every_definition_of_a_repeated_id_is_left_out() {
        let lenses = [
            lens("twice", &["a"]),
            lens("once", &["x"]),
            lens("twice", &["b"]),
        ];

        let (usable, errors) = usable_lenses(&lenses);

        assert_eq!(
            usable
                .iter()
                .map(|lens| lens.id.as_str())
                .collect::<Vec<_>>(),
            ["once"]
        );
        assert_eq!(errors, [LensError::DuplicateId("twice".to_string())]);
    }

    #[test]
    fn each_lens_is_told_its_own_problem() {
        let lenses = [
            lens("fine", &["cat"]),
            lens("", &["cat"]),
            lens("twice", &["cat"]),
            lens("twice", &["cat"]),
            lens("idle", &[]),
        ];

        assert_eq!(
            lens_problems(&lenses),
            [
                None,
                Some(LensError::EmptyId),
                Some(LensError::DuplicateId("twice".to_string())),
                Some(LensError::DuplicateId("twice".to_string())),
                Some(LensError::NothingToRun("idle".to_string())),
            ]
        );
    }

    #[test]
    fn a_new_lens_is_usable_as_it_is_made() {
        let lens = Lens::new("lens");

        assert_eq!(lens_problems(std::slice::from_ref(&lens)), [None]);
        assert_eq!(lens.context, default_context());
        assert_eq!(lens.timeout_seconds, default_timeout_seconds());
    }

    #[test]
    fn a_new_id_is_one_no_lens_has() {
        let lenses = [Lens::new("lens"), Lens::new("lens-2")];

        assert_eq!(unused_lens_id(&[], "lens"), "lens");
        assert_eq!(unused_lens_id(&lenses, "lens"), "lens-3");
        assert_eq!(unused_lens_id(&lenses, "translate"), "translate");
    }

    #[test]
    fn changing_the_agent_drops_what_only_the_old_one_read() {
        let mut lens = Lens {
            system: Some("be brief".to_string()),
            endpoint: Some("http://127.0.0.1:11434".to_string()),
            context_length: Some(8192),
            api_key_command: vec!["pass".to_string()],
            ..agent("server", LensAgent::Ollama, Some("qwen"), Some("p"))
        };

        lens.set_agent(Some(LensAgent::Openai));
        assert_eq!(lens.context_length, None, "only Ollama sizes the context");
        assert_eq!(lens.system.as_deref(), Some("be brief"));

        lens.set_agent(Some(LensAgent::Claude));
        assert_eq!(
            (&lens.system, &lens.endpoint, lens.api_key_command.len()),
            (&None, &None, 0)
        );
        assert_eq!(
            lens.model.as_deref(),
            Some("qwen"),
            "every agent takes a model"
        );

        lens.program = Some(PathBuf::from("/opt/claude"));
        lens.set_agent(Some(LensAgent::Codex));
        assert_eq!(lens.program, None, "claude's program is not codex's");
        lens.program = Some(PathBuf::from("/opt/codex"));
        lens.set_agent(None);
        assert_eq!(
            (&lens.program, &lens.model, &lens.prompt),
            (&None, &None, &None)
        );
        assert_eq!(
            lens_problems(&[lens.clone()]),
            [Some(LensError::NothingToRun("server".to_string()))]
        );

        lens.command = vec!["cat".to_string()];
        lens.set_agent(Some(LensAgent::Codex));
        assert!(
            lens.command.is_empty(),
            "an agent runs instead of the command"
        );
    }

    #[test]
    fn a_shortcut_is_read_as_the_keybindings_are() {
        let with = |shortcut: &str| Lens {
            shortcut: Some(shortcut.to_string()),
            ..Lens::new("l")
        };

        assert_eq!(lens_problems(&[with("Cmd+Shift+t")]), [None]);
        assert_eq!(lens_problems(&[with("g t")]), [None]);
        assert!(matches!(
            lens_problems(&[with("Cmd+")])[0],
            Some(LensError::Shortcut(_, _))
        ));
    }

    #[test]
    fn an_answer_of_the_nothing_mark_alone_says_nothing() {
        assert!(says_nothing(""));
        assert!(says_nothing("  \n"));
        assert!(says_nothing(NOTHING_TO_ADD));
        assert!(says_nothing("\n<nothing/>\n"));
        assert!(says_nothing("`<nothing/>`"));
        assert!(says_nothing("<NOTHING />"));

        assert!(!says_nothing("Nothing to note."));
        assert!(!says_nothing("<nothing/> but the second line is odd."));
    }

    #[test]
    fn a_lens_that_stops_being_a_page_stops_going_by_block() {
        let mut lens = Lens {
            unit: LensUnit::Block,
            ..Lens::new("page")
        };
        lens.display = LensDisplay::Page;

        lens.set_display(LensDisplay::Annotate);

        assert_eq!(lens.unit, LensUnit::Document);
    }
}
