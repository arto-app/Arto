use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        /// Block source line start (1-based, from data-source-line)
        #[serde(default)]
        source_line: Option<u32>,
        /// Block source line end (1-based, from data-source-line-end)
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
}

#[derive(Props, Clone, PartialEq)]
pub struct ContentContextMenuProps {
    pub position: (i32, i32),
    pub context: ContentContext,
    pub has_selection: bool,
    pub selected_text: String,
    pub current_file: Option<PathBuf>,
    pub base_dir: PathBuf,
    pub source_line: Option<u32>,
    pub source_line_end: Option<u32>,
    pub table_csv: Option<String>,
    pub table_tsv: Option<String>,
    pub table_markdown: Option<String>,
    pub table_source_line: Option<u32>,
    pub table_source_line_end: Option<u32>,
    pub on_close: EventHandler<()>,
}
