//! Highlights: places in a document the reader marked, kept for the next
//! time it is opened.
//!
//! A highlight is a place, not a word — that is a pinned search — so it
//! belongs to one document and is kept with it: one record per document in
//! the app's data, never in the document itself, which Arto only reads.
//!
//! The place is named by the text the reader saw rather than by the source:
//! the selection is made in the rendered page, and a selection across a bold
//! word or a link has no reliable way back to source columns. So a highlight
//! carries the words it covers, a little of what surrounds them, and where
//! they were last found (see [`TextAnchor`]); the page finds them again, and
//! says where, which is what lets a highlight follow its words through an
//! edit. Words the page cannot find any more keep their record — they are
//! shown as lost rather than dropped, since the reader may want them back.

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::utils::data_store::{record_file_name, write_atomically};

pub use crate::highlight_color::HighlightColor;

pub mod card;
pub mod page;

/// The version of the record format, so an older one is read as empty
/// rather than misread.
const VERSION: u32 = 1;

/// Unique identifier for a highlight.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HighlightId(String);

impl HighlightId {
    /// Generate a new unique ID.
    pub fn new() -> Self {
        Self(format!("hl_{}", Uuid::new_v4().simple()))
    }
}

impl Default for HighlightId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HighlightId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for HighlightId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for HighlightId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Where a highlight is, as the page names it.
///
/// The shape of the W3C Web Annotation text quote and text position
/// selectors, with a source line beside them: the quote is what is looked
/// for, the position and the line decide between several places that quote
/// could be. The page's text is its rendered text with code blocks,
/// diagrams and maths left out, and offsets count what JavaScript counts
/// (UTF-16 code units) — see `frontend/src/text-anchor.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAnchor {
    /// The words highlighted.
    pub exact: String,
    /// What comes right before them.
    pub prefix: String,
    /// What comes right after them.
    pub suffix: String,
    /// Where they start in the page's text.
    pub start: u32,
    /// The first source line of the block they start in.
    pub line: u32,
}

/// A place the reader marked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub id: HighlightId,
    pub color: HighlightColor,
    pub created_at: DateTime<Utc>,
    pub anchor: TextAnchor,
    /// What the reader wrote about the words, if anything. Never empty: a
    /// note cleared is no note (see [`set_note`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Highlight {
    pub fn new(anchor: TextAnchor, color: HighlightColor) -> Self {
        Self {
            id: HighlightId::new(),
            color,
            created_at: Utc::now(),
            anchor,
            note: None,
        }
    }
}

/// One document's highlights, as they are written.
#[derive(Debug, Serialize, Deserialize)]
struct Record {
    version: u32,
    /// The document, so a record can be told apart by a person looking at
    /// the directory — its file name is a hash.
    path: String,
    highlights: Vec<Highlight>,
}

