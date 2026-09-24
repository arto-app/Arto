//! Asking Codex through `codex app-server`, the JSON-RPC protocol its IDE
//! integrations speak.
//!
//! `codex exec --json` reports the answer only once it is whole, so a long
//! translation showed nothing for minutes. The app server reports the answer
//! as it is written (`item/agentMessage/delta`), at the price of a
//! conversation: the thread is named by the server, so the turn can only be
//! started once the server has answered.
//!
//! The app server has no `--ignore-user-config`. What the user's
//! configuration adds that a lens must not have — MCP servers above all,
//! which hand the model tools — is turned off for the thread instead: the
//! configuration is read first, and every MCP server it names is disabled.

use super::runner::{self, GroupGuard, RunError, MAX_OUTPUT, STDERR_LINES, STDERR_TAIL_BYTES};
use serde_json::{json, Value};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const INITIALIZE: u64 = 1;
const READ_CONFIG: u64 = 2;
const START_THREAD: u64 = 3;
const START_TURN: u64 = 4;

/// What the thread is started with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Thread<'a> {
    /// Replaces Codex's own instructions.
    pub instructions: &'a str,
    pub cwd: Option<&'a Path>,
}

/// Run the app server `command`, ask it `message` in a thread of its own and
/// return every line it wrote until the turn ended — for
/// [`super::agent::Decoder`] to read the answer out of.
///
/// `on_output` is called with the lines written so far each time one
/// arrives. The server is killed, with whatever it started, once the turn
/// has ended or when the returned future is dropped.
pub(crate) async fn run(
    command: &[String],
    path: Option<&OsStr>,
    thread: Thread<'_>,
    message: &str,
    timeout: Duration,
    mut on_output: impl FnMut(&str),
) -> Result<String, RunError> {
    let (program, args) = command.split_first().ok_or(RunError::NoCommand)?;
    let mut process = Command::new(program);
    process
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    process.process_group(0);
    if let Some(cwd) = thread.cwd {
        process.current_dir(cwd);
    }
    if let Some(path) = path {
        process.env("PATH", path);
    }
    let mut child = process.spawn().map_err(|source| RunError::Spawn {
        program: program.clone(),
        source,
    })?;
    // Never disarmed: the server does not exit when the turn ends.
    let _group = GroupGuard::new(child.id());
    let mut stdin = child.stdin.take().ok_or_else(closed)?;
    let mut stdout = BufReader::new(child.stdout.take().ok_or_else(closed)?);
    let mut stderr = Box::pin(runner::read_tail(child.stderr.take(), STDERR_TAIL_BYTES));

    let exchange = async move {
        send(
            &mut stdin,
            json!({"id": INITIALIZE, "method": "initialize", "params": {
                "clientInfo": {"name": "arto", "version": env!("CARGO_PKG_VERSION")},
            }}),
        )
        .await?;
        send(&mut stdin, json!({"method": "initialized"})).await?;
        send(&mut stdin, read_config(thread)).await?;

        let mut output = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            if stdout.read_line(&mut line).await.map_err(RunError::Io)? == 0 {
                // The server went away before the turn ended.
                return Ok((output, false));
            }
            if output.len() + line.len() > MAX_OUTPUT {
                return Err(RunError::TooLarge);
            }
            output.push_str(&line);
            if !line.ends_with('\n') {
                output.push('\n');
            }
            on_output(&output);
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match reply(&event, thread, message) {
                Step::Send(request) => send(&mut stdin, request).await?,
                Step::Wait => {}
                Step::Done => return Ok((output, true)),
            }
        }
    };
    // stderr is drained alongside, or a server logging freely would block on
    // a full pipe; it is only read at its end when the server went away.
    let mut exchange = Box::pin(tokio::time::timeout(timeout, exchange));
    let mut tail = None;
    let exchanged = loop {
        tokio::select! {
            exchanged = &mut exchange => break exchanged,
            bytes = &mut stderr, if tail.is_none() => tail = Some(bytes),
        }
    };
    let (output, ended) = match exchanged.map_err(|_| RunError::Timeout(timeout))? {
        Err(RunError::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => {
            (String::new(), false)
        }
        exchanged => exchanged?,
    };
    // Closes stdin.
    drop(exchange);
    if !ended {
        // Whatever the exit status, a server that went away before the turn
        // ended left no whole answer behind.
        let status = child.wait().await.map_err(RunError::Io)?;
        let stderr = match tail {
            Some(tail) => tail,
            None => stderr.await,
        };
        return Err(RunError::Failed {
            status: status.code(),
            stderr: runner::tail_lines(&String::from_utf8_lossy(&stderr), STDERR_LINES),
        });
    }
    Ok(output)
}

