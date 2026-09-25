//! The agents a lens can name, and what sets each apart for everything that
//! reads a lens without running it: the configuration's checks, the recipes
//! and the preferences.
//!
//! Adding an agent is a variant here and its [`AgentProfile`]; how it is run
//! and how its answer is read belong to the desktop app's adapter for it.

use serde::{Deserialize, Serialize};

/// Something Arto knows how to ask and how to read the answer of: the lens
/// gives it a prompt instead of a whole command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LensAgent {
    /// The `claude` command-line agent.
    Claude,
    /// The `codex` command-line agent.
    Codex,
    /// An Ollama server, through its own API, which unlike its
    /// OpenAI-compatible one takes the context length with each request.
    Ollama,
    /// A server with an OpenAI-compatible chat completions API.
    Openai,
}

/// Something an agent is kept from unless the lens allows it.
///
/// An agent is run as a plain text transformer by default, because the
/// document it is handed can talk it into anything its tools can do. What a
/// lens allows is the reader's decision for that lens; it is never implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LensCapability {
    /// Search the web and read what it finds, which a fact check needs.
    /// What leaves the machine is what the agent was already handed.
    WebSearch,
    /// Read the files around the document.
    ReadFiles,
    /// Run commands. The agent's own sandbox, where it has one, still
    /// keeps them from writing.
    Shell,
}

impl LensCapability {
    /// Every capability, in the order the preferences offer them.
    pub const ALL: [LensCapability; 3] = [Self::WebSearch, Self::ReadFiles, Self::Shell];

    /// Whether it lets the agent reach what is on this machine beyond the
    /// document — which a document can ask it to copy into its answer, or
    /// out through a search.
    pub fn is_local(self) -> bool {
        matches!(self, Self::ReadFiles | Self::Shell)
    }

    /// What the preferences call it.
    pub fn label(self) -> &'static str {
        match self {
            Self::WebSearch => "Search the web",
            Self::ReadFiles => "Read files around the document",
            Self::Shell => "Run commands",
        }
    }

    /// What allowing it means, beside the choice in the preferences.
    pub fn description(self) -> &'static str {
        match self {
            Self::WebSearch => {
                "For a fact check. The agent may search for what the text says and read the pages it finds; what it sends out is what it was already handed."
            }
            Self::ReadFiles => {
                "The document can ask the agent to read the files beside it and write them into its answer, or send them out through a search. Allow it only for documents you trust. Opened again by itself, the lens shows what it answered before and asks nothing."
            }
            Self::Shell => {
                "Everything reading files allows, and whatever a command can do: codex keeps its commands from writing; claude does not. Allow it only for documents you trust."
            }
        }
    }
}

impl std::fmt::Display for LensCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let spelled = match self {
            Self::WebSearch => "webSearch",
            Self::ReadFiles => "readFiles",
            Self::Shell => "shell",
        };
        f.write_str(spelled)
    }
}

/// What sets an agent apart from the others, short of running it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentProfile {
    /// What the agent is called in the preferences.
    pub name: &'static str,
    /// What choosing it means, for someone setting a lens up from a recipe.
    pub introduction: &'static str,
    /// What it is, beside the choice in a lens's settings.
    pub description: &'static str,
    pub reach: AgentReach,
    /// What a lens can allow it.
    pub capabilities: &'static [LensCapability],
    /// The models a recipe picks, when the agent names them the same for
    /// everyone; a server's are the reader's to name.
    pub recipe_models: Option<RecipeModels>,
}

/// How an agent is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentReach {
    /// A program Arto runs, found by this name when the lens does not say
    /// where it is.
    Program { name: &'static str },
    /// A server Arto asks over HTTP.
    Server(ServerProfile),
}

/// What sets a server apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerProfile {
    /// Where it is when the lens does not say.
    pub endpoint: &'static str,
    /// What the endpoint setting takes.
    pub endpoint_hint: &'static str,
    /// Whether it is elsewhere and wants a key as a rule, so that a recipe
    /// asks for the endpoint and the key up front.
    pub remote: bool,
    /// Whether the context length can be set with each request.
    pub context_length: bool,
}

/// The models a recipe picks for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeModels {
    /// For a whole document.
    pub document: &'static str,
    /// For a block at a time.
    pub block: &'static str,
}

impl LensAgent {
    /// Every agent, in the order the preferences offer them.
    pub const ALL: [LensAgent; 4] = [Self::Claude, Self::Codex, Self::Ollama, Self::Openai];

    pub fn profile(self) -> AgentProfile {
        match self {
            Self::Claude => AgentProfile {
                name: "Claude",
                introduction: "The claude command, signed in as you already are.",
                description: "The claude command, signed in as you already are.",
                reach: AgentReach::Program { name: "claude" },
                capabilities: &LensCapability::ALL,
                recipe_models: Some(RecipeModels {
                    document: "sonnet",
                    block: "haiku",
                }),
            },
            Self::Codex => AgentProfile {
                name: "Codex",
                introduction: "The codex command, signed in as you already are.",
                description: "The codex command, signed in as you already are.",
                reach: AgentReach::Program { name: "codex" },
                capabilities: &LensCapability::ALL,
                recipe_models: None,
            },
            Self::Ollama => AgentProfile {
                name: "Ollama",
                introduction: "A model running on this machine through Ollama; nothing leaves it.",
                description: "An Ollama server, through its own API.",
                reach: AgentReach::Server(ServerProfile {
                    endpoint: "http://127.0.0.1:11434",
                    endpoint_hint: "Left empty, http://127.0.0.1:11434.",
                    remote: false,
                    context_length: true,
                }),
                capabilities: &[],
                recipe_models: None,
            },
            Self::Openai => AgentProfile {
                name: "OpenAI",
                introduction: "OpenAI, or a server with an OpenAI-compatible API.",
                description: "A server with an OpenAI-compatible API — LM Studio, llama.cpp, OpenAI and the like.",
                reach: AgentReach::Server(ServerProfile {
                    endpoint: "https://api.openai.com/v1",
                    endpoint_hint: "The base URL, the part before /chat/completions. Left empty, https://api.openai.com/v1 — OpenAI itself.",
                    remote: true,
                    context_length: false,
                }),
                capabilities: &[],
                recipe_models: None,
            },
        }
    }

    /// The server it is, if Arto asks it over HTTP rather than running a
    /// program.
    pub fn server(self) -> Option<ServerProfile> {
        match self.profile().reach {
            AgentReach::Server(server) => Some(server),
            AgentReach::Program { .. } => None,
        }
    }

    /// Whether Arto asks it over HTTP rather than running a program.
    pub fn is_server(self) -> bool {
        self.server().is_some()
    }
}
