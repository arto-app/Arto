//! Starting, stopping and closing lenses in a window: asking the page for
//! blocks, running the command, and showing each answer as it arrives.

mod page;
mod policy;

use super::agent;
use super::cache::ResultCache;
use super::job::{self, Block, Job};
use super::messages::Translated;
use super::session::{self, Runner};
use super::store::{Record, STORE};
use super::stream::finished_blocks;
use super::{next_serial, peek_open_runs, LensRun, RenderedSource, Scope};
use crate::config::CONFIG;
use crate::markdown;
use crate::state::AppState;
use crate::utils::task::spawn_detached_task;
use arto_config::{says_nothing, usable_lenses, Lens, LensDisplay, LensUnit};
use dioxus::core::Task;
use dioxus::prelude::*;
use page::*;
use parking_lot::Mutex;
use policy::*;
use rust_i18n::t;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

/// How much of earlier answers is kept for re-rendered documents, across
/// windows.
const CACHE_BYTES: usize = 64 * 1024 * 1024;

static CACHE: LazyLock<Mutex<ResultCache>> =
    LazyLock::new(|| Mutex::new(ResultCache::new(CACHE_BYTES)));

/// How often an answer that is still arriving is rendered again.
const STREAM_INTERVAL: Duration = Duration::from_millis(250);

/// The lenses the menus offer, in the order they are configured.
pub(crate) fn offered_lenses() -> Vec<Lens> {
    let config = CONFIG.read();
    let (usable, errors) = usable_lenses(&config.lenses);
    for error in errors {
        tracing::warn!(%error, "lens left out");
    }
    usable.into_iter().cloned().collect()
}

/// Look at `scope` through the lens `lens_id`, showing what it answered
/// before where it has.
///
/// A lens takes the place of its own earlier run. A page lens is applied —
/// takes the document's places — and the page lens applied before waits to
/// be switched back to; a lens over the page shows beside the others.
pub(crate) fn start(state: AppState, lens_id: &str, scope: Scope) {
    start_with(state, lens_id, scope, Policy::Kept, true);
}

/// Ask the run `token`'s agent again about whatever it answered from text
/// that has changed since.
pub(crate) fn regenerate(state: AppState, token: u64) {
    rerun(state, token, Policy::Regenerate);
}

/// Ask the run `token`'s agent again about everything, whatever it answered
/// before.
pub(crate) fn regenerate_all(state: AppState, token: u64) {
    rerun(state, token, Policy::All);
}

fn rerun(state: AppState, token: u64, policy: Policy) {
    let Some((lens_id, scope)) = with_run(state, token, |run| (run.lens_id.clone(), run.scope))
    else {
        return;
    };
    start_with(state, &lens_id, scope, policy, true);
}

/// Show the run `token`'s answer again, from what it answered before. For a
/// page lens, this switches the page to it, and the one applied before
/// waits to be switched back to. A lens hidden while it was still being
/// answered carries on where it was.
pub(crate) fn show(state: AppState, token: u64) {
    let Some((lens_id, scope, applied, running)) = with_run(state, token, |run| {
        (
            run.lens_id.clone(),
            run.scope,
            run.applied,
            run.is_running(),
        )
    }) else {
        return;
    };
    if applied {
        return;
    }
    let policy = if running {
        Policy::Regenerate
    } else {
        Policy::Kept
    };
    start_with(state, &lens_id, scope, policy, true);
}

/// Show or hide the lens at `place` among the offered lenses, as its
/// shortcut does: open it when it is not open, hide it when it shows, and
/// show it when it is hidden. It is shown over the document, or over the
/// block the cursor is on for a lens that is on a block only.
pub(crate) fn toggle(state: AppState, place: usize) {
    let Some(lens) = offered_lenses().into_iter().nth(place) else {
        return;
    };
    let scope = shortcut_scope(&lens);
    let run = peek_open_runs(&state)
        .into_iter()
        .find(|run| run.lens_id == lens.id && run.scope == scope);
    match run {
        Some(run) if run.applied => hide_run(state, run.token),
        Some(run) => show(state, run.token),
        None => start(state, &lens.id, scope),
    }
}

