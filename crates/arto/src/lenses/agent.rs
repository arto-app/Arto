//! The agents a lens can name instead of a command, behind one shape: how
//! what a lens runs is reached, and how the answer is read out of what it
//! writes.
//!
//! This module holds what every agent shares — finding a program, asking a
//! server, reading a stream a line at a time. What sets one agent apart
//! lives in its adapter in `agent/`: a [`ProgramAgent`] for a command-line
//! agent Arto runs, a [`ServerAgent`] for a server it asks over HTTP. The
//! agent's static facts — its name, where a server is by default — are the
//! configuration's ([`arto_config::AgentProfile`]).
//!
//! Adding an agent is its variant and profile in `arto-config`, an adapter
//! module here, and its line in [`adapter`].
//!
//! Every command-line agent is run as a plain text transformer — no tools it
//! could act with, no session left behind, as little of its own
//! configuration as it allows — because a lens asks a question about a text,
//! and anything the agent loads beyond that (MCP servers, hooks, the
//! `CLAUDE.md` files around the document) only makes each run slower and its
//! answer less predictable. The flags and the formats in the adapters are
//! the agents' own and change with them; the recorded outputs in their tests
//! are what keeps them honest about them.

mod claude;
mod codex;
mod ollama;
mod openai;

use arto_config::{AgentReach, Lens, LensAgent, LensCapability};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// What an agent is told it is, in place of its own system prompt where it
/// allows one.
pub(crate) const SYSTEM_PROMPT: &str = "You transform Markdown for a reader. The message holds an \
    instruction and the text to work on. Follow the instruction exactly and write only what \
    it asks for: no preamble, no explanation, no code fence around the answer.";

/// Reads the answer out of what an agent writes, one event at a time.
pub(crate) trait Reader: Sync {
    /// The JSON in `line`, for an agent that wraps its events.
    fn event<'a>(&self, line: &'a str) -> &'a str {
        line
    }

    /// Take in `event`, one of the agent's, parsed.
    fn read(&self, event: &Value, answer: &mut Answer);

    /// The models `listing` names, which the agent wrote when asked what it
    /// offers.
    fn models(&self, listing: &str) -> Result<Vec<String>, String>;
}

/// A command-line agent Arto runs.
pub(crate) trait ProgramAgent: Reader {
    /// The arguments after the program, asking `model` when the lens names
    /// one and letting the agent do what `allow` lists — only what its
    /// profile offers, which the configuration has checked.
    fn args(&self, model: Option<&str>, allow: &[LensCapability]) -> Vec<String>;

    /// How the request is handed over.
    fn conversation(&self) -> Conversation {
        Conversation::Stdin
    }

    /// The arguments, in place of [`Self::args`], that make it print the
    /// models it offers.
    fn models_args(&self) -> &'static [&'static str];
}

/// How a program is spoken to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Conversation {
    /// The request on stdin, the answer on stdout.
    Stdin,
    /// Codex's app server protocol (see [`super::app_server`]).
    AppServer,
}

/// A server Arto asks over HTTP.
pub(crate) trait ServerAgent: Reader {
    /// Where, under the endpoint, it is asked.
    fn chat_path(&self) -> &'static str;

    /// Where, under the endpoint, it lists its models.
    fn models_path(&self) -> &'static str;

    /// The request body asking `server` about `message`: a chat completion
    /// by default, which most servers take.
    fn body(&self, server: &Server, message: &str) -> Value {
        chat_body(server, message)
    }
}

/// The adapter for one agent.
#[derive(Clone, Copy)]
pub(crate) enum Adapter {
    Program(&'static dyn ProgramAgent),
    Server(&'static dyn ServerAgent),
}

impl Adapter {
    fn reader(self) -> &'static dyn Reader {
        match self {
            Self::Program(agent) => agent,
            Self::Server(agent) => agent,
        }
    }
}

/// The adapter for `agent`.
pub(crate) fn adapter(agent: LensAgent) -> Adapter {
    match agent {
        LensAgent::Claude => Adapter::Program(&claude::Claude),
        LensAgent::Codex => Adapter::Program(&codex::Codex),
        LensAgent::Ollama => Adapter::Server(&ollama::Ollama),
        LensAgent::Openai => Adapter::Server(&openai::OpenAi),
    }
}

/// What reads `agent`'s answer and the models it offers.
pub(crate) fn reader(agent: LensAgent) -> &'static dyn Reader {
    adapter(agent).reader()
}

