/// Information about a heading extracted from markdown
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingInfo {
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text content
    pub text: String,
    /// Generated anchor ID for linking
    pub id: String,
}
