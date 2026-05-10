//! Stable per-session tab identifier.
//!
//! Used as a join key for state attached to a specific tab (AI overlays,
//! chat sessions, …). Generating IDs from a process-wide atomic counter
//! means a tab keeps the same id across reorders, drag-and-drop, and
//! cross-window transfers — index-based keys would silently re-target
//! state to the wrong tab when surrounding tabs close or move.
//!
//! Not persisted: state.json never sees these. Two distinct sessions can
//! mint identical numeric values without conflict.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u64);

static NEXT: AtomicU64 = AtomicU64::new(1);

impl TabId {
    /// Allocate a fresh, never-before-seen [`TabId`].
    pub fn fresh() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TabId {
    /// Every default-constructed [`Tab`] gets a unique id, so AI state
    /// keyed by [`TabId`] never collides between tabs even when the rest
    /// of their fields happen to match.
    fn default() -> Self {
        Self::fresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_yields_unique_ids() {
        let a = TabId::fresh();
        let b = TabId::fresh();
        assert_ne!(a, b);
    }

    #[test]
    fn default_yields_unique_ids() {
        let a = TabId::default();
        let b = TabId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn cloned_id_compares_equal_to_original() {
        let a = TabId::fresh();
        let b = a;
        assert_eq!(a, b);
    }
}
