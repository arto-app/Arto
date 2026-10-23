//! Pinned Search feature for persistent keyword highlighting.
//!
//! This module provides:
//! - `PinnedSearch`: A pinned search query with color
//! - `PinnedSearches`: Collection of pinned searches with persistence
//! - `PINNED_SEARCHES`: Global static for app-wide access and cross-window sync

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use uuid::Uuid;

use crate::utils::data_store::{app_data_file, JsonFile, Shared};

pub use crate::highlight_color::HighlightColor;

/// Unique identifier for a pinned search.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PinnedSearchId(String);

impl PinnedSearchId {
    /// Generate a new unique ID.
    pub fn new() -> Self {
        Self(format!("ps_{}", Uuid::new_v4().simple()))
    }
}

impl Default for PinnedSearchId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PinnedSearchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PinnedSearchId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for PinnedSearchId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A pinned search entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedSearch {
    /// Unique identifier.
    pub id: PinnedSearchId,
    /// Search pattern (plain text, not regex).
    pub pattern: String,
    /// Highlight color.
    pub color: HighlightColor,
    /// Case-sensitive matching.
    pub case_sensitive: bool,
    /// Disabled (highlight not shown but kept in list).
    #[serde(default)]
    pub disabled: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl PinnedSearch {
    /// Create a new pinned search with the given pattern.
    pub fn new(pattern: impl Into<String>, color: HighlightColor) -> Self {
        Self {
            id: PinnedSearchId::new(),
            pattern: pattern.into(),
            color,
            case_sensitive: false,
            disabled: false,
            created_at: Utc::now(),
        }
    }
}

/// Pinned searches storage (saved to pinned-searches.json).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedSearches {
    /// File format version.
    #[serde(default = "default_version")]
    pub version: u32,
    /// List of pinned searches.
    #[serde(default)]
    pub pinned_searches: Vec<PinnedSearch>,
}

fn default_version() -> u32 {
    1
}

impl Default for PinnedSearches {
    fn default() -> Self {
        Self {
            version: 1,
            pinned_searches: Vec::new(),
        }
    }
}

impl PinnedSearches {
    /// Pin a pattern, case-insensitively.
    ///
    /// A pattern that a case-insensitive pin already marks returns that pin,
    /// shown again if it was hidden, instead of adding a copy: the frontend
    /// lets the first pin claim every occurrence, so a copy would match
    /// nothing and keep the highlight alive after the first was removed.
    ///
    /// Also returns whether the collection changed, so that pinning a
    /// pattern already shown neither rewrites the file nor makes every
    /// window reapply its highlights.
    pub fn add(&mut self, pattern: impl Into<String>) -> (&PinnedSearch, bool) {
        let pattern = pattern.into();
        let (index, changed) = match self.position_of_case_insensitive(&pattern) {
            Some(index) => {
                let pinned = &mut self.pinned_searches[index];
                let was_disabled = std::mem::replace(&mut pinned.disabled, false);
                (index, was_disabled)
            }
            None => {
                let color = self.next_color();
                self.pinned_searches.push(PinnedSearch::new(pattern, color));
                (self.pinned_searches.len() - 1, true)
            }
        };
        (&self.pinned_searches[index], changed)
    }

    fn position_of_case_insensitive(&self, pattern: &str) -> Option<usize> {
        let pattern = pattern.to_lowercase();
        self.pinned_searches
            .iter()
            .position(|p| !p.case_sensitive && p.pattern.to_lowercase() == pattern)
    }

    /// Remove a pinned search by ID.
    pub fn remove(&mut self, id: &PinnedSearchId) -> bool {
        let len_before = self.pinned_searches.len();
        self.pinned_searches.retain(|p| &p.id != id);
        self.pinned_searches.len() < len_before
    }

    /// Update the color of a pinned search.
    pub fn set_color(&mut self, id: &PinnedSearchId, color: HighlightColor) -> bool {
        if let Some(pinned) = self.pinned_searches.iter_mut().find(|p| &p.id == id) {
            pinned.color = color;
            true
        } else {
            false
        }
    }

