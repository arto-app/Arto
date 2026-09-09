//! What has been read, and when.
//!
//! A reader loses nothing by closing a document, so "open" was never a state
//! worth keeping — the only thing that turns out to matter is what was read
//! and how recently. That is this list, and it is the one behind every way
//! the interface offers to go back: the palette's resting state, the list
//! dropped from the breadcrumb, the panel's history face, and the welcome page.
//!
//! Grouping coarsens with age. The last few days are worth separating by day;
//! a year ago, the month is as fine as anyone needs, and a year before that,
//! the year. [`group`] is where that happens, and it is pure so the
//! boundaries can be tested against a fixed clock.

use crate::scroll_anchor::ScrollAnchor;
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Weekday};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::broadcast;

/// How many visits are kept.
///
/// There is no expiry: reading something a year ago is still a fact about
/// where to find it again. Only the count is bounded, and generously — but a
/// visit is one document on one day, so a document read every day costs a row
/// a day, and the ceiling is reached in months of heavy reading rather than
/// years. What falls off is the oldest rows, which are also the ones nothing
/// but the history itself still names.
///
/// It is a count rather than a size because the file is written whole every
/// time a document is opened: at this ceiling it is about a megabyte, which
/// is a write worth not thinking about, and ten times that would not be.
pub const MAX_VISITS: usize = 5_000;

/// One document, when it was last read, and how far into it the reader had
/// got.
///
/// The position is part of what "I have read this" means: a document opened
/// again from the history opens where it was left, rather than at a top the
/// reader has already been past. A history written before positions were kept
/// has none, which reads as the top.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub path: PathBuf,
    pub at: DateTime<Local>,
    #[serde(default)]
    pub anchor: ScrollAnchor,
}

impl Visit {
    pub fn new(path: impl Into<PathBuf>, at: DateTime<Local>) -> Self {
        Self {
            path: path.into(),
            at,
            anchor: ScrollAnchor::TOP,
        }
    }
}

/// A heading in the history list.
///
/// The order of the variants is the order they appear, newest first, and no
/// two of them can hold the same day: [`bucket_for`] tries them in this order
/// and stops at the first that fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Today,
    Yesterday,
    /// This week, apart from today and yesterday.
    ThisWeek,
    LastWeek,
    /// A month of the current year, older than last week.
    Month(i32, u32),
    /// An earlier year, whole.
    Year(i32),
}

impl Bucket {
    /// The heading this group is drawn under.
    ///
    /// Every window on the history writes the same words for the same group,
    /// so the panel's face, the welcome page and anything else naming a group
    /// share one definition rather than three that can drift.
    pub fn heading(&self) -> String {
        match self {
            Self::Today => "Today".to_string(),
            Self::Yesterday => "Yesterday".to_string(),
            Self::ThisWeek => "This week".to_string(),
            Self::LastWeek => "Last week".to_string(),
            Self::Month(year, month) => format!("{year}-{month:02}"),
            Self::Year(year) => year.to_string(),
        }
    }
}

/// Which heading a day belongs under, relative to `today`.
pub fn bucket_for(day: NaiveDate, today: NaiveDate) -> Bucket {
    // A clock that has gone backwards should not invent a bucket of its own.
    if day >= today {
        return Bucket::Today;
    }
    if today.checked_sub_days(Days::new(1)) == Some(day) {
        return Bucket::Yesterday;
    }

    let this_week = today.week(Weekday::Mon).first_day();
    if day >= this_week {
        return Bucket::ThisWeek;
    }
    if let Some(last_week) = this_week.checked_sub_days(Days::new(7)) {
        if day >= last_week {
            return Bucket::LastWeek;
        }
    }

    if day.year() == today.year() {
        Bucket::Month(day.year(), day.month())
    } else {
        Bucket::Year(day.year())
    }
}

/// Split visits into headed groups, newest first.
///
/// `visits` must already be newest first — [`Visits`] keeps it that way — so
/// the groups come out in order and each is a run of neighbours rather than a
/// separate pass over the list.
pub fn group(visits: &[Visit], now: DateTime<Local>) -> Vec<(Bucket, Vec<&Visit>)> {
    let today = now.date_naive();
    let mut groups: Vec<(Bucket, Vec<&Visit>)> = Vec::new();

    for visit in visits {
        let bucket = bucket_for(visit.at.date_naive(), today);
        match groups.last_mut() {
            Some((last, entries)) if *last == bucket => entries.push(visit),
            _ => groups.push((bucket, vec![visit])),
        }
    }

    groups
}