/// The records, in one directory.
pub struct Store {
    root: PathBuf,
    /// Serializes the read-modify-write of a record across windows.
    lock: Mutex<()>,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Mutex::new(()),
        }
    }

    /// The highlights kept on `document`, empty when there are none or the
    /// record cannot be read.
    pub fn load(&self, document: &Path) -> Vec<Highlight> {
        fs::read(self.record_path(document))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            .filter(|record| record.version == VERSION)
            .map(|record| record.highlights)
            .unwrap_or_default()
    }

    /// Keep `highlights` as those on `document`. With none left the record
    /// goes, rather than staying as an empty file for every document ever
    /// marked once.
    pub fn save(&self, document: &Path, highlights: &[Highlight]) -> io::Result<()> {
        let path = self.record_path(document);
        if highlights.is_empty() {
            return match fs::remove_file(&path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        let record = Record {
            version: VERSION,
            path: document_name(document),
            highlights: highlights.to_vec(),
        };
        write_atomically(&path, &serde_json::to_vec_pretty(&record)?)
    }

    /// Change the highlights on `document` and keep the result, if `change`
    /// says it changed anything. Returns what it said.
    pub fn change(
        &self,
        document: &Path,
        change: impl FnOnce(&mut Vec<Highlight>) -> bool,
    ) -> io::Result<bool> {
        let _guard = self.lock.lock();
        let mut highlights = self.load(document);
        if !change(&mut highlights) {
            return Ok(false);
        }
        self.save(document, &highlights)?;
        Ok(true)
    }

    fn record_path(&self, document: &Path) -> PathBuf {
        self.root
            .join(record_file_name(&[&document_name(document)], "json"))
    }
}

/// What a document is filed under: the file its path leads to, spelled as
/// the filesystem spells it, so a document reached through a link or by
/// another spelling has one record.
fn document_name(document: &Path) -> String {
    let real = document
        .canonicalize()
        .unwrap_or_else(|_| document.to_path_buf());
    crate::utils::paths::true_spelling(&real)
        .to_string_lossy()
        .into_owned()
}

/// Whether two paths name the same document's highlights.
pub fn same_document(a: &Path, b: &Path) -> bool {
    a == b || document_name(a) == document_name(b)
}

/// Take away the highlights `ids` names. Returns whether any was there.
pub fn remove(highlights: &mut Vec<Highlight>, ids: &[HighlightId]) -> bool {
    let before = highlights.len();
    highlights.retain(|highlight| !ids.contains(&highlight.id));
    highlights.len() < before
}

/// Draw the highlight `id` in `color`. Returns whether that changed it.
pub fn set_color(highlights: &mut [Highlight], id: &HighlightId, color: HighlightColor) -> bool {
    match highlights.iter_mut().find(|highlight| &highlight.id == id) {
        Some(highlight) if highlight.color != color => {
            highlight.color = color;
            true
        }
        _ => false,
    }
}

/// Write `note` on the highlight `id`, without the blank lines and spaces
/// around it; nothing but white space takes the note away. Returns whether
/// that changed it.
pub fn set_note(highlights: &mut [Highlight], id: &HighlightId, note: &str) -> bool {
    let note = Some(note.trim()).filter(|note| !note.is_empty());
    match highlights.iter_mut().find(|highlight| &highlight.id == id) {
        Some(highlight) if highlight.note.as_deref() != note => {
            highlight.note = note.map(str::to_string);
            true
        }
        _ => false,
    }
}

/// Note that the page found the highlight `id` at `start`, in the block on
/// `line`. Returns whether that is somewhere else than it was.
pub fn rebase(highlights: &mut [Highlight], id: &HighlightId, start: u32, line: u32) -> bool {
    match highlights.iter_mut().find(|highlight| &highlight.id == id) {
        Some(highlight) if (highlight.anchor.start, highlight.anchor.line) != (start, line) => {
            highlight.anchor.start = start;
            highlight.anchor.line = line;
            true
        }
        _ => false,
    }
}

/// The store the app uses, or `None` where the system names no data
/// directory.
static STORE: LazyLock<Option<Store>> = LazyLock::new(|| {
    dirs::data_local_dir().map(|dir| Store::new(dir.join("arto").join("highlights")))
});

/// Broadcast when a document's highlights change, with the document.
///
/// Every window showing that document draws them again; the others ignore it.
pub static HIGHLIGHTS_CHANGED: LazyLock<broadcast::Sender<PathBuf>> =
    LazyLock::new(|| broadcast::channel(16).0);

/// The colour the reader chose last, which is what a highlight made from the
/// keyboard is drawn in. Remembered for as long as the app runs.
static LAST_COLOR: RwLock<HighlightColor> = RwLock::new(HighlightColor::Green);

pub fn last_color() -> HighlightColor {
    *LAST_COLOR.read()
}

/// The highlights kept on `document`.
pub fn load(document: &Path) -> Vec<Highlight> {
    STORE
        .as_ref()
        .map(|store| store.load(document))
        .unwrap_or_default()
}

/// Change the highlights on `document`, and tell the windows showing it
/// when `announce` is set and something changed. Returns whether `f`
/// changed anything, or nothing when the highlights could not be kept.
fn change(
    document: &Path,
    announce: bool,
    f: impl FnOnce(&mut Vec<Highlight>) -> bool,
) -> Option<bool> {
    let store = STORE.as_ref()?;
    match store.change(document, f) {
        Ok(changed) => {
            if changed && announce {
                HIGHLIGHTS_CHANGED.send(document.to_path_buf()).ok();
            }
            Some(changed)
        }
        Err(error) => {
            tracing::warn!(%error, document = %document.display(), "highlights were not kept");
            None
        }
    }
}

/// Highlight what `anchor` names in `color`, which becomes the colour a
/// highlight is drawn in from now on. Returns the new highlight's id, or
/// nothing when it could not be kept.
pub fn add(document: &Path, anchor: TextAnchor, color: HighlightColor) -> Option<HighlightId> {
    *LAST_COLOR.write() = color;
    let highlight = Highlight::new(anchor, color);
    let id = highlight.id.clone();
    change(document, true, |highlights| {
        highlights.push(highlight);
        true
    })?;
    Some(id)
}

/// Take away the highlights `ids` names.
pub fn remove_highlights(document: &Path, ids: &[HighlightId]) {
    change(document, true, |highlights| remove(highlights, ids));
}

/// Draw the highlight `id` in `color`.
pub fn recolor(document: &Path, id: &HighlightId, color: HighlightColor) {
    *LAST_COLOR.write() = color;
    change(document, true, |highlights| {
        set_color(highlights, id, color)
    });
}

/// Write `note` on the highlight `id` (see [`set_note`]). Returns whether the
/// highlight is there to carry it: one taken away meanwhile, in another
/// window, leaves the note with nowhere to go.
pub fn annotate(document: &Path, id: &HighlightId, note: &str) -> bool {
    let mut present = false;
    let kept = change(document, true, |highlights| {
        present = highlights.iter().any(|highlight| highlight.id == *id);
        set_note(highlights, id, note)
    });
    kept.is_some() && present
}

/// Keep where the page found each highlight in `moves`, as `(id, start,
/// line)`.
///
/// Not announced: it is what the page just said, and every window showing
/// the document finds the same places for itself.
pub fn rebase_all(document: &Path, moves: &[(HighlightId, u32, u32)]) {
    change(document, false, |highlights| {
        moves.iter().fold(false, |moved, (id, start, line)| {
            rebase(highlights, id, *start, *line) || moved
        })
    });
}

/// Where the page found a highlight, if it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighlightPlace {
    /// On the page, under the heading with this id — `None` above the first.
    Found { heading: Option<String> },
    /// Its words are not on the page any more.
    Lost,
}

