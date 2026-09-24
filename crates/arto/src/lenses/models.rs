//! The models an agent offers, for the preferences to suggest.
//!
//! Each agent says so differently: `codex` prints its catalog, Ollama and
//! OpenAI-compatible servers list theirs over HTTP. `claude` has no list; its
//! help names some of the aliases it takes, which are what a lens should ask
//! for anyway, since an alias follows the latest model of its line.

use super::agent::{self, Invocation, Transport};
use super::{http, runner, session};
use arto_config::{Lens, LensAgent};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

/// How long an agent may take to say what it offers.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(20);

/// The aliases `claude` takes that its help may not name.
const CLAUDE_ALIASES: [&str; 3] = ["opus", "sonnet", "haiku"];

/// What decides the models on offer: the agent, and where and as whom it
/// is asked. The model itself and the prompt do not.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSource {
    pub agent: LensAgent,
    pub program: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub api_key_command: Vec<String>,
}

impl ModelSource {
    /// Where `lens` would find its models, if it runs an agent.
    pub fn of(lens: &Lens) -> Option<Self> {
        Some(Self {
            agent: lens.agent?,
            program: lens.program.clone(),
            endpoint: lens.endpoint.clone(),
            api_key_command: lens.api_key_command.clone(),
        })
    }

    fn lens(&self) -> Lens {
        let mut lens = Lens::new("models");
        lens.set_agent(Some(self.agent));
        lens.program = self.program.clone();
        lens.endpoint = self.endpoint.clone();
        lens.api_key_command = self.api_key_command.clone();
        lens
    }
}

/// What each source offered when it was last asked, so that opening a lens
/// again does not ask again.
static OFFERED: LazyLock<Mutex<HashMap<ModelSource, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Forget what every source offered, so each is asked again: a key stored
/// or removed can change what a server lists, and the key is not part of
/// the source.
pub fn forget_offered_models() {
    OFFERED.lock().clear();
}

/// The models `source` offers, or why they could not be told.
pub async fn available_models(source: &ModelSource) -> Result<Vec<String>, String> {
    if let Some(models) = OFFERED.lock().get(source) {
        return Ok(models.clone());
    }
    let models = look_up(source).await?;
    OFFERED.lock().insert(source.clone(), models.clone());
    Ok(models)
}

