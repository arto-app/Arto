//! What passes between a run and the page it shows on: the blocks the page
//! offers, and the calls into `window.Arto.lenses` that show the answers.

use super::super::job::Block;
use super::super::Scope;
use super::{update, Run};
use arto_markdown::SourceRange;
use dioxus::document::EvalError;
use dioxus::prelude::*;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::time::Duration;

/// How often the page is asked again for blocks it has not rendered yet.
const COLLECT_ATTEMPTS: usize = 20;
const COLLECT_INTERVAL: Duration = Duration::from_millis(50);

/// Show `html`, the answer of the run so far: in the page's places for a
/// page lens, in the header otherwise — marked `outdated` when it is about
/// what the document said before.
pub(super) fn show_html(run: &Run, page: bool, html: String, outdated: bool) {
    if !page {
        run.update(|lens_run| lens_run.html = Some(html));
        return;
    }
    let mut eval = document::eval(&format!(
        "dioxus.send(window.Arto?.lenses?.showPage?.({}, {}, {}) ?? 0);",
        json(&run.token),
        json(&html),
        json(&outdated)
    ));
    let (state, token) = (run.state, run.token);
    // While the answer streams in, the count is how many of the page's
    // blocks it has reached.
    spawn(async move {
        if let Ok(reached) = eval.recv::<usize>().await {
            update(state, token, |lens_run| {
                if lens_run.is_running() {
                    lens_run.done = lens_run.done.max(reached);
                }
            });
        }
    });
}

/// One of the document's top-level blocks, as the page reports it.
#[derive(Debug, serde::Deserialize)]
pub(super) struct PageBlock {
    pub(super) range: Option<SourceRange>,
    /// Prose to hand over, rather than code, a diagram or a formula.
    pub(super) translatable: bool,
}

/// The page, ready to show a page lens's answer.
#[derive(Debug, serde::Deserialize)]
pub(super) struct Begun {
    generation: Option<u64>,
    /// How many of the document's top-level blocks an answer to the whole
    /// document takes the places of: all but its raw HTML, which the answer
    /// shows escaped rather than as blocks.
    pub(super) total: usize,
    pub(super) blocks: Vec<PageBlock>,
}

/// Have the page get ready to show a page lens's answer over the render of
/// `generation`, waiting for it to show that render; the document's
/// top-level blocks, whose places the answer takes.
pub(super) async fn begin_page(generation: u64, token: u64) -> Option<Begun> {
    let js = format!(
        "dioxus.send(window.Arto?.lenses?.beginPage?.({}) ?? {{ generation: null, total: 0, blocks: [] }});",
        json(&token)
    );
    match eval_until(&js, |begun: &Begun| begun.generation == Some(generation)).await {
        Ok(begun) => begun,
        Err(error) => {
            tracing::debug!(?error, "the page did not get ready for the lens");
            None
        }
    }
}

/// Ask the page for the blocks of `scope`, tagged for the run `token`,
/// waiting for it to show the render of `generation`: right after a
/// re-render the page may still hold the previous one, whose ranges point
/// into a different source.
pub(super) async fn collect(
    scope: Scope,
    label: &str,
    generation: u64,
    token: u64,
) -> Option<Vec<Block>> {
    #[derive(serde::Deserialize)]
    struct Offered {
        generation: Option<u64>,
        blocks: Vec<Block>,
    }

    let scope = match scope {
        Scope::Cursor => "cursor",
        Scope::Document => "document",
    };
    let js = format!(
        "dioxus.send(window.Arto?.lenses?.collect?.({}, {}, {}) ?? {{ generation: null, blocks: [] }});",
        json(&token),
        json(&scope),
        json(&label)
    );
    let current = |offered: &Offered| offered.generation == Some(generation);
    match eval_until(&js, current).await {
        Ok(Some(offered)) => Some(offered.blocks),
        Ok(None) => {
            tracing::debug!(
                generation,
                "the page never showed the render the lens looked at"
            );
            None
        }
        Err(error) => {
            tracing::debug!(?error, "the page did not answer for its blocks");
            None
        }
    }
}

async fn eval_until<T: DeserializeOwned>(
    js: &str,
    accept: impl Fn(&T) -> bool,
) -> Result<Option<T>, EvalError> {
    until_accepted(
        || async {
            let mut eval = document::eval(js);
            eval.recv::<T>().await
        },
        accept,
    )
    .await
}

/// Ask with `ask` until its answer is `accept`ed, waiting between the
/// asks; nothing once the attempts run out, and the error of the first ask
/// that fails, which asking again would not mend.
async fn until_accepted<T, E, F>(
    mut ask: impl FnMut() -> F,
    accept: impl Fn(&T) -> bool,
) -> Result<Option<T>, E>
where
    F: Future<Output = Result<T, E>>,
{
    for _ in 0..COLLECT_ATTEMPTS {
        let answer = ask().await?;
        if accept(&answer) {
            return Ok(Some(answer));
        }
        tokio::time::sleep(COLLECT_INTERVAL).await;
    }
    Ok(None)
}

pub(super) fn eval_call(function: &str, args: &[String]) {
    let js = format!("window.Arto?.lenses?.{function}?.({});", args.join(", "));
    document::eval(&js);
}

pub(super) fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("a JSON value serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test]
    async fn the_page_is_asked_until_it_shows_the_render_looked_at() {
        let asked = Cell::new(0);
        let answer = until_accepted(
            || {
                asked.set(asked.get() + 1);
                std::future::ready(Ok::<_, ()>(asked.get()))
            },
            |generation| *generation == 3,
        )
        .await;

        assert_eq!(answer, Ok(Some(3)));
        assert_eq!(asked.get(), 3);
    }

    #[tokio::test]
    async fn a_page_that_does_not_answer_is_not_asked_again() {
        let asked = Cell::new(0);
        let answer = until_accepted(
            || {
                asked.set(asked.get() + 1);
                std::future::ready(Err::<u64, _>("gone"))
            },
            |_| true,
        )
        .await;

        assert_eq!(answer, Err("gone"));
        assert_eq!(asked.get(), 1);
    }

    #[tokio::test]
    async fn a_page_that_never_shows_the_render_is_given_up_on() {
        let asked = Cell::new(0);
        let answer = until_accepted(
            || {
                asked.set(asked.get() + 1);
                std::future::ready(Ok::<_, ()>(0))
            },
            |generation| *generation == 1,
        )
        .await;

        assert_eq!(answer, Ok(None));
        assert_eq!(asked.get(), COLLECT_ATTEMPTS);
    }
}
