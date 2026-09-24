//! Running a lens's command on its requests: a bounded number at a time,
//! each answer handed on the moment it arrives.

use super::agent::{ApiKey, Decoder, Input, Invocation, Transport, SYSTEM_PROMPT};
use super::job::{Job, Request};
use super::runner;
use super::{app_server, http};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

/// How the command of a lens is run.
#[derive(Debug, Clone)]
pub(crate) struct Runner {
    pub invocation: Arc<Invocation>,
    /// The directory of the document, so that a command reading files the
    /// document names finds them where the document does.
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub concurrency: usize,
}

impl Runner {
    /// Run the command once on `request`: an agent is asked in prose, a
    /// command is handed JSON. `on_output` gets the answer so far each time
    /// more of it arrives.
    pub(crate) async fn run(
        &self,
        request: &Request,
        mut on_output: impl FnMut(&str),
    ) -> Result<String, String> {
        let invocation = &self.invocation;
        let input = match &invocation.input {
            Input::Json => request.to_json(),
            Input::Message { prompt } => request.to_message(prompt.as_deref()),
        };
        let mut decoder = Decoder::new(invocation.format);
        let output = match &invocation.transport {
            Transport::Process { argv } => {
                runner::run(
                    argv,
                    self.cwd.as_deref(),
                    invocation.path.as_deref(),
                    &input,
                    self.timeout,
                    |written| on_output(decoder.feed(written)),
                )
                .await
            }
            Transport::AppServer { argv } => {
                let thread = app_server::Thread {
                    instructions: SYSTEM_PROMPT,
                    cwd: self.cwd.as_deref(),
                };
                app_server::run(
                    argv,
                    invocation.path.as_deref(),
                    thread,
                    &input,
                    self.timeout,
                    |written| on_output(decoder.feed(written)),
                )
                .await
            }
            Transport::Server(server) => {
                let headers = match api_key(&server.api_key, invocation.path.as_deref()).await {
                    Ok(Some(key)) => vec![("authorization".to_string(), format!("Bearer {key}"))],
                    Ok(None) => Vec::new(),
                    Err(error) => return Err(error.to_string()),
                };
                http::post(
                    &server.url,
                    headers,
                    server.body(&input),
                    self.timeout,
                    |written| on_output(decoder.feed(written)),
                )
                .await
            }
        }
        .map_err(|error| error.to_string())?;
        decoder.finish(&output)
    }
}

/// The API key `key` names. A program that prints it is run for every
/// request, so that a rotated key is picked up without restarting, and is
/// looked for along `path`; a stored key is kept once read (see
/// [`super::secrets`]), and a server with none stored is asked without one.
pub(crate) async fn api_key(
    key: &ApiKey,
    path: Option<&std::ffi::OsStr>,
) -> Result<Option<String>, runner::RunError> {
    let key = match key {
        ApiKey::None => return Ok(None),
        ApiKey::Stored(account) => {
            let account = account.clone();
            let stored = tokio::task::spawn_blocking(move || super::secrets::read(&account))
                .await
                .map_err(|error| runner::RunError::ApiKey(error.to_string()))?
                .map_err(runner::RunError::ApiKey)?;
            match stored {
                Some(key) => key,
                None => return Ok(None),
            }
        }
        ApiKey::Command(argv) => runner::run(argv, None, path, "", API_KEY_TIMEOUT, |_| {})
            .await
            .map_err(|error| runner::RunError::ApiKey(error.to_string()))?,
    };
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err(runner::RunError::ApiKey("it is empty".to_string()));
    }
    Ok(Some(key))
}

/// How long the program that prints an API key may take — a password
/// manager may ask to be unlocked first.
const API_KEY_TIMEOUT: Duration = Duration::from_secs(60);