/// What the page found when it drew a document's highlights. Sent by
/// `frontend/src/user-highlights.ts`.
#[derive(Debug, Clone, Deserialize)]
pub struct PageReport {
    /// The document, as the app named it when it asked.
    pub doc: String,
    placed: Vec<Placed>,
    orphans: Vec<HighlightId>,
}

#[derive(Debug, Clone, Deserialize)]
struct Placed {
    id: HighlightId,
    start: u32,
    line: u32,
    heading: Option<String>,
}

impl PageReport {
    /// Where each highlight is.
    pub fn places(&self) -> std::collections::HashMap<HighlightId, HighlightPlace> {
        let found = self.placed.iter().map(|placed| {
            let place = HighlightPlace::Found {
                heading: placed.heading.clone(),
            };
            (placed.id.clone(), place)
        });
        let lost = self
            .orphans
            .iter()
            .map(|id| (id.clone(), HighlightPlace::Lost));
        found.chain(lost).collect()
    }

    /// The highlights of `highlights` found somewhere else than they were
    /// kept, as `(id, start, line)` for [`rebase_all`].
    pub fn moves(&self, highlights: &[Highlight]) -> Vec<(HighlightId, u32, u32)> {
        self.placed
            .iter()
            .filter(|placed| {
                highlights.iter().any(|highlight| {
                    highlight.id == placed.id
                        && (highlight.anchor.start, highlight.anchor.line)
                            != (placed.start, placed.line)
                })
            })
            .map(|placed| (placed.id.clone(), placed.start, placed.line))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_report_says_where_each_highlight_is_and_which_moved() {
        let stay = Highlight::new(anchor("a", 3, 1), HighlightColor::Green);
        let moved = Highlight::new(anchor("b", 10, 2), HighlightColor::Green);
        let lost = Highlight::new(anchor("c", 20, 3), HighlightColor::Green);
        let json = serde_json::json!({
            "doc": "/a.md",
            "placed": [
                { "id": stay.id, "start": 3, "line": 1, "heading": null },
                { "id": moved.id, "start": 14, "line": 4, "heading": "more" },
            ],
            "orphans": [lost.id],
        });
        let report: PageReport = serde_json::from_value(json).unwrap();

        let places = report.places();
        assert_eq!(places[&stay.id], HighlightPlace::Found { heading: None });
        assert_eq!(
            places[&moved.id],
            HighlightPlace::Found {
                heading: Some("more".to_string())
            }
        );
        assert_eq!(places[&lost.id], HighlightPlace::Lost);
        assert_eq!(
            report.moves(&[stay, moved.clone(), lost]),
            vec![(moved.id, 14, 4)]
        );
    }

    fn anchor(exact: &str, start: u32, line: u32) -> TextAnchor {
        TextAnchor {
            exact: exact.to_string(),
            prefix: "before ".to_string(),
            suffix: " after".to_string(),
            start,
            line,
        }
    }

    fn store(dir: &tempfile::TempDir) -> Store {
        Store::new(dir.path().to_path_buf())
    }

    #[test]
    fn highlights_kept_are_there_when_the_document_is_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let highlight = Highlight::new(anchor("words", 10, 3), HighlightColor::Pink);
        store(&dir)
            .save(&doc, std::slice::from_ref(&highlight))
            .unwrap();

        assert_eq!(store(&dir).load(&doc), vec![highlight]);
        assert!(store(&dir).load(&dir.path().join("b.md")).is_empty());
    }