/// Whether a visit answers a filter query.
///
/// The palette, the panel's history face and the welcome page all narrow the same
/// list, so they narrow it the same way: a query that finds a document in one
/// of them finds it in all three.
///
/// Terms are separated by whitespace and all of them must match, each against
/// the whole path rather than the name alone — so `guide arto` finds
/// `~/arto/docs/guide.md` however the two words are ordered, and `docs/`
/// narrows to a directory. Matching ignores case, which is what a reader
/// typing quickly expects.
pub fn matches(visit: &Visit, query: &str) -> bool {
    matches_path(&visit.path, query)
}

/// The same rule, for the lists that are not visits — the places kept and the
/// documents starred. One query narrows a screen, so it has to mean the same
/// thing in every list on it.
pub fn matches_path(path: &Path, query: &str) -> bool {
    let haystack = path.to_string_lossy().to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

/// When a document was last read, said in whatever unit still adds something.
///
/// The heading above a row already says the day, so repeating it there is a
/// column of the same date over and over: within a day or two the hour is what
/// distinguishes one row from the next, within the week the weekday does, and
/// only past that is the date itself worth the space.
pub fn short_when(at: DateTime<Local>, now: DateTime<Local>) -> String {
    let days = (now.date_naive() - at.date_naive()).num_days();
    match days {
        0 | 1 => at.format("%H:%M").to_string(),
        2..=6 => at.format("%a").to_string(),
        _ => at.format("%-m/%-d").to_string(),
    }
}

/// When `path` was last read, if it ever was.
pub fn last_read(path: &Path) -> Option<DateTime<Local>> {
    VISITS
        .read()
        .items
        .iter()
        .find(|visit| visit.path == path)
        .map(|visit| visit.at)
}

/// When something under `dir` was last read, if anything ever was.
///
/// A folder is not read; the documents in it are. The newest of those is what
/// says when the reader was last there.
pub fn last_read_under(dir: &Path) -> Option<DateTime<Local>> {
    VISITS
        .read()
        .items
        .iter()
        .find(|visit| visit.path.starts_with(dir))
        .map(|visit| visit.at)
}

/// The history read as documents rather than as days: one row each, newest
/// first.
///
/// The list itself keeps a visit per day, because what was read yesterday is
/// a fact about yesterday that reading it again today cannot change. The
/// windows that group by day want exactly that. The ones that are about
/// *documents* — the palette, the trace in the margin — want the other
/// reading: the same document on two days is one answer to "where have I
/// been", not two.
pub fn documents(visits: &[Visit]) -> impl Iterator<Item = &Visit> {
    let mut seen = std::collections::HashSet::new();
    visits
        .iter()
        .filter(move |visit| seen.insert(visit.path.as_path()))
}

/// The documents answering a query, newest first.
pub fn filter<'a>(visits: &'a [Visit], query: &str) -> Vec<&'a Visit> {
    documents(visits).filter(|v| matches(v, query)).collect()
}

/// The visit list, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Visits {
    pub items: Vec<Visit>,
}

impl Visits {
    /// Note that `path` was read at `at`.
    ///
    /// One row per document per day. Reading something again today is the
    /// same day's reading still going on, so the row moves to the front and
    /// takes the later time; reading it again tomorrow is a new fact, and the
    /// row it had yesterday stays where it is. The list is grouped by day
    /// wherever it is drawn, and moving yesterday's row into today would say
    /// that yesterday's reading never happened.
    ///
    /// The new row starts where the last one left off: how far into a
    /// document the reader had got is a fact about the document, not about
    /// the day, and opening it tomorrow has to land where opening it an hour
    /// from now would.
    pub fn record(&mut self, path: impl Into<PathBuf>, at: DateTime<Local>) {
        let day = at.date_naive();
        let mut visit = Visit::new(path, at);
        visit.anchor = self.position(&visit.path).unwrap_or(ScrollAnchor::TOP);
        self.items
            .retain(|other| other.path != visit.path || other.at.date_naive() != day);
        self.items.insert(0, visit);
        self.items.truncate(MAX_VISITS);
    }

