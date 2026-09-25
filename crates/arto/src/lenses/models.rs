//! The models an agent offers, for the preferences to suggest.
//!
//! A program is run with the arguments its adapter names in place of the
//! lens's, a server is asked where its adapter says it lists them; what
//! either says back is the adapter's to read.

use super::agent::{self, Adapter, Invocation, Transport};
use super::{http, runner, session};
use arto_config::{Lens, LensAgent};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

/// How long an agent may take to say what it offers.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(20);

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
    let listing = match (&invocation.transport, agent::adapter(source.agent)) {
        (Transport::Process { argv } | Transport::AppServer { argv }, Adapter::Program(agent)) => {
            run(&invocation, argv, agent.models_args()).await?
        }
        (Transport::Server(server), Adapter::Server(_)) => {
            let key = session::api_key(&server.api_key, invocation.path.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            let headers = key
                .map(|key| vec![("authorization".to_string(), format!("Bearer {key}"))])
                .unwrap_or_default();
            http::get(&server.models_url, headers, LOOKUP_TIMEOUT)
                .await
                .map_err(|error| error.to_string())?
        }
        _ => return Ok(Vec::new()),
    };
    agent::reader(source.agent).models(&listing)
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

#[cfg(test)]
mod tests {
    use super::*;

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