/// A chat completion request asking `server` about `message`, streamed.
pub(crate) fn chat_body(server: &Server, message: &str) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = &server.system {
        messages.push(json!({"role": "system", "content": system}));
    }
    messages.push(json!({"role": "user", "content": message}));
    json!({"model": server.model, "messages": messages, "stream": true})
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
    let default = lens.agent?.server()?.endpoint;
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
    pub agent: LensAgent,
    pub url: String,
    pub models_url: String,
    pub model: String,
    pub system: Option<String>,
    /// The context to load the model with, for a server that takes it;
    /// sized to the request when `None`.
    pub context_length: Option<u32>,
    pub api_key: ApiKey,
}

impl Server {
    /// The request body asking the server about `message`.
    pub(crate) fn body(&self, message: &str) -> String {
        match adapter(self.agent) {
            Adapter::Server(agent) => agent.body(self, message).to_string(),
            Adapter::Program(_) => unreachable!("{:?} is not a server", self.agent),
        }
    }
}

/// How what a lens runs is reached.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Transport {
    /// A program, run with the request on stdin.
    Process { argv: Vec<String> },
    /// `codex app-server`, asked in a thread of its own (see
    /// [`super::app_server`]).
    AppServer { argv: Vec<String> },
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
    /// The agent whose answer is read out of what it writes; a command of
    /// the reader's own writes the answer itself.
    pub agent: Option<LensAgent>,
    pub transport: Transport,
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
            agent: None,
            transport: Transport::Process {
                argv: lens.command.clone(),
            },
            input: Input::Json,
            path: search_path(None),
        };
    };
    let model = lens.model.as_deref().filter(|model| !model.is_empty());
    let (transport, path) = match (agent.profile().reach, adapter(agent)) {
        (AgentReach::Program { name }, Adapter::Program(program_agent)) => {
            let program = lens
                .program
                .clone()
                .or_else(|| find_program(name))
                .unwrap_or_else(|| PathBuf::from(name));
            let mut argv = vec![program.to_string_lossy().into_owned()];
            argv.extend(program_agent.args(model, &lens.allow));
            let transport = match program_agent.conversation() {
                Conversation::Stdin => Transport::Process { argv },
                Conversation::AppServer => Transport::AppServer { argv },
            };
            (transport, search_path(program.parent()))
        }
        (AgentReach::Server(profile), Adapter::Server(server_agent)) => {
            // Trimmed as `key_account` files the key: a URL pasted with a
            // trailing newline would otherwise be asked as written and fail.
            let endpoint = lens
                .endpoint
                .as_deref()
                .map(str::trim)
                .filter(|endpoint| !endpoint.is_empty())
                .unwrap_or(profile.endpoint)
                .trim_end_matches('/');
            let api_key = if !lens.api_key_command.is_empty() {
                ApiKey::Command(lens.api_key_command.clone())
            } else {
                key_account(lens).map_or(ApiKey::None, ApiKey::Stored)
            };
            let server = Server {
                agent,
                url: format!("{endpoint}{}", server_agent.chat_path()),
                models_url: format!("{endpoint}{}", server_agent.models_path()),
                model: model.unwrap_or_default().to_string(),
                system: lens.system.clone(),
                context_length: lens.context_length,
                api_key,
            };
            (Transport::Server(server), search_path(None))
        }
        _ => unreachable!("{agent:?}: its profile and its adapter reach it differently"),
    };
    Invocation {
        agent: Some(agent),
        transport,
        input: Input::Message { prompt },
        path,
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

/// The answer as an agent's events tell it.
#[derive(Debug, Default)]
pub(crate) struct Answer {
    text: String,
    /// The message whose deltas `text` is made of, for an agent that names
    /// them.
    item: Option<String>,
    /// The answer as the agent reported it when it finished, which wins
    /// over what was pieced together from the stream.
    reported: Option<String>,
    error: Option<String>,
}

impl Answer {
    /// Add `delta` to the answer.
    pub(crate) fn push(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    /// Drop what was pieced together so far: a new message begins, and
    /// only the last message is the answer.
    pub(crate) fn start_over(&mut self) {
        self.text.clear();
    }

    /// Add `delta` to the answer when it belongs to message `item`; one
    /// that belongs to another starts the answer over, so that only the
    /// last message is the answer.
    pub(crate) fn push_to(&mut self, item: &str, delta: &str) {
        if self.item.as_deref() != Some(item) {
            self.item = Some(item.to_string());
            self.text.clear();
        }
        self.text.push_str(delta);
    }

    /// The whole answer, as the agent reports it when it is done.
    pub(crate) fn report(&mut self, text: impl Into<String>) {
        self.reported = Some(text.into());
    }

    /// Why the agent has no answer.
    pub(crate) fn fail(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    fn finish(self) -> Result<String, String> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.reported.unwrap_or(self.text)),
        }
    }
}

