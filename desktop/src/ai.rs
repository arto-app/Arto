//! AI integration: provider invocation, prompt rendering, secret storage.
//!
//! High-level flow:
//! 1. UI picks an `AiProvider` from `Config::ai.providers`.
//! 2. `prompt::render` produces the user message body using the current
//!    document's content / selection / path.
//! 3. `client::stream_chat` issues the streaming request to the configured
//!    endpoint and feeds chunks back to the caller.
//!
//! Secrets referenced by `AiAuth::Bearer { auth_ref }` etc. are resolved via
//! `keychain::get_secret`. Secrets are never written to `config.json`.

#![allow(dead_code)] // UI consumers land in a follow-up step.

mod client;
mod keychain;
mod prompt;
mod protocol;

pub use client::{stream_chat, AiClientError, AiMessage, StreamEvent};
pub use keychain::{delete_secret, new_auth_ref, set_secret};
pub use prompt::{render as render_prompt, PromptInputs};