/// Reads the configuration as the thread will see it from its `cwd`, so the
/// MCP servers a project layer there adds are turned off too.
fn read_config(thread: Thread<'_>) -> Value {
    json!({"id": READ_CONFIG, "method": "config/read", "params": {
        "cwd": thread.cwd.and_then(Path::to_str),
    }})
}

fn closed() -> RunError {
    RunError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
}

async fn send(stdin: &mut tokio::process::ChildStdin, message: Value) -> Result<(), RunError> {
    let mut line = message.to_string();
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(RunError::Io)?;
    stdin.flush().await.map_err(RunError::Io)
}

/// What to do after the server wrote `event`.
#[derive(Debug, PartialEq)]
enum Step {
    Send(Value),
    Wait,
    Done,
}

fn reply(event: &Value, thread: Thread<'_>, message: &str) -> Step {
    let id = event["id"].as_u64();
    if event.get("method").is_some() {
        return match (id, event["method"].as_str()) {
            // A request from the server — an approval, most likely. A lens
            // grants nothing; with approvals off none should come.
            (Some(_), _) => Step::Send(json!({
                "id": event["id"],
                "error": {"code": -32601, "message": "not supported by Arto"},
            })),
            (None, Some("turn/completed")) => Step::Done,
            _ => Step::Wait,
        };
    }
    if event.get("error").is_some() {
        return Step::Done;
    }
    match id {
        Some(READ_CONFIG) => Step::Send(
            json!({"id": START_THREAD, "method": "thread/start", "params": {
                "ephemeral": true,
                "sandbox": "read-only",
                "approvalPolicy": "never",
                "baseInstructions": thread.instructions,
                "cwd": thread.cwd.and_then(Path::to_str),
                "config": without_mcp_servers(&event["result"]["config"]),
            }}),
        ),
        Some(START_THREAD) => {
            Step::Send(json!({"id": START_TURN, "method": "turn/start", "params": {
                "threadId": event["result"]["thread"]["id"],
                "input": [{"type": "text", "text": message}],
            }}))
        }
        _ => Step::Wait,
    }
}

