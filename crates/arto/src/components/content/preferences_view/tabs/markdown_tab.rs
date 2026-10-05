use super::super::form_controls::{OptionCardItem, OptionCards, ToggleRow};
use crate::config::Config;
use crate::markdown::{RawHtml, RenderOptions};
use dioxus::prelude::*;
use rust_i18n::t;

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

            h3 { class: "preference-section-title", {t!("preferences.markdown.syntax.title").to_string()} }

            ToggleRow {
                label: t!("preferences.markdown.syntax.auto_link_urls.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.auto_link_urls.description").to_string()),
                checked: markdown.auto_link_urls,
                on_change: move |on| config.write().markdown.auto_link_urls = on,
                shipped: Some(defaults.auto_link_urls),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.math.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.math.description").to_string()),
                checked: markdown.math,
                on_change: move |on| config.write().markdown.math = on,
                shipped: Some(defaults.math),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.wiki_links.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.wiki_links.description").to_string()),
                checked: markdown.wiki_links,
                on_change: move |on| config.write().markdown.wiki_links = on,
                shipped: Some(defaults.wiki_links),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.superscript.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.superscript.description").to_string()),
                checked: markdown.superscript,
                on_change: move |on| config.write().markdown.superscript = on,
                shipped: Some(defaults.superscript),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.subscript.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.subscript.description").to_string()),
                checked: markdown.subscript,
                on_change: move |on| config.write().markdown.subscript = on,
                shipped: Some(defaults.subscript),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.definition_lists.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.definition_lists.description").to_string()),
                checked: markdown.definition_lists,
                on_change: move |on| config.write().markdown.definition_lists = on,
                shipped: Some(defaults.definition_lists),
            }

            ToggleRow {
                label: t!("preferences.markdown.syntax.heading_attributes.label").to_string(),
                description: Some(t!("preferences.markdown.syntax.heading_attributes.description").to_string()),
                checked: markdown.heading_attributes,
                on_change: move |on| config.write().markdown.heading_attributes = on,
                shipped: Some(defaults.heading_attributes),
            }

            h3 { class: "preference-section-title", {t!("preferences.markdown.typography.title").to_string()} }

            ToggleRow {
                label: t!("preferences.markdown.typography.smart_punctuation.label").to_string(),
                description: Some(t!("preferences.markdown.typography.smart_punctuation.description").to_string()),
                checked: markdown.smart_punctuation,
                on_change: move |on| config.write().markdown.smart_punctuation = on,
                shipped: Some(defaults.smart_punctuation),
            }

            ToggleRow {
                label: t!("preferences.markdown.typography.cjk_emphasis.label").to_string(),
                description: Some(t!("preferences.markdown.typography.cjk_emphasis.description").to_string()),
                checked: markdown.cjk_emphasis,
                on_change: move |on| config.write().markdown.cjk_emphasis = on,
                shipped: Some(defaults.cjk_emphasis),
            }

            h3 { class: "preference-section-title", {t!("preferences.markdown.headings.title").to_string()} }

            ToggleRow {
                label: t!("preferences.markdown.headings.permalinks.label").to_string(),
                description: Some(t!("preferences.markdown.headings.permalinks.description").to_string()),
                checked: markdown.heading_permalinks,
                on_change: move |on| config.write().markdown.heading_permalinks = on,
                shipped: Some(defaults.heading_permalinks),
            }

            h3 { class: "preference-section-title", {t!("preferences.markdown.raw_html.title").to_string()} }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.markdown.raw_html.label").to_string()} }
                    p {
                        class: "preference-description",
                        {t!("preferences.markdown.raw_html.description").to_string()}
                    }
                }
                OptionCards {
                    name: "markdown-raw-html".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Allow,
                            title: t!("preferences.markdown.raw_html.allow.title").to_string(),
                            description: Some(t!("preferences.markdown.raw_html.allow.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Filter,
                            title: t!("preferences.markdown.raw_html.filter.title").to_string(),
                            description: Some(t!("preferences.markdown.raw_html.filter.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RawHtml::Escape,
                            title: t!("preferences.markdown.raw_html.escape.title").to_string(),
                            description: Some(t!("preferences.markdown.raw_html.escape.description").to_string()),
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
