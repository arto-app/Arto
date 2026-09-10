use super::super::form_controls::{OptionCardItem, OptionCards, ToggleRow};
use crate::config::Config;
use crate::markdown::{RawHtml, RenderOptions};
use dioxus::prelude::*;

/// What the renderer reads out of a document.
///
/// GFM — tables, task lists, strikethrough, footnotes — is not offered here.
/// Those are what a document written for GitHub contains, and a reader that
/// showed them as literal pipes and brackets would be broken rather than
/// configured. What is offered is the layer above that, where a construct's
/// syntax collides with prose somebody actually writes.
#[component]
pub fn MarkdownTab(config: Signal<Config>) -> Element {
    let markdown = config.read().markdown.clone();
    let defaults = RenderOptions::default();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Syntax" }

            ToggleRow {
                label: "Bare URLs become links".to_string(),
                description: Some("A URL written on its own, without the brackets that make a link.".to_string()),
                checked: markdown.auto_link_urls,
                on_change: move |on| config.write().markdown.auto_link_urls = on,
                shipped: Some(defaults.auto_link_urls),
            }

            ToggleRow {
                label: "Math".to_string(),
                description: Some("$…$ and $$…$$ are formulas. Off leaves the dollars to a document that writes them for something else — two shell variables on one line otherwise pair up into a formula.".to_string()),
                checked: markdown.math,
                on_change: move |on| config.write().markdown.math = on,
                shipped: Some(defaults.math),
            }

            ToggleRow {
                label: "Wiki links".to_string(),
                description: Some("[[Page]] and [[Page|label]] open another document, the way Obsidian writes them.".to_string()),
                checked: markdown.wiki_links,
                on_change: move |on| config.write().markdown.wiki_links = on,
                shipped: Some(defaults.wiki_links),
            }

            ToggleRow {
                label: "Superscript".to_string(),
                description: Some("^text^ is raised.".to_string()),
                checked: markdown.superscript,
                on_change: move |on| config.write().markdown.superscript = on,
                shipped: Some(defaults.superscript),
            }

            ToggleRow {
                label: "Subscript".to_string(),
                description: Some("~text~ is lowered. Off keeps a lone tilde literal, which is what a document writing ~5 minutes means by it.".to_string()),
                checked: markdown.subscript,
                on_change: move |on| config.write().markdown.subscript = on,
                shipped: Some(defaults.subscript),
            }

            ToggleRow {
                label: "Definition lists".to_string(),
                description: Some("A term followed by a line starting with a colon becomes a definition list.".to_string()),
                checked: markdown.definition_lists,
                on_change: move |on| config.write().markdown.definition_lists = on,
                shipped: Some(defaults.definition_lists),
            }

            ToggleRow {
                label: "Heading attributes".to_string(),
                description: Some("A trailing {#id .class} on a heading names it, instead of showing up in its text.".to_string()),
                checked: markdown.heading_attributes,
                on_change: move |on| config.write().markdown.heading_attributes = on,
                shipped: Some(defaults.heading_attributes),
            }

            h3 { class: "preference-section-title", "Typography" }

            ToggleRow {
                label: "Smart punctuation".to_string(),
                description: Some("Straight quotes curl, and -- and ... become a dash and an ellipsis.".to_string()),
                checked: markdown.smart_punctuation,
                on_change: move |on| config.write().markdown.smart_punctuation = on,
                shipped: Some(defaults.smart_punctuation),
            }

            ToggleRow {
                label: "Emphasis against Japanese punctuation".to_string(),
                description: Some("**強調。** is emphasized. CommonMark reads the punctuation beside a delimiter to decide whether it may pair, and Japanese sets punctuation right against the words — so off is what GitHub shows, and on is what the document meant.".to_string()),
                checked: markdown.cjk_emphasis,
                on_change: move |on| config.write().markdown.cjk_emphasis = on,
                shipped: Some(defaults.cjk_emphasis),
            }

            h3 { class: "preference-section-title", "Headings" }

            ToggleRow {
                label: "Permalink beside each heading".to_string(),
                description: Some("A # that links to the heading, the way GitHub shows one.".to_string()),
                checked: markdown.heading_permalinks,
                on_change: move |on| config.write().markdown.heading_permalinks = on,
                shipped: Some(defaults.heading_permalinks),
            }

            h3 { class: "preference-section-title", "Raw HTML" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "HTML written into the Markdown" }
                    p {
                        class: "preference-description",
                        "A document may embed markup the Markdown syntax cannot say — a <kbd>, a <details>, an <img> with a width — and passing it through is what makes that work. The same passthrough lets a <style> or a <script> restyle the page around the document."
                    }
                }
                OptionCards {
                    name: "markdown-raw-html".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Allow,
                            title: "Allow".to_string(),
                            description: Some("Every tag is written through".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Filter,
                            title: "Filter".to_string(),
                            description: Some("Markup works; the tags that reach past the document do not".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Escape,
                            title: "Escape".to_string(),
                            description: Some("The document shows its own markup".to_string()),
                        },
                    ],
                    selected: markdown.raw_html,
                    on_change: move |raw_html| {
                        config.write().markdown.raw_html = raw_html;
                    },
                    shipped: Some(defaults.raw_html),
                }
            }
        }
    }
}
