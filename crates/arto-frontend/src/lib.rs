//! Arto's frontend, for an application that places `arto-markdown`'s output
//! in a page of its own.
//!
//! The HTML `arto-markdown` writes leaves three things to script: diagrams
//! (`preprocessed-mermaid`), formulas (`preprocessed-math*`) and the colours
//! of code. This crate carries the build of Arto's frontend that draws them in
//! someone else's page, together with the library bundles `arto page` and
//! Quick Look embed, so a host's documents look and behave the way they do in
//! Arto.
//!
//! A host places the rendered fragment as
//!
//! ```html
//! <div class="markdown-viewer"><article class="markdown-body">…</article></div>
//! ```
//!
//! loads [`STYLESHEET`] and [`RUNTIME`], then the scripts
//! [`Libraries::for_fragment`] names for that fragment, and calls
//! `ArtoRenderer.render(viewer)` on the `.markdown-viewer` element. The
//! script API is described in the repository's `docs/spec/embedding.md`.
//!
//! ```
//! use arto_frontend::{Libraries, RUNTIME, STYLESHEET};
//!
//! let fragment = r#"<pre class="preprocessed-mermaid" data-original-content="graph TD;">graph TD;</pre>"#;
//! let scripts: Vec<&str> = std::iter::once(RUNTIME)
//!     .chain(Libraries::for_fragment(fragment).scripts())
//!     .collect();
//!
//! assert!(!STYLESHEET.is_empty());
//! assert_eq!(scripts.len(), 2);
//! ```

use std::collections::HashSet;
use std::sync::LazyLock;

/// The stylesheet for a document in a host's page.
///
/// Every rule starts at `.markdown-viewer`: the design tokens are declared on
/// that element and the theme is named on it, so nothing outside it is
/// restyled, whatever the host names on its own root.
pub const STYLESHEET: &str = include_str!("../assets/frontend/embed.css");

/// The runtime for a document in a host's page, published as
/// `window.ArtoRenderer`. It does nothing until it is handed an element.
pub const RUNTIME: &str = include_str!("../assets/frontend/embed.iife.js");

/// Mermaid, for a fragment that draws a diagram. Load it after [`RUNTIME`].
///
/// It is three quarters of the frontend by weight and it is a bundle of its
/// own for that reason: the IIFE format cannot code-split, so the only way to
/// leave a library out of a page is to have built it separately.
pub const MERMAID: &str = include_str!("../assets/frontend/page-mermaid.iife.js");

/// KaTeX, for a fragment that sets a formula. Load it after [`RUNTIME`].
///
/// It carries the rasterizer behind the block's copy-as-image button too,
/// which has nothing but a formula to draw.
pub const MATH: &str = include_str!("../assets/frontend/page-math.iife.js");

/// highlight.js with the languages it calls common, which is a seventh of the
/// library and what nearly every document writes its code blocks in. Load it
/// after [`RUNTIME`].
pub const HLJS_COMMON: &str = include_str!("../assets/frontend/page-hljs-common.iife.js");

/// highlight.js with every language, for the fragments that name one the
/// common build does not have. Load it after [`RUNTIME`].
pub const HLJS_ALL: &str = include_str!("../assets/frontend/page-hljs-all.iife.js");

/// The names each highlight.js build answers to, aliases included, one per
/// line and written by the frontend build beside the bundle it describes.
///
/// Reading them rather than naming languages here is what keeps the choice
/// honest across a highlight.js release: a set hard-coded in this crate would
/// go quietly wrong the first time upstream moved a language in or out of
/// `common`, and the page would lose its colours with nothing failing.
const HLJS_COMMON_MANIFEST: &str = include_str!("../assets/frontend/page-hljs-common.txt");
const HLJS_ALL_MANIFEST: &str = include_str!("../assets/frontend/page-hljs-all.txt");

static HLJS_COMMON_NAMES: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HLJS_COMMON_MANIFEST.lines().collect());
static HLJS_ALL_NAMES: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HLJS_ALL_MANIFEST.lines().collect());

