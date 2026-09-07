//! Where the reader is in a document.
//!
//! The renderer's counterpart is `frontend/src/scroll-anchor.ts`, which
//! produces these values and puts them back.

use serde::{Deserialize, Serialize};

/// A place in a document, named by content rather than by pixels.
///
/// A pixel offset only means the same place while every block is the height
/// it was when the offset was taken, and blocks change height after the
/// document appears: diagrams and formulas are drawn when the reader comes
/// near them, and a file can be edited while it is open. Naming the block
/// instead survives both.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScrollAnchor {
    /// 1-based line of the file the block at the top of the view came from,
    /// as the rendered HTML reports it in `data-source-line`. Zero is the top
    /// of the document.
    pub line: u32,
    /// How far into that block the top edge of the view sits, from 0 at its
    /// first pixel to just under 1 at its last.
    pub fraction: f32,
}

impl ScrollAnchor {
    /// The top of the document, where a newly opened one starts.
    pub const TOP: Self = Self {
        line: 0,
        fraction: 0.0,
    };

    /// Whether this is the top, which restoring can take as a plain jump
    /// rather than having to find a block first.
    pub fn is_top(&self) -> bool {
        self.line == 0
    }
}

impl Default for ScrollAnchor {
    fn default() -> Self {
        Self::TOP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_top_of_the_document() {
        assert_eq!(ScrollAnchor::default(), ScrollAnchor::TOP);
        assert!(ScrollAnchor::default().is_top());
    }

    #[test]
    fn a_named_line_is_not_the_top() {
        assert!(!ScrollAnchor {
            line: 1,
            fraction: 0.0
        }
        .is_top());
    }

    #[test]
    fn it_survives_the_round_trip_to_the_renderer() {
        let anchor = ScrollAnchor {
            line: 420,
            fraction: 0.25,
        };
        let json = serde_json::to_string(&anchor).expect("serializes");
        assert_eq!(json, r#"{"line":420,"fraction":0.25}"#);
        assert_eq!(
            serde_json::from_str::<ScrollAnchor>(&json).expect("deserializes"),
            anchor
        );
    }
}
