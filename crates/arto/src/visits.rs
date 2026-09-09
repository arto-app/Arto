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
/// where to find it again. Only the count is bounded, and generously.
pub const MAX_VISITS: usize = 5_000;

/// One document, and when it was last read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub path: PathBuf,
    pub at: DateTime<Local>,
}

impl Visit {
    pub fn new(path: impl Into<PathBuf>, at: DateTime<Local>) -> Self {
        Self {
            path: path.into(),
            at,
        }
    }
}

/// A heading in the history list.
///
/// The order of the variants is the order they appear, newest first, and no
/// two of them can hold the same day: [`bucket_for`] tries them in this order
/// and stops at the first that fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The visits answering a query, newest first.
pub fn filter<'a>(visits: &'a [Visit], query: &str) -> Vec<&'a Visit> {
    visits.iter().filter(|v| matches(v, query)).collect()
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
    /// A document read again moves to the front rather than appearing twice:
    /// the list answers "where have I been", and the same place twice over is
    /// not two answers.
    pub fn record(&mut self, path: impl Into<PathBuf>, at: DateTime<Local>) {
        let path = path.into();
        self.items.retain(|visit| visit.path != path);
        self.items.insert(0, Visit::new(path, at));
        self.items.truncate(MAX_VISITS);
    }

    /// Forget one document, for a reader who would rather it were not listed.
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

    /// One row per document, however the file spells them.
    ///
    /// A history written before the spellings were folded together holds the
    /// same document twice — once as it was typed, once as its own folders
    /// name it. The list is newest first, so the first of a pair is the one
    /// worth keeping.
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
                .filter(|visit| seen.insert(visit.path.clone()))
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

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // 2026-04-16 is a Thursday, so the week runs Monday the 13th to Sunday
    // the 19th, and the week before it starts on Monday the 6th.
    const TODAY: (i32, u32, u32) = (2026, 4, 16);

    fn today() -> NaiveDate {
        day(TODAY.0, TODAY.1, TODAY.2)
    }

    fn at(y: i32, m: u32, d: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

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
    fn reading_something_again_moves_it_rather_than_repeating_it() {
        let mut visits = Visits::default();
        visits.record("/a.md", at(2026, 4, 14));
        visits.record("/b.md", at(2026, 4, 15));
        visits.record("/a.md", at(2026, 4, 16));

        assert_eq!(visits.items.len(), 2);
        assert_eq!(visits.items[0].path, PathBuf::from("/a.md"));
        assert_eq!(visits.items[0].at, at(2026, 4, 16));
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
