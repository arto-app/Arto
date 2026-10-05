//! Running the command of a lens once: the input goes in on stdin, the
//! answer comes back on stdout — as it is written, for a lens that shows it
//! while it arrives.

use rust_i18n::t;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// The most a command may write to stdout. Past it the command is stopped:
/// no answer worth showing is this long, and one that keeps going would
/// hold the whole of it in memory.
pub(crate) const MAX_OUTPUT: usize = 16 * 1024 * 1024;

/// How much of the end of stderr is kept, and how many of its lines a
/// failure reports — enough to say why it failed, not a chatty tool's log.
pub(crate) const STDERR_TAIL_BYTES: usize = 64 * 1024;
pub(crate) const STDERR_LINES: usize = 5;

/// Why a run of the command produced no answer.
///
/// Its message is what the reader is shown for a failed run, so it is
/// written in the interface's language rather than derived by `thiserror`.
#[derive(Debug)]
pub(crate) enum RunError {
    NoCommand,
    Spawn {
        program: String,
        source: std::io::Error,
    },
    Io(std::io::Error),
    Timeout(Duration),
    TooLarge,
    Failed {
        status: Option<i32>,
        stderr: String,
    },
    NotUtf8,
    Unreachable(String),
    Status {
        status: u16,
        message: String,
    },
    ApiKey(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NoCommand => t!("lenses.errors.no_command"),
            Self::Spawn { program, source } => {
                t!("lenses.errors.spawn", program = program, reason = source)
            }
            Self::Io(source) => t!("lenses.errors.io", reason = source),
            Self::Timeout(timeout) => t!("lenses.errors.timeout", seconds = timeout.as_secs()),
            Self::TooLarge => t!("lenses.errors.too_large", mib = MAX_OUTPUT / 1024 / 1024),
            Self::Failed { status, stderr } => t!(
                "lenses.errors.failed",
                status = status_suffix(*status),
                stderr = stderr_suffix(stderr)
            ),
            Self::NotUtf8 => t!("lenses.errors.not_utf8"),
            Self::Unreachable(reason) => t!("lenses.errors.unreachable", reason = reason),
            Self::Status { status, message } => t!(
                "lenses.errors.status",
                status = status,
                message = stderr_suffix(message)
            ),
            Self::ApiKey(reason) => t!("lenses.errors.api_key", reason = reason),
        };
        f.write_str(&message)
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

fn status_suffix(status: Option<i32>) -> String {
    status.map_or_else(String::new, |code| {
        t!("lenses.errors.with_status", code = code).into_owned()
    })
}

fn stderr_suffix(stderr: &str) -> String {
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

/// Run `command` (program and arguments, no shell) in `cwd`, hand it
/// `input` on stdin and return what it wrote to stdout.
///
/// `on_output` is called with everything written so far each time more
/// arrives, cut at the last complete character.
///
/// The command runs in a process group of its own, and the whole group is
/// killed when the returned future is dropped before the command finished:
/// cancelling a run — the reader stopped the lens, or the document changed
/// under it — leaves no helper it started working on an answer nobody will
/// read.
pub(crate) async fn run(
    command: &[String],
    cwd: Option<&Path>,
    path: Option<&OsStr>,
    input: &str,
    timeout: Duration,
    on_output: impl FnMut(&str),
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
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    if let Some(path) = path {
        process.env("PATH", path);
    }
    let mut child = process.spawn().map_err(|source| RunError::Spawn {
        program: program.clone(),
        source,
    })?;
    // Declared after the child so that it is dropped first, while the child
    // is not yet reaped and its group id cannot have been reused.
    let mut group = GroupGuard::new(child.id());

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    // Written alongside the reads: a command that answers before it has read
    // all of its input would otherwise block on a full stdout pipe while we
    // block on a full stdin pipe.
    let write = async move {
        if let Some(mut stdin) = stdin {
            stdin.write_all(input.as_bytes()).await?;
            stdin.shutdown().await?;
        }
        Ok::<_, std::io::Error>(())
    };
    let exchange = async {
        let (written, output, stderr) = tokio::join!(
            write,
            read_output(stdout, on_output),
            read_tail(stderr, STDERR_TAIL_BYTES),
        );
        // A command may exit without reading all of its input; that is its
        // business, and its status says whether it went well.
        if let Err(error) = written {
            if error.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(RunError::Io(error));
            }
        }
        let output = output?;
        let status = child.wait().await.map_err(RunError::Io)?;
        Ok((status, output, stderr))
    };
    let (status, output, stderr) = tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| RunError::Timeout(timeout))??;
    group.disarm();

