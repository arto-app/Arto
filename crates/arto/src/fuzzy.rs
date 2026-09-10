//! One way of narrowing a list, wherever the app narrows one.
//!
//! Every list the reader types into — the palette's commands, the history,
//! what is starred, the folders kept, the files under the one being worked in
//! — is narrowed by this module, so a query means the same thing in all of
//! them. The rule is `fzf`'s rather than "contains": the characters of each
//! term have to appear in order, and how well they appear decides where the
//! row lands.
//!
//! Ranking is the half that "contains" never had. A fuzzy query matches far
//! more than a substring does, so what saves it from being noise is that the
//! best match is first: [`Query::score`] is what every list sorts on, and a
//! tie leaves the rows in the order they arrived — newest first for the
//! history, as listed for the commands.
//!
//! What is typed is a whole `fzf` pattern, not one word:
//!
//! | Typed | Finds |
//! | --- | --- |
//! | `gd` | terms are fuzzy: `**g**uide.m**d**` |
//! | `guide arto` | every term matches, in any order |
//! | `'guide` | that exact run of characters |
//! | `^docs` | at the start |
//! | `md$` | at the end |
//! | `!draft` | and not this |
//!
//! Case is smart: a lowercase term ignores case, and one typed with a capital
//! in it means the capital.
//!
//! The engine is [`nucleo_matcher`], the matcher behind Helix's pickers. It is
//! pure Rust, so it behaves the same on every platform Arto runs on, and it
//! scores a candidate in about a microsecond — the palette's whole list is
//! rescored on each keystroke and the keystroke does not wait for it.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::cell::RefCell;
use std::path::Path;

thread_local! {
    /// The matcher's scratch space, kept between calls.
    ///
    /// A [`Matcher`] owns the allocations the algorithm works in, and it is
    /// meant to be reused: building one per candidate would allocate more
    /// than the match itself costs. Two of them, because scoring text and
    /// scoring paths want different bonuses and the choice is baked into the
    /// matcher's configuration.
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::new());
}

/// The allocations every match borrows.
struct Scratch {
    /// For prose: a word after a space is a strong place to match.
    text: Matcher,
    /// For paths: a component after a separator is, and spaces are ordinary.
    path: Matcher,
    /// Where a haystack is widened to characters, when it is not already.
    haystack: Vec<char>,
}

impl Scratch {
    fn new() -> Self {
        Self {
            text: Matcher::new(Config::DEFAULT),
            path: Matcher::new(Config::DEFAULT.match_paths()),
            haystack: Vec::new(),
        }
    }
}

/// One run of a candidate's text, and whether the query put it there.
///
/// What [`Query::highlight`] answers with: the row draws the matched runs
/// differently, which is what makes a fuzzy match legible — the reason a row
/// is in the list is the reason it is spelled the way it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub matched: bool,
}

/// How well a candidate answered, and how much of it there was.
///
/// Two numbers because one is not enough to order a list: fuzzy scores tie
/// often — every candidate that spells the query out exactly scores alike —
/// and the shorter of two candidates that answer equally well is the one
/// that answered with less left over. `Close All Windows` beats `Close All
/// Child Windows` on that alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rank {
    /// The matcher's score. Higher is better, and the numbers only mean
    /// anything against each other.
    pub score: u32,
    /// How long the candidate was, in characters.
    pub length: usize,
}

impl Rank {
    /// What an order is decided on, for the candidate offered `at`: the
    /// better score, then the shorter candidate, then the one offered first.
    ///
    /// The third is what keeps the history newest-first and the commands in
    /// the order they are listed, once the first two have nothing left to
    /// say — and it is part of the key rather than a property of the sort,
    /// so that [`best`] can throw a candidate away the moment it knows the
    /// answer without the order depending on how many it kept.
    fn key(&self, at: usize) -> (std::cmp::Reverse<u32>, usize, usize) {
        (std::cmp::Reverse(self.score), self.length, at)
    }
}

