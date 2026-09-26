//! The Links face of the sidebar panel: the documents that link to the one
//! on screen.
//!
//! A face rather than a section of the document or a list in the gutter,
//! because it answers a question asked while reading — "what else talks
//! about this?" — and the panel is where a list sits beside the document
//! without taking its place. The search itself is [`crate::backlinks`]'s;
//! this is only what it found, drawn as the panel draws every other list.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::backlinks::{ResolvedLink, Source, BACKLINKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::context_menu::{
    open_row_context_menu, SidebarItemKind, SidebarRowRole,
};
use crate::components::sidebar::row_actions::RowActions;
use crate::files::FILES_CHANGED;
use crate::state::{AppState, FocusedPanel};
use tokio::sync::broadcast::error::RecvError;

/// Ask for the document on screen to be searched for, if it has not been
/// lately.
fn ensure(state: &AppState) {
    if let Some((root, target)) = state.backlinks_scope() {
        crate::backlinks::ensure(&root, &target);
    }
}

#[component]
pub fn LinksFace() -> Element {
    let state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);

    // Read inside the effect, so a new document on screen is a new search.
    use_effect(move || ensure(&state));

    use_future(move || async move {
        let mut listed = FILES_CHANGED.subscribe();
        let mut searched = BACKLINKS_CHANGED.subscribe();
        loop {
            let result = tokio::select! {
                result = listed.recv() => result,
                result = searched.recv() => result,
            };
            if matches!(result, Err(RecvError::Closed)) {
                break;
            }
            // A listing that has just arrived is what a search was waiting
            // for, and a search that finished may have been thrown away by a
            // reload; asking again covers both, and is nothing when the
            // search on hand is fresh.
            ensure(&state);
            *revision.write() += 1;
        }
    });

    // Read so a finished search redraws the list; the number says nothing.
    let _ = revision();
    let scope = state.backlinks_scope();
    let found = scope
        .as_ref()
        .and_then(|(root, target)| crate::backlinks::lookup(root, target));
    let cursor = if *state.focused_panel.read() == FocusedPanel::Panel {
        state.panel_cursor.read().clone()
    } else {
        None
    };
    let empty = match (&scope, &found) {
        (None, _) if state.current_file().is_none() => {
            Some("Open a document to see what links to it")
        }
        (None, _) => Some("Open a folder to see what links here"),
        (Some(_), None) => Some("Scanning\u{2026}"),
        (Some(_), Some(found)) if found.sources.is_empty() => Some("No backlinks"),
        _ => None,
    };

    rsx! {
        div {
            class: "left-sidebar-face",

            div {
                class: "left-sidebar-face-list",

                div {
                    class: "left-sidebar-root-group-label",
                    span { "Linked from" }
                }

                if let Some(message) = empty {
                    div { class: "left-sidebar-explorer-empty", "{message}" }
                }

                if let Some(found) = found.as_ref() {
                    for source in found.sources.iter() {
                        SourceRow {
                            key: "{source.path.display()}",
                            source: source.clone(),
                            is_keyboard_focused: cursor
                                .as_ref()
                                .is_some_and(|(_, at)| *at == source.path),
                        }
                    }
                    if found.partial {
                        div {
                            class: "left-sidebar-explorer-empty",
                            "Only part of this folder was searched"
                        }
                    }
                }
            }
        }
    }
}

/// One document that links here, and under it each line that does.
#[component]
fn SourceRow(source: Source, is_keyboard_focused: bool) -> Element {
    let mut state = use_context::<AppState>();
    let path = source.path.clone();
    let first_line = source.first_line();

    rsx! {
        div {
            class: "left-sidebar-tree-node-content left-sidebar-links-row",
            class: if is_keyboard_focused { "keyboard-focused" },
            onclick: {
                let path = path.clone();
                move |_| match first_line {
                    Some(line) => state.open_from_panel_at(&path, line),
                    None => state.open_from_panel(&path),
                }
            },
            oncontextmenu: {
                let path = path.clone();
                move |evt: Event<MouseData>| {
                    open_row_context_menu(
                        state,
                        &path,
                        SidebarItemKind::File,
                        SidebarRowRole::Entry,
                        &evt,
                    );
                }
            },
            Icon { name: IconName::File, size: 16, class: "left-sidebar-tree-icon" }
            span {
                class: "left-sidebar-tree-label",
                DocumentName { path: path.clone() }
            }
            RowActions { path: path.clone() }
        }
        for link in source.links.iter() {
            LinkExcerpt {
                key: "{link.line}",
                path: path.clone(),
                link: link.clone(),
            }
        }
    }
}

/// The line a link was written on, with the link picked out; opens the
/// source there.
#[component]
fn LinkExcerpt(path: PathBuf, link: ResolvedLink) -> Element {
    let mut state = use_context::<AppState>();
    let (before, marked, after) = link.excerpt.parts();
    let line = link.line;

    rsx! {
        div {
            class: "left-sidebar-links-excerpt",
            title: "Line {line}",
            onclick: move |_| state.open_from_panel_at(&path, line),
            "{before}"
            mark { "{marked}" }
            "{after}"
        }
    }
}
