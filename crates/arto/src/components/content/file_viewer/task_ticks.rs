//! Ticks the reader makes on the page, written to the file on screen.

use dioxus::prelude::*;
use serde::Deserialize;
use std::path::Path;

use crate::config::CONFIG;
use crate::lenses::RenderedSource;
use crate::state::AppState;
use crate::tasks::{self, tick_file, Tick, TickError, TickFileError};
use crate::utils::page_runtime::once_arto_has;

/// A tick, and the render of the page it was made on.
#[derive(Debug, Deserialize)]
struct TickRequest {
    generation: u64,
    #[serde(flatten)]
    tick: Tick,
}

/// Hook to write the ticks the page sends to the file it was drawn from, and
/// to tell the page whether ticking is on.
pub(super) fn use_task_ticks(state: AppState) {
    use_effect(move || {
        let _ = state.config_revision.read();
        let on = CONFIG.read().reading.tick_tasks;
        document::eval(&once_arto_has(
            "tasks.setEnabled",
            &format!("window.Arto.tasks.setEnabled({on});"),
        ));
    });

    use_hook(move || {
        let mut eval = document::eval(&once_arto_has(
            "tasks.onTick",
            "window.Arto.tasks.onTick((tick) => dioxus.send(tick));",
        ));
        spawn(async move {
            while let Ok(TickRequest { generation, tick }) = eval.recv::<TickRequest>().await {
                // The page may still show the render before the one just
                // made, or the document being left; its lines are not the
                // file's.
                let rendered = state
                    .rendered_source
                    .peek()
                    .clone()
                    .filter(|rendered| rendered.generation == generation);
                let Some(rendered) = rendered else {
                    tracing::info!(?tick, "The tick was made on a page no longer on screen");
                    show_as_the_file_has_it(state, generation, tick);
                    continue;
                };
                // One at a time: a second tick of the same task reads the
                // file the first one wrote.
                let written = tokio::task::spawn_blocking(move || write(&rendered, tick)).await;
                if matches!(written, Ok(true)) {
                    tick_the_source(state, generation, tick);
                } else {
                    show_as_the_file_has_it(state, generation, tick);
                }
            }
        });
    });
}

/// Write `tick` to the file `rendered` was drawn from, and whether it was
/// written.
fn write(rendered: &RenderedSource, tick: Tick) -> bool {
    let reading = CONFIG.read().reading.clone();
    if !reading.tick_tasks {
        return false;
    }
    let file: &Path = &rendered.path;
    match tick_file(file, &rendered.source, tick) {
        Ok(before) => {
            if reading.show_changes {
                crate::baselines::carry_tick(file, &before, tick);
            }
            tracing::info!(
                ?file,
                line = tick.line,
                checked = tick.checked,
                "Ticked a task"
            );
            true
        }
        Err(TickFileError::Tick(error @ (TickError::Stale | TickError::NotATask))) => {
            tracing::info!(?file, ?tick, %error, "The tick was not written");
            false
        }
        Err(TickFileError::Io(error)) => {
            tracing::warn!(?file, ?tick, %error, "The tick could not be written");
            false
        }
    }
}

/// Tick the task in the source the page of render `generation` was drawn
/// from, as it was ticked in the file.
///
/// The checkbox on the page already shows the tick, while the file watcher
/// draws the page again only a moment later; leaving the document in
/// between would otherwise keep the source without the tick as the version
/// read, and mark the reader's own tick as a change on the next visit. Only
/// the one character is taken over, not the file as it is now: whatever else
/// changed in it has not been drawn. The character swaps for another of the
/// same length, so every offset into the source stays where the page's
/// blocks say it is, and keeping the generation tells whatever follows the
/// page that nothing moved.
fn tick_the_source(state: AppState, generation: u64, tick: Tick) {
    let mut rendered = state.rendered_source;
    let mut rendered = rendered.write();
    if let Some(rendered) = rendered
        .as_mut()
        .filter(|rendered| rendered.generation == generation)
    {
        if let Ok(source) = tasks::tick(&rendered.source, tick) {
            rendered.source = source.into();
        }
    }
}

/// Put the checkbox back to what the source of the page of render
/// `generation` says — what the page was drawn from, with the ticks written
/// since — on that page alone: the tick was not written.
fn show_as_the_file_has_it(state: AppState, generation: u64, tick: Tick) {
    let checked = state
        .rendered_source
        .peek()
        .as_ref()
        .filter(|rendered| rendered.generation == generation)
        .and_then(|rendered| tasks::checked_at(&rendered.source, tick.line, tick.column));
    let Some(checked) = checked else {
        return;
    };
    let Tick { line, column, .. } = tick;
    document::eval(&format!(
        "window.Arto?.tasks?.show?.({generation}, {{ line: {line}, column: {column} }}, {checked});"
    ));
}