/// Overrides that turn off every MCP server `config` names.
fn without_mcp_servers(config: &Value) -> Value {
    let servers = config["mcp_servers"].as_object();
    Value::Object(
        servers
            .into_iter()
            .flat_map(|servers| servers.keys())
            .map(|name| (format!("mcp_servers.{name}.enabled"), Value::Bool(false)))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const THREAD: Thread<'static> = Thread {
        instructions: "Be brief.",
        cwd: None,
    };

    #[test]
    fn the_thread_starts_once_the_configuration_is_read_without_its_mcp_servers() {
        let event = json!({"id": READ_CONFIG, "result": {"config": {
            "mcp_servers": {"node_repl": {"command": "x"}, "docs": {"url": "y"}},
        }}});
        let Step::Send(request) = reply(&event, THREAD, "hi") else {
            panic!("no thread started");
        };
        assert_eq!(request["method"], "thread/start");
        assert_eq!(request["params"]["baseInstructions"], "Be brief.");
        assert_eq!(request["params"]["sandbox"], "read-only");
        assert_eq!(request["params"]["ephemeral"], true);
        assert_eq!(
            request["params"]["config"],
            json!({"mcp_servers.node_repl.enabled": false, "mcp_servers.docs.enabled": false})
        );
    }

    #[test]
    fn the_configuration_is_read_from_where_the_thread_runs() {
        let cwd = Path::new("/docs/project");
        let request = read_config(Thread {
            cwd: Some(cwd),
            ..THREAD
        });
        assert_eq!(request["method"], "config/read");
        assert_eq!(request["params"]["cwd"], "/docs/project");
    }

    #[test]
    fn the_turn_asks_the_message_in_the_thread_the_server_named() {
        let event = json!({"id": START_THREAD, "result": {"thread": {"id": "t-1"}}});
        let Step::Send(request) = reply(&event, THREAD, "Translate.") else {
            panic!("no turn started");
        };
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "t-1");
        assert_eq!(request["params"]["input"][0]["text"], "Translate.");
    }

    #[test]
    fn a_request_from_the_server_is_refused() {
        let event =
            json!({"id": 0, "method": "item/commandExecution/requestApproval", "params": {}});
        let Step::Send(response) = reply(&event, THREAD, "") else {
            panic!("not answered");
        };
        assert_eq!(response["id"], 0);
        assert!(response.get("error").is_some());
    }

    #[test]
    fn the_exchange_ends_with_the_turn_or_a_refused_request() {
        let completed = json!({"method": "turn/completed", "params": {}});
        assert_eq!(reply(&completed, THREAD, ""), Step::Done);
        let refused = json!({"id": START_TURN, "error": {"message": "no"}});
        assert_eq!(reply(&refused, THREAD, ""), Step::Done);
        let delta = json!({"method": "item/agentMessage/delta", "params": {"delta": "x"}});
        assert_eq!(reply(&delta, THREAD, ""), Step::Wait);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_server_is_driven_through_its_turn_and_left_running_no_longer() {
        let dir = tempfile::tempdir().unwrap();
        let received = dir.path().join("received");
        // A server that answers each request in turn, streams two deltas,
        // and would then stay up far longer than the test waits.
        let script = indoc::indoc! {r#"
            read l; echo "$l" >> "$0"; echo '{"id":1,"result":{}}'
            read l; echo "$l" >> "$0"
            read l; echo "$l" >> "$0"; echo '{"id":2,"result":{"config":{}}}'
            read l; echo "$l" >> "$0"; echo '{"id":3,"result":{"thread":{"id":"t"}}}'
            read l; echo "$l" >> "$0"; echo '{"id":4,"result":{}}'
            echo '{"method":"item/agentMessage/delta","params":{"itemId":"m","delta":"one"}}'
            echo '{"method":"item/agentMessage/delta","params":{"itemId":"m","delta":" two"}}'
            echo '{"method":"turn/completed","params":{"turn":{"status":"completed"}}}'
            sleep 30
        "#};
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            script.to_string(),
            received.to_str().unwrap().to_string(),
        ];
        let mut seen = 0;
        let started = std::time::Instant::now();
        let output = run(
            &command,
            None,
            THREAD,
            "Say it.",
            Duration::from_secs(10),
            |_| seen += 1,
        )
        .await
        .unwrap();

        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            output.ends_with("\"status\":\"completed\"}}}\n"),
            "{output}"
        );
        assert!(seen >= 7, "{seen}");
        let received = std::fs::read_to_string(&received).unwrap();
        let methods: Vec<String> = received
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["method"].to_string())
            .collect();
        assert_eq!(
            methods,
            [
                "\"initialize\"",
                "\"initialized\"",
                "\"config/read\"",
                "\"thread/start\"",
                "\"turn/start\""
            ]
        );
    }

    /// Asks the installed `codex` for real; run with `--ignored` to see the
    /// answer arrive in pieces.
    #[tokio::test]
    #[ignore = "asks the installed codex, which needs a login and the network"]
    async fn the_installed_codex_streams_its_answer() {
        use super::super::agent::{self, Decoder, Transport};
        let lens: arto_config::Lens = serde_json::from_value(
            json!({"id": "t", "label": "T", "display": "page", "agent": "codex"}),
        )
        .unwrap();
        let invocation = agent::invocation(&lens);
        let Transport::AppServer { argv } = &invocation.transport else {
            panic!("not the app server");
        };
        let mut decoder = Decoder::new(invocation.format);
        let started = std::time::Instant::now();
        let mut arrivals = Vec::new();
        let output = run(
            argv,
            invocation.path.as_deref(),
            Thread {
                instructions: agent::SYSTEM_PROMPT,
                cwd: None,
            },
            "Write three short paragraphs about the sea, separated by blank lines.",
            Duration::from_secs(120),
            |written| {
                let answer = decoder.feed(written);
                if arrivals.last().map(|(_, length)| *length) != Some(answer.len()) {
                    arrivals.push((started.elapsed(), answer.len()));
                }
            },
        )
        .await
        .unwrap();
        let answer = decoder.finish(&output).unwrap();

        eprintln!("{answer}\n\narrivals (elapsed, length): {arrivals:?}");
        assert!(arrivals.len() > 2, "the answer came whole: {arrivals:?}");
        assert_eq!(arrivals.last().unwrap().1, answer.len());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_server_that_exits_early_fails_with_its_stderr() {
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo 'not logged in' >&2; exit 1".to_string(),
        ];
        let error = run(&command, None, THREAD, "", Duration::from_secs(10), |_| {})
            .await
            .unwrap_err();
        match error {
            RunError::Failed { status, stderr } => {
                assert_eq!(status, Some(1));
                assert_eq!(stderr, "not logged in");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_server_that_exits_cleanly_before_the_turn_ends_still_fails() {
        let script = indoc::indoc! {r#"
            read l; echo '{"id":1,"result":{}}'
            read l
            read l; echo '{"id":2,"result":{"config":{}}}'
            read l; echo 'shutting down' >&2; exit 0
        "#};
        let command = vec!["sh".to_string(), "-c".to_string(), script.to_string()];
        let error = run(&command, None, THREAD, "", Duration::from_secs(10), |_| {})
            .await
            .unwrap_err();
        match error {
            RunError::Failed { status, stderr } => {
                assert_eq!(status, Some(0));
                assert_eq!(stderr, "shutting down");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
