//! What a run decides from what it is given alone: when its lens asks, what
//! each place shows from what was answered before, and which open runs a
//! change in the window or the configuration closes.

use super::super::store::{Key, Record};
use super::super::{LensRun, RenderedSource, Scope};
use arto_config::Lens;
use std::collections::HashSet;

/// When a lens asks its agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Policy {
    /// Only when it has answered nothing about the document yet: what it
    /// answered before is shown, and what changed since is marked outdated
    /// for the reader to regenerate — opening a document is not a reason to
    /// spend minutes and money asking again.
    Kept,
    /// For every place whose answer is not about the text as it is now, or
    /// that has none — what changed, and what a stopped run left.
    Regenerate,
    /// For every place, whatever it answered before.
    All,
    /// As `Kept`, for a lens the reader did not just ask for — one that
    /// comes back with its document, or follows it as it changes. A lens
    /// that reaches what is on this machine beyond the document asks
    /// nothing then, as `Recall`.
    Resumed,
    /// Never: what it answered before is shown, and nothing else.
    Recall,
}

/// What `policy` comes to for `lens`.
///
/// A lens allowed to read the files around a document is one the reader
/// trusts with the documents they open it on — not with whatever the
/// document became since, or any document it comes back with: a file that
/// changed on disk may now ask it to read the files beside it and write
/// them into its answer.
pub(super) fn resolved(policy: Policy, lens: &Lens) -> Policy {
    match policy {
        Policy::Resumed if lens.reaches_local() => Policy::Recall,
        Policy::Resumed => Policy::Kept,
        policy => policy,
    }
}

/// Whether `lens` is on what `scope` looks at.
pub(super) fn is_on(lens: &Lens, scope: Scope) -> bool {
    match scope {
        Scope::Cursor => lens.on_block(),
        Scope::Document => lens.on_document(),
    }
}

/// What a lens's shortcut shows it over.
pub(super) fn shortcut_scope(lens: &Lens) -> Scope {
    if lens.on_document() {
        Scope::Document
    } else {
        Scope::Cursor
    }
}

/// The runs among `open` that opening `lens` takes the place of: its own.
pub(super) fn replaced_by(open: &[LensRun], lens: &Lens) -> Vec<u64> {
    open.iter()
        .filter(|run| run.lens_id == lens.id)
        .map(|run| run.token)
        .collect()
}

/// The runs among `open` whose lens `offered` has no longer, has with
/// another display, or has on something other than what the run looks at.
pub(super) fn unoffered(open: &[LensRun], offered: &[Lens]) -> Vec<u64> {
    open.iter()
        .filter(|run| {
            !offered.iter().any(|lens| {
                lens.id == run.lens_id && lens.display == run.display && is_on(lens, run.scope)
            })
        })
        .map(|run| run.token)
        .collect()
}

#[derive(Debug, PartialEq)]
pub(super) enum Follow {
    Keep,
    Rerun(String),
    Close,
}

/// What becomes of `run` now that the page shows `rendered`.
///
/// A lens over the whole document looks again when the document is
/// re-rendered: what did not change answers at once from what it answered
/// before, and what did is marked outdated rather than asked about again.
/// A lens over one block is closed: the block cannot be told apart from its
/// neighbours once the page is rebuilt.
///
/// Another document, or none, closes any lens: the reader went elsewhere,
/// and a lens that followed would start runs nobody asked for.
pub(super) fn following(run: &LensRun, rendered: Option<&RenderedSource>) -> Follow {
    let Some(rendered) = rendered.filter(|rendered| rendered.path == run.path) else {
        return Follow::Close;
    };
    if rendered.generation == run.generation {
        Follow::Keep
    } else if run.scope == Scope::Document {
        Follow::Rerun(run.lens_id.clone())
    } else {
        Follow::Close
    }
}

/// What a place shows, found in what the lens answered before.
#[derive(Debug, PartialEq)]
pub(super) enum Found {
    /// An answer to the request as it is now.
    Fresh(String),
    /// An answer to what the place said before it changed.
    Stale(String),
    /// Nothing, and nothing is asked until the reader regenerates.
    Missing,
    /// Nothing yet: the agent is asked.
    Ask,
}

/// What the place `slot`, whose request is `key`, shows, given what was
/// answered `before` — `claimed` being the requests found fresh anywhere.
pub(super) fn found(
    before: &Record,
    policy: Policy,
    slot: &str,
    key: &Key,
    claimed: &HashSet<Key>,
) -> Found {
    if policy == Policy::All {
        return Found::Ask;
    }
    if let Some(answer) = before.fresh(key) {
        Found::Fresh(answer.to_string())
    } else if policy == Policy::Regenerate || (policy != Policy::Recall && before.is_empty()) {
        Found::Ask
    } else if let Some(answer) = before.stale(slot, claimed) {
        Found::Stale(answer.to_string())
    } else {
        Found::Missing
    }
}