/// The best `cap` of what was ranked, best first.
///
/// Only `cap` of them are ever held. A query typed into a folder of twenty
/// thousand files can match every one of them, and sorting twenty thousand
/// candidates to draw sixteen rows is most of what a keystroke would cost:
/// each candidate is instead compared against the worst one kept so far and
/// forgotten if it loses, which is one comparison for nearly all of them.
///
/// What comes out is exactly what a stable sort of everything, cut to `cap`,
/// would have produced — the arrival order is inside the key rather than in
/// the sort — so the caller still hands its candidates over in the order it
/// would want them in.
///
/// `cap` is meant to be a screenful. What is kept is held in order by
/// insertion, which is the cheapest thing there is for a handful and the
/// wrong shape for thousands.
pub fn best<T>(ranked: impl IntoIterator<Item = (Rank, T)>, cap: usize) -> Vec<T> {
    type Key = (std::cmp::Reverse<u32>, usize, usize);

    if cap == 0 {
        return Vec::new();
    }
    // A cap larger than any list it could be asked of; the reserve is what it
    // is worth allocating up front rather than what a caller said.
    let mut kept: Vec<(Key, T)> = Vec::with_capacity(cap.min(64));

    for (at, (rank, item)) in ranked.into_iter().enumerate() {
        let key = rank.key(at);
        if kept.len() == cap {
            // Arrival is part of the key and only ever grows, so a candidate
            // that ties the worst kept one loses to it: this is the whole of
            // the fast path.
            if kept[kept.len() - 1].0 < key {
                continue;
            }
            kept.pop();
        }
        let at = kept.partition_point(|(kept, _)| *kept < key);
        kept.insert(at, (key, item));
    }

    kept.into_iter().map(|(_, item)| item).collect()
}

/// What the reader typed, parsed once and asked of many candidates.
///
/// Parsing is not free — a pattern is one allocation per term — and a list of
/// a few thousand rows asks the same question of every one of them, so the
/// question is built once and borrowed.
#[derive(Debug, Clone, Default)]
pub struct Query {
    pattern: Pattern,
}

impl Query {
    /// Parse what was typed.
    pub fn new(query: &str) -> Self {
        Self {
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
        }
    }

    /// Whether nothing was asked, so that every candidate answers.
    ///
    /// Whitespace alone is nothing asked: the terms are what is parsed, and a
    /// query of spaces has none.
    pub fn is_empty(&self) -> bool {
        self.pattern.atoms.is_empty()
    }

    /// How well `text` answers, or [`None`] if it does not.
    pub fn rank(&self, text: &str) -> Option<Rank> {
        Some(Rank {
            score: self.matched(text, false)?,
            length: text.chars().count(),
        })
    }

    /// The same, for a path.
    ///
    /// A path is matched whole, so `docs guide` finds `~/arto/docs/guide.md`
    /// however the two terms are ordered, and a term is worth more where it
    /// starts a component than in the middle of one.
    ///
    /// What the file is called counts twice. A query is nearly always a
    /// document's name rather than a folder's, so a path whose last component
    /// answers on its own is scored again on that component alone and the two
    /// added: `guide` then finds `notes/guide.md` above `guide/notes.md`,
    /// which is what was meant.
    pub fn rank_path(&self, path: &Path) -> Option<Rank> {
        let whole = path.to_string_lossy();
        let score = self.matched(&whole, true)?;
        let named = path
            .file_name()
            .and_then(|name| self.matched(&name.to_string_lossy(), true))
            .unwrap_or(0);
        Some(Rank {
            score: score + named,
            length: whole.chars().count(),
        })
    }

    /// `text`, split into the runs the query matched and the runs it did not.
    ///
    /// Adjacent runs of a kind are one span, and text that matched nothing is
    /// one span of the whole string — so a row that did not match, or a query
    /// that asked nothing, costs one span and no work.
    pub fn highlight(&self, text: &str) -> Vec<Span> {
        let whole = || {
            vec![Span {
                text: text.to_string(),
                matched: false,
            }]
        };
        if self.is_empty() || text.is_empty() {
            return whole();
        }

        let mut indices = Vec::new();
        let found = SCRATCH.with_borrow_mut(|scratch| {
            let haystack = Utf32Str::new(text, &mut scratch.haystack);
            self.pattern
                .indices(haystack, &mut scratch.text, &mut indices)
                .is_some()
        });
        if !found {
            return whole();
        }
        // One index per matched character, but a term is matched on its own
        // and the terms can overlap, so the same character can arrive twice
        // and out of order.
        indices.sort_unstable();
        indices.dedup();

        spans(text, &indices)
    }

