//! Streaming AI chat client.
//!
//! Wraps `ureq` (sync) inside `tokio::task::spawn_blocking` and delivers
//! incremental text deltas to the caller via an mpsc channel. Cancellation
//! is cooperative — the worker checks an `AtomicBool` between SSE lines.

use anyhow::{anyhow, Context, Result};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::ai::keychain;
use crate::ai::protocol::openai;
use crate::config::{AiAuth, AiProtocol, AiProvider};

/// Role of a chat message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

impl ChatRole {
    fn as_str(self) -> &'static str {
        match self {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }
}

/// One chat message.
#[derive(Debug, Clone)]
pub struct AiMessage {
    pub role: ChatRole,
    pub content: String,
}

impl AiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// Events delivered by [`stream_chat`] over the receiver.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// One incremental text delta from the assistant.
    Delta(String),
    /// Stream completed normally.
    Done,
    /// Stream failed; included as the final event.
    Error(String),
}

/// Handle to a running streaming request. Drop to leak the worker; call
/// [`cancel`] to signal the worker to stop early.
pub struct StreamHandle {
    cancel: Arc<AtomicBool>,
}

impl StreamHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Errors produced while preparing a request. Errors during the stream are
/// instead delivered as [`StreamEvent::Error`].
#[derive(Debug, Error)]
pub enum AiClientError {
    #[error("missing keychain secret for auth_ref {auth_ref}")]
    MissingSecret { auth_ref: String },
    #[error("unsupported protocol")]
    UnsupportedProtocol,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Start a streaming chat request. Returns immediately after spawning a
/// background worker; events are delivered via the returned receiver.
pub fn stream_chat(
    provider: &AiProvider,
    messages: Vec<AiMessage>,
) -> std::result::Result<(StreamHandle, mpsc::UnboundedReceiver<StreamEvent>), AiClientError> {
    if !matches!(provider.protocol, AiProtocol::OpenAiCompatible) {
        return Err(AiClientError::UnsupportedProtocol);
    }

    let auth_header = resolve_auth_header(&provider.auth)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::unbounded_channel();

    let provider = provider.clone();
    let cancel_for_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        let result = run_openai_stream(&provider, &messages, auth_header, &tx, &cancel_for_worker);
        match result {
            Ok(()) => {
                let _ = tx.send(StreamEvent::Done);
            }
            Err(err) => {
                let _ = tx.send(StreamEvent::Error(format!("{err:#}")));
            }
        }
    });

    Ok((StreamHandle { cancel }, rx))
}

/// Resolve [`AiAuth`] into the `(header_name, header_value)` pair to attach,
/// loading any secret from the keychain.
fn resolve_auth_header(auth: &AiAuth) -> Result<Option<(String, String)>, AiClientError> {
    match auth {
        AiAuth::None => Ok(None),
        AiAuth::Bearer { auth_ref } => {
            let secret = keychain::get_secret(auth_ref)
                .map_err(AiClientError::Other)?
                .ok_or_else(|| AiClientError::MissingSecret {
                    auth_ref: auth_ref.clone(),
                })?;
            Ok(Some((
                "Authorization".to_string(),
                format!("Bearer {secret}"),
            )))
        }
        AiAuth::Header { name, auth_ref } => {
            let secret = keychain::get_secret(auth_ref)
                .map_err(AiClientError::Other)?
                .ok_or_else(|| AiClientError::MissingSecret {
                    auth_ref: auth_ref.clone(),
                })?;
            Ok(Some((name.clone(), secret)))
        }
    }
}

