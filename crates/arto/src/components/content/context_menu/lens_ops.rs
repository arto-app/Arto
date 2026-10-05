use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::components::icon::IconName;
use crate::lenses::{self, LensRun, Scope};
use crate::state::AppState;
use arto_config::Lens;

/// The configured lenses, over the block the menu was opened on or the whole
/// document.
///
/// A lens not yet open is an item that opens it. One that is open is a
/// submenu of what can be done with it — show or hide it, stop it,
/// regenerate what changed, forget its answers — so the menu holds one row
/// per lens however many are open. Each submenu holds the lenses that are
/// on what it looks at — a page lens is always on a whole document — and a
/// submenu with nothing in it is not drawn.
#[component]
pub(super) fn LensItems(on_close: EventHandler<()>) -> Element {
    let offered = lenses::offered_lenses();
    let submenus = [
        (
            t!("context_menu.lenses.on_block").to_string(),
            Scope::Cursor,
            offered
                .iter()
                .filter(|lens| lens.on_block())
                .cloned()
                .collect::<Vec<_>>(),
        ),
        (
            t!("context_menu.lenses.on_document").to_string(),
            Scope::Document,
            offered
                .iter()
                .filter(|lens| lens.on_document())
                .cloned()
                .collect::<Vec<_>>(),
        ),
    ];

    rsx! {
        for (label, scope, lenses) in submenus {
            if !lenses.is_empty() {
                ContextMenuSubmenu {
                    key: "{label}",
                    label: label,
                    icon: Some(IconName::Aperture),
                    for lens in lenses {
                        LensItem { key: "{lens.id}", lens, scope, on_close }
                    }
                }
            }
        }
    }
}

/// One lens in a submenu: an item that opens it, or, while it is open over
/// `scope`, a submenu of what can be done with it.
#[component]
fn LensItem(lens: Lens, scope: Scope, on_close: EventHandler<()>) -> Element {
    let state = use_context::<AppState>();
    let open = lenses::open_runs(&state)
        .into_iter()
        .find(|run| run.lens_id == lens.id && run.scope == scope);
    let id = lens.id.clone();
    let start = move |_| {
        lenses::start(state, &id, scope);
        on_close.call(());
    };

    // A lens allowed to reach the files around the document says so where
    // it is opened, so that it is not opened over a document not trusted
    // with it by mistake for another.
    let icon = if lens.reaches_local() {
        IconName::AlertTriangle
    } else {
        IconName::Add
    };
    match open {
        None => rsx! {
            ContextMenuItem { label: lens.label.clone(), icon: Some(icon), on_click: start }
        },
        Some(run) => rsx! {
            ContextMenuSubmenu {
                label: lens.label.clone(),
                icon: Some(if run.applied { IconName::Aperture } else { IconName::ApertureOff }),
                OpenLensItems { run, on_close }
            }
        },
    }
}

/// What can be done with the open lens `run`.
#[component]
fn OpenLensItems(run: LensRun, on_close: EventHandler<()>) -> Element {
    let state = use_context::<AppState>();
    let token = run.token;
    // Deleting what took minutes to answer asks twice: the first click only
    // turns the item into the question.
    let mut confirming = use_signal(|| false);
    let act = move |action: fn(AppState, u64)| {
        move |_| {
            action(state, token);
            on_close.call(());
        }
    };

    rsx! {
        if run.applied {
            ContextMenuItem { label: t!("context_menu.lenses.hide").to_string(), icon: Some(IconName::ApertureOff), on_click: act(lenses::hide_run) }
        } else {
            ContextMenuItem { label: t!("context_menu.lenses.show").to_string(), icon: Some(IconName::Aperture), on_click: act(lenses::show) }
        }
        if run.is_running() {
            ContextMenuItem { label: t!("context_menu.lenses.stop").to_string(), icon: Some(IconName::Close), on_click: act(lenses::stop_run) }
        } else if run.unanswered > 0 {
            ContextMenuItem {
                label: t!("context_menu.lenses.continue", count = run.stale()).to_string(),
                icon: Some(IconName::Refresh),
                on_click: act(lenses::regenerate),
            }
        } else if run.outdated > 0 {
            ContextMenuItem {
                label: t!("context_menu.lenses.regenerate_changed", count = run.stale()).to_string(),
                icon: Some(IconName::Refresh),
                on_click: act(lenses::regenerate),
            }
        } else if run.failed > 0 {
            ContextMenuItem {
                label: t!("context_menu.lenses.retry_failed", count = run.failed).to_string(),
                icon: Some(IconName::Refresh),
                on_click: act(lenses::regenerate),
            }
        }
        if !run.is_running() {
            ContextMenuItem {
                label: t!("context_menu.lenses.regenerate_all").to_string(),
                icon: Some(IconName::Refresh),
                on_click: act(lenses::regenerate_all),
            }
        }
        if run.scope == Scope::Cursor {
            // Open over one block, it looks at the block the menu was
            // opened on only when asked again.
            ContextMenuItem {
                label: t!("context_menu.lenses.look_at_block").to_string(),
                icon: Some(IconName::Aperture),
                on_click: {
                    let id = run.lens_id.clone();
                    move |_| {
                        lenses::start(state, &id, Scope::Cursor);
                        on_close.call(());
                    }
                },
            }
        } else {
            ContextMenuItem {
                label: if confirming() { t!("context_menu.lenses.confirm_forget").to_string() } else { t!("context_menu.lenses.forget").to_string() },
                icon: Some(IconName::Trash),
                on_click: move |_| {
                    if confirming() {
                        lenses::forget(state, token);
                        on_close.call(());
                    } else {
                        confirming.set(true);
                    }
                },
            }
        }
    }
}