/// Reads the answer out of what the command writes, as it writes it.
pub(crate) struct Decoder {
    /// `None` for a command whose output is the answer itself.
    reader: Option<&'static dyn Reader>,
    /// How much of the output has been read, up to the last whole line.
    consumed: usize,
    answer: Answer,
}

impl Decoder {
    /// A decoder for what `agent` writes, or a command of the reader's own
    /// when it is `None`.
    pub(crate) fn new(agent: Option<LensAgent>) -> Self {
        Self {
            reader: agent.map(reader),
            consumed: 0,
            answer: Answer::default(),
        }
    }

    /// The answer so far, given everything the command has written so far.
    pub(crate) fn feed<'a>(&'a mut self, output: &'a str) -> &'a str {
        let Some(reader) = self.reader else {
            return output;
        };
        while let Some(end) = output[self.consumed..].find('\n') {
            let line = &output[self.consumed..self.consumed + end];
            self.consumed += end + 1;
            read_line(reader, line, &mut self.answer);
        }
        &self.answer.text
    }

    /// The whole answer, given everything the command wrote; or why the
    /// agent says it has none.
    pub(crate) fn finish(mut self, output: &str) -> Result<String, String> {
        let Some(reader) = self.reader else {
            return Ok(output.to_string());
        };
        self.feed(output);
        read_line(reader, &output[self.consumed..], &mut self.answer);
        self.answer.finish()
    }
}

fn read_line(reader: &dyn Reader, line: &str, answer: &mut Answer) {
    let line = reader.event(line.trim());
    if let Ok(event) = serde_json::from_str::<Value>(line) {
        reader.read(&event, answer);
    }
}

/// The text `value` holds, if it is a string.
pub(crate) fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