    /// Note how far into a document the reader has got.
    ///
    /// The order is left alone: this is not a visit, it is the same visit
    /// still going on, and moving the row would make scrolling look like
    /// reading something new.
    pub fn keep_position(&mut self, path: &Path, anchor: ScrollAnchor) {
        if let Some(visit) = self.items.iter_mut().find(|visit| visit.path == path) {
            visit.anchor = anchor;
        }
    }

    /// Where the reader had got to, if this document has been read before.
    pub fn position(&self, path: &Path) -> Option<ScrollAnchor> {
        self.items
            .iter()
            .find(|visit| visit.path == path)
            .map(|visit| visit.anchor)
    }

    /// Forget one document, for a reader who would rather it were not listed.
    ///
    /// Every day of it, not the row that was pointed at: what is being asked
    /// for is that the document not be in the history, and leaving yesterday's
    /// row behind would answer a question nobody asked.
    pub fn forget(&mut self, path: &Path) {
        self.items.retain(|visit| visit.path != path);
    }

    pub fn grouped(&self, now: DateTime<Local>) -> Vec<(Bucket, Vec<&Visit>)> {
        group(&self.items, now)
    }

    fn path() -> PathBuf {
        crate::bookmarks::data_file("visits.json")
    }

    /// Load the list, or start an empty one.
    ///
    /// A file that cannot be read or parsed is not worth failing over: the
    /// history is a convenience, and losing it costs a reader nothing they
    /// cannot get back by opening the document again.
    pub fn load() -> Self {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str::<Self>(&content)
                .map(Self::folded)
                .unwrap_or_else(|err| {
                    tracing::warn!(?path, %err, "Ignoring unreadable visit history");
                    Self::default()
                }),
            Err(_) => Self::default(),
        }
    }

    /// One row per document per day, however the file spells them.
    ///
    /// A history written before the spellings were folded together holds the
    /// same document twice on the same day — once as it was typed, once as
    /// its own folders name it — and one written before the day was part of
    /// what a row says holds every reading of it. The list is newest first,
    /// so the first of a day's rows is the one worth keeping.
    fn folded(self) -> Self {
        let mut seen = std::collections::HashSet::new();
        Self {
            items: self
                .items
                .into_iter()
                .map(|visit| Visit {
                    path: crate::utils::paths::true_spelling(&visit.path),
                    ..visit
                })
                .filter(|visit| seen.insert((visit.path.clone(), visit.at.date_naive())))
                .collect(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
    }
}

/// The history, shared by every window.
pub static VISITS: LazyLock<RwLock<Visits>> = LazyLock::new(|| RwLock::new(Visits::load()));

/// Announced whenever the history changes, so open windows redraw.
pub static VISITS_CHANGED: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);

/// Note where the reader has got to, in memory alone.
///
/// Called as a document is left rather than as it is scrolled: the window
/// already holds the live position, so the history only needs it at the
/// moment the row stops being the one being read. It goes to disk with the
/// next save — the visit that follows, or the window closing.
pub fn keep_position(path: &Path, anchor: ScrollAnchor) {
    let path = crate::utils::paths::true_spelling(path);
    VISITS.write().keep_position(&path, anchor);
}

/// Where the reader had got to in a document they have read before.
pub fn position(path: &Path) -> Option<ScrollAnchor> {
    let path = crate::utils::paths::true_spelling(path);
    VISITS.read().position(&path)
}

/// Write the history out, positions and all.
pub fn save_visits() {
    if let Err(err) = VISITS.read().save() {
        tracing::warn!(%err, "Failed to save visit history");
    }
}

/// Record a visit, persist it, and tell the other windows.
pub fn record_visit(path: impl Into<PathBuf>) {
    let mut visits = VISITS.write();
    // Two spellings of one document would be two rows in every list that
    // reads this, and the reader would have opened the same file twice.
    visits.record(
        crate::utils::paths::true_spelling(&path.into()),
        Local::now(),
    );
    if let Err(err) = visits.save() {
        tracing::warn!(%err, "Failed to save visit history");
    }
    drop(visits);
    let _ = VISITS_CHANGED.send(());
}