    /// Toggle the disabled state of a pinned search.
    pub fn toggle_disabled(&mut self, id: &PinnedSearchId) -> bool {
        if let Some(pinned) = self.pinned_searches.iter_mut().find(|p| &p.id == id) {
            pinned.disabled = !pinned.disabled;
            true
        } else {
            false
        }
    }

    /// Check if a pattern is already pinned.
    #[cfg(test)]
    pub fn contains_pattern(&self, pattern: &str) -> bool {
        self.pinned_searches.iter().any(|p| p.pattern == pattern)
    }

    /// Get the next color to use.
    ///
    /// New pins follow a fixed color rotation based on the latest pinned color.
    /// This keeps color assignment predictable even immediately after app startup.
    fn next_color(&self) -> HighlightColor {
        self.pinned_searches
            .last()
            .map(|p| p.color.next())
            .unwrap_or_default()
    }
}

/// Every pinned search, shared by every window and announced when it changes.
///
/// The file is removed rather than written empty, so a reader who never pins
/// anything has no file for it.
pub static PINNED_SEARCHES: LazyLock<Shared<PinnedSearches>> = LazyLock::new(|| {
    Shared::load(
        JsonFile::at(app_data_file("pinned-searches.json"))
            .discarded_when(|pinned| pinned.pinned_searches.is_empty()),
    )
});

/// Pin a pattern and broadcast the change.
///
/// Returns the ID of the pin that marks the pattern, which is an existing
/// one when the pattern was already pinned (see [`PinnedSearches::add`]).
pub fn add_pinned_search(pattern: impl Into<String>) -> PinnedSearchId {
    PINNED_SEARCHES.update(|pinned| {
        let (search, changed) = pinned.add(pattern);
        (search.id.clone(), changed)
    })
}

/// Remove a pinned search and broadcast the change.
///
/// Returns `true` if the pinned search was found and removed.
pub fn remove_pinned_search(id: &PinnedSearchId) -> bool {
    PINNED_SEARCHES.update(|pinned| {
        let removed = pinned.remove(id);
        (removed, removed)
    })
}

/// Update the color of a pinned search and broadcast the change.
pub fn set_pinned_search_color(id: &PinnedSearchId, color: HighlightColor) -> bool {
    PINNED_SEARCHES.update(|pinned| {
        let found = pinned.set_color(id, color);
        (found, found)
    })
}