    fn matched(&self, text: &str, is_path: bool) -> Option<u32> {
        if self.is_empty() {
            return Some(0);
        }
        SCRATCH.with_borrow_mut(|scratch| {
            let haystack = Utf32Str::new(text, &mut scratch.haystack);
            let matcher = if is_path {
                &mut scratch.path
            } else {
                &mut scratch.text
            };
            self.pattern.score(haystack, matcher)
        })
    }
}

/// Cut `text` into runs at the character positions in `matched`.
///
/// `matched` is sorted and holds character positions, which is what the
/// matcher counts in; the string is walked once and cut where the answer
/// changes.
fn spans(text: &str, matched: &[u32]) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut next = matched.iter().copied().peekable();

    for (at, ch) in text.chars().enumerate() {
        let at = at as u32;
        while next.peek().is_some_and(|index| *index < at) {
            next.next();
        }
        let hit = next.peek() == Some(&at);
        match spans.last_mut() {
            Some(last) if last.matched == hit => last.text.push(ch),
            _ => spans.push(Span {
                text: ch.to_string(),
                matched: hit,
            }),
        }
    }

    spans
}

/// Split spans at a character position, so two parts of one name can be drawn
/// differently while the match runs across both.
///
/// A document is named `folder/file.md` and the folder is drawn a step
/// quieter, but the query was asked of the whole name — so the answer is cut
/// where the drawing is, rather than asked twice.
pub fn split_spans(spans: Vec<Span>, at: usize) -> (Vec<Span>, Vec<Span>) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut seen = 0;

    for span in spans {
        let len = span.text.chars().count();
        if seen >= at {
            after.push(span);
        } else if seen + len <= at {
            seen += len;
            before.push(span);
        } else {
            let cut = at - seen;
            let head: String = span.text.chars().take(cut).collect();
            let tail: String = span.text.chars().skip(cut).collect();
            before.push(Span {
                text: head,
                matched: span.matched,
            });
            after.push(Span {
                text: tail,
                matched: span.matched,
            });
            seen += len;
        }
    }

    (before, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Whether a query finds `text` at all, which most of these ask and none
    /// of the app does: every list that narrows also ranks.
    fn matches(text: &str, query: &str) -> bool {
        Query::new(query).rank(text).is_some()
    }

    fn matches_path(path: &Path, query: &str) -> bool {
        Query::new(query).rank_path(path).is_some()
    }

    fn score_of(query: &Query, path: &str) -> u32 {
        query.rank_path(Path::new(path)).unwrap().score
    }

    fn text_of(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn matched_of(spans: &[Span]) -> String {
        spans
            .iter()
            .filter(|span| span.matched)
            .map(|span| span.text.as_str())
            .collect()
    }

    #[test]
    fn characters_in_order_are_enough() {
        assert!(matches("Close All Windows", "cw"));
        assert!(matches("Close All Windows", "close"));
        assert!(!matches("Close All Windows", "wc"));
    }

    #[test]
    fn every_term_has_to_appear_in_any_order() {
        assert!(matches("Close All Windows", "close windows"));
        assert!(matches("Close All Windows", "windows close"));
        assert!(!matches("Close All Windows", "close tabs"));
    }

    #[test]
    fn a_lowercase_query_ignores_case_and_a_capital_means_it() {
        assert!(matches(
            "Close All Windows",
            "WINDOWS".to_lowercase().as_str()
        ));
        assert!(matches("Close All Windows", "Windows"));
        assert!(!matches("close all windows", "Windows"));
    }

    #[test]
    fn the_fzf_operators_are_understood() {
        assert!(matches("guide.md", "'guide"));
        assert!(!matches("g-u-i-d-e.md", "'guide"));
        assert!(matches("guide.md", "^gu"));
        assert!(!matches("a guide.md", "^gu"));
        assert!(matches("guide.md", "md$"));
        assert!(!matches("guide.markdown", "md$"));
        assert!(matches("guide.md", "guide !draft"));
        assert!(!matches("guide-draft.md", "guide !draft"));
    }

    #[test]
    fn an_empty_query_answers_yes_with_no_score_of_its_own() {
        let query = Query::new("   ");
        assert!(query.is_empty());
        assert_eq!(query.rank("anything").map(|rank| rank.score), Some(0));
        assert_eq!(query.rank("").map(|rank| rank.score), Some(0));
    }

    #[test]
    fn a_path_is_matched_whole_and_in_any_order() {
        let path = PathBuf::from("/home/reader/arto/docs/guide.md");
        assert!(matches_path(&path, "guide arto"));
        assert!(matches_path(&path, "docs/gd"));
        assert!(!matches_path(&path, "guide notes"));
    }

    #[test]
    fn the_closer_match_scores_higher() {
        let query = Query::new("guide");
        let near = score_of(&query, "/w/docs/guide.md");
        let far = score_of(&query, "/w/docs/g-u-i-d-e-lines.md");
        assert!(near > far, "{near} should beat {far}");
    }

    #[test]
    fn a_name_scores_above_the_same_letters_in_the_folders() {
        let query = Query::new("guide");
        let named = score_of(&query, "/w/notes/guide.md");
        let buried = score_of(&query, "/w/guide/notes/something.md");
        assert!(named > buried, "{named} should beat {buried}");
    }

    #[test]
    fn the_shorter_of_two_equal_answers_comes_first() {
        let query = Query::new("close all windows");
        let ranked = best(
            [
                (query.rank("Close All Child Windows").unwrap(), "child"),
                (query.rank("Close All Windows").unwrap(), "all"),
            ],
            usize::MAX,
        );
        assert_eq!(ranked, vec!["all", "child"]);
    }

    #[test]
    fn ranking_stops_where_it_was_told_to() {
        let query = Query::new("md");
        let ranked = best(
            [
                (query.rank("a.md").unwrap(), "a"),
                (query.rank("b.md").unwrap(), "b"),
            ],
            1,
        );
        assert_eq!(ranked, vec!["a"]);
    }

    #[test]
    fn highlighting_marks_the_characters_that_answered() {
        let spans = Query::new("gd").highlight("guide.md");
        assert_eq!(text_of(&spans), "guide.md");
        assert_eq!(matched_of(&spans), "gd");
    }

    #[test]
    fn highlighting_marks_every_term() {
        let spans = Query::new("gu md").highlight("guide.md");
        assert_eq!(text_of(&spans), "guide.md");
        assert_eq!(matched_of(&spans), "gumd");
    }

    #[test]
    fn nothing_asked_and_nothing_found_are_one_span() {
        let spans = Query::new("").highlight("guide.md");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].matched);

        let spans = Query::new("zzz").highlight("guide.md");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].matched);
    }

    #[test]
    fn highlighting_counts_characters_rather_than_bytes() {
        let spans = Query::new("あd").highlight("あいう.md");
        assert_eq!(text_of(&spans), "あいう.md");
        assert_eq!(matched_of(&spans), "あd");
    }

    #[test]
    fn spans_split_where_the_name_is_drawn_in_two() {
        let spans = Query::new("dg").highlight("docs/guide.md");
        let (folder, name) = split_spans(spans, "docs/".chars().count());
        assert_eq!(text_of(&folder), "docs/");
        assert_eq!(text_of(&name), "guide.md");
        assert_eq!(matched_of(&folder), "d");
        assert_eq!(matched_of(&name), "g");
    }

    #[test]
    fn splitting_at_either_end_leaves_one_side_empty() {
        let spans = Query::new("g").highlight("guide.md");
        let (before, after) = split_spans(spans.clone(), 0);
        assert!(before.is_empty());
        assert_eq!(text_of(&after), "guide.md");

        let (before, after) = split_spans(spans, "guide.md".chars().count());
        assert_eq!(text_of(&before), "guide.md");
        assert!(after.is_empty());
    }
}