/// Drop one document from the history, persist it, and tell the other windows.
pub fn forget_visit(path: &Path) {
    let mut visits = VISITS.write();
    visits.forget(path);
    if let Err(err) = visits.save() {
        tracing::warn!(%err, "Failed to save visit history");
    }
    drop(visits);
    let _ = VISITS_CHANGED.send(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn the_unit_is_whatever_the_heading_has_not_said() {
        let now = Local.with_ymd_and_hms(2026, 9, 9, 12, 0, 0).unwrap();
        let at = |month, day, hour, minute| {
            Local
                .with_ymd_and_hms(2026, month, day, hour, minute, 0)
                .unwrap()
        };

        // Today and yesterday are named above the row, so the hour is what is
        // left to say.
        assert_eq!(short_when(at(9, 9, 9, 31), now), "09:31");
        assert_eq!(short_when(at(9, 8, 19, 44), now), "19:44");
        // Inside the week the day is the distinction.
        assert_eq!(short_when(at(9, 5, 17, 31), now), "Sat");
        // Past that, the date.
        assert_eq!(short_when(at(8, 30, 19, 44), now), "8/30");
    }

    fn at(y: i32, m: u32, d: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    /// The same day at a named hour, for what happens inside one day.
    fn at_hour(y: i32, m: u32, d: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, hour, 0, 0).unwrap()
    }

    #[test]
    fn a_kept_position_comes_back_and_leaves_the_order_alone() {
        let mut visits = Visits::default();
        visits.record("/docs/first.md", at(2026, 9, 8));
        visits.record("/docs/second.md", at(2026, 9, 9));

        let place = ScrollAnchor {
            line: 42,
            fraction: 0.25,
        };
        visits.keep_position(Path::new("/docs/first.md"), place);

        assert_eq!(visits.position(Path::new("/docs/first.md")), Some(place));
        // Reading on is not visiting again: the newest is still the newest.
        assert_eq!(visits.items[0].path, PathBuf::from("/docs/second.md"));
    }

    #[test]
    fn a_document_never_read_has_no_position() {
        let mut visits = Visits::default();
        visits.record("/docs/first.md", at(2026, 9, 8));
        assert_eq!(visits.position(Path::new("/docs/other.md")), None);
        // One that has been read but not scrolled starts at the top.
        assert_eq!(
            visits.position(Path::new("/docs/first.md")),
            Some(ScrollAnchor::TOP)
        );
    }

    #[test]
    fn a_history_written_before_positions_existed_reads_as_the_top() {
        let json = r#"{"items":[{"path":"/docs/first.md","at":"2026-09-08T12:00:00+09:00"}]}"#;
        let visits: Visits = serde_json::from_str(json).expect("loads");
        assert_eq!(visits.items[0].anchor, ScrollAnchor::TOP);
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // 2026-04-16 is a Thursday, so the week runs Monday the 13th to Sunday
    // the 19th, and the week before it starts on Monday the 6th.
    const TODAY: (i32, u32, u32) = (2026, 4, 16);

    fn today() -> NaiveDate {
        day(TODAY.0, TODAY.1, TODAY.2)
    }

    // === bucket_for(): the boundaries, from both sides ===

    #[test]
    fn today_and_yesterday_stand_alone() {
        assert_eq!(bucket_for(day(2026, 4, 16), today()), Bucket::Today);
        assert_eq!(bucket_for(day(2026, 4, 15), today()), Bucket::Yesterday);
    }

    #[test]
    fn this_week_starts_on_monday() {
        assert_eq!(bucket_for(day(2026, 4, 14), today()), Bucket::ThisWeek);
        assert_eq!(bucket_for(day(2026, 4, 13), today()), Bucket::ThisWeek);
        assert_eq!(bucket_for(day(2026, 4, 12), today()), Bucket::LastWeek);
    }

    #[test]
    fn last_week_is_seven_days_and_no_more() {
        assert_eq!(bucket_for(day(2026, 4, 6), today()), Bucket::LastWeek);
        assert_eq!(
            bucket_for(day(2026, 4, 5), today()),
            Bucket::Month(2026, 4),
            "the day before last week falls back to its month"
        );
    }

    #[test]
    fn earlier_in_the_year_groups_by_month() {
        assert_eq!(
            bucket_for(day(2026, 3, 31), today()),
            Bucket::Month(2026, 3)
        );
        assert_eq!(bucket_for(day(2026, 1, 1), today()), Bucket::Month(2026, 1));
    }

    #[test]
    fn earlier_years_group_whole() {
        assert_eq!(bucket_for(day(2025, 12, 31), today()), Bucket::Year(2025));
        assert_eq!(bucket_for(day(2023, 6, 1), today()), Bucket::Year(2023));
    }

    #[test]
    fn a_day_in_the_future_is_treated_as_today() {
        // Clock skew, or a file touched across a timezone change.
        assert_eq!(bucket_for(day(2026, 4, 17), today()), Bucket::Today);
    }

    #[test]
    fn yesterday_wins_over_the_week_it_belongs_to() {
        // On a Monday, yesterday is a Sunday that belongs to last week.
        let monday = day(2026, 4, 13);
        assert_eq!(bucket_for(day(2026, 4, 12), monday), Bucket::Yesterday);
    }

    // === group() ===

    #[test]
    fn group_keeps_the_order_and_runs_neighbours_together() {
        let visits = vec![
            Visit::new("/a.md", at(2026, 4, 16)),
            Visit::new("/b.md", at(2026, 4, 16)),
            Visit::new("/c.md", at(2026, 4, 15)),
            Visit::new("/d.md", at(2026, 3, 2)),
            Visit::new("/e.md", at(2024, 7, 9)),
        ];

        let groups = group(&visits, at(TODAY.0, TODAY.1, TODAY.2));
        let headings: Vec<_> = groups.iter().map(|(bucket, _)| *bucket).collect();

        assert_eq!(
            headings,
            vec![
                Bucket::Today,
                Bucket::Yesterday,
                Bucket::Month(2026, 3),
                Bucket::Year(2024),
            ]
        );
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[0].1[0].path, PathBuf::from("/a.md"));
    }

    #[test]
    fn group_of_nothing_is_nothing() {
        assert!(group(&[], at(TODAY.0, TODAY.1, TODAY.2)).is_empty());
    }

    // === Visits ===

    #[test]
    fn recording_puts_the_newest_first() {
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 14));
        visits.record("/b.md", at(2026, 4, 15));

        assert_eq!(visits.items[0].path, PathBuf::from("/b.md"));
        assert_eq!(visits.items[1].path, PathBuf::from("/a.md"));
    }

    #[test]
    fn reading_something_again_the_same_day_moves_it_rather_than_repeating_it() {
        let mut visits = Visits::default();
        visits.record("/a.md", at_hour(2026, 4, 16, 9));
        visits.record("/b.md", at_hour(2026, 4, 16, 10));
        visits.record("/a.md", at_hour(2026, 4, 16, 11));

        assert_eq!(visits.items.len(), 2);
        assert_eq!(visits.items[0].path, PathBuf::from("/a.md"));
        assert_eq!(visits.items[0].at, at_hour(2026, 4, 16, 11));
    }

    #[test]
    fn reading_something_again_another_day_leaves_the_day_it_was_read() {
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 15));
        visits.record("/a.md", at(2026, 4, 16));

        assert_eq!(visits.items.len(), 2);
        assert_eq!(visits.items[0].at, at(2026, 4, 16));
        assert_eq!(visits.items[1].at, at(2026, 4, 15));
    }

    #[test]
    fn a_new_day_carries_the_place_the_reader_had_got_to() {
        let place = ScrollAnchor {
            line: 42,
            fraction: 0.25,
        };
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 15));
        visits.keep_position(Path::new("/a.md"), place);
        visits.record("/a.md", at(2026, 4, 16));

        assert_eq!(visits.position(Path::new("/a.md")), Some(place));
        // And yesterday still says where the reader was yesterday.
        assert_eq!(visits.items[1].anchor, place);
    }

    #[test]
    fn the_lists_about_documents_see_one_row_each() {
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 14));
        visits.record("/b.md", at(2026, 4, 15));
        visits.record("/a.md", at(2026, 4, 16));

        let documents: Vec<&PathBuf> = documents(&visits.items).map(|v| &v.path).collect();
        assert_eq!(
            documents,
            vec![&PathBuf::from("/a.md"), &PathBuf::from("/b.md")]
        );
    }

    #[test]
    fn loading_keeps_one_row_per_document_per_day() {
        // A history written before the day was part of what a row says holds
        // the same document twice in one day.
        let visits = Visits {
            items: vec![
                Visit::new("/a.md", at_hour(2026, 4, 16, 11)),
                Visit::new("/a.md", at_hour(2026, 4, 16, 9)),
                Visit::new("/a.md", at(2026, 4, 15)),
            ],
        }
        .folded();

        assert_eq!(visits.items.len(), 2);
        assert_eq!(visits.items[0].at, at_hour(2026, 4, 16, 11));
        assert_eq!(visits.items[1].at, at(2026, 4, 15));
    }

    #[test]
    fn the_ceiling_drops_the_oldest() {
        let mut visits = Visits::default();
        for i in 0..MAX_VISITS + 10 {
            visits.record(format!("/{i}.md"), at(2026, 4, 16));
        }

        assert_eq!(visits.items.len(), MAX_VISITS);
        assert_eq!(visits.items[0].path, PathBuf::from("/5009.md"));
    }

    #[test]
    fn forgetting_removes_one_entry() {
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 14));
        visits.record("/b.md", at(2026, 4, 15));
        visits.forget(Path::new("/a.md"));

        assert_eq!(visits.items.len(), 1);
        assert_eq!(visits.items[0].path, PathBuf::from("/b.md"));
    }

    #[test]
    fn the_list_survives_a_serialisation_roundtrip() {
        let mut visits = Visits::default();
        visits.record("/notes/today.md", at(2026, 4, 16));

        let json = serde_json::to_string_pretty(&visits).unwrap();
        let parsed: Visits = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, visits);
    }

    #[test]
    fn every_group_names_itself() {
        assert_eq!(Bucket::Today.heading(), "Today");
        assert_eq!(Bucket::Yesterday.heading(), "Yesterday");
        assert_eq!(Bucket::ThisWeek.heading(), "This week");
        assert_eq!(Bucket::LastWeek.heading(), "Last week");
        assert_eq!(Bucket::Month(2026, 3).heading(), "2026-03");
        assert_eq!(Bucket::Year(2024).heading(), "2024");
    }

    // === matches(): one rule for the palette, the face and the welcome page ===

    #[test]
    fn an_empty_query_matches_everything() {
        let visit = Visit::new("/home/reader/notes/guide.md", at(2026, 4, 16));
        assert!(matches(&visit, ""));
        assert!(matches(&visit, "   "));
    }

    #[test]
    fn matching_ignores_case_and_spans_the_whole_path() {
        let visit = Visit::new("/home/reader/Arto/docs/Guide.md", at(2026, 4, 16));
        assert!(matches(&visit, "guide"));
        assert!(matches(&visit, "docs/"));
        assert!(matches(&visit, "ARTO"));
        assert!(!matches(&visit, "readme"));
    }

    #[test]
    fn every_term_must_match_in_any_order() {
        let visit = Visit::new("/home/reader/arto/docs/guide.md", at(2026, 4, 16));
        assert!(matches(&visit, "guide arto"));
        assert!(matches(&visit, "arto guide"));
        assert!(!matches(&visit, "guide missing"));
    }

    #[test]
    fn filter_keeps_the_order_it_was_given() {
        let visits = vec![
            Visit::new("/notes/b.md", at(2026, 4, 16)),
            Visit::new("/notes/a.md", at(2026, 4, 15)),
            Visit::new("/other/c.md", at(2026, 4, 14)),
        ];
        let found = filter(&visits, "notes");
        assert_eq!(
            found
                .iter()
                .map(|v| crate::utils::paths::short_name(&v.path))
                .collect::<Vec<_>>(),
            vec!["notes/b.md", "notes/a.md"]
        );
    }
}
