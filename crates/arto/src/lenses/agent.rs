//! The agents a lens can name instead of a command — command-line agents it
//! runs, and servers it asks over HTTP — and how the answer is read out of
//! what each writes.
//!
//! Every command-line agent is run as a plain text transformer — no tools it
//! could act with, no session left behind, as little of its own
//! configuration as it allows — because a lens asks a question about a text,
//! and anything the agent loads beyond that (MCP servers, hooks, the
//! `CLAUDE.md` files around the document) only makes each run slower and its
//! answer less predictable. The flags and the formats below are the agents'
//! own and change with them; the recorded outputs in the tests are what
//! keeps this module honest about them.

use arto_config::{Lens, LensAgent};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Where an Ollama server is when the lens does not say.
const OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";

/// Where an OpenAI-compatible server is when the lens does not say: OpenAI
/// itself.
const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1";

/// The bounds of the context an Ollama model is loaded with when the lens
/// leaves it to Arto.
const MIN_CONTEXT: usize = 4096;
const MAX_CONTEXT: usize = 131_072;

/// What an agent is told it is, in place of its own system prompt where it
/// allows one.
const SYSTEM_PROMPT: &str = "You transform Markdown for a reader. The message holds an \
    instruction and the text to work on. Follow the instruction exactly and write only what \
    it asks for: no preamble, no explanation, no code fence around the answer.";

/// How the answer is written to stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    /// The answer itself, as it is generated.
    Text,
    /// Claude's `stream-json` events; the text is in the deltas.
    ClaudeStream,
    /// Codex's JSONL events; the answer arrives whole, as the agent message.
    CodexEvents,
    /// Ollama's chat stream: one JSON object per line, the text in its
    /// message.
    OllamaChat,
    /// An OpenAI-compatible chat completion stream: server-sent events whose
    /// data carries the text in the choice's delta.
    OpenAiStream,
}

/// Which API a server speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Api {
    Ollama,
    OpenAi,
}

/// Where a server's API key comes from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ApiKey {
    None,
    /// The system's credential store, under this account (see
    /// [`super::secrets`]); a server with no key stored is asked without one.
    Stored(String),
    /// A program and its arguments that print it.
    Command(Vec<String>),
}

/// What the API key of the server `lens` asks is filed under, if it asks a
/// server: its endpoint, or the one it defaults to.
pub(crate) fn key_account(lens: &Lens) -> Option<String> {
    let default = match lens.agent? {
        LensAgent::Ollama => OLLAMA_ENDPOINT,
        LensAgent::Openai => OPENAI_ENDPOINT,
        LensAgent::Claude | LensAgent::Codex => return None,
    };
    let endpoint = lens
        .endpoint
        .as_deref()
        .filter(|endpoint| !endpoint.trim().is_empty())
        .unwrap_or(default);
    (!endpoint.is_empty()).then(|| super::secrets::account(endpoint))
}

/// How a request reaches a server.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Server {
    pub api: Api,
    pub url: String,
    pub model: String,
    pub system: Option<String>,
    /// Ollama: the context to load the model with; sized to the request
    /// when `None`.
    pub context_length: Option<u32>,
    pub api_key: ApiKey,
}

impl Server {
    /// Where the server lists the models it offers.
    pub(crate) fn models_url(&self) -> String {
        let (chat, models) = match self.api {
            Api::Ollama => ("/api/chat", "/api/tags"),
            Api::OpenAi => ("/chat/completions", "/models"),
        };
        let base = self.url.strip_suffix(chat).unwrap_or(&self.url);
        format!("{base}{models}")
    }

    /// The request body asking the server about `message`.
    pub(crate) fn body(&self, message: &str) -> String {
        let mut messages = Vec::new();
        if let Some(system) = &self.system {
            messages.push(json!({"role": "system", "content": system}));
        }
        messages.push(json!({"role": "user", "content": message}));
        let mut body = json!({"model": self.model, "messages": messages, "stream": true});
        if self.api == Api::Ollama {
            let context = self
                .context_length
                .map_or_else(|| context_for(message), |length| length as usize);
            body["options"] = json!({ "num_ctx": context });
        }
        body.to_string()
    }
}