/// Toggle the disabled state of a pinned search and broadcast the change.
pub fn toggle_pinned_search_disabled(id: &PinnedSearchId) -> bool {
    PINNED_SEARCHES.update(|pinned| {
        let found = pinned.toggle_disabled(id);
        (found, found)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_search_id_generation() {
        let id1 = PinnedSearchId::new();
        let id2 = PinnedSearchId::new();
        assert_ne!(id1, id2);
        assert!(id1.0.starts_with("ps_"));
    }

    #[test]
    fn test_pinned_search_creation() {
        let pinned = PinnedSearch::new("TODO", HighlightColor::Orange);
        assert_eq!(pinned.pattern, "TODO");
        assert_eq!(pinned.color, HighlightColor::Orange);
        assert!(!pinned.case_sensitive);
        assert!(!pinned.disabled);
    }

    #[test]
    fn test_pinned_searches_add_remove() {
        let mut searches = PinnedSearches::default();

        let search = searches.add("TODO").0;
        let id = search.id.clone();
        assert_eq!(searches.pinned_searches.len(), 1);
        assert!(searches.contains_pattern("TODO"));

        assert!(searches.remove(&id));
        assert_eq!(searches.pinned_searches.len(), 0);
        assert!(!searches.contains_pattern("TODO"));
    }

    #[test]
    fn test_pinned_searches_color_distribution() {
        let mut searches = PinnedSearches::default();

        // Add 5 pinned searches - should use all 5 colors
        searches.add("A");
        searches.add("B");
        searches.add("C");
        searches.add("D");
        searches.add("E");

        let colors: Vec<_> = searches.pinned_searches.iter().map(|p| p.color).collect();
        assert!(colors.contains(&HighlightColor::Green));
        assert!(colors.contains(&HighlightColor::Blue));
        assert!(colors.contains(&HighlightColor::Pink));
        assert!(colors.contains(&HighlightColor::Orange));
        assert!(colors.contains(&HighlightColor::Purple));
    }

    #[test]
    fn test_pinned_searches_next_color_uses_existing_last_color() {
        let mut searches = PinnedSearches::default();
        searches
            .pinned_searches
            .push(PinnedSearch::new("Existing", HighlightColor::Blue));

        let new_pin = searches.add("New").0;
        assert_eq!(new_pin.color, HighlightColor::Pink);
    }

    #[test]
    fn test_pinned_searches_add_returns_existing_pin_for_same_pattern() {
        let mut searches = PinnedSearches::default();
        let first = searches.add("TODO").0.clone();

        let again = searches.add("TODO").0.clone();

        assert_eq!(again, first);
        assert_eq!(searches.pinned_searches, vec![first]);
    }

    #[test]
    fn test_pinned_searches_add_ignores_case_when_finding_existing_pin() {
        let mut searches = PinnedSearches::default();
        let first = searches.add("Todo").0.clone();

        let again = searches.add("TODO").0.clone();

        assert_eq!(again.id, first.id);
        assert_eq!(again.pattern, "Todo");
        assert_eq!(searches.pinned_searches.len(), 1);
    }

    #[test]
    fn test_pinned_searches_add_shows_existing_pin_again_when_hidden() {
        let mut searches = PinnedSearches::default();
        let id = searches.add("TODO").0.id.clone();
        searches.toggle_disabled(&id);

        let again = searches.add("TODO").0;

        assert_eq!(again.id, id);
        assert!(!again.disabled);
    }

    #[test]
    fn test_pinned_searches_add_reports_whether_it_changed_anything() {
        let mut searches = PinnedSearches::default();
        let (_, added) = searches.add("TODO");
        let id = searches.pinned_searches[0].id.clone();
        assert!(added);

        let (_, already_shown) = searches.add("TODO");
        assert!(!already_shown);

        searches.toggle_disabled(&id);
        let (_, shown_again) = searches.add("TODO");
        assert!(shown_again);
    }

    #[test]
    fn test_pinned_searches_add_does_not_reuse_case_sensitive_pin() {
        // A case-sensitive pin marks fewer words than the case-insensitive
        // pin being added, so reusing it would leave some words unmarked.
        let mut searches = PinnedSearches::default();
        let mut existing = PinnedSearch::new("TODO", HighlightColor::Green);
        existing.case_sensitive = true;
        searches.pinned_searches.push(existing);

        searches.add("TODO");

        assert_eq!(searches.pinned_searches.len(), 2);
    }

    #[test]
    fn test_pinned_searches_toggle_disabled() {
        let mut searches = PinnedSearches::default();
        let search = searches.add("TODO").0;
        let id = search.id.clone();

        assert!(!searches.pinned_searches[0].disabled);

        assert!(searches.toggle_disabled(&id));
        assert!(searches.pinned_searches[0].disabled);

        assert!(searches.toggle_disabled(&id));
        assert!(!searches.pinned_searches[0].disabled);
    }

    #[test]
    fn test_pinned_searches_set_color() {
        let mut searches = PinnedSearches::default();
        let search = searches.add("TODO").0;
        let id = search.id.clone();

        assert!(searches.set_color(&id, HighlightColor::Pink));
        assert_eq!(searches.pinned_searches[0].color, HighlightColor::Pink);
    }

    #[test]
    fn test_pinned_searches_serialization() {
        let mut searches = PinnedSearches::default();
        searches.add("TODO");
        searches.pinned_searches[0].disabled = true;

        let json = serde_json::to_string_pretty(&searches).unwrap();
        let parsed: PinnedSearches = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.pinned_searches.len(), 1);
        assert_eq!(parsed.pinned_searches[0].pattern, "TODO");
        assert!(parsed.pinned_searches[0].disabled);
    }
}