/// The libraries a rendered fragment calls for.
///
/// Most documents are prose, and each library is paid for by every page that
/// carries it, so a fragment is given only the ones it has something for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Libraries {
    /// The fragment draws a diagram.
    pub mermaid: bool,
    /// The fragment sets a formula.
    pub math: bool,
    /// How much of highlight.js the fragment's code blocks need.
    pub highlighting: Highlighting,
}

/// How much of highlight.js a fragment has to be given.
///
/// Ordered, because a document is read block by block and what it needs is
/// the most any one block asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Highlighting {
    /// Nothing to colour, or nothing that highlight.js could colour anyway.
    None,
    /// Every language named is one the common build has — or a block names
    /// none, and the language has to be detected, which is what that set is
    /// upstream's answer for.
    Common,
    /// Something only the whole library can colour.
    Every,
}

impl Libraries {
    /// The libraries `fragment`, HTML as `arto-markdown` renders it, calls
    /// for.
    pub fn for_fragment(fragment: impl AsRef<str>) -> Self {
        let fragment = fragment.as_ref();
        Self {
            mermaid: draws_diagram(fragment),
            math: sets_math(fragment),
            highlighting: highlighting_for(fragment),
        }
    }

    /// The bundles to load after [`RUNTIME`], in the order they are to be
    /// loaded.
    pub fn scripts(self) -> impl Iterator<Item = &'static str> {
        let highlighter = match self.highlighting {
            Highlighting::None => None,
            Highlighting::Common => Some(HLJS_COMMON),
            Highlighting::Every => Some(HLJS_ALL),
        };
        [
            self.mermaid.then_some(MERMAID),
            self.math.then_some(MATH),
            highlighter,
        ]
        .into_iter()
        .flatten()
    }
}

/// Whether the document sets a formula to typeset.
///
/// Math is typeset in the page rather than in the HTML handed to it, so a body
/// with no `preprocessed-math*` container never reaches for KaTeX — nor for a
/// KaTeX glyph, which is what lets the stylesheet's faces go as well.
fn sets_math(body_html: &str) -> bool {
    body_html.contains("preprocessed-math")
}

/// Whether the document holds a diagram to draw.
///
/// `arto-markdown` writes the container; the fenced form the renderer also
/// accepts is looked for among the code blocks rather than anywhere in the
/// body, because a document that merely quotes `language-mermaid` in a code
/// sample would otherwise carry the largest bundle of them all for nothing.
/// The name is matched as written: the renderer selects it with
/// `code.language-mermaid`, and a class selector is case-sensitive.
fn draws_diagram(body_html: &str) -> bool {
    body_html.contains("preprocessed-mermaid")
        || code_block_languages(body_html).any(|language| language == Some("mermaid"))
}

/// Which build of highlight.js, if any, the body calls for.
fn highlighting_for(body_html: &str) -> Highlighting {
    code_block_languages(body_html)
        .map(|language| match language {
            None => Highlighting::Common,
            // `hljs.getLanguage` lowercases before it looks a name up, so a
            // fence written `JSON` colours like `json` and has to be counted
            // as `json` here — matched as written, the page would leave the
            // library out and show the block the app colours as plain text.
            Some(name) => match name.to_ascii_lowercase().as_str() {
                // Another renderer owns these two, which is why the frontend
                // takes them out of highlight.js rather than colouring them.
                "mermaid" | "math" => Highlighting::None,
                name if HLJS_COMMON_NAMES.contains(name) => Highlighting::Common,
                name if HLJS_ALL_NAMES.contains(name) => Highlighting::Every,
                // A name highlight.js has never heard of stays plain text
                // whichever build is embedded, so it asks for neither.
                _ => Highlighting::None,
            },
        })
        .max()
        .unwrap_or(Highlighting::None)
}