async fn look_up(source: &ModelSource) -> Result<Vec<String>, String> {
    let invocation = agent::invocation(&source.lens());
    match (&invocation.transport, source.agent) {
        (Transport::Process { argv }, LensAgent::Claude) => {
            let help = run(&invocation, argv, &["--help"]).await?;
            Ok(claude_models(&help))
        }
        (Transport::AppServer { argv }, LensAgent::Codex) => {
            let catalog = run(&invocation, argv, &["debug", "models"]).await?;
            codex_models(&catalog)
        }
        (Transport::Server(server), agent) => {
            let key = session::api_key(&server.api_key, invocation.path.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            let headers = key
                .map(|key| vec![("authorization".to_string(), format!("Bearer {key}"))])
                .unwrap_or_default();
            let url = server.models_url();
            let body = http::get(&url, headers, LOOKUP_TIMEOUT)
                .await
                .map_err(|error| error.to_string())?;
            match agent {
                LensAgent::Ollama => ollama_models(&body),
                _ => openai_models(&body),
            }
        }
        _ => Ok(Vec::new()),
    }
}

/// Run the agent's program alone with `args` in place of the flags a lens
/// runs it with.
async fn run(invocation: &Invocation, argv: &[String], args: &[&str]) -> Result<String, String> {
    let program = argv.first().ok_or("the agent has no program")?;
    let command: Vec<String> = std::iter::once(program.clone())
        .chain(args.iter().map(|arg| arg.to_string()))
        .collect();
    runner::run(
        &command,
        None,
        invocation.path.as_deref(),
        "",
        LOOKUP_TIMEOUT,
        |_| {},
    )
    .await
    .map_err(|error| error.to_string())
}

/// The aliases `claude --help` gives as examples for `--model`, then the
/// ones it is known to take.
///
/// Only aliases: an alias follows the latest model of its line, which is
/// what a lens wants, while the full name the help gives beside them is an
/// example that pins one release — and not necessarily the latest.
fn claude_models(help: &str) -> Vec<String> {
    let option = help
        .split("--model")
        .nth(1)
        .map(|rest| {
            // The option's description ends where the next option starts.
            rest.lines()
                .take_while(|line| !line.trim_start().starts_with('-') || line.contains("<model>"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    // Quotes are not paired by counting: an apostrophe in the prose, as in
    // "model's", would shift every pair after it. A name is whatever runs
    // from one quote to the next without a character a name cannot have.
    let quoted = option
        .match_indices('\'')
        .filter_map(|(at, _)| {
            let rest = &option[at + 1..];
            rest.find('\'').map(|end| &rest[..end])
        })
        // An alias is a bare word; a full name has a version in it.
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase()))
        .map(str::to_string);
    let mut models: Vec<String> = Vec::new();
    for name in quoted.chain(CLAUDE_ALIASES.iter().map(|alias| alias.to_string())) {
        if !models.contains(&name) {
            models.push(name);
        }
    }
    models
}

/// The models `codex debug models` lists for choosing: its catalog holds
/// internal ones too, which it marks as hidden.
fn codex_models(catalog: &str) -> Result<Vec<String>, String> {
    let catalog: Value = serde_json::from_str(catalog).map_err(|error| error.to_string())?;
    Ok(catalog["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|model| model["visibility"].as_str() != Some("hide"))
        .filter_map(|model| model["slug"].as_str().map(str::to_string))
        .collect())
}

/// The models an Ollama server has pulled, from `/api/tags`.
fn ollama_models(body: &str) -> Result<Vec<String>, String> {
    let tags: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    Ok(tags["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| model["name"].as_str().map(str::to_string))
        .collect())
}

/// The models an OpenAI-compatible server lists at `/models`, in name order.
fn openai_models(body: &str) -> Result<Vec<String>, String> {
    let list: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    let mut models: Vec<String> = list["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| model["id"].as_str().map(str::to_string))
        .collect();
    models.sort();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn claude_offers_the_aliases_its_help_gives_and_the_known_ones() {
        let help = indoc! {"
              --fallback-model <model>              Enable automatic fallback to 'sonnet'
              --model <model>                       Model for the current session. Provide
                                                    an alias for the latest model (e.g.
                                                    'fable', 'opus', or 'sonnet') or a
                                                    model's full name (e.g.
                                                    'claude-fable-5').
              -n, --name <name>                     Set a display name ('mine')
        "};

        assert_eq!(claude_models(help), ["fable", "opus", "sonnet", "haiku"]);
    }

    #[test]
    fn claude_offers_its_aliases_when_its_help_names_none() {
        assert_eq!(claude_models("Usage: claude"), ["opus", "sonnet", "haiku"]);
    }

    #[test]
    fn codex_offers_what_its_catalog_does_not_hide() {
        let catalog = r#"{"models": [
            {"slug": "gpt-reserve", "visibility": "hide"},
            {"slug": "gpt-5.6-sol", "visibility": "list"},
            {"slug": "gpt-5.5", "visibility": "list"}
        ]}"#;

        assert_eq!(codex_models(catalog).unwrap(), ["gpt-5.6-sol", "gpt-5.5"]);
    }

    #[test]
    fn ollama_offers_what_it_has_pulled() {
        let tags = r#"{"models": [{"name": "qwen3:4b-instruct", "size": 1}]}"#;

        assert_eq!(ollama_models(tags).unwrap(), ["qwen3:4b-instruct"]);
    }

    #[test]
    fn an_openai_server_offers_its_list_in_name_order() {
        let list = r#"{"object": "list", "data": [{"id": "gpt-b"}, {"id": "gpt-a"}]}"#;

        assert_eq!(openai_models(list).unwrap(), ["gpt-a", "gpt-b"]);
    }

    #[test]
    fn what_decides_the_models_leaves_the_prompt_out() {
        let mut lens = Lens::new("a");
        let before = ModelSource::of(&lens);
        lens.prompt = Some("something else".to_string());
        lens.model = Some("opus".to_string());

        assert_eq!(ModelSource::of(&lens), before);
        lens.set_agent(None);
        assert_eq!(ModelSource::of(&lens), None);
    }
}
