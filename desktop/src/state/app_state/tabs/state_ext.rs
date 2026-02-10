//! AppState extension methods for tab management.
//!
//! # Testing Note
//!
//! These methods are NOT unit tested because:
//!
//! 1. AppState uses Dioxus Signals (`Signal<T>`), which require a Dioxus runtime
//! 2. Signal operations (read/write) panic outside of Dioxus components
//! 3. Creating a Dioxus runtime for unit tests adds significant complexity
//!
//! These methods are tested via:
//! - Integration tests that run within a Dioxus app context
//! - Manual testing through the UI
//! - The Tab/TabContent module tests cover the underlying data structures
//!   (see `tabs/tab.rs` and `tabs/content.rs` for unit tests)

use super::content::TabContent;
use super::open_file_options::OpenFileOptions;
use super::tab::Tab;
use crate::history::HistoryManager;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Get a tab by index (returns a clone)
    ///
    /// Used in Prepare phase of Two-Phase Commit.
    /// Note: Clone cost is low (~2-10 KB for typical tabs with history).
    pub fn get_tab(&self, index: usize) -> Option<Tab> {
        self.tabs.read().get(index).cloned()
    }

    /// Get a read-only copy of the current active tab
    pub fn current_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.read();
        let active_index = *self.active_tab.read();
        tabs.get(active_index).cloned()
    }

    /// Update the current active tab using a closure
    pub fn update_current_tab<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut Tab),
    {
        let active_index = *self.active_tab.read();
        let mut tabs = self.tabs.write();

        if let Some(tab) = tabs.get_mut(active_index) {
            update_fn(tab);
        }
    }

    /// Close a tab at index.
    /// If no tabs remain, closes the window.
    ///
    /// Returns `true` if the tab was closed successfully.
    /// Returns `false` if the index was out of bounds.
    ///
    /// Note: When the last tab is closed, this method also closes the window.
    /// The caller cannot distinguish between "tab closed" and "window closed"
    /// from the return value alone.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.take_tab(index).is_some() {
            // Close window if no tabs remain
            if self.tabs.read().is_empty() {
                dioxus::desktop::window().close();
            }
            true
        } else {
            false
        }
    }

    /// Remove a tab at index and return it.
    /// Unlike close_tab, does NOT close the window if no tabs remain.
    /// Used for drag operations where the tab may be re-inserted.
    pub fn take_tab(&mut self, index: usize) -> Option<Tab> {
        let mut tabs = self.tabs.write();

        if index >= tabs.len() {
            return None;
        }

        let tab = tabs.remove(index);

        // Update active tab index
        let current_active = *self.active_tab.read();
        let new_active = match current_active.cmp(&index) {
            std::cmp::Ordering::Greater => current_active - 1,
            std::cmp::Ordering::Equal if current_active >= tabs.len() => {
                tabs.len().saturating_sub(1)
            }
            _ => current_active,
        };

        if new_active != current_active && !tabs.is_empty() {
            drop(tabs); // Release borrow before updating
            self.active_tab.set(new_active);
        }

        Some(tab)
    }

    /// Insert tab at specified position
    /// Returns the index where the tab was inserted
    pub fn insert_tab(&mut self, tab: Tab, index: usize) -> usize {
        let mut tabs = self.tabs.write();
        let insert_index = index.min(tabs.len()); // Clamp to valid range
        tabs.insert(insert_index, tab);
        insert_index
    }

    /// Add a tab and optionally switch to it
    pub fn add_tab(&mut self, tab: Tab, switch_to: bool) -> usize {
        let tabs_len = self.tabs.read().len();
        let index = self.insert_tab(tab, tabs_len);
        if switch_to {
            self.switch_to_tab(index);
        }
        index
    }

    /// Add a file tab and optionally switch to it
    pub fn add_file_tab(&mut self, file: impl Into<PathBuf>, switch_to: bool) -> usize {
        self.add_tab(Tab::new(file.into()), switch_to)
    }

    /// Add an empty tab and optionally switch to it
    pub fn add_empty_tab(&mut self, switch_to: bool) -> usize {
        self.add_tab(Tab::default(), switch_to)
    }

    /// Switch to a specific tab by index (internal, no scroll restoration).
    ///
    /// This is the core tab-switching logic without scroll position handling.
    /// Used by both `switch_to_tab()` and `open_file()`.
    fn switch_to_tab_no_restore(&mut self, index: usize) {
        let tabs = self.tabs.read();
        if index >= tabs.len() {
            return;
        }
        let current_index = *self.active_tab.read();
        drop(tabs);

        if index == current_index {
            return;
        }

        // Save current scroll position to the departing tab's history
        let scroll = *self.current_scroll_position.read();
        self.update_current_tab(|tab| {
            tab.history.save_scroll_position(scroll);
        });

        self.active_tab.set(index);
    }

    /// Switch to a specific tab by index.
    ///
    /// Preserves scroll position: saves the current tab's scroll position to its
    /// history entry, and sets up the target tab's scroll position for restoration.
    /// This ensures that switching between tabs doesn't reset scroll to the top.
    pub fn switch_to_tab(&mut self, index: usize) {
        let tabs = self.tabs.read();
        if index >= tabs.len() {
            return;
        }

        // Extract target tab's historical scroll position
        let target_scroll = if !tabs[index].is_no_file() {
            tabs[index]
                .history
                .current()
                .map(|entry| crate::state::PendingScroll::Position(entry.scroll_position))
        } else {
            None
        };
        drop(tabs);

        // Set pending scroll to historical position
        if let Some(scroll) = target_scroll {
            self.pending_scroll.set(Some(scroll));
        }

        self.switch_to_tab_no_restore(index);
    }

    /// Check if the current active tab has no file (NoFile tab, Inline content, or FileError)
    /// None, Inline content, and FileError can be replaced when opening a file
    pub fn is_current_tab_no_file(&self) -> bool {
        self.current_tab()
            .map(|tab| tab.is_no_file())
            .unwrap_or(false)
    }

    /// Find the index of a tab that has the specified file open
    pub fn find_tab_with_file(&self, file: impl AsRef<Path>) -> Option<usize> {
        let file = file.as_ref();
        let tabs = self.tabs.read();
        tabs.iter()
            .position(|tab| tab.file().map(|f| f == file).unwrap_or(false))
    }

    /// Open a file with options.
    ///
    /// By default, reuses NoFile tab or existing tab with the same file if possible.
    /// Options can control navigation behavior and scroll position.
    pub fn open_file(&mut self, file: impl AsRef<Path>, options: OpenFileOptions) {
        let file = file.as_ref();

        // Set pending scroll from options (unified setting point)
        if let Some(line) = options.line {
            self.pending_scroll
                .set(Some(crate::state::PendingScroll::Line(line)));
        }

        if options.in_current_tab {
            // Force navigation in current tab (for markdown links)
            self.update_current_tab(|tab| {
                tab.navigate_to(file);
            });
        } else {
            // Standard tab reuse logic
            if let Some(tab_index) = self.find_tab_with_file(file) {
                // Switch to the existing tab (pending_scroll already set, don't restore history)
                self.switch_to_tab_no_restore(tab_index);
            } else if self.is_current_tab_no_file() {
                // If current tab is NoFile, open the file in it
                self.update_current_tab(|tab| {
                    tab.navigate_to(file);
                });
            } else {
                // Otherwise, create a new tab
                self.add_file_tab(file, true);
            }
        }
    }

    /// Navigate to a file in the current tab (for in-tab navigation like markdown links)
    /// Always opens in current tab regardless of whether file is open elsewhere
    pub fn navigate_to_file(&mut self, file: impl Into<PathBuf>) {
        self.update_current_tab(|tab| {
            tab.navigate_to(file);
        });
    }

    /// Open preferences in a tab. Reuses existing preferences tab if found.
    pub fn open_preferences(&mut self) {
        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        if let Some(index) = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences))
        {
            drop(tabs);
            self.switch_to_tab(index);
            return;
        }
        drop(tabs);

        // Check if current tab is empty (None, Inline, or FileError) - reuse it
        if self.is_current_tab_no_file() {
            self.update_current_tab(|tab| {
                tab.content = TabContent::Preferences;
            });
        } else {
            // Create new tab with preferences
            let mut tabs = self.tabs.write();
            tabs.push(Tab {
                content: TabContent::Preferences,
                history: HistoryManager::new(),
                pinned: false,
            });
            let new_index = tabs.len() - 1;
            drop(tabs);
            self.active_tab.set(new_index);
        }
    }

    /// Toggle preferences tab. Opens if not present, closes if currently active.
    pub fn toggle_preferences(&mut self) {
        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        let preferences_index = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences));
        drop(tabs);

        if let Some(index) = preferences_index {
            // Preferences tab exists - check if it's the active tab
            let active_index = *self.active_tab.read();
            if active_index == index {
                // Close the preferences tab
                self.close_tab(index);
            } else {
                // Switch to the preferences tab
                self.switch_to_tab(index);
            }
        } else {
            // No preferences tab - open new one
            self.open_preferences();
        }
    }

    /// Save current scroll position and go back in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for back navigation from UI (menu/header buttons).
    /// It saves the current scroll position before navigating so forward can restore it.
    pub fn save_scroll_and_go_back(&mut self) -> bool {
        let position = *self.current_scroll_position.read();
        self.save_current_scroll_position(position);
        self.go_back_in_history()
    }

    /// Save current scroll position and go forward in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for forward navigation from UI (menu/header buttons).
    pub fn save_scroll_and_go_forward(&mut self) -> bool {
        let position = *self.current_scroll_position.read();
        self.save_current_scroll_position(position);
        self.go_forward_in_history()
    }

    /// Go back in history for the current tab.
    /// Returns true if navigation occurred.
    ///
    /// Sets pending_scroll_position to restore scroll position after navigation.
    /// Note: Prefer `save_scroll_and_go_back` for UI-triggered navigation.
    pub fn go_back_in_history(&mut self) -> bool {
        let active_index = *self.active_tab.read();

        // First, get the target entry info without modifying content
        let target = {
            let mut tabs = self.tabs.write();
            tabs.get_mut(active_index).and_then(|tab| {
                tab.history
                    .go_back()
                    .map(|entry| (entry.path.clone(), entry.scroll_position))
            })
        };

        if let Some((path, scroll)) = target {
            tracing::debug!(
                ?path,
                scroll,
                "go_back_in_history: restoring scroll position"
            );
            // Set pending scroll BEFORE changing content
            // This ensures FileViewer sees the scroll position when it loads
            self.pending_scroll
                .set(Some(crate::state::PendingScroll::Position(scroll)));

            // Now change content, which triggers re-render
            let mut tabs = self.tabs.write();
            if let Some(tab) = tabs.get_mut(active_index) {
                tab.content = TabContent::File(path);
            }
            return true;
        }
        false
    }

    /// Go forward in history for the current tab.
    /// Returns true if navigation occurred.
    ///
    /// Sets pending_scroll_position to restore scroll position after navigation.
    /// Note: Prefer `save_scroll_and_go_forward` for UI-triggered navigation.
    pub fn go_forward_in_history(&mut self) -> bool {
        let active_index = *self.active_tab.read();

        // First, get the target entry info without modifying content
        let target = {
            let mut tabs = self.tabs.write();
            tabs.get_mut(active_index).and_then(|tab| {
                tab.history
                    .go_forward()
                    .map(|entry| (entry.path.clone(), entry.scroll_position))
            })
        };

        if let Some((path, scroll)) = target {
            tracing::debug!(
                ?path,
                scroll,
                "go_forward_in_history: restoring scroll position"
            );
            // Set pending scroll BEFORE changing content
            // This ensures FileViewer sees the scroll position when it loads
            self.pending_scroll
                .set(Some(crate::state::PendingScroll::Position(scroll)));

            // Now change content, which triggers re-render
            let mut tabs = self.tabs.write();
            if let Some(tab) = tabs.get_mut(active_index) {
                tab.content = TabContent::File(path);
            }
            return true;
        }
        false
    }

    /// Save the current scroll position to the current history entry.
    ///
    /// Call this before navigating away to preserve scroll position for back/forward.
    pub fn save_current_scroll_position(&mut self, scroll: f64) {
        self.update_current_tab(|tab| {
            let current_path = tab.history.current_path().map(|p| p.to_path_buf());
            tracing::debug!(
                ?current_path,
                scroll,
                "Saving scroll position to history entry"
            );
            tab.history.save_scroll_position(scroll);
        });
    }

    /// Close all tabs except the one at `keep_index`.
    /// Pinned tabs are always preserved (not closed).
    /// Switches active tab to the kept tab after closing.
    pub fn close_others(&mut self, keep_index: usize) {
        let mut tabs = self.tabs.write();
        if keep_index >= tabs.len() {
            return;
        }

        // Collect tabs to retain: pinned tabs + the kept tab
        let mut new_tabs = Vec::new();
        let mut new_active = 0;
        for (i, tab) in tabs.drain(..).enumerate() {
            if tab.pinned || i == keep_index {
                if i == keep_index {
                    new_active = new_tabs.len();
                }
                new_tabs.push(tab);
            }
        }
        *tabs = new_tabs;
        drop(tabs);
        self.active_tab.set(new_active);
    }

    /// Close all unpinned tabs.
    /// If no tabs remain after closing, closes the window.
    /// Preserves the current active tab if it's pinned.
    pub fn close_all_unpinned(&mut self) {
        // Capture current active tab before modification
        let current_active = *self.active_tab.read();

        let mut tabs = self.tabs.write();

        // Calculate new index for the current active tab if it's pinned
        let mut preserved_active_index: Option<usize> = None;
        if current_active < tabs.len() && tabs[current_active].pinned {
            let mut pinned_before_or_at_active = 0usize;
            for (i, tab) in tabs.iter().enumerate() {
                if tab.pinned {
                    if i == current_active {
                        preserved_active_index = Some(pinned_before_or_at_active);
                        break;
                    }
                    pinned_before_or_at_active += 1;
                }
            }
        }

        tabs.retain(|tab| tab.pinned);

        if tabs.is_empty() {
            drop(tabs);
            dioxus::desktop::window().close();
        } else {
            // Preserve the current active tab if it's pinned; otherwise, fall back to last pinned tab
            let new_active = preserved_active_index.unwrap_or_else(|| tabs.len().saturating_sub(1));
            drop(tabs);
            self.active_tab.set(new_active);
        }
    }

    /// Toggle pin state of the tab at `index`.
    /// When pinning, the tab moves to the end of the pinned group (left side).
    /// When unpinning, the tab moves to the start of the unpinned group.
    /// Only updates active_tab if the toggled tab was currently active.
    pub fn toggle_pin(&mut self, index: usize) {
        // Capture current active tab before modification
        let current_active = *self.active_tab.read();

        let mut tabs = self.tabs.write();
        if index >= tabs.len() {
            return;
        }

        let is_pinned = tabs[index].pinned;
        tabs[index].pinned = !is_pinned;

        // Move tab to appropriate position
        let tab = tabs.remove(index);
        let pinned_count = tabs.iter().filter(|t| t.pinned).count();

        let new_index = if tab.pinned {
            // Pinning: insert at end of pinned group
            pinned_count
        } else {
            // Unpinning: insert at start of unpinned group
            pinned_count
        };
        tabs.insert(new_index, tab);

        drop(tabs);

        // Only update active_tab if we toggled the currently active tab
        if index == current_active {
            self.active_tab.set(new_index);
        } else {
            // Adjust current active tab index based on remove/insert effects
            let adjusted_active = if index < current_active && new_index >= current_active {
                // Tab moved from before to at-or-after current → shift current left
                current_active.saturating_sub(1)
            } else if index > current_active && new_index <= current_active {
                // Tab moved from after to at-or-before current → shift current right
                current_active + 1
            } else {
                // No adjustment needed
                current_active
            };
            self.active_tab.set(adjusted_active);
        }
    }

    /// Reload the current tab.
    /// For file tabs, this re-reads the file from disk.
    /// For other tab types, this forces a re-render.
    pub fn reload_current_tab(&mut self) {
        // Get the current file path if it's a file tab
        let file_path = self
            .current_tab()
            .and_then(|tab| tab.file().map(|p| p.to_path_buf()));

        if let Some(path) = file_path {
            // Re-navigate to the same file to force reload
            self.update_current_tab(|tab| {
                tab.navigate_to(path);
            });
        } else {
            // For non-file tabs, trigger a reactive update by touching the tabs signal
            // This forces components watching the signal to re-render
            let mut tabs = self.tabs.write();
            let active_index = *self.active_tab.read();
            if let Some(tab) = tabs.get_mut(active_index) {
                // Touch the tab to trigger change detection
                let content = tab.content.clone();
                tab.content = content;
            }
        }
    }
}