/// Take the run `token`'s answer off the page. The lens stays open, to be
/// shown again at once, and the document opens with it hidden; a run still
/// being answered goes on filing its answers.
///
/// A lens over one block is closed instead: there is nothing to show it
/// over again once the reader moves on.
pub(crate) fn hide_run(state: AppState, token: u64) {
    let Some((scope, page, applied)) = with_run(state, token, |run| {
        (run.scope, run.display == LensDisplay::Page, run.applied)
    }) else {
        return;
    };
    if scope == Scope::Cursor {
        dismiss(state, token);
        return;
    }
    if !applied {
        return;
    }
    update(state, token, |run| run.applied = false);
    remember(state, token);
    if page {
        eval_call("restorePage", &[]);
    } else {
        eval_call("restoreMarks", &[json(&token)]);
    }
}

/// Hide every lens showing, and show the document as written.
pub(crate) fn hide(state: AppState) {
    for run in peek_open_runs(&state) {
        hide_run(state, run.token);
    }
}

/// Delete what the run `token`'s lens answered about the document and take
/// the lens out of the window: the document no longer opens with it, and it
/// asks afresh when it is opened again.
pub(crate) fn forget(state: AppState, token: u64) {
    let Some((path, lens_id, scope)) = with_run(state, token, |run| {
        (run.path.clone(), run.lens_id.clone(), run.scope)
    }) else {
        return;
    };
    dismiss(state, token);
    if scope == Scope::Document {
        if let Some(store) = STORE.as_ref() {
            store.delete(&path, &lens_id);
        }
    }
}

/// Note whether the document shows the run `token`'s answer, for the next
/// time it is opened.
fn remember(state: AppState, token: u64) {
    let Some((path, lens_id, scope, applied)) = with_run(state, token, |run| {
        (
            run.path.clone(),
            run.lens_id.clone(),
            run.scope,
            run.applied,
        )
    }) else {
        return;
    };
    if let (Scope::Document, Some(store)) = (scope, STORE.as_ref()) {
        store.remember(&path, &lens_id, applied);
    }
}

/// Start `lens_id` over `scope`, `shown` or opened hidden. A hidden run asks
/// nothing and shows nothing until it is shown.
fn start_with(state: AppState, lens_id: &str, scope: Scope, policy: Policy, shown: bool) {
    let Some(lens) = offered_lenses().into_iter().find(|lens| lens.id == lens_id) else {
        tracing::warn!(lens_id, "no usable lens with this id");
        return;
    };
    // A page is a document of its own, so a page lens always looks at the
    // whole of one.
    let scope = if lens.display == LensDisplay::Page {
        Scope::Document
    } else {
        scope
    };
    // What the lens is not on it does not open over — a lens remembered
    // with a document before it was set to a block, say.
    if !is_on(&lens, scope) {
        return;
    }
    let policy = resolved(policy, &lens);
    make_room(state, &lens, shown);
    let Some(source) = state.rendered_source.peek().clone() else {
        return;
    };
    let token = open_run(state, &lens, scope, &source, shown);
    if !shown {
        return;
    }
    let task = ask(state, lens, source, scope, token, policy);
    update(state, token, |run| run.task = Some(task));
}

/// Take out the runs opening `lens` takes the place of, and, for a page lens
/// `shown`, the answer of the page lens applied now, to be switched back to.
fn make_room(state: AppState, lens: &Lens, shown: bool) {
    for token in replaced_by(&peek_open_runs(&state), lens) {
        dismiss(state, token);
    }
    if lens.display == LensDisplay::Page && shown {
        shelve_applied(state);
    }
}