/// A context that holds `message` and an answer as long as it, rounded up
/// so that requests of a similar size share a loaded model. A character is
/// counted as a token, which overestimates English and fits Japanese.
fn context_for(message: &str) -> usize {
    let needed = message.chars().count() * 2 + 1024;
    needed.next_power_of_two().clamp(MIN_CONTEXT, MAX_CONTEXT)
}

/// How what a lens runs is reached.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Transport {
    /// A program, run with the request on stdin.
    Process { argv: Vec<String> },
    /// A server, asked over HTTP.
    Server(Server),
}

/// How a request is handed to what a lens runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Input {
    /// As JSON, for a command of the reader's own.
    Json,
    /// As a message for an agent: the prompt, if the lens has one, then the
    /// text.
    Message { prompt: Option<String> },
}

/// How to run a lens's command.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Invocation {
    pub transport: Transport,
    pub format: OutputFormat,
    /// How the request is handed over.
    pub input: Input,
    /// `PATH` for a program — the command, or the one that prints an API
    /// key: the directories agents are installed in, which an app started
    /// from the Finder does not have.
    pub path: Option<OsString>,
}

/// How to run `lens`.
pub(crate) fn invocation(lens: &Lens) -> Invocation {
    let prompt = lens
        .prompt
        .clone()
        .filter(|prompt| !prompt.trim().is_empty());
    let Some(agent) = lens.agent else {
        return Invocation {
            transport: Transport::Process {
                argv: lens.command.clone(),
            },
            format: OutputFormat::Text,
            input: Input::Json,
            path: search_path(None),
        };
    };
    let model = lens.model.as_deref().filter(|model| !model.is_empty());
    let (api, format, default_endpoint, path) = match agent {
        LensAgent::Ollama => (
            Api::Ollama,
            OutputFormat::OllamaChat,
            OLLAMA_ENDPOINT,
            "/api/chat",
        ),
        LensAgent::Openai => (
            Api::OpenAi,
            OutputFormat::OpenAiStream,
            OPENAI_ENDPOINT,
            "/chat/completions",
        ),
        LensAgent::Claude | LensAgent::Codex => {
            return program_invocation(lens, agent, model, prompt)
        }
    };
    // Trimmed as `key_account` files the key: a URL pasted with a trailing
    // newline would otherwise be asked as written and fail.
    let endpoint = lens
        .endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .unwrap_or(default_endpoint);
    let api_key = if !lens.api_key_command.is_empty() {
        ApiKey::Command(lens.api_key_command.clone())
    } else {
        key_account(lens).map_or(ApiKey::None, ApiKey::Stored)
    };
    Invocation {
        transport: Transport::Server(Server {
            api,
            url: format!("{}{path}", endpoint.trim_end_matches('/')),
            model: model.unwrap_or_default().to_string(),
            system: lens.system.clone(),
            context_length: lens.context_length,
            api_key,
        }),
        format,
        input: Input::Message { prompt },
        path: search_path(None),
    }
}