/// The language of every block the page would colour, `None` where the fence
/// named none and the language has to be detected.
///
/// A block is a `<code>` opening immediately inside a `<pre>`, which is what
/// the frontend highlights (`pre code`) and what the pipeline writes for a
/// fenced block. An inline code span is a `<code>` anywhere else, and a
/// diagram's or a formula's `<pre>` holds no `<code>` at all.
fn code_block_languages(body_html: &str) -> impl Iterator<Item = Option<&str>> {
    body_html.match_indices("<pre").filter_map(|(at, _)| {
        let tag_end = body_html[at..].find('>')? + 1;
        let attributes = body_html[at + tag_end..]
            .trim_start()
            .strip_prefix("<code")?;
        let end = attributes.find('>')?;
        Some(language_class(&attributes[..end]))
    })
}

/// The language a `<code>`'s attributes name, if any.
///
/// The marker is looked for as a class of its own anywhere in the list, which
/// is how the frontend reads it off `className` — a body carrying raw HTML may
/// have written the classes in any order, and taking only the first one would
/// call a labelled block unlabelled.
fn language_class(attributes: &str) -> Option<&str> {
    const CLASS: &str = "class=\"";
    const MARKER: &str = "language-";

    let at = attributes.find(CLASS)? + CLASS.len();
    let list = &attributes[at..];
    let list = &list[..list.find('"')?];
    list.split_ascii_whitespace()
        .find_map(|class| class.strip_prefix(MARKER))
        // `language-` on its own names nothing; the frontend detects instead.
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many bundles a fragment pulls in after the runtime.
    fn libraries_for(fragment: &str) -> usize {
        Libraries::for_fragment(fragment).scripts().count()
    }

    #[test]
    fn test_a_prose_fragment_calls_for_no_library() {
        assert_eq!(
            Libraries::for_fragment("<p>hi</p>"),
            Libraries {
                mermaid: false,
                math: false,
                highlighting: Highlighting::None,
            }
        );
        assert_eq!(libraries_for("<p>hi</p>"), 0);
    }

    #[test]
    fn test_a_fragment_that_draws_a_diagram_calls_for_mermaid() {
        let diagram = r#"<pre class="preprocessed-mermaid" data-original-content="graph TD;">graph TD;</pre>"#;

        assert!(Libraries::for_fragment(diagram).mermaid);
        assert_eq!(
            Libraries::for_fragment(diagram)
                .scripts()
                .collect::<Vec<_>>(),
            [MERMAID]
        );
    }

    #[test]
    fn test_a_fragment_that_sets_a_formula_calls_for_katex() {
        let math = r#"<span class="preprocessed-math-inline" data-original-content="x">x</span>"#;

        assert!(Libraries::for_fragment(math).math);
        assert_eq!(
            Libraries::for_fragment(math).scripts().collect::<Vec<_>>(),
            [MATH]
        );
    }

    #[test]
    fn test_the_scripts_come_in_one_order_whatever_the_fragment_holds() {
        let everything = Libraries {
            mermaid: true,
            math: true,
            highlighting: Highlighting::Every,
        };

        assert_eq!(
            everything.scripts().collect::<Vec<_>>(),
            [MERMAID, MATH, HLJS_ALL]
        );
    }

    #[test]
    fn test_the_stylesheet_names_nothing_outside_the_viewer() {
        // A host owns its root, and may name a `data-theme` or a token of its
        // own there; the build confines the sheet so neither is touched.
        assert!(!STYLESHEET.contains(":root"));
        assert!(STYLESHEET.contains(".markdown-viewer[data-theme=dark]"));
        assert!(STYLESHEET.contains(".markdown-viewer:not([data-theme])"));
    }

    #[test]
    fn test_the_runtime_is_published_where_the_libraries_look_for_it() {
        // Each library bundle registers itself on `window.ArtoRenderer`.
        assert!(RUNTIME.contains("ArtoRenderer"));
        assert!(MERMAID.contains("ArtoRenderer"));
    }

    #[test]
    fn test_a_document_with_no_code_block_carries_no_highlighter() {
        assert_eq!(highlighting_for("<p>hi</p>"), Highlighting::None);
        // An inline code span is not a block, and is not coloured.
        assert_eq!(
            highlighting_for("<p>the <code>--flag</code> flag</p>"),
            Highlighting::None
        );
        assert_eq!(libraries_for("<p>the <code>--flag</code> flag</p>"), 0);
    }

    #[test]
    fn test_a_usual_language_takes_the_common_build() {
        let rust = r#"<pre data-source-range="1:1-3:3"><code data-source-range="2:1-2:12" class="language-rust">fn main() {}</code></pre>"#;

        assert_eq!(highlighting_for(rust), Highlighting::Common);
        assert_eq!(libraries_for(rust), 1);
    }

    #[test]
    fn test_an_unlabelled_block_takes_the_common_build_to_detect_with() {
        let plain = r#"<pre data-source-range="1:1-3:3"><code data-source-range="2:1-2:15">anything at all</code></pre>"#;

        assert_eq!(highlighting_for(plain), Highlighting::Common);
    }

    #[test]
    fn test_a_language_outside_the_common_set_takes_the_whole_library() {
        let unusual = r#"<pre><code class="language-brainfuck">+++.</code></pre>"#;
        // The premise: this is a language highlight.js has and `common` does
        // not, which is what makes the whole library the only build that can
        // colour it.
        assert!(!HLJS_COMMON_NAMES.contains("brainfuck"));
        assert!(HLJS_ALL_NAMES.contains("brainfuck"));

        assert_eq!(highlighting_for(unusual), Highlighting::Every);
    }

    #[test]
    fn test_the_most_any_block_asks_for_is_what_the_page_carries() {
        let both = r#"<pre><code class="language-rust">fn main() {}</code></pre><pre><code class="language-brainfuck">+++.</code></pre>"#;

        assert_eq!(highlighting_for(both), Highlighting::Every);
    }

    #[test]
    fn test_an_alias_is_the_language_it_stands_for() {
        // `sh` is `bash`, and the manifest carries the aliases for that
        // reason: a fence rarely says the language's own name.
        let shell = r#"<pre><code class="language-sh">ls</code></pre>"#;

        assert_eq!(highlighting_for(shell), Highlighting::Common);
    }

    #[test]
    fn test_a_language_named_in_capitals_is_the_language_it_names() {
        // `hljs.getLanguage` lowercases, so the app colours this block; a page
        // that read the name as written would embed no library at all.
        let shouty = r#"<pre><code class="language-JSON">{"a":1}</code></pre>"#;

        assert_eq!(highlighting_for(shouty), Highlighting::Common);
        assert_eq!(libraries_for(shouty), 1);
    }

    #[test]
    fn test_the_marker_is_found_wherever_it_sits_in_the_class_list() {
        // Raw HTML in the body may write the classes in any order.
        let second = r#"<pre><code class="hljs language-brainfuck">+++.</code></pre>"#;

        assert_eq!(highlighting_for(second), Highlighting::Every);
    }

    #[test]
    fn test_a_body_that_only_quotes_the_diagram_class_carries_no_mermaid() {
        // A document showing the frontend's own contract in a code sample
        // holds the text, not a diagram, and Mermaid is the largest bundle
        // there is to embed by mistake.
        let quoting = r#"<pre><code class="language-html">&lt;code class="language-mermaid"&gt;</code></pre>"#;

        assert!(!draws_diagram(quoting));
    }

    #[test]
    fn test_a_name_highlight_js_does_not_know_asks_for_nothing() {
        let unknown = r#"<pre><code class="language-notalanguage">x</code></pre>"#;

        assert_eq!(highlighting_for(unknown), Highlighting::None);
        assert_eq!(libraries_for(unknown), 0);
    }

    #[test]
    fn test_a_diagram_block_is_drawn_rather_than_coloured() {
        // The fenced form the renderer also accepts: Mermaid draws it, and
        // highlight.js is told not to colour it, so the page carries one
        // library rather than two.
        let fenced = r#"<pre><code class="language-mermaid">graph TD;</code></pre>"#;

        assert_eq!(highlighting_for(fenced), Highlighting::None);
        assert_eq!(libraries_for(fenced), 1);
    }
}