/// Open a run of `lens` over `scope` of `source` in the window, `shown` or
/// hidden, noted for the next time the document is opened; its token.
fn open_run(
    mut state: AppState,
    lens: &Lens,
    scope: Scope,
    source: &RenderedSource,
    shown: bool,
) -> u64 {
    let token = next_serial();
    let run = LensRun {
        lens_id: lens.id.clone(),
        label: lens.label.clone(),
        display: lens.display,
        scope,
        path: source.path.clone(),
        generation: source.generation,
        token,
        total: 0,
        done: 0,
        failed: 0,
        outdated: 0,
        unanswered: 0,
        task: None,
        html: None,
        error: None,
        applied: shown,
    };
    if lens.display == LensDisplay::Page {
        state.page_lenses.write().push(run);
    } else {
        state.overlay_lenses.write().push(run);
    }
    remember(state, token);
    token
}

/// Have the agent of `lens` asked for the run `token`, in the way the lens
/// shows over `scope`; the task that stops it.
fn ask(
    state: AppState,
    lens: Lens,
    source: RenderedSource,
    scope: Scope,
    token: u64,
    policy: Policy,
) -> Task {
    spawn_detached_task(async move {
        let run = Run {
            state,
            lens: &lens,
            source: &source,
            token,
            policy,
        };
        match (lens.display, scope) {
            (LensDisplay::Annotate, _) => per_block(run, scope).await,
            (LensDisplay::Popover, Scope::Cursor) => on_block(run).await,
            (LensDisplay::Popover | LensDisplay::Page, _) => on_document(run).await,
        }
        update(state, token, |run| run.task = None);
    })
}

/// Close the lenses the configuration no longer offers as they were
/// opened — taken out, broken, or shown another way — whose Show and
/// Regenerate would otherwise find nothing to run. The document still
/// remembers them, and opens with them again once they are offered.
pub(crate) fn close_unoffered(state: AppState) {
    for token in unoffered(&peek_open_runs(&state), &offered_lenses()) {
        dismiss(state, token);
    }
}

/// Take the applied page lens's answer off the page, to be switched back
/// to; a run still being answered goes on filing its answers.
fn shelve_applied(state: AppState) {
    let applied: Vec<u64> = state
        .page_lenses
        .peek()
        .iter()
        .filter(|run| run.applied)
        .map(|run| run.token)
        .collect();
    for token in applied {
        hide_run(state, token);
    }
}

/// Stop every running lens; what they have shown stays.
pub(crate) fn stop(state: AppState) {
    for run in peek_open_runs(&state) {
        stop_run(state, run.token);
    }
}

/// Stop the run `token`; what it has shown stays, and what it had yet to
/// answer is counted as unanswered, for the reader to continue.
pub(crate) fn stop_run(state: AppState, token: u64) {
    let Some((task, page)) = with_run(state, token, |run| {
        let task = run.task.take();
        if task.is_some() {
            run.unanswered += run.pending();
        }
        (task, run.display == LensDisplay::Page)
    }) else {
        return;
    };
    if let Some(task) = task {
        task.cancel();
        if !page {
            eval_call("settle", &[json(&token)]);
        }
    }
}

/// Stop the run `token` and take it and what it put on the page out of the
/// window, leaving the document to open with it again.
fn dismiss(mut state: AppState, token: u64) {
    stop_run(state, token);
    let page = state
        .page_lenses
        .peek()
        .iter()
        .position(|run| run.token == token);
    if let Some(index) = page {
        let removed = state.page_lenses.write().remove(index);
        if removed.applied {
            eval_call("restorePage", &[]);
        }
        return;
    }
    let before = state.overlay_lenses.peek().len();
    state
        .overlay_lenses
        .write()
        .retain(|run| run.token != token);
    if state.overlay_lenses.peek().len() != before {
        eval_call("restoreMarks", &[json(&token)]);
    }
}

