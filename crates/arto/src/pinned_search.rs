//! Pinned Search feature for persistent keyword highlighting.
//!
//! This module provides:
//! - `PinnedSearch`: A pinned search query with color
//! - `PinnedSearches`: Collection of pinned searches with persistence
//! - `PINNED_SEARCHES`: Global static for app-wide access
//! - `PINNED_SEARCHES_CHANGED`: Broadcast channel for cross-window sync

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::broadcast;
use uuid::Uuid;

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

/// Highlight color for pinned searches.
///
/// Note: Yellow is excluded because it's reserved for the active search
/// (which has navigation support). This visual distinction helps users
/// differentiate between navigable search results and persistent pinned highlights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightColor {
    #[default]
    Green,
    Blue,
    Pink,
    Orange,
    Purple,
}

impl HighlightColor {
    /// All available colors for pinned searches (Yellow excluded).
    pub const ALL: [HighlightColor; 5] = [
        HighlightColor::Green,
        HighlightColor::Blue,
        HighlightColor::Pink,
        HighlightColor::Orange,
        HighlightColor::Purple,
    ];

    /// Return the next color in the fixed rotation order.
    pub fn next(self) -> HighlightColor {
        match self {
            HighlightColor::Green => HighlightColor::Blue,
            HighlightColor::Blue => HighlightColor::Pink,
            HighlightColor::Pink => HighlightColor::Orange,
            HighlightColor::Orange => HighlightColor::Purple,
            HighlightColor::Purple => HighlightColor::Green,
        }
    }

    /// Get CSS class name for this color.
    pub fn css_class(&self) -> &'static str {
        match self {
            HighlightColor::Green => "highlight-green",
            HighlightColor::Blue => "highlight-blue",
            HighlightColor::Pink => "highlight-pink",
            HighlightColor::Orange => "highlight-orange",
            HighlightColor::Purple => "highlight-purple",
        }
    }

    /// Get the color name for JavaScript.
    pub fn to_js_name(self) -> &'static str {
        match self {
            HighlightColor::Green => "green",
            HighlightColor::Blue => "blue",
            HighlightColor::Pink => "pink",
            HighlightColor::Orange => "orange",
            HighlightColor::Purple => "purple",
        }
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
    /// Get the pinned searches file path.
    fn path() -> PathBuf {
        const FILENAME: &str = "pinned-searches.json";
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("arto");
            path.push(FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(FILENAME);
            return path;
        }

        PathBuf::from(FILENAME)
    }

    /// Load pinned searches from file or return empty.
    pub fn load() -> Self {
        let path = Self::path();

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save pinned searches to file.
    pub fn save(&self) {
        let path = Self::path();

        tracing::debug!(path = %path.display(), count = self.pinned_searches.len(), "Saving pinned searches");

        // If no pinned searches, remove the file
        if self.pinned_searches.is_empty() {
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    tracing::error!(?e, "Failed to remove empty pinned searches file");
                }
            }
            return;
        }

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::error!(?e, "Failed to create pinned searches directory");
                return;
            }
        }

        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    tracing::error!(?e, "Failed to save pinned searches");
                }
            }
            Err(e) => {
                tracing::error!(?e, "Failed to serialize pinned searches");
            }
        }
    }

    /// Add a new pinned search.
    pub fn add(&mut self, pattern: impl Into<String>) -> &PinnedSearch {
        let color = self.next_color();
        let pinned = PinnedSearch::new(pattern, color);
        self.pinned_searches.push(pinned);
        self.pinned_searches.last().unwrap()
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

/// Global pinned searches instance.
pub static PINNED_SEARCHES: LazyLock<RwLock<PinnedSearches>> =
    LazyLock::new(|| RwLock::new(PinnedSearches::load()));

/// Broadcast channel for pinned search changes.
///
/// All windows subscribe to this to update their UI when pinned searches change.
/// The payload is empty since subscribers should read from PINNED_SEARCHES directly.
pub static PINNED_SEARCHES_CHANGED: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(10).0);

/// Add a pinned search and broadcast the change.
///
/// Returns the ID of the newly created pinned search.
pub fn add_pinned_search(pattern: impl Into<String>) -> PinnedSearchId {
    let id = {
        let mut pinned = PINNED_SEARCHES.write();
        let search = pinned.add(pattern);
        let id = search.id.clone();
        pinned.save();
        id
    };
    PINNED_SEARCHES_CHANGED.send(()).ok();
    id
}

/// Remove a pinned search and broadcast the change.
///
/// Returns `true` if the pinned search was found and removed.
pub fn remove_pinned_search(id: &PinnedSearchId) -> bool {
    let result = {
        let mut pinned = PINNED_SEARCHES.write();
        let result = pinned.remove(id);
        if result {
            pinned.save();
        }
        result
    };
    if result {
        PINNED_SEARCHES_CHANGED.send(()).ok();
    }
    result
}

/// Update the color of a pinned search and broadcast the change.
pub fn set_pinned_search_color(id: &PinnedSearchId, color: HighlightColor) -> bool {
    let result = {
        let mut pinned = PINNED_SEARCHES.write();
        let result = pinned.set_color(id, color);
        if result {
            pinned.save();
        }
        result
    };
    if result {
        PINNED_SEARCHES_CHANGED.send(()).ok();
    }
    result
}

/// Toggle the disabled state of a pinned search and broadcast the change.
pub fn toggle_pinned_search_disabled(id: &PinnedSearchId) -> bool {
    let result = {
        let mut pinned = PINNED_SEARCHES.write();
        let result = pinned.toggle_disabled(id);
        if result {
            pinned.save();
        }
        result
    };
    if result {
        PINNED_SEARCHES_CHANGED.send(()).ok();
    }
    result
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
    fn test_highlight_color_css_class() {
        assert_eq!(HighlightColor::Green.css_class(), "highlight-green");
        assert_eq!(HighlightColor::Blue.css_class(), "highlight-blue");
        assert_eq!(HighlightColor::Pink.css_class(), "highlight-pink");
        assert_eq!(HighlightColor::Orange.css_class(), "highlight-orange");
        assert_eq!(HighlightColor::Purple.css_class(), "highlight-purple");
    }

    #[test]
    fn test_highlight_color_next_rotation() {
        assert_eq!(HighlightColor::Green.next(), HighlightColor::Blue);
        assert_eq!(HighlightColor::Blue.next(), HighlightColor::Pink);
        assert_eq!(HighlightColor::Pink.next(), HighlightColor::Orange);
        assert_eq!(HighlightColor::Orange.next(), HighlightColor::Purple);
        assert_eq!(HighlightColor::Purple.next(), HighlightColor::Green);
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

        let search = searches.add("TODO");
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

        let new_pin = searches.add("New");
        assert_eq!(new_pin.color, HighlightColor::Pink);
    }

    #[test]
    fn test_pinned_searches_toggle_disabled() {
        let mut searches = PinnedSearches::default();
        let search = searches.add("TODO");
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
        let search = searches.add("TODO");
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