/// File in `next` what the place `slot`, whose request is `key`, keeps from
/// `before` as it was `found` there. A place asked again keeps its old
/// answer until the new one comes, so a run stopped or failed half-way
/// loses nothing that was on file.
pub(super) fn file(next: &mut Record, before: &Record, slot: &str, key: &Key, found: &Found) {
    match found {
        Found::Fresh(answer) => next.put(slot, key, answer),
        Found::Stale(_) | Found::Ask => next.carry(before, slot),
        Found::Missing => {}
    }
}

/// The requests among `keys` answered fresh in `before`.
pub(super) fn claimed<'a>(
    before: &Record,
    keys: impl IntoIterator<Item = &'a Key>,
) -> HashSet<Key> {
    keys.into_iter()
        .filter(|key| before.fresh(key).is_some())
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_config::LensDisplay;
    use std::path::PathBuf;

    fn run(display: LensDisplay, scope: Scope, rendered: &RenderedSource) -> LensRun {
        LensRun {
            lens_id: "lens".to_string(),
            label: "Lens".to_string(),
            display,
            scope,
            path: rendered.path.clone(),
            generation: rendered.generation,
            token: 1,
            total: 0,
            done: 0,
            failed: 0,
            outdated: 0,
            unanswered: 0,
            task: None,
            html: None,
            error: None,
            applied: true,
        }
    }

    fn rendered(path: &str) -> RenderedSource {
        RenderedSource::new(PathBuf::from(path), "# Title\n")
    }

    #[test]
    fn a_lens_stays_over_the_render_it_looked_at() {
        let page = rendered("/notes/a.md");
        let run = run(LensDisplay::Page, Scope::Document, &page);

        assert_eq!(following(&run, Some(&page)), Follow::Keep);
    }

    #[test]
    fn a_lens_over_the_document_looks_again_at_its_rerender() {
        let run = run(LensDisplay::Page, Scope::Document, &rendered("/notes/a.md"));

        assert_eq!(
            following(&run, Some(&rendered("/notes/a.md"))),
            Follow::Rerun("lens".to_string())
        );
    }

    #[test]
    fn a_popover_over_the_document_looks_again_and_one_over_a_block_closes() {
        let page = rendered("/notes/a.md");
        let popover = run(LensDisplay::Popover, Scope::Document, &page);
        let block = run(LensDisplay::Annotate, Scope::Cursor, &page);

        let rerendered = rendered("/notes/a.md");
        assert_eq!(
            following(&popover, Some(&rerendered)),
            Follow::Rerun("lens".to_string())
        );
        assert_eq!(following(&block, Some(&rerendered)), Follow::Close);
    }

    fn record(entries: &[(&str, u8, &str)]) -> Record {
        let mut record = Record::default();
        for (slot, key, answer) in entries {
            record.put(slot, &[*key; 32], answer);
        }
        record
    }

    #[test]
    fn a_place_whose_text_is_unchanged_is_answered_from_before() {
        let before = record(&[("block:0", 1, "kept")]);

        assert_eq!(
            found(&before, Policy::Kept, "block:0", &[1; 32], &HashSet::new()),
            Found::Fresh("kept".to_string())
        );
    }

    #[test]
    fn a_place_whose_text_changed_shows_its_old_answer_without_asking() {
        let before = record(&[("block:0", 1, "old")]);

        assert_eq!(
            found(&before, Policy::Kept, "block:0", &[2; 32], &HashSet::new()),
            Found::Stale("old".to_string())
        );
        assert_eq!(
            found(&before, Policy::Kept, "block:1", &[3; 32], &HashSet::new()),
            Found::Missing
        );
    }

    #[test]
    fn the_agent_is_asked_the_first_time_and_when_the_reader_regenerates() {
        let before = record(&[("block:0", 1, "old")]);

        assert_eq!(
            found(
                &Record::default(),
                Policy::Kept,
                "block:0",
                &[2; 32],
                &HashSet::new()
            ),
            Found::Ask
        );
        assert_eq!(
            found(
                &before,
                Policy::Regenerate,
                "block:0",
                &[2; 32],
                &HashSet::new()
            ),
            Found::Ask
        );
        assert_eq!(
            found(
                &before,
                Policy::Regenerate,
                "block:0",
                &[1; 32],
                &HashSet::new()
            ),
            Found::Fresh("old".to_string()),
            "regenerating asks again only about what changed"
        );
    }

    #[test]
    fn a_lens_opens_only_over_what_it_is_on_and_its_shortcut_follows() {
        let mut lens = Lens::new("explain");
        assert!(is_on(&lens, Scope::Cursor) && is_on(&lens, Scope::Document));
        assert_eq!(shortcut_scope(&lens), Scope::Document);

        lens.on = arto_config::LensTarget::Block;
        assert!(is_on(&lens, Scope::Cursor));
        assert!(!is_on(&lens, Scope::Document));
        assert_eq!(shortcut_scope(&lens), Scope::Cursor);

        lens.on = arto_config::LensTarget::Document;
        assert!(!is_on(&lens, Scope::Cursor));
        assert_eq!(shortcut_scope(&lens), Scope::Document);
    }

    #[test]
    fn a_lens_that_reads_local_files_asks_nothing_when_it_comes_back_by_itself() {
        let mut lens = Lens::new("digger");
        assert_eq!(resolved(Policy::Resumed, &lens), Policy::Kept);
        lens.allow = vec![arto_config::LensCapability::WebSearch];
        assert_eq!(resolved(Policy::Resumed, &lens), Policy::Kept);
        lens.allow = vec![arto_config::LensCapability::ReadFiles];
        assert_eq!(resolved(Policy::Resumed, &lens), Policy::Recall);
        assert_eq!(
            resolved(Policy::Kept, &lens),
            Policy::Kept,
            "the reader asked for it"
        );

        let before = record(&[("block:0", 1, "old")]);
        let nothing = HashSet::new();
        assert_eq!(
            found(
                &Record::default(),
                Policy::Recall,
                "block:0",
                &[1; 32],
                &nothing
            ),
            Found::Missing,
            "not even the first time"
        );
        assert_eq!(
            found(&before, Policy::Recall, "block:0", &[1; 32], &nothing),
            Found::Fresh("old".to_string())
        );
        assert_eq!(
            found(&before, Policy::Recall, "block:0", &[2; 32], &nothing),
            Found::Stale("old".to_string())
        );
    }

    #[test]
    fn an_answer_asked_again_stays_on_file_until_the_new_one_comes() {
        let before = record(&[("block:0", 1, "old"), ("block:1", 2, "gone")]);
        let mut next = Record::default();

        file(&mut next, &before, "block:0", &[1; 32], &Found::Ask);
        file(&mut next, &before, "block:1", &[3; 32], &Found::Missing);

        assert_eq!(next.fresh(&[1; 32]), Some("old"));
        assert_eq!(next.fresh(&[2; 32]), None);
    }

    #[test]
    fn regenerating_all_asks_again_even_about_what_is_unchanged() {
        let before = record(&[("block:0", 1, "kept")]);

        assert_eq!(
            found(&before, Policy::All, "block:0", &[1; 32], &HashSet::new()),
            Found::Ask
        );
    }

    #[test]
    fn another_document_closes_the_lens() {
        let run = run(LensDisplay::Page, Scope::Document, &rendered("/notes/a.md"));

        assert_eq!(
            following(&run, Some(&rendered("/notes/b.md"))),
            Follow::Close
        );
        assert_eq!(following(&run, None), Follow::Close);
    }

    fn lens(id: &str, display: &str) -> Lens {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "label": id,
            "display": display,
            "agent": "claude",
        }))
        .unwrap()
    }

    fn open(runs: &[(&str, LensDisplay, u64)]) -> Vec<LensRun> {
        let page = rendered("/notes/a.md");
        runs.iter()
            .map(|&(id, display, token)| LensRun {
                lens_id: id.to_string(),
                token,
                ..run(display, Scope::Document, &page)
            })
            .collect()
    }

    #[test]
    fn a_page_lens_replaces_only_its_own_earlier_run_and_the_others_wait() {
        let open = open(&[
            ("translate", LensDisplay::Page, 1),
            ("gloss", LensDisplay::Annotate, 2),
            ("summary", LensDisplay::Popover, 3),
        ]);

        assert!(replaced_by(&open, &lens("rewrite", "page")).is_empty());
        assert_eq!(replaced_by(&open, &lens("translate", "page")), vec![1]);
    }

    #[test]
    fn a_lens_over_the_page_replaces_only_its_own_earlier_run() {
        let open = open(&[
            ("translate", LensDisplay::Page, 1),
            ("gloss", LensDisplay::Annotate, 2),
            ("explain", LensDisplay::Annotate, 3),
        ]);

        assert_eq!(replaced_by(&open, &lens("gloss", "annotate")), vec![2]);
        assert!(replaced_by(&open, &lens("summary", "popover")).is_empty());
    }

    #[test]
    fn a_lens_no_longer_offered_as_it_was_opened_is_closed() {
        let open = open(&[
            ("translate", LensDisplay::Page, 1),
            ("gloss", LensDisplay::Annotate, 2),
            ("removed", LensDisplay::Annotate, 3),
        ]);
        let offered = [lens("translate", "page"), lens("gloss", "popover")];

        assert_eq!(unoffered(&open, &offered), vec![2, 3]);
    }

    #[test]
    fn a_lens_no_longer_on_what_it_was_opened_over_is_closed() {
        let open = open(&[
            ("explain", LensDisplay::Popover, 1),
            ("gloss", LensDisplay::Annotate, 2),
        ]);
        let mut explain = lens("explain", "popover");
        explain.on = arto_config::LensTarget::Block;
        let offered = [explain, lens("gloss", "annotate")];

        assert_eq!(unoffered(&open, &offered), vec![1]);
    }
}
