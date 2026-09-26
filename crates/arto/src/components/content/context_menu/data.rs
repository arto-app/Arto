use serde::{Deserialize, Serialize};

/// Context type for right-click detection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentContext {
    /// General content (no specific element)
    General,
    /// Link element
    Link { href: String },
    /// Image element
    Image { src: String, alt: Option<String> },
    /// Code block
    CodeBlock {
        content: String,
        language: Option<String>,
        /// Block source line start (1-based, the first line of data-source-range)
        #[serde(default)]
        source_line: Option<u32>,
        /// Block source line end (1-based, the last line of data-source-range)
        #[serde(default)]
        source_line_end: Option<u32>,
    },
    /// Mermaid diagram
    Mermaid { source: String },
    /// Math block (display math or math code block)
    MathBlock { source: String },
}

/// Context menu data from JavaScript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuData {
    pub context: ContentContext,
    pub x: i32,
    pub y: i32,
    /// Whether there is selected text
    pub has_selection: bool,
    /// The selected text (captured at context menu open time)
    #[serde(default)]
    pub selected_text: String,
    /// Source line number at click/selection start position (1-based)
    #[serde(default)]
    pub source_line: Option<u32>,
    /// Source line number at selection end position (1-based, same as source_line for single line)
    #[serde(default)]
    pub source_line_end: Option<u32>,
    /// Table data as CSV (if right-clicked within a table)
    #[serde(default)]
    pub table_csv: Option<String>,
    /// Table data as TSV (if right-clicked within a table)
    #[serde(default)]
    pub table_tsv: Option<String>,
    /// Table data as Markdown (if right-clicked within a table)
    #[serde(default)]
    pub table_markdown: Option<String>,
    /// Table source line start (1-based)
    #[serde(default)]
    pub table_source_line: Option<u32>,
    /// Table source line end (1-based)
    #[serde(default)]
    pub table_source_line_end: Option<u32>,
    /// The reader's highlights the selection touches, or the one clicked on
    #[serde(default)]
    pub highlight_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_highlights_under_a_click_are_read_and_default_to_none() {
        let base = serde_json::json!({
            "context": { "type": "general" },
            "x": 1,
            "y": 2,
            "has_selection": false,
        });
        let data: ContextMenuData = serde_json::from_value(base.clone()).unwrap();
        assert!(data.highlight_ids.is_empty());

        let mut with = base;
        with["highlight_ids"] = serde_json::json!(["hl_1", "hl_2"]);
        let data: ContextMenuData = serde_json::from_value(with).unwrap();
        assert_eq!(data.highlight_ids, ["hl_1", "hl_2"]);
    }
}
