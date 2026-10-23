/// Why a document could not be rendered.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The Markdown parser gave up on the document — on one nested deeper
    /// than its recursion is allowed to go, for one.
    ///
    /// The parser's own error is the source, boxed so that this type names
    /// no engine type: the engine can change without this enum changing.
    #[error("failed to parse Markdown")]
    Parse(#[source] Box<dyn std::error::Error + Send + Sync>),
}