    #[test]
    fn a_record_says_which_document_it_is_for() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);
        store
            .save(
                &doc,
                &[Highlight::new(anchor("x", 0, 1), HighlightColor::Blue)],
            )
            .unwrap();

        let written = fs::read_to_string(store.record_path(&doc)).unwrap();
        let record: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(record["version"], 1);
        assert_eq!(record["path"], document_name(&doc));
        assert_eq!(record["highlights"][0]["anchor"]["exact"], "x");
        assert_eq!(record["highlights"][0]["color"], "blue");
        assert!(record["highlights"][0].get("note").is_none());
    }

    #[test]
    fn a_document_with_no_highlights_left_has_no_record() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);
        store
            .save(
                &doc,
                &[Highlight::new(anchor("x", 0, 1), HighlightColor::Blue)],
            )
            .unwrap();

        store.save(&doc, &[]).unwrap();

        assert!(!store.record_path(&doc).exists());
        // Saving nothing over nothing is not an error either.
        store.save(&doc, &[]).unwrap();
    }

    #[test]
    fn a_record_that_cannot_be_read_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);
        fs::write(store.record_path(&doc), b"{ not json").unwrap();

        assert!(store.load(&doc).is_empty());
    }

    #[test]
    fn a_record_of_another_version_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);
        fs::write(
            store.record_path(&doc),
            br#"{"version":2,"path":"a.md","highlights":[]}"#,
        )
        .unwrap();

        assert!(store.load(&doc).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_document_reached_by_another_spelling_has_the_same_highlights() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("Notes.md"), "# Notes").unwrap();
        let store = Store::new(root.join("store"));
        let highlight = Highlight::new(anchor("x", 0, 1), HighlightColor::Blue);
        store
            .save(&root.join("Notes.md"), std::slice::from_ref(&highlight))
            .unwrap();

        assert_eq!(store.load(&root.join("notes.md")), vec![highlight]);
        assert!(same_document(
            &root.join("Notes.md"),
            &root.join("notes.md")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_document_reached_through_a_link_has_the_same_highlights() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("notes.md"), "# Notes").unwrap();
        std::os::unix::fs::symlink(root.join("notes.md"), root.join("link.md")).unwrap();
        let store = Store::new(root.join("store"));
        let highlight = Highlight::new(anchor("x", 0, 1), HighlightColor::Blue);
        store
            .save(&root.join("link.md"), std::slice::from_ref(&highlight))
            .unwrap();

        assert_eq!(store.load(&root.join("notes.md")), vec![highlight]);
        assert!(same_document(&root.join("link.md"), &root.join("notes.md")));
    }

    #[test]
    fn a_change_is_kept_only_when_it_changed_something() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);

        assert!(!store.change(&doc, |_| false).unwrap());
        assert!(!store.record_path(&doc).exists());

        assert!(store
            .change(&doc, |highlights| {
                highlights.push(Highlight::new(anchor("x", 0, 1), HighlightColor::Blue));
                true
            })
            .unwrap());
        assert_eq!(store.load(&doc).len(), 1);
    }

    #[test]
    fn where_the_page_found_a_highlight_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.md");
        let store = store(&dir);
        let highlight = Highlight::new(anchor("words", 10, 3), HighlightColor::Pink);
        let id = highlight.id.clone();
        store.save(&doc, &[highlight]).unwrap();

        assert!(store
            .change(&doc, |highlights| rebase(highlights, &id, 42, 7))
            .unwrap());

        let again = store.load(&doc);
        assert_eq!((again[0].anchor.start, again[0].anchor.line), (42, 7));
        // Found where it already was, nothing moved.
        assert!(!store
            .change(&doc, |highlights| rebase(highlights, &id, 42, 7))
            .unwrap());
    }

    #[test]
    fn highlights_are_removed_and_recoloured_by_id() {
        let first = Highlight::new(anchor("a", 0, 1), HighlightColor::Green);
        let second = Highlight::new(anchor("b", 5, 2), HighlightColor::Green);
        let mut highlights = vec![first.clone(), second.clone()];

        assert!(set_color(
            &mut highlights,
            &second.id,
            HighlightColor::Orange
        ));
        assert!(!set_color(
            &mut highlights,
            &second.id,
            HighlightColor::Orange
        ));
        assert!(!set_color(
            &mut highlights,
            &HighlightId::new(),
            HighlightColor::Blue
        ));
        assert_eq!(highlights[1].color, HighlightColor::Orange);

        assert!(remove(&mut highlights, std::slice::from_ref(&first.id)));
        assert!(!remove(&mut highlights, &[first.id]));
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].id, second.id);
    }

    #[test]
    fn a_note_is_kept_trimmed_and_an_empty_one_takes_the_note_away() {
        let highlight = Highlight::new(anchor("a", 0, 1), HighlightColor::Green);
        let id = highlight.id.clone();
        let mut highlights = vec![highlight];

        assert!(set_note(&mut highlights, &id, "  remember this \n"));
        assert_eq!(highlights[0].note.as_deref(), Some("remember this"));

        // The same words again, spaced differently, change nothing.
        assert!(!set_note(&mut highlights, &id, "remember this"));

        assert!(set_note(&mut highlights, &id, " \n\t"));
        assert_eq!(highlights[0].note, None);
        assert!(!set_note(&mut highlights, &id, ""));

        assert!(!set_note(&mut highlights, &HighlightId::new(), "elsewhere"));
    }

    #[test]
    fn a_note_on_record_is_read_back() {
        let json = r#"{
            "id": "hl_1",
            "color": "purple",
            "createdAt": "2026-01-01T00:00:00Z",
            "anchor": {"exact": "x", "prefix": "", "suffix": "", "start": 0, "line": 1},
            "note": "remember"
        }"#;
        let highlight: Highlight = serde_json::from_str(json).unwrap();
        assert_eq!(highlight.note.as_deref(), Some("remember"));
        assert_eq!(highlight.id, HighlightId::from("hl_1".to_string()));
    }

    #[test]
    fn ids_are_unique_and_named_as_highlights() {
        let id = HighlightId::new();
        assert_ne!(id, HighlightId::new());
        assert!(id.as_ref().starts_with("hl_"));
    }
}