/// Lets an adapter's tests read what the agent wrote, and look at how a lens
/// runs it.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use arto_config::LensDisplay;

    pub(crate) fn lens(agent: Option<LensAgent>, model: Option<&str>) -> Lens {
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
            allow: Vec::new(),
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
            on: Default::default(),
            shortcut: None,
        }
    }

    pub(crate) fn argv(run: &Invocation) -> &[String] {
        match &run.transport {
            Transport::Process { argv } | Transport::AppServer { argv } => argv,
            other => panic!("not a program: {other:?}"),
        }
    }

    pub(crate) fn server(run: &Invocation) -> &Server {
        match &run.transport {
            Transport::Server(server) => server,
            other => panic!("not a server: {other:?}"),
        }
    }

    /// The answer in `output`, all of what `agent` wrote.
    pub(crate) fn answer(agent: LensAgent, output: &str) -> Result<String, String> {
        Decoder::new(Some(agent)).finish(output)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn every_agent_is_reached_the_way_its_profile_says() {
        for agent in LensAgent::ALL {
            let program = matches!(agent.profile().reach, AgentReach::Program { .. });
            assert_eq!(
                matches!(adapter(agent), Adapter::Program(_)),
                program,
                "{agent:?}"
            );
            // Would panic if the two disagreed.
            invocation(&lens(Some(agent), Some("m")));
        }
    }

    #[test]
    fn a_command_runs_as_written_and_reads_json() {
        let run = invocation(&lens(None, None));
        assert_eq!(argv(&run), ["my-tool", "--flag"]);
        assert_eq!(run.agent, None);
        assert_eq!(run.input, Input::Json);
    }

    #[test]
    fn an_agent_is_handed_the_prompt_as_a_message() {
        for agent in LensAgent::ALL {
            let run = invocation(&lens(Some(agent), Some("m")));
            assert_eq!(run.agent, Some(agent));
            assert_eq!(
                run.input,
                Input::Message {
                    prompt: Some("Translate.".to_string())
                }
            );
        }
    }

    #[test]
    fn the_program_directory_comes_first_on_the_path() {
        let run = invocation(&lens(Some(LensAgent::Claude), None));
        assert_eq!(argv(&run)[0], "/opt/agents/bin/agent");
        let path = run.path.expect("a path");
        let first = std::env::split_paths(&path).next();
        assert_eq!(first, Some(PathBuf::from("/opt/agents/bin")));
    }

    #[test]
    fn a_server_is_asked_under_its_endpoint_or_the_one_it_defaults_to() {
        let mut openai = lens(Some(LensAgent::Openai), Some("m"));
        openai.endpoint = Some("http://gpu-box:1234/v1/".to_string());
        let run = invocation(&openai);
        assert_eq!(server(&run).url, "http://gpu-box:1234/v1/chat/completions");
        assert_eq!(server(&run).models_url, "http://gpu-box:1234/v1/models");

        let ollama = invocation(&lens(Some(LensAgent::Ollama), Some("m")));
        assert_eq!(server(&ollama).url, "http://127.0.0.1:11434/api/chat");
    }

    #[test]
    fn a_server_is_asked_its_chat_with_the_system_prompt() {
        let mut openai = lens(Some(LensAgent::Openai), Some("gpt-5-mini"));
        openai.system = Some("Japanese".to_string());
        let body: Value =
            serde_json::from_str(&server(&invocation(&openai)).body("Hello")).unwrap();
        assert_eq!(body["model"], "gpt-5-mini");
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"],
            json!([
                {"role": "system", "content": "Japanese"},
                {"role": "user", "content": "Hello"},
            ])
        );
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
        assert_eq!(
            key_account(&ollama).as_deref(),
            Some("http://127.0.0.1:11434")
        );
        assert_eq!(key_account(&lens(Some(LensAgent::Claude), None)), None);
        assert_eq!(
            key_account(&lens(Some(LensAgent::Openai), Some("m"))).as_deref(),
            Some("https://api.openai.com/v1"),
            "OpenAI itself when no endpoint is set"
        );
    }

    #[test]
    fn a_line_still_being_written_is_not_read_yet() {
        let line = r#"{"model":"m","message":{"content":"one"}}"#;
        let output = format!("{line}\n{}", &line[..20]);
        let mut decoder = Decoder::new(Some(LensAgent::Ollama));
        assert_eq!(decoder.feed(&output), "one");
    }

    #[test]
    fn text_is_the_answer_as_it_is() {
        let mut decoder = Decoder::new(None);
        assert_eq!(decoder.feed("half"), "half");
        assert_eq!(
            decoder.finish("half and whole").as_deref(),
            Ok("half and whole")
        );
    }

    #[test]
    fn only_the_last_message_is_the_answer() {
        let mut answer = Answer::default();
        answer.push_to("a", "Let me look.");
        answer.push_to("b", "Ans");
        answer.push_to("b", "wer");
        assert_eq!(answer.finish().as_deref(), Ok("Answer"));
    }

    #[test]
    fn what_the_agent_reports_wins_and_a_failure_wins_over_both() {
        let mut answer = Answer::default();
        answer.push("pieced");
        answer.report("reported");
        assert_eq!(answer.finish().as_deref(), Ok("reported"));

        let mut answer = Answer::default();
        answer.report("reported");
        answer.fail("no");
        assert_eq!(answer.finish(), Err("no".to_string()));
    }
}