/// Run `jobs` and call `report` with each job and its answer, or the reason
/// it has none, in the order they finish.
///
/// No more jobs than the runner's concurrency exist at once; the rest wait
/// in line. Dropping the returned future cancels every run in flight and
/// kills its process, which is how a lens is stopped.
pub(crate) async fn run_jobs(
    jobs: Vec<Job>,
    runner: Runner,
    mut report: impl FnMut(Job, Result<String, String>),
) {
    let mut waiting: VecDeque<Job> = jobs.into();
    let mut running = JoinSet::new();
    let concurrency = runner.concurrency.max(1);

    loop {
        while running.len() < concurrency {
            let Some(job) = waiting.pop_front() else {
                break;
            };
            let runner = runner.clone();
            running.spawn(async move {
                let answer = runner.run(&job.request, |_| {}).await;
                (job, answer)
            });
        }
        let Some(finished) = running.join_next().await else {
            return;
        };
        if let Ok((job, answer)) = finished {
            report(job, answer);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::agent::OutputFormat;
    use super::super::job;
    use super::*;
    use arto_config::{Lens, LensDisplay};
    use std::path::Path;

    fn lens() -> Lens {
        Lens {
            id: "t".to_string(),
            label: "T".to_string(),
            display: LensDisplay::Popover,
            agent: None,
            model: None,
            prompt: None,
            program: None,
            system: None,
            endpoint: None,
            api_key_command: Vec::new(),
            context_length: None,
            command: Vec::new(),
            context: 0,
            concurrency: 1,
            timeout_seconds: 10,
            unit: Default::default(),
            shortcut: None,
        }
    }

    fn job(id: &str, markdown: &str) -> Job {
        job::whole(&lens(), Path::new("/doc.md"), markdown, None, id)
    }

    fn runner(script: &str, concurrency: usize, prompt: Option<&str>) -> Runner {
        Runner {
            invocation: Arc::new(Invocation {
                transport: Transport::Process {
                    argv: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
                },
                format: OutputFormat::Text,
                input: match prompt {
                    Some(prompt) => Input::Message {
                        prompt: Some(prompt.to_string()),
                    },
                    None => Input::Json,
                },
                path: None,
            }),
            cwd: None,
            timeout: Duration::from_secs(10),
            concurrency,
        }
    }

    async fn collect(jobs: Vec<Job>, runner: Runner) -> Vec<(String, Result<String, String>)> {
        let mut results = Vec::new();
        run_jobs(jobs, runner, |job, answer| results.push((job.id, answer))).await;
        results.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    #[tokio::test]
    async fn a_command_reads_json_and_every_job_reports_its_answer() {
        let results = collect(
            vec![job("a", "one"), job("b", "two")],
            runner("cat", 2, None),
        )
        .await;

        assert_eq!(results.len(), 2);
        for ((id, answer), markdown) in results.iter().zip(["one", "two"]) {
            let input: serde_json::Value = serde_json::from_str(answer.as_ref().unwrap()).unwrap();
            assert_eq!(input["markdown"], markdown, "{id}");
        }
    }

    #[tokio::test]
    async fn an_agent_is_asked_in_prose() {
        let results = collect(vec![job("a", "one")], runner("cat", 1, Some("Shout it."))).await;

        let answer = results[0].1.as_ref().unwrap();
        assert_eq!(answer, "Shout it.\n\none\n");
    }

    #[tokio::test]
    async fn a_failure_is_reported_with_its_reason() {
        let results = collect(vec![job("a", "")], runner("echo nope >&2; exit 1", 1, None)).await;

        let (_, answer) = &results[0];
        assert!(answer.as_ref().unwrap_err().contains("nope"), "{answer:?}");
    }

    #[tokio::test]
    async fn no_more_than_the_allowed_runs_are_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        // Each run reports how many others were running when it started.
        let script = format!(
            "cd '{}'; n=$(ls | wc -l | tr -d ' '); touch $$; sleep 0.2; rm $$; printf %s $n",
            dir.path().display()
        );
        let jobs = ["a", "b", "c"].iter().map(|id| job(id, "")).collect();

        let results = collect(jobs, runner(&script, 1, None)).await;

        assert!(
            results
                .iter()
                .all(|(_, answer)| answer.as_deref() == Ok("0")),
            "{results:?}"
        );
    }
}