    if !status.success() {
        return Err(RunError::Failed {
            status: status.code(),
            stderr: tail_lines(&String::from_utf8_lossy(&stderr), STDERR_LINES),
        });
    }
    String::from_utf8(output).map_err(|_| RunError::NotUtf8)
}

/// Read `stdout` to the end, reporting what has arrived as it arrives.
async fn read_output(
    stdout: Option<impl AsyncRead + Unpin>,
    mut on_output: impl FnMut(&str),
) -> Result<Vec<u8>, RunError> {
    let mut output = Vec::new();
    let Some(mut stdout) = stdout else {
        return Ok(output);
    };
    let mut chunk = vec![0; 8192];
    loop {
        let read = stdout.read(&mut chunk).await.map_err(RunError::Io)?;
        if read == 0 {
            return Ok(output);
        }
        if output.len() + read > MAX_OUTPUT {
            return Err(RunError::TooLarge);
        }
        output.extend_from_slice(&chunk[..read]);
        on_output(complete_text(&output));
    }
}

/// `bytes` up to its last complete character: output read so far may end
/// inside one.
pub(crate) fn complete_text(bytes: &[u8]) -> &str {
    match std::str::from_utf8(bytes) {
        Ok(text) => text,
        // `valid_up_to` is a char boundary by definition.
        Err(error) => std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or_default(),
    }
}

/// The last `limit` bytes of `stderr`.
pub(crate) async fn read_tail(stderr: Option<impl AsyncRead + Unpin>, limit: usize) -> Vec<u8> {
    let mut tail = Vec::new();
    let Some(mut stderr) = stderr else {
        return tail;
    };
    let mut chunk = vec![0; 8192];
    while let Ok(read) = stderr.read(&mut chunk).await {
        if read == 0 {
            break;
        }
        tail.extend_from_slice(&chunk[..read]);
        if tail.len() > limit {
            tail.drain(..tail.len() - limit);
        }
    }
    tail
}

/// The last `count` non-empty lines of `text`.
pub(crate) fn tail_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

/// Kills the process group of a command that has not finished when it is
/// dropped. `kill_on_drop` reaches only the command itself; this reaches
/// what it started.
pub(crate) struct GroupGuard {
    #[cfg_attr(not(unix), allow(dead_code))]
    pid: Option<u32>,
}

impl GroupGuard {
    pub(crate) fn new(pid: Option<u32>) -> Self {
        Self { pid }
    }