/// How to run the command-line `agent` for `lens`.
fn program_invocation(
    lens: &Lens,
    agent: LensAgent,
    model: Option<&str>,
    prompt: Option<String>,
) -> Invocation {
    let name = match agent {
        LensAgent::Claude => "claude",
        _ => "codex",
    };
    let program = lens
        .program
        .clone()
        .or_else(|| find_program(name))
        .unwrap_or_else(|| PathBuf::from(name));
    let mut argv = vec![program.to_string_lossy().into_owned()];
    let format = match agent {
        LensAgent::Claude => {
            argv.extend(
                [
                    "-p",
                    "--no-session-persistence",
                    "--tools",
                    "",
                    "--strict-mcp-config",
                    "--setting-sources",
                    "",
                    "--disable-slash-commands",
                    "--system-prompt",
                    SYSTEM_PROMPT,
                    "--output-format",
                    "stream-json",
                    "--include-partial-messages",
                    "--verbose",
                ]
                .map(String::from),
            );
            if let Some(model) = model {
                argv.extend(["--model".to_string(), model.to_string()]);
            }
            OutputFormat::ClaudeStream
        }
        _ => {
            argv.extend(
                [
                    "exec",
                    "--json",
                    "--ephemeral",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "--ignore-user-config",
                    "--ignore-rules",
                    "--color",
                    "never",
                    // A read-only sandbox still lets the agent run commands
                    // that read files, and the document it is handed can ask
                    // it to — to read what lies beside the document and write
                    // it into the answer. A lens hands over text to be read,
                    // so every tool is turned off.
                    "--disable",
                    "shell_tool",
                    "--disable",
                    "unified_exec",
                    "--disable",
                    "apps",
                    "--disable",
                    "browser_use",
                    "--disable",
                    "computer_use",
                    "--disable",
                    "image_generation",
                    "-c",
                    "web_search=\"disabled\"",
                ]
                .map(String::from),
            );
            if let Some(model) = model {
                argv.extend(["--model".to_string(), model.to_string()]);
            }
            // The message comes on stdin.
            argv.push("-".to_string());
            OutputFormat::CodexEvents
        }
    };
    Invocation {
        transport: Transport::Process { argv },
        format,
        input: Input::Message { prompt },
        path: search_path(program.parent()),
    }
}

/// Where command-line tools are commonly installed, beyond what `PATH`
/// holds for an app started from the Finder or the Dock.
fn common_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/run/current-system/sw/bin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = dirs::home_dir() {
        for relative in [
            ".local/bin",
            ".nix-profile/bin",
            ".bun/bin",
            ".npm-global/bin",
            ".volta/bin",
            ".cargo/bin",
        ] {
            directories.push(home.join(relative));
        }
    }
    if let Ok(user) = std::env::var("USER") {
        directories.push(PathBuf::from(format!("/etc/profiles/per-user/{user}/bin")));
    }
    directories
}

/// The directories to look for programs in: the program's own first — an
/// agent written in a scripting language looks for its interpreter beside
/// itself — then `PATH`, then the common ones.
fn search_directories(program_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = program_dir.map(Path::to_path_buf).into_iter().collect();
    if let Some(path) = std::env::var_os("PATH") {
        directories.extend(std::env::split_paths(&path));
    }
    directories.extend(common_directories());
    let mut seen = std::collections::HashSet::new();
    directories.retain(|directory| seen.insert(directory.clone()));
    directories
}

fn search_path(program_dir: Option<&Path>) -> Option<OsString> {
    std::env::join_paths(search_directories(program_dir)).ok()
}