/// Keep the lenses in step with the page on screen, and open with a
/// document the lenses it had open when it was last read, shown or hidden
/// as they were.
pub(crate) fn follow_rerender(state: AppState) {
    let rendered = state.rendered_source.read().clone();
    for run in peek_open_runs(&state) {
        match following(&run, rendered.as_ref()) {
            Follow::Keep => {}
            Follow::Rerun(lens_id) => start_with(
                state,
                &lens_id,
                Scope::Document,
                Policy::Resumed,
                run.applied,
            ),
            Follow::Close => dismiss(state, run.token),
        }
    }

    let (Some(rendered), Some(store)) = (rendered, STORE.as_ref()) else {
        return;
    };
    let open: Vec<String> = peek_open_runs(&state)
        .into_iter()
        .map(|run| run.lens_id)
        .collect();
    for lens in store.remembered(&rendered.path) {
        if !open.contains(&lens.id) {
            start_with(
                state,
                &lens.id,
                Scope::Document,
                Policy::Resumed,
                lens.shown,
            );
        }
    }
}

/// Apply `change` to the run `token` and return what it returned, if the
/// run is still open in the window.
fn with_run<R>(
    mut state: AppState,
    token: u64,
    change: impl FnOnce(&mut LensRun) -> R,
) -> Option<R> {
    let in_page = state
        .page_lenses
        .peek()
        .iter()
        .position(|run| run.token == token);
    if let Some(index) = in_page {
        return Some(change(&mut state.page_lenses.write()[index]));
    }
    let index = state
        .overlay_lenses
        .peek()
        .iter()
        .position(|run| run.token == token)?;
    Some(change(&mut state.overlay_lenses.write()[index]))
}

/// Apply `change` to the run `token`, if it is still open in the window.
fn update(state: AppState, token: u64, change: impl FnOnce(&mut LensRun)) {
    with_run(state, token, change);
}

/// Whether the run `token` is still open in the window, over the page on
/// screen.
fn is_current(state: AppState, token: u64, generation: u64) -> bool {
    let rendered = state
        .rendered_source
        .peek()
        .as_ref()
        .map(|rendered| rendered.generation);
    rendered == Some(generation) && with_run(state, token, |_| ()).is_some()
}

fn runner(lens: &Lens, source: &RenderedSource) -> Runner {
    Runner {
        invocation: Arc::new(agent::invocation(lens)),
        cwd: source.path.parent().map(Into::into),
        timeout: Duration::from_secs(lens.timeout_seconds),
        concurrency: lens.concurrency,
    }
}

/// One run of a lens over one render of a document, and when it asks.
#[derive(Clone, Copy)]
struct Run<'a> {
    state: AppState,
    lens: &'a Lens,
    source: &'a RenderedSource,
    token: u64,
    policy: Policy,
}

