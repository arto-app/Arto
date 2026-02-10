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
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
}

/// Scope defining where a pinned search is active.
///
/// When a user pins a search from the Fuzzy Finder, the current context
/// (file or directory) is captured as the scope. The pinned search only
/// produces DOM highlights when the scope matches the current view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "path")]
pub enum PinnedScope {
    /// Active only when viewing this specific file.
    File(PathBuf),
    /// Active for any file under this directory.
    Directory(PathBuf),
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
    /// Scope defining where this pinned search is active.
    pub scope: PinnedScope,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl PinnedSearch {
    /// Create a new pinned search with the given pattern and scope.
    pub fn new(pattern: impl Into<String>, color: HighlightColor, scope: PinnedScope) -> Self {
        Self {
            id: PinnedSearchId::new(),
            pattern: pattern.into(),
            color,
            case_sensitive: false,
            disabled: false,
            scope,
            created_at: Utc::now(),
        }
    }

    /// Check if this pinned search is in scope for the given context.
    ///
    /// Returns `true` if the current file/directory matches the scope,
    /// regardless of the `disabled` flag (disabled controls visibility,
    /// not scope validity).
    pub fn is_in_scope(&self, current_file: Option<&Path>, current_dir: Option<&Path>) -> bool {
        match &self.scope {
            PinnedScope::File(path) => current_file.is_some_and(|f| f == path),
            PinnedScope::Directory(dir) => current_dir.is_some_and(|d| d == dir),
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

    /// Add a new pinned search with the given scope.
    pub fn add(&mut self, pattern: impl Into<String>, scope: PinnedScope) -> &PinnedSearch {
        let color = self.next_color();
        let pinned = PinnedSearch::new(pattern, color, scope);
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

    /// Get the next color to use (least used color).
    fn next_color(&self) -> HighlightColor {
        let mut usage: HashMap<HighlightColor, usize> = HashMap::new();
        for pinned in &self.pinned_searches {
            *usage.entry(pinned.color).or_insert(0) += 1;
        }

        // Find least used color
        HighlightColor::ALL
            .into_iter()
            .min_by_key(|c| usage.get(c).unwrap_or(&0))
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

/// Add a pinned search with the given scope and broadcast the change.
///
/// Returns the ID of the newly created pinned search.
pub fn add_pinned_search(pattern: impl Into<String>, scope: PinnedScope) -> PinnedSearchId {
    let id = {
        let mut pinned = PINNED_SEARCHES.write();
        let search = pinned.add(pattern, scope);
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

    /// Helper to create a file scope for tests.
    fn file_scope(path: &str) -> PinnedScope {
        PinnedScope::File(PathBuf::from(path))
    }

    /// Helper to create a directory scope for tests.
    fn dir_scope(path: &str) -> PinnedScope {
        PinnedScope::Directory(PathBuf::from(path))
    }

    #[test]
    fn test_pinned_search_creation() {
        let pinned = PinnedSearch::new("TODO", HighlightColor::Orange, file_scope("/test/file.md"));
        assert_eq!(pinned.pattern, "TODO");
        assert_eq!(pinned.color, HighlightColor::Orange);
        assert!(!pinned.case_sensitive);
        assert!(!pinned.disabled);
        assert_eq!(pinned.scope, file_scope("/test/file.md"));
    }

    #[test]
    fn test_pinned_searches_add_remove() {
        let mut searches = PinnedSearches::default();

        let search = searches.add("TODO", file_scope("/a.md"));
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
        let scope = dir_scope("/project");

        // Add 5 pinned searches - should use all 5 colors
        searches.add("A", scope.clone());
        searches.add("B", scope.clone());
        searches.add("C", scope.clone());
        searches.add("D", scope.clone());
        searches.add("E", scope.clone());

        let colors: Vec<_> = searches.pinned_searches.iter().map(|p| p.color).collect();
        assert!(colors.contains(&HighlightColor::Green));
        assert!(colors.contains(&HighlightColor::Blue));
        assert!(colors.contains(&HighlightColor::Pink));
        assert!(colors.contains(&HighlightColor::Orange));
        assert!(colors.contains(&HighlightColor::Purple));
    }

    #[test]
    fn test_pinned_searches_toggle_disabled() {
        let mut searches = PinnedSearches::default();
        let search = searches.add("TODO", file_scope("/a.md"));
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
        let search = searches.add("TODO", file_scope("/a.md"));
        let id = search.id.clone();

        assert!(searches.set_color(&id, HighlightColor::Pink));
        assert_eq!(searches.pinned_searches[0].color, HighlightColor::Pink);
    }

    #[test]
    fn test_pinned_searches_serialization() {
        let mut searches = PinnedSearches::default();
        searches.add("TODO", file_scope("/test/file.md"));
        searches.pinned_searches[0].disabled = true;

        let json = serde_json::to_string_pretty(&searches).unwrap();
        let parsed: PinnedSearches = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.pinned_searches.len(), 1);
        assert_eq!(parsed.pinned_searches[0].pattern, "TODO");
        assert!(parsed.pinned_searches[0].disabled);
        assert_eq!(parsed.pinned_searches[0].scope, file_scope("/test/file.md"));
    }

    #[test]
    fn test_pinned_scope_serialization() {
        // File scope
        let scope = file_scope("/path/to/file.md");
        let json = serde_json::to_string(&scope).unwrap();
        let parsed: PinnedScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, scope);

        // Directory scope
        let scope = dir_scope("/path/to/dir");
        let json = serde_json::to_string(&scope).unwrap();
        let parsed: PinnedScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, scope);
    }

    #[test]
    fn test_is_in_scope_file() {
        let pinned = PinnedSearch::new(
            "TODO",
            HighlightColor::Green,
            file_scope("/project/readme.md"),
        );

        // Matching file
        assert!(pinned.is_in_scope(Some(Path::new("/project/readme.md")), None));

        // Different file
        assert!(!pinned.is_in_scope(Some(Path::new("/project/other.md")), None));

        // No file
        assert!(!pinned.is_in_scope(None, None));
    }

    #[test]
    fn test_is_in_scope_directory() {
        let pinned = PinnedSearch::new("TODO", HighlightColor::Green, dir_scope("/project"));

        // Matching directory
        assert!(pinned.is_in_scope(
            Some(Path::new("/project/file.md")),
            Some(Path::new("/project"))
        ));

        // Different directory
        assert!(!pinned.is_in_scope(Some(Path::new("/other/file.md")), Some(Path::new("/other"))));

        // No directory
        assert!(!pinned.is_in_scope(Some(Path::new("/project/file.md")), None));
    }

    #[test]
    fn test_is_in_scope_disabled_still_in_scope() {
        let mut pinned = PinnedSearch::new(
            "TODO",
            HighlightColor::Green,
            file_scope("/project/readme.md"),
        );
        pinned.disabled = true;

        // disabled flag doesn't affect scope check
        assert!(pinned.is_in_scope(Some(Path::new("/project/readme.md")), None));
    }
}