/// The first executable called `name` in the search directories.
fn find_program(name: &str) -> Option<PathBuf> {
    search_directories(None)
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Reads the answer out of what the command writes, as it writes it.
pub(crate) struct Decoder {
    format: OutputFormat,
    /// How much of the output has been read, up to the last whole line.
    consumed: usize,
    answer: String,
    /// The answer as the agent reported it when it finished, which wins
    /// over what was pieced together from the stream.
    reported: Option<String>,
    error: Option<String>,
}

impl Decoder {
    pub(crate) fn new(format: OutputFormat) -> Self {
        Self {
            format,
            consumed: 0,
            answer: String::new(),
            reported: None,
            error: None,
        }
    }

    /// The answer so far, given everything the command has written so far.
    pub(crate) fn feed<'a>(&'a mut self, output: &'a str) -> &'a str {
        if self.format == OutputFormat::Text {
            return output;
        }
        while let Some(end) = output[self.consumed..].find('\n') {
            let line = &output[self.consumed..self.consumed + end];
            self.consumed += end + 1;
            self.read_event(line);
        }
        &self.answer
    }

    /// The whole answer, given everything the command wrote; or why the
    /// agent says it has none.
    pub(crate) fn finish(mut self, output: &str) -> Result<String, String> {
        if self.format == OutputFormat::Text {
            return Ok(output.to_string());
        }
        self.feed(output);
        let rest = output[self.consumed..].to_string();
        self.read_event(&rest);
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.reported.unwrap_or(self.answer)),
        }
    }

    fn read_event(&mut self, line: &str) {
        let line = line.trim();
        // A server-sent event carries its JSON after `data:`.
        let line = match self.format {
            OutputFormat::OpenAiStream => line.strip_prefix("data:").map_or(line, str::trim),
            _ => line,
        };
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let text = |value: &Value| value.as_str().map(str::to_string);
        match (self.format, event["type"].as_str()) {
            (OutputFormat::ClaudeStream, Some("stream_event")) => {
                let delta = &event["event"]["delta"];
                if event["event"]["type"] == "content_block_delta" && delta["type"] == "text_delta"
                {
                    self.answer
                        .push_str(delta["text"].as_str().unwrap_or_default());
                }
            }
            (OutputFormat::ClaudeStream, Some("result")) => {
                if event["is_error"].as_bool().unwrap_or(false) {
                    self.error = text(&event["result"]).or_else(|| text(&event["subtype"]));
                } else {
                    self.reported = text(&event["result"]);
                }
            }
            (OutputFormat::CodexEvents, Some("item.completed")) => {
                if event["item"]["type"] == "agent_message" {
                    if let Some(message) = text(&event["item"]["text"]) {
                        self.answer = message;
                    }
                }
            }
            (OutputFormat::CodexEvents, Some("turn.failed")) => {
                self.error = text(&event["error"]["message"]).or(Some("codex failed".to_string()));
            }
            (OutputFormat::CodexEvents, Some("error")) => {
                self.error = text(&event["message"]).or(Some("codex failed".to_string()));
            }
            (OutputFormat::OllamaChat, _) => {
                if let Some(error) = text(&event["error"]) {
                    self.error = Some(error);
                } else if let Some(content) = event["message"]["content"].as_str() {
                    self.answer.push_str(content);
                }
            }
            (OutputFormat::OpenAiStream, _) => {
                let choice = &event["choices"][0];
                if let Some(error) =
                    text(&event["error"]["message"]).or_else(|| text(&event["error"]))
                {
                    self.error = Some(error);
                } else if let Some(content) = choice["delta"]["content"].as_str() {
                    self.answer.push_str(content);
                } else if let Some(content) = text(&choice["message"]["content"]) {
                    // A server that does not stream answers whole.
                    self.reported = Some(content);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_config::LensDisplay;

    fn lens(agent: Option<LensAgent>, model: Option<&str>) -> Lens {
        Lens {
            id: "t".to_string(),
            label: "T".to_string(),
            display: LensDisplay::Page,
            agent,
            model: model.map(str::to_string),
            prompt: Some("Translate.".to_string()),
            program: Some(PathBuf::from("/opt/agents/bin/agent")),
            system: None,
            endpoint: None,
            api_key_command: Vec::new(),
            context_length: None,
            command: if agent.is_none() {
                vec!["my-tool".to_string(), "--flag".to_string()]
            } else {
                Vec::new()
            },
            context: 2,
            concurrency: 1,
            timeout_seconds: 60,
            unit: Default::default(),
            shortcut: None,
        }
    }

    fn argv(run: &Invocation) -> &[String] {
        match &run.transport {
            Transport::Process { argv } => argv,
            other => panic!("not a program: {other:?}"),
        }
    }

    fn server(run: &Invocation) -> &Server {
        match &run.transport {
            Transport::Server(server) => server,
            other => panic!("not a server: {other:?}"),
        }
    }

    #[test]
    fn a_command_runs_as_written_and_reads_json() {
        let run = invocation(&lens(None, None));
        assert_eq!(argv(&run), ["my-tool", "--flag"]);
        assert_eq!(run.format, OutputFormat::Text);
        assert_eq!(run.input, Input::Json);
    }

    #[test]
    fn claude_runs_bare_and_streams() {
        let run = invocation(&lens(Some(LensAgent::Claude), Some("sonnet")));
        let argv = argv(&run);
        assert_eq!(argv[0], "/opt/agents/bin/agent");
        assert_eq!(&argv[1..3], ["-p", "--no-session-persistence"]);
        for flag in [
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--include-partial-messages",
        ] {
            assert!(argv.iter().any(|arg| arg == flag), "{flag}: {argv:?}");
        }
        let tools = argv.iter().position(|arg| arg == "--tools").unwrap();
        assert_eq!(argv[tools + 1], "");
        assert_eq!(&argv[argv.len() - 2..], ["--model", "sonnet"]);
        assert_eq!(run.format, OutputFormat::ClaudeStream);
        assert_eq!(
            run.input,
            Input::Message {
                prompt: Some("Translate.".to_string())
            }
        );
    }

    #[test]
    fn codex_runs_read_only_and_reads_stdin() {
        let run = invocation(&lens(Some(LensAgent::Codex), None));
        let argv = argv(&run);
        assert_eq!(&argv[1..3], ["exec", "--json"]);
        let sandbox = argv.iter().position(|arg| arg == "--sandbox").unwrap();
        assert_eq!(argv[sandbox + 1], "read-only");
        let disabled: Vec<&str> = argv
            .windows(2)
            .filter(|pair| pair[0] == "--disable")
            .map(|pair| pair[1].as_str())
            .collect();
        for tool in ["shell_tool", "unified_exec"] {
            assert!(disabled.contains(&tool), "{tool} is left on: {argv:?}");
        }
        assert!(!argv.iter().any(|arg| arg == "--model"));
        assert_eq!(argv.last().map(String::as_str), Some("-"));
        assert_eq!(run.format, OutputFormat::CodexEvents);
    }

    #[test]
    fn ollama_is_asked_on_its_own_api_with_a_context_sized_to_the_request() {
        let run = invocation(&lens(Some(LensAgent::Ollama), Some("qwen3:4b-instruct")));
        let server = server(&run);
        assert_eq!(server.url, "http://127.0.0.1:11434/api/chat");
        assert_eq!(run.format, OutputFormat::OllamaChat);

        let body: Value = serde_json::from_str(&server.body("short")).unwrap();
        assert_eq!(body["model"], "qwen3:4b-instruct");
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"],
            json!([{"role": "user", "content": "short"}])
        );
        assert_eq!(body["options"]["num_ctx"], MIN_CONTEXT);

        let long: Value = serde_json::from_str(&server.body(&"語".repeat(10_000))).unwrap();
        assert_eq!(long["options"]["num_ctx"], 32_768);
    }

    #[test]
    fn a_context_length_the_lens_names_wins() {
        let mut ollama = lens(Some(LensAgent::Ollama), Some("m"));
        ollama.context_length = Some(8192);
        ollama.endpoint = Some("http://gpu-box:11434/".to_string());
        let run = invocation(&ollama);

        assert_eq!(server(&run).url, "http://gpu-box:11434/api/chat");
        let body: Value = serde_json::from_str(&server(&run).body("x")).unwrap();
        assert_eq!(body["options"]["num_ctx"], 8192);
    }

    #[test]
    fn a_context_is_bounded() {
        assert_eq!(context_for(""), MIN_CONTEXT);
        assert_eq!(context_for(&"x".repeat(1_000_000)), MAX_CONTEXT);
    }

    #[test]
    fn an_openai_server_is_asked_its_chat_completions_with_the_system_prompt() {
        let mut openai = lens(Some(LensAgent::Openai), Some("gpt-5-mini"));
        openai.endpoint = Some("https://api.openai.com/v1".to_string());
        openai.system = Some("Japanese".to_string());
        openai.api_key_command = vec!["security".to_string(), "find-generic-password".to_string()];
        let run = invocation(&openai);
        let server = server(&run);

        assert_eq!(server.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(
            server.api_key,
            ApiKey::Command(openai.api_key_command.clone())
        );
        assert_eq!(run.format, OutputFormat::OpenAiStream);
        let body: Value = serde_json::from_str(&server.body("Hello")).unwrap();
        assert_eq!(
            body["messages"],
            json!([
                {"role": "system", "content": "Japanese"},
                {"role": "user", "content": "Hello"},
            ])
        );
        assert!(body.get("options").is_none(), "{body}");
    }

    #[test]
    fn an_api_key_comes_from_the_credential_store_unless_a_command_prints_it() {
        let mut openai = lens(Some(LensAgent::Openai), Some("m"));
        openai.endpoint = Some("http://localhost:1234/v1/".to_string());
        assert_eq!(
            server(&invocation(&openai)).api_key,
            ApiKey::Stored("http://localhost:1234/v1".to_string())
        );

        openai.api_key_command = vec!["pass".to_string()];
        assert_eq!(
            server(&invocation(&openai)).api_key,
            ApiKey::Command(vec!["pass".to_string()])
        );
    }

    #[test]
    fn an_endpoint_is_asked_where_its_key_is_filed_whatever_space_surrounds_it() {
        let mut openai = lens(Some(LensAgent::Openai), Some("m"));
        openai.endpoint = Some(" http://localhost:1234/v1 \n".to_string());
        assert_eq!(
            server(&invocation(&openai)).url,
            "http://localhost:1234/v1/chat/completions"
        );
        assert_eq!(
            server(&invocation(&openai)).api_key,
            ApiKey::Stored("http://localhost:1234/v1".to_string())
        );

        openai.endpoint = Some("  ".to_string());
        assert_eq!(
            server(&invocation(&openai)).url,
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn a_server_has_a_key_account_even_at_its_default_endpoint() {
        let ollama = lens(Some(LensAgent::Ollama), Some("m"));
        assert_eq!(key_account(&ollama).as_deref(), Some(OLLAMA_ENDPOINT));
        assert_eq!(key_account(&lens(Some(LensAgent::Claude), None)), None);
        assert_eq!(
            key_account(&lens(Some(LensAgent::Openai), Some("m"))).as_deref(),
            Some(OPENAI_ENDPOINT),
            "OpenAI itself when no endpoint is set"
        );
    }

    #[test]
    fn the_program_directory_comes_first_on_the_path() {
        let run = invocation(&lens(Some(LensAgent::Claude), None));
        let path = run.path.expect("a path");
        let first = std::env::split_paths(&path).next();
        assert_eq!(first, Some(PathBuf::from("/opt/agents/bin")));
    }

    /// What `claude -p --output-format stream-json --include-partial-messages`
    /// wrote for a short answer, cut to the events that matter and a few
    /// that do not.
    const CLAUDE_STREAM: &str = concat!(
        r##"{"type":"system","subtype":"init","session_id":"s"}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"message_start","message":{}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"# 見出"}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"し\n\n本文。"}}}"##,
        "\n",
        r##"{"type":"assistant","message":{"content":[{"type":"text","text":"# 見出し\n\n本文。"}]}}"##,
        "\n",
        r##"{"type":"result","subtype":"success","is_error":false,"result":"# 見出し\n\n本文。"}"##,
        "\n",
    );

    #[test]
    fn claude_answers_as_its_deltas_arrive() {
        let mut decoder = Decoder::new(OutputFormat::ClaudeStream);
        let line_end = |n: usize| CLAUDE_STREAM.match_indices('\n').nth(n).unwrap().0 + 1;
        assert_eq!(decoder.feed(&CLAUDE_STREAM[..line_end(2)]), "");
        // A line the command is still writing is not read yet.
        assert_eq!(decoder.feed(&CLAUDE_STREAM[..line_end(2) + 20]), "");
        assert_eq!(decoder.feed(&CLAUDE_STREAM[..line_end(3)]), "# 見出");
        assert_eq!(decoder.feed(CLAUDE_STREAM), "# 見出し\n\n本文。");
        assert_eq!(
            decoder.finish(CLAUDE_STREAM).as_deref(),
            Ok("# 見出し\n\n本文。")
        );
    }

    #[test]
    fn claude_reports_a_failure() {
        let output = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"Credit balance is too low"}"#;
        assert_eq!(
            Decoder::new(OutputFormat::ClaudeStream).finish(output),
            Err("Credit balance is too low".to_string())
        );
    }

    /// What `codex exec --json` wrote for a short answer.
    const CODEX_EVENTS: &str = concat!(
        r#"{"type":"thread.started","thread_id":"t"}"#,
        "\n",
        r#"{"type":"turn.started"}"#,
        "\n",
        r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"thinking"}}"#,
        "\n",
        r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"The sea whispers.\nWaves answer."}}"#,
        "\n",
        r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        "\n",
    );

    #[test]
    fn codex_answers_with_its_message() {
        let mut decoder = Decoder::new(OutputFormat::CodexEvents);
        assert_eq!(
            decoder.feed(CODEX_EVENTS),
            "The sea whispers.\nWaves answer."
        );
        assert_eq!(
            decoder.finish(CODEX_EVENTS).as_deref(),
            Ok("The sea whispers.\nWaves answer.")
        );
    }

    #[test]
    fn codex_reports_a_failure() {
        let output = concat!(
            r#"{"type":"turn.started"}"#,
            "\n",
            r#"{"type":"turn.failed","error":{"message":"stream disconnected"}}"#,
        );
        assert_eq!(
            Decoder::new(OutputFormat::CodexEvents).finish(output),
            Err("stream disconnected".to_string())
        );
    }

    /// What Ollama's `/api/chat` streams for a short answer.
    const OLLAMA_CHAT: &str = concat!(
        r##"{"model":"m","message":{"role":"assistant","content":"# 見"},"done":false}"##,
        "\n",
        r##"{"model":"m","message":{"role":"assistant","content":"出し\n\n本文。"},"done":false}"##,
        "\n",
        r##"{"model":"m","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}"##,
        "\n",
    );

    #[test]
    fn ollama_answers_as_its_messages_arrive() {
        let mut decoder = Decoder::new(OutputFormat::OllamaChat);
        let first_line = OLLAMA_CHAT.find('\n').unwrap() + 1;
        assert_eq!(decoder.feed(&OLLAMA_CHAT[..first_line]), "# 見");
        assert_eq!(
            decoder.finish(OLLAMA_CHAT).as_deref(),
            Ok("# 見出し\n\n本文。")
        );
    }

    #[test]
    fn ollama_reports_a_failure() {
        let output = r#"{"error":"model \"qwen9\" not found, try pulling it first"}"#;
        assert_eq!(
            Decoder::new(OutputFormat::OllamaChat).finish(output),
            Err(r#"model "qwen9" not found, try pulling it first"#.to_string())
        );
    }

    /// What an OpenAI-compatible server streams for a short answer.
    const OPENAI_STREAM: &str = concat!(
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":""}}]}"#,
        "\n\n",
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"こんにち"}}]}"#,
        "\n\n",
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"は"},"finish_reason":"stop"}]}"#,
        "\n\n",
        "data: [DONE]\n\n",
    );

    #[test]
    fn an_openai_server_answers_as_its_events_arrive() {
        let mut decoder = Decoder::new(OutputFormat::OpenAiStream);
        let two_events = OPENAI_STREAM.match_indices("\n\n").nth(1).unwrap().0 + 2;
        assert_eq!(decoder.feed(&OPENAI_STREAM[..two_events]), "こんにち");
        assert_eq!(decoder.finish(OPENAI_STREAM).as_deref(), Ok("こんにちは"));
    }

    #[test]
    fn an_openai_server_that_does_not_stream_answers_whole() {
        let output = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"全文"}}]}"#;
        assert_eq!(
            Decoder::new(OutputFormat::OpenAiStream)
                .finish(output)
                .as_deref(),
            Ok("全文")
        );
    }

    #[test]
    fn an_openai_server_reports_a_failure() {
        let output =
            r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#;
        assert_eq!(
            Decoder::new(OutputFormat::OpenAiStream).finish(output),
            Err("Incorrect API key provided".to_string())
        );
    }

    #[test]
    fn text_is_the_answer_as_it_is() {
        let mut decoder = Decoder::new(OutputFormat::Text);
        assert_eq!(decoder.feed("half"), "half");
        assert_eq!(
            decoder.finish("half and whole").as_deref(),
            Ok("half and whole")
        );
    }
}
