//! Lenses: commands the reader configured to look at a document through.
//!
//! A lens hands its command the source — one block at a time with its
//! neighbours as context, or the whole of what it looks at in one go — and
//! shows the answer with the document, which itself is never changed:
//!
//! - `annotate` keeps each block's answer beside it, shown on hover over a
//!   mark in the margin (`frontend/src/lenses.ts`);
//! - `popover` shows its one answer on request: from the header for the
//!   document, or on hover beside the block it looked at;
//! - `page` shows its answer — a document of its own, such as a
//!   translation — in the document's places, block by block from the top
//!   as it is written, with each block's original on hover.

mod agent;
mod app_server;
mod cache;
mod controller;
mod http;
mod job;
mod models;
mod runner;
mod secrets;
mod session;
mod store;
mod stream;

pub(crate) use agent::key_account;
pub(crate) use controller::{
    close_unoffered, follow_rerender, forget, hide, hide_run, offered_lenses, regenerate,
    regenerate_all, show, start, stop, stop_run, toggle,
};
pub(crate) use job::message_shape;
pub(crate) use models::{available_models, forget_offered_models, ModelSource};
pub(crate) use secrets::{forget_key, is_stored, store_key};

use crate::state::AppState;
use arto_config::LensDisplay;
use dioxus::core::Task;
use dioxus::prelude::ReadableExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A number no earlier call returned, across windows.
fn next_serial() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The source a document was rendered from, as it was when it was rendered:
/// the ranges on the page point into this text, not into whatever the file
/// holds by the time a lens reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedSource {
    pub path: PathBuf,
    pub source: Arc<str>,
    /// Distinct for every render, so an answer can tell whether the page it
    /// was meant for is still the one on screen.
    pub generation: u64,
}

impl RenderedSource {
    pub fn new(path: PathBuf, source: impl Into<Arc<str>>) -> Self {
        Self {
            path,
            source: source.into(),
            generation: next_serial(),
        }
    }
}

/// What a lens looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The block the keyboard cursor is on — the one a right-click marks.
    Cursor,
    Document,
}

/// The lenses open in a window: its page lenses first, then the lenses over
/// the page, each in the order they were opened.
///
/// A page lens takes the document's places, so only one is applied at a
/// time and the others wait to be switched to; the others only mark blocks
/// or open a popover, so any number can show beside it and each other.
pub(crate) fn open_runs(state: &AppState) -> Vec<LensRun> {
    state
        .page_lenses
        .read()
        .iter()
        .chain(state.overlay_lenses.read().iter())
        .cloned()
        .collect()
}

/// [`open_runs`] without subscribing the caller to the lenses: for the
/// controller, which reads them to act on them. Read from an effect — the
/// one following the page's renders — a subscription would run it again on
/// every answer a lens files.
fn peek_open_runs(state: &AppState) -> Vec<LensRun> {
    state
        .page_lenses
        .peek()
        .iter()
        .chain(state.overlay_lenses.peek().iter())
        .cloned()
        .collect()
}

/// A lens running in a window, or whose answer it is showing.
#[derive(Debug, Clone, PartialEq)]
pub struct LensRun {
    pub lens_id: String,
    pub label: String,
    pub display: LensDisplay,
    pub scope: Scope,
    /// The document the lens looked at.
    pub path: PathBuf,
    /// The render the lens looked at.
    pub generation: u64,
    /// Tags what the page marks for this run, so that nothing meant for an
    /// earlier run lands on the page this one marked.
    pub token: u64,
    /// How many places the lens answers for — blocks looked at one by one,
    /// or a page's blocks its answer takes the places of — and how each
    /// stands: answered about the text as it is, answered about what it
    /// said before it changed, not answered — none on file, or the run
    /// was stopped first — or failed. What is left is still being asked.
    pub total: usize,
    pub done: usize,
    pub outdated: usize,
    pub unanswered: usize,
    pub failed: usize,
    /// Present while the lens is running.
    pub task: Option<Task>,
    /// A popover over the document: its answer as rendered so far.
    pub html: Option<String>,
    /// Why a run over the whole produced no answer.
    pub error: Option<String>,
    /// Whether its answer is shown: a lens stays open while hidden, to be
    /// shown again at once, and only one page lens is shown at a time.
    pub applied: bool,
}

impl LensRun {
    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }

    /// The places still being asked about.
    pub fn pending(&self) -> usize {
        self.total
            .saturating_sub(self.done + self.outdated + self.unanswered + self.failed)
    }

    /// The places without an answer about the text as it is now — changed
    /// since, never asked, or failed: what the reader may ask again about
    /// without asking again about the rest.
    pub fn stale(&self) -> usize {
        self.outdated + self.unanswered + self.failed
    }
}