    pub(crate) fn disarm(&mut self) {
        self.pid = None;
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid.and_then(|pid| i32::try_from(pid).ok()) {
            // SAFETY: `kill` takes plain integers and has no memory effects.
            // The group id is the unreaped child's pid, so it names no other
            // group.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const LONG: Duration = Duration::from_secs(10);

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".to_string(), "-c".to_string(), script.to_string()]
    }

    async fn quiet(command: &[String], input: &str, timeout: Duration) -> Result<String, RunError> {
        run(command, None, None, input, timeout, |_| {}).await
    }

    #[tokio::test]
    async fn the_input_goes_in_and_the_output_comes_back() {
        let output = quiet(&sh("tr a-z A-Z"), "hello", LONG).await.unwrap();
        assert_eq!(output, "HELLO");
    }

    #[tokio::test]
    async fn the_output_is_reported_as_it_arrives() {
        let mut seen = Vec::new();
        let output = run(
            &sh("printf one; sleep 0.2; printf ' two'"),
            None,
            None,
            "",
            LONG,
            |text| seen.push(text.to_string()),
        )
        .await
        .unwrap();

        assert_eq!(output, "one two");
        assert_eq!(seen.first().map(String::as_str), Some("one"));
        assert_eq!(seen.last().map(String::as_str), Some("one two"));
    }

    #[tokio::test]
    async fn a_character_split_across_writes_is_reported_whole() {
        let mut seen = Vec::new();
        // "語" is e8 aa 9e; the first write stops inside it.
        let script = r"printf '\346\227\245\350'; sleep 0.2; printf '\252\236'";
        let output = run(&sh(script), None, None, "", LONG, |text| {
            seen.push(text.to_string())
        })
        .await
        .unwrap();

        assert_eq!(output, "日語");
        assert_eq!(seen.first().map(String::as_str), Some("日"));
    }

    #[tokio::test]
    async fn a_large_input_does_not_deadlock() {
        let input = "x".repeat(1 << 20);
        let output = quiet(&sh("cat"), &input, LONG).await.unwrap();
        assert_eq!(output.len(), input.len());
    }

    #[tokio::test]
    async fn a_command_that_ignores_its_input_still_answers() {
        let input = "x".repeat(1 << 20);
        let output = quiet(&sh("echo done"), &input, LONG).await.unwrap();
        assert_eq!(output, "done\n");
    }

    #[tokio::test]
    async fn it_runs_in_the_directory_it_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let output = run(&sh("pwd -P"), Some(dir.path()), None, "", LONG, |_| {})
            .await
            .unwrap();
        assert_eq!(
            output.trim_end(),
            dir.path().canonicalize().unwrap().to_str().unwrap()
        );
    }

    #[tokio::test]
    async fn a_failure_keeps_its_status_and_the_end_of_stderr() {
        let script = "for i in 1 2 3 4 5 6 7; do echo line $i >&2; done; exit 3";
        let error = quiet(&sh(script), "", LONG).await.unwrap_err();
        match error {
            RunError::Failed { status, stderr } => {
                assert_eq!(status, Some(3));
                assert_eq!(stderr, "line 3\nline 4\nline 5\nline 6\nline 7");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn too_much_output_stops_the_command() {
        let error = quiet(&sh("yes"), "", LONG).await.unwrap_err();
        assert!(matches!(error, RunError::TooLarge), "{error:?}");
    }

    #[tokio::test]
    async fn a_missing_program_cannot_start() {
        let command = vec!["/nonexistent/arto-lens".to_string()];
        let error = quiet(&command, "", LONG).await.unwrap_err();
        assert!(matches!(error, RunError::Spawn { .. }), "{error:?}");
    }

    #[tokio::test]
    async fn an_empty_command_does_not_run() {
        let error = quiet(&[], "", LONG).await.unwrap_err();
        assert!(matches!(error, RunError::NoCommand), "{error:?}");
    }

    #[tokio::test]
    async fn output_that_is_not_utf8_is_refused() {
        let error = quiet(&sh(r"printf '\377'"), "", LONG).await.unwrap_err();
        assert!(matches!(error, RunError::NotUtf8), "{error:?}");
    }

    #[tokio::test]
    async fn a_timeout_kills_what_the_command_started_too() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("finished");
        // The helper runs in the background, so killing `sh` alone would
        // leave it to write the marker.
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            r#"(sleep 1; touch "$0") & wait"#.to_string(),
            marker.to_str().unwrap().to_string(),
        ];

        let error = quiet(&command, "", Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(error, RunError::Timeout(_)), "{error:?}");

        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!marker.exists(), "a helper kept running after the timeout");
    }

    #[test]
    fn the_tail_skips_blank_lines() {
        assert_eq!(tail_lines("a\n\nb\n  \nc\n", 2), "b\nc");
        assert_eq!(tail_lines("", 2), "");
    }
}
