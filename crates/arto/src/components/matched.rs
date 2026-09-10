//! Text with the characters a query found in it marked.
//!
//! A fuzzy match can land anywhere in a name — the query `gd` finds
//! `guide.md` — so a list that only said "this matched" would leave the
//! reader to work out why. Marking the characters answers that, and it is
//! also what makes the difference between two similar rows visible without
//! reading either of them.

use crate::fuzzy::{Query, Span};
use dioxus::prelude::*;

/// `text`, with what `query` found in it marked.
///
/// An empty query marks nothing, so a list that is not typed into can pass
/// this through without a second component.
#[component]
pub fn Matched(text: String, #[props(default)] query: String) -> Element {
    rsx! {
        Spans { spans: Query::new(&query).highlight(&text) }
    }
}

/// Runs of text, the matched ones marked.
///
/// Apart from [`Matched`] because a name that is drawn in two pieces — the
/// folder a step quieter than the file — is matched as one string and then
/// cut, and each piece arrives here already spanned.
#[component]
pub fn Spans(spans: Vec<Span>) -> Element {
    rsx! {
        for (index, span) in spans.into_iter().enumerate() {
            if span.matched {
                span { key: "{index}", class: "fuzzy-match", "{span.text}" }
            } else {
                span { key: "{index}", "{span.text}" }
            }
        }
    }
}