impl Run<'_> {
    /// Whether the run is still open over the page on screen.
    fn is_current(&self) -> bool {
        is_current(self.state, self.token, self.source.generation)
    }

    fn update(&self, change: impl FnOnce(&mut LensRun)) {
        update(self.state, self.token, change);
    }

    fn runner(&self) -> Runner {
        runner(self.lens, self.source)
    }

    fn render(&self, markdown: &str) -> Result<String, String> {
        render(markdown, &self.source.path)
    }

    /// Render `answers` off the thread the window runs on: answers kept on
    /// disk come back all at once as a document opens, and rendering a
    /// page's worth of them there would hold the document up.
    async fn render_all(&self, answers: Vec<String>) -> Vec<Result<String, String>> {
        let path = self.source.path.clone();
        let count = answers.len();
        tokio::task::spawn_blocking(move || {
            answers
                .iter()
                .map(|answer| render(answer, &path))
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_else(|error| vec![Err(error.to_string()); count])
    }

    /// [`Run::render_all`] for the answers `kept` from before, each beside
    /// its job.
    async fn render_kept(
        &self,
        kept: Vec<(Job, String, bool)>,
    ) -> Vec<(Job, Result<String, String>, bool)> {
        let rendered = self
            .render_all(kept.iter().map(|(_, answer, _)| answer.clone()).collect())
            .await;
        kept.into_iter()
            .zip(rendered)
            .map(|((job, _, stale), html)| (job, html, stale))
            .collect()
    }
}

fn render(markdown: &str, path: &Path) -> Result<String, String> {
    markdown::render_detached(markdown, path)
        .map_err(|error| t!("lenses.errors.render", reason = error).into_owned())
}

/// The place in a record for an answer about the whole document.
const DOCUMENT: &str = "document";

/// What a lens answered about the document before, as it was read when the
/// run began, and what it answers now, written to disk as it goes. Only the
/// places the document has now go into what is written, so the answers for
/// places it no longer has drop out.
struct Kept {
    before: Record,
    next: Record,
    written: Record,
    document: std::path::PathBuf,
    lens_id: String,
    /// Whether what is answered goes to disk. A run over one block does
    /// not: its places are counted among its own targets, not the
    /// document's, and filing them would put its answer in place of the
    /// document's record.
    keeps: bool,
}

impl Kept {
    /// Nothing from before, and nothing written: a run over one block,
    /// asked on the spot.
    fn unkept(run: &Run<'_>) -> Self {
        Self {
            before: Record::default(),
            next: Record::default(),
            written: Record::default(),
            document: run.source.path.clone(),
            lens_id: run.lens.id.clone(),
            keeps: false,
        }
    }

    /// Read what the lens answered before, off the thread the window runs
    /// on: a translation of a long document is a large file.
    async fn load(run: &Run<'_>) -> Self {
        let document = run.source.path.clone();
        let lens_id = run.lens.id.clone();
        let before = {
            let (document, lens_id) = (document.clone(), lens_id.clone());
            tokio::task::spawn_blocking(move || {
                STORE
                    .as_ref()
                    .map(|store| store.load(&document, &lens_id))
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        };
        Self {
            written: before.clone(),
            before,
            next: Record::default(),
            document,
            lens_id,
            keeps: true,
        }
    }

    /// Write what the lens answers now, if it differs from what is written.
    fn save(&mut self) {
        if !self.keeps || self.next == self.written {
            return;
        }
        let Some(store) = STORE.as_ref() else {
            return;
        };
        match store.save(&self.document, &self.lens_id, &self.next) {
            Ok(()) => self.written = self.next.clone(),
            Err(error) => tracing::warn!(%error, "a lens's answers were not kept"),
        }
    }

    /// File each of `jobs` at its place `slot` as `policy` finds it in what
    /// was answered before, and write what is filed.
    fn sort(&mut self, policy: Policy, jobs: Vec<Job>, slot: impl Fn(&Job) -> String) -> Sorted {
        let claimed = claimed(&self.before, jobs.iter().map(|job| &job.cache_key));
        let mut sorted = Sorted::default();
        for job in jobs {
            let place = slot(&job);
            let found = found(&self.before, policy, &place, &job.cache_key, &claimed);
            file(&mut self.next, &self.before, &place, &job.cache_key, &found);
            match found {
                Found::Fresh(answer) => sorted.kept.push((job, answer, false)),
                Found::Stale(answer) => sorted.kept.push((job, answer, true)),
                Found::Missing => sorted.missing.push(job),
                Found::Ask => sorted.pending.push(job),
            }
        }
        self.save();
        sorted
    }
}

/// One run per target block, each answer kept beside its block.
async fn per_block(run: Run<'_>, scope: Scope) {
    let Some(blocks) = collect(scope, &run.lens.label, run.source.generation, run.token).await
    else {
        return;
    };
    let jobs = {
        let text = Arc::clone(&run.source.source);
        job::per_block(run.lens, &run.source.path, &blocks, move |block| {
            markdown::block_content(&*text, block.kind, &block.range, block.until)
        })
    };
    run.update(|lens_run| lens_run.total = jobs.len());

    // The targets in order, whichever of them the page offered: a place is
    // a target's position among them.
    let slots: HashMap<String, String> = jobs
        .iter()
        .enumerate()
        .map(|(index, job)| (job.id.clone(), format!("block:{index}")))
        .collect();
    let mut kept = match scope {
        Scope::Document => Kept::load(&run).await,
        Scope::Cursor => Kept::unkept(&run),
    };
    let sorted = kept.sort(run.policy, jobs, |job| slots[&job.id].clone());
    let rendered = run.render_kept(sorted.kept).await;
    if !run.is_current() {
        return;
    }
    annotate_kept(&run, rendered, sorted.missing.len());
    annotate_asked(&run, sorted.pending, &slots, &mut kept).await;
}

/// What a run's places come to from what was answered before, in the order
/// of its jobs.
#[derive(Default)]
struct Sorted {
    /// Answered before, each marked outdated or not.
    kept: Vec<(Job, String, bool)>,
    /// Without an answer, and not asked.
    missing: Vec<Job>,
    /// To be asked.
    pending: Vec<Job>,
}

/// How many places a step showed an answer in, and in how many the answer
/// failed to render.
#[derive(Default)]
struct Tally {
    done: usize,
    outdated: usize,
    failed: usize,
}

impl Tally {
    fn shown(&mut self, stale: bool) {
        if stale {
            self.outdated += 1;
        } else {
            self.done += 1;
        }
    }

    fn add_to(&self, lens_run: &mut LensRun) {
        lens_run.done += self.done;
        lens_run.outdated += self.outdated;
        lens_run.failed += self.failed;
    }
}

/// Show what was answered before beside the blocks, in one call to the page
/// rather than one per block, and count the places left `missing`.
fn annotate_kept(run: &Run, rendered: Vec<(Job, Result<String, String>, bool)>, missing: usize) {
    let mut shown = Vec::new();
    let mut tally = Tally::default();
    for (job, html, stale) in rendered {
        match html {
            Ok(html) => {
                tally.shown(stale);
                shown.push((job.id, html, stale));
            }
            Err(reason) => {
                tally.failed += 1;
                eval_call("fail", &[json(&run.token), json(&job.id), json(&reason)]);
            }
        }
    }
    eval_call("annotateAll", &[json(&run.token), json(&shown)]);
    run.update(|lens_run| {
        tally.add_to(lens_run);
        lens_run.unanswered += missing;
    });
}

/// Ask about the blocks of `pending`, marked so on the page, and show and
/// keep each answer at its place among `slots` as it comes.
async fn annotate_asked(
    run: &Run<'_>,
    pending: Vec<Job>,
    slots: &HashMap<String, String>,
    kept: &mut Kept,
) {
    let ids: Vec<&str> = pending.iter().map(|job| job.id.as_str()).collect();
    eval_call("markPending", &[json(&run.token), json(&ids)]);
    session::run_jobs(pending, run.runner(), |job, answer| {
        if let Some(answer) = annotate(run, &job, answer.map_err(|error| error.translated())) {
            kept.next.put(&slots[&job.id], &job.cache_key, &answer);
            kept.save();
        }
    })
    .await;
}

/// Show `answer`, just written, beside the block of `job`; the answer, once
/// it rendered. One that says there is nothing to add is kept as an empty
/// answer, which leaves the block unmarked.
fn annotate(run: &Run, job: &Job, answer: Result<String, String>) -> Option<String> {
    if !run.is_current() {
        return None;
    }
    let answer = answer.map(|answer| {
        if says_nothing(&answer) {
            String::new()
        } else {
            answer
        }
    });
    match answer.and_then(|answer| run.render(&answer).map(|html| (answer, html))) {
        Ok((answer, html)) => {
            eval_call(
                "annotate",
                &[json(&run.token), json(&job.id), json(&html), json(&false)],
            );
            run.update(|lens_run| lens_run.done += 1);
            Some(answer)
        }
        Err(reason) => {
            eval_call("fail", &[json(&run.token), json(&job.id), json(&reason)]);
            run.update(|lens_run| lens_run.failed += 1);
            None
        }
    }
}

/// One run over the text of the cursor's block, its answer kept beside it.
///
/// Asked for on the spot about one block, it is not kept on disk: there is
/// no place for it to come back to.
async fn on_block(run: Run<'_>) {
    let Some(blocks) = collect(
        Scope::Cursor,
        &run.lens.label,
        run.source.generation,
        run.token,
    )
    .await
    else {
        return;
    };
    let targets: Vec<&Block> = blocks.iter().filter(|block| block.target).collect();
    let (Some(first), Some(last)) = (targets.first(), targets.last()) else {
        return;
    };
    let range = arto_markdown::SourceRange {
        start: first.range.start,
        end: last.range.end,
    };
    let Some(text) = arto_markdown::source_text(&run.source.source, &range) else {
        return;
    };
    let job = job::whole(run.lens, &run.source.path, &text, Some(range), &first.id);
    run.update(|lens_run| lens_run.total = 1);
    eval_call("markPending", &[json(&run.token), json(&[&job.id])]);

    let cached = if run.policy == Policy::All {
        None
    } else {
        CACHE.lock().get(&job.cache_key).map(str::to_string)
    };
    let answer = match cached {
        Some(answer) => Ok(answer),
        None => run
            .runner()
            .run(&job.request, |_| {})
            .await
            .map_err(|error| error.translated()),
    };
    if let Some(answer) = annotate(&run, &job, answer) {
        CACHE.lock().insert(job.cache_key, answer);
    }
}

/// One run over the whole document, its answer shown as it arrives: for a
/// page lens, block by block in the document's places; for a popover, in
/// the header.
async fn on_document(run: Run<'_>) {
    let page = run.lens.display == LensDisplay::Page;
    let total = if page {
        let Some(begun) = begin_page(run.source.generation, run.token).await else {
            return;
        };
        if run.lens.unit == LensUnit::Block {
            return by_block(run, &begun.blocks).await;
        }
        begun.total
    } else {
        1
    };
    run.update(|lens_run| lens_run.total = total);
    let job = job::whole(run.lens, &run.source.path, &run.source.source, None, "");
    let mut kept = Kept::load(&run).await;
    let found = found(
        &kept.before,
        run.policy,
        DOCUMENT,
        &job.cache_key,
        &HashSet::new(),
    );
    file(
        &mut kept.next,
        &kept.before,
        DOCUMENT,
        &job.cache_key,
        &found,
    );
    match found {
        Found::Fresh(answer) => show_kept_document(&run, page, total, kept, answer, false).await,
        Found::Stale(answer) => show_kept_document(&run, page, total, kept, answer, true).await,
        Found::Missing => run.update(|lens_run| lens_run.unanswered = total),
        Found::Ask => show_asked_document(&run, page, total, &job, kept).await,
    }
}

/// Show `answer`, what was answered about the document before, over the
/// `total` places it takes — marked outdated when it is `stale`.
async fn show_kept_document(
    run: &Run<'_>,
    page: bool,
    total: usize,
    mut kept: Kept,
    answer: String,
    stale: bool,
) {
    kept.save();
    let html = run.render_all(vec![answer]).await.remove(0);
    if !run.is_current() {
        return;
    }
    match html {
        Ok(html) => {
            show_html(run, page, html, stale);
            run.update(|lens_run| {
                if stale {
                    lens_run.outdated = total;
                } else {
                    lens_run.done = total;
                }
            });
        }
        Err(reason) => run.update(|lens_run| lens_run.error = Some(reason)),
    }
}

/// Ask about the whole document, showing the answer over the `total` places
/// it takes as it arrives, and keep it once it is complete.
async fn show_asked_document(run: &Run<'_>, page: bool, total: usize, job: &Job, mut kept: Kept) {
    let mut last_render: Option<Instant> = None;
    let mut shown = 0;
    let answer = run
        .runner()
        .run(&job.request, |written| {
            let finished = finished_blocks(written);
            let due = last_render.is_none_or(|at| at.elapsed() >= STREAM_INTERVAL);
            if finished.len() > shown && due && run.is_current() {
                shown = finished.len();
                last_render = Some(Instant::now());
                if let Ok(html) = run.render(finished) {
                    show_html(run, page, html, false);
                }
            }
        })
        .await
        .map_err(|error| error.translated());
    if !run.is_current() {
        return;
    }
    match answer.and_then(|answer| run.render(&answer).map(|html| (answer, html))) {
        Ok((answer, html)) => {
            show_html(run, page, html, false);
            run.update(|lens_run| lens_run.done = total);
            kept.next.put(DOCUMENT, &job.cache_key, &answer);
            kept.save();
        }
        Err(reason) => run.update(|lens_run| lens_run.error = Some(reason)),
    }
}

/// What a block of a page lens shows once every block above it has shown:
/// its answer rendered, marked outdated or not, or nothing.
type Reveal = (Job, Option<Result<String, String>>, bool);

/// A page lens that hands the document over one top-level block at a time:
/// the runs go on side by side, and their answers take the document's
/// places from the top down, each as soon as every block above it has one.
/// A block without an answer — code, a diagram, a run that failed — keeps
/// its place as written.
async fn by_block(run: Run<'_>, blocks: &[PageBlock]) {
    let jobs = page_block_jobs(&run, blocks);
    run.update(|lens_run| lens_run.total = jobs.len());

    // A place is the block's position among the document's top-level
    // blocks, which is its id.
    let slot = |job: &Job| format!("block:{}", job.id);
    let mut in_order = InOrder::new(&jobs);
    let mut kept = Kept::load(&run).await;
    let sorted = kept.sort(run.policy, jobs, slot);
    run.update(|lens_run| lens_run.unanswered += sorted.missing.len());
    for job in sorted.missing {
        in_order.ready(job, None, false);
    }
    for (job, html, stale) in run.render_kept(sorted.kept).await {
        in_order.ready(job, Some(html), stale);
    }
    in_order.reveal(&run);
    session::run_jobs(sorted.pending, run.runner(), |job, answer| {
        if let Ok(answer) = &answer {
            kept.next.put(&slot(&job), &job.cache_key, answer);
            kept.save();
        }
        let html = answer
            .map_err(|error| error.translated())
            .and_then(|answer| run.render(&answer));
        in_order.ready(job, Some(html), false);
        in_order.reveal(&run);
    })
    .await;
}

/// One job for each of `blocks` there is prose to hand over in, named after
/// the block's position among them.
fn page_block_jobs(run: &Run, blocks: &[PageBlock]) -> Vec<Job> {
    blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.translatable)
        .filter_map(|(index, block)| {
            let range = block.range?;
            let text = arto_markdown::source_text(&run.source.source, &range)?;
            Some(job::whole(
                run.lens,
                &run.source.path,
                &text,
                Some(range),
                &index.to_string(),
            ))
        })
        .collect()
}

/// The blocks of a page lens that are ready to show, held until every block
/// above them is.
struct InOrder {
    order: Vec<String>,
    ready: HashMap<String, Reveal>,
    next: usize,
}

impl InOrder {
    fn new(jobs: &[Job]) -> Self {
        Self {
            order: jobs.iter().map(|job| job.id.clone()).collect(),
            ready: HashMap::new(),
            next: 0,
        }
    }

    fn ready(&mut self, job: Job, html: Option<Result<String, String>>, stale: bool) {
        self.ready.insert(job.id.clone(), (job, html, stale));
    }

    /// Show every block whose turn has come, in one call to the page.
    fn reveal(&mut self, run: &Run) {
        let mut shown = Vec::new();
        let mut tally = Tally::default();
        while let Some((job, html, stale)) = self
            .order
            .get(self.next)
            .and_then(|id| self.ready.remove(id))
        {
            self.next += 1;
            match html {
                None => {}
                Some(Ok(html)) => {
                    tally.shown(stale);
                    shown.push((job.id, html, stale));
                }
                Some(Err(reason)) => {
                    tracing::debug!(%reason, block = %job.id, "a block of a page lens kept its place");
                    tally.failed += 1;
                }
            }
        }
        if !run.is_current() {
            return;
        }
        if !shown.is_empty() {
            eval_call("showAnswers", &[json(&run.token), json(&shown)]);
        }
        run.update(|lens_run| tally.add_to(lens_run));
    }
}