fn run_openai_stream(
    provider: &AiProvider,
    messages: &[AiMessage],
    auth_header: Option<(String, String)>,
    tx: &mpsc::UnboundedSender<StreamEvent>,
    cancel: &AtomicBool,
) -> Result<()> {
    // Build OpenAI-shape body. Borrow message contents to avoid copies.
    let chat_messages: Vec<openai::ChatMessage<'_>> = messages
        .iter()
        .map(|m| openai::ChatMessage {
            role: m.role.as_str(),
            content: &m.content,
        })
        .collect();
    let body = openai::ChatRequest {
        model: &provider.model,
        messages: chat_messages,
        stream: true,
        temperature: provider.temperature,
    };
    let body_json = serde_json::to_string(&body).context("serialize chat request")?;

    let mut request = ureq::post(&provider.api_url)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream");
    if let Some((name, value)) = auth_header.as_ref() {
        request = request.header(name.as_str(), value.as_str());
    }

    let response = request
        .send(body_json)
        .with_context(|| format!("HTTP request to {} failed", provider.api_url))?;

    let status = response.status();
    if !status.is_success() {
        let text = response
            .into_body()
            .read_to_string()
            .unwrap_or_else(|_| "<failed to read error body>".to_string());
        return Err(anyhow!("HTTP {status}: {text}"));
    }

    let reader = BufReader::new(response.into_body().into_reader());
    parse_sse_stream(reader, tx, cancel)
}

/// Parse an SSE stream of OpenAI-shape chunks, sending each text delta
/// over `tx`. Returns when the stream ends (either via `[DONE]` or EOF) or
/// when `cancel` is set.
fn parse_sse_stream<R: BufRead>(
    reader: R,
    tx: &mpsc::UnboundedSender<StreamEvent>,
    cancel: &AtomicBool,
) -> Result<()> {
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let line = line.context("read SSE line")?;
        let payload = match line.strip_prefix("data:") {
            Some(rest) => rest.trim(),
            None => continue, // ignore comments, event:, id:, blank lines
        };
        if payload.is_empty() {
            continue;
        }
        if payload == "[DONE]" {
            return Ok(());
        }
        let chunk: openai::StreamChunk = match serde_json::from_str(payload) {
            Ok(c) => c,
            Err(err) => {
                tracing::debug!(?err, %payload, "skip malformed SSE chunk");
                continue;
            }
        };
        if let Some(delta) = openai::extract_delta(&chunk) {
            if tx.send(StreamEvent::Delta(delta.to_string())).is_err() {
                // Receiver dropped — caller no longer interested.
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn collect_events(input: &str) -> (Vec<String>, bool) {
        let cancel = AtomicBool::new(false);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let result = parse_sse_stream(Cursor::new(input.as_bytes()), &tx, &cancel);
        drop(tx);

        let mut deltas = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let StreamEvent::Delta(s) = ev {
                deltas.push(s);
            }
        }
        (deltas, result.is_ok())
    }

    #[test]
    fn parses_simple_stream_with_done_terminator() {
        let input = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\
                     data: [DONE]\n";
        let (deltas, ok) = collect_events(input);
        assert!(ok);
        assert_eq!(deltas, vec!["Hello".to_string(), " world".to_string()]);
    }

    #[test]
    fn ignores_blank_lines_and_comments() {
        let input = ":heartbeat\n\
                     \n\
                     data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\
                     event: ping\n\
                     data: [DONE]\n";
        let (deltas, ok) = collect_events(input);
        assert!(ok);
        assert_eq!(deltas, vec!["x".to_string()]);
    }

    #[test]
    fn skips_malformed_chunks_without_aborting() {
        let input = "data: not-json\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\
                     data: [DONE]\n";
        let (deltas, ok) = collect_events(input);
        assert!(ok);
        assert_eq!(deltas, vec!["ok".to_string()]);
    }

    #[test]
    fn skips_role_only_chunks() {
        let input = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\
                     data: [DONE]\n";
        let (deltas, _) = collect_events(input);
        assert_eq!(deltas, vec!["hi".to_string()]);
    }

    #[test]
    fn cancellation_stops_processing() {
        let input = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\
                     data: [DONE]\n";
        let cancel = AtomicBool::new(true);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let _ = parse_sse_stream(Cursor::new(input.as_bytes()), &tx, &cancel);
        drop(tx);
        // No deltas should have been sent because cancel was already set.
        assert!(rx.try_recv().is_err());
    }
}
