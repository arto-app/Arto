use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::lenses::{LensRun, Scope};
use crate::state::AppState;
use arto_config::LensDisplay;

/// The header's hold on the lenses a window is looking through: a few
/// glyphs, since the header is narrow and the document is what is being
/// read.
///
/// An hourglass turns over while any lens is being answered, and opens onto
/// what is and how far it has got. The lens glyph opens the menu of what
/// the page shows — which page lens, or the document as written, and which
/// lenses over the page — where a lens still being answered turns an
/// hourglass of its own; it carries a dot while one has answers from before
/// the document changed, or failed. The summaries opened over the document
/// share one more glyph, which opens them together. Nothing is drawn while
/// no lens is open.
#[component]
pub fn LensControls() -> Element {
    let state = use_context::<AppState>();
    let runs = crate::lenses::open_runs(&state);
    let working: Vec<LensRun> = runs
        .iter()
        .filter(|run| run.is_running())
        .cloned()
        .collect();
    let summaries: Vec<LensRun> = runs
        .iter()
        .filter(|run| {
            run.applied && run.display == LensDisplay::Popover && run.scope == Scope::Document
        })
        .cloned()
        .collect();

    rsx! {
        if !working.is_empty() {
            WorkingLenses { runs: working }
        }
        if !runs.is_empty() {
            LensMenu { runs }
        }
        if !summaries.is_empty() {
            SummaryAnswers { runs: summaries }
        }
    }
}

/// How far a lens being answered has got: a count of places, or — for a
/// popover, which has one answer — that it is on its way.
fn progress(run: &LensRun) -> String {
    match run.display {
        LensDisplay::Popover => "working…".to_string(),
        _ => format!("{}/{}", run.done + run.outdated + run.failed, run.total),
    }
}

/// What hovering the lens glyph says: a line for each lens with answers
/// from before the document changed, places left unanswered — none on
/// file, or a run stopped first — or a failure.
fn attention_lines(runs: &[LensRun]) -> Vec<String> {
    let mut lines = Vec::new();
    for run in runs.iter().filter(|run| !run.is_running()) {
        if run.outdated > 0 {
            lines.push(format!("{} — {} outdated", run.label, run.outdated));
        }
        if run.unanswered > 0 {
            lines.push(format!("{} — {} not answered", run.label, run.unanswered));
        }
        if run.error.is_some() || run.failed > 0 {
            lines.push(format!("{} — failed", run.label));
        }
    }
    lines
}

/// What asking again about only what is not answered for the text as it is
/// does: continue a stopped run, retry what failed, regenerate what
/// changed, or several at once.
fn update_label(run: &LensRun) -> String {
    if run.outdated == 0 && run.unanswered == 0 {
        return format!("Retry what failed in {} ({})", run.label, run.failed);
    }
    match (run.outdated, run.unanswered + run.failed) {
        (outdated, 0) => format!("Regenerate what changed in {} ({outdated})", run.label),
        (0, unanswered) => format!("Continue {} ({unanswered} left)", run.label),
        (outdated, unanswered) => format!(
            "Continue {} and regenerate what changed ({})",
            run.label,
            outdated + unanswered
        ),
    }
}

/// The hourglass shown while lenses are being answered, and the list of
/// them it opens, each with the way to stop it.
#[component]
fn WorkingLenses(runs: Vec<LensRun>) -> Element {
    let state = use_context::<AppState>();
    let mut open = use_signal(|| false);
    let title = runs
        .iter()
        .map(|run| format!("{} — {}", run.label, progress(run)))
        .collect::<Vec<_>>()
        .join("\n");

    rsx! {
        div {
            class: "lens-popover-anchor",
            button {
                class: "nav-button lens-working",
                class: if open() { "active" },
                "aria-haspopup": "menu",
                "aria-expanded": open(),
                title: "{title}",
                onclick: move |_| open.set(!open()),
                Icon { name: IconName::Hourglass, class: "lens-hourglass" }
            }
            if open() {
                div {
                    class: "lens-answer-backdrop",
                    onclick: move |_| open.set(false),
                }
                div {
                    class: "lens-switch-menu",
                    role: "menu",
                    div { class: "lens-switch-heading", "Working" }
                    for run in runs {
                        div {
                            key: "{run.token}",
                            class: "lens-switch-item lens-working-row",
                            span {
                                class: "lens-switch-check",
                                Icon { name: IconName::Hourglass, class: "lens-hourglass" }
                            }
                            span { "{run.label}" }
                            span { class: "lens-switch-note", "{progress(&run)}" }
                            button {
                                class: "lens-row-action",
                                title: "Stop {run.label}",
                                onclick: move |_| crate::lenses::stop_run(state, run.token),
                                Icon { name: IconName::Close, size: 12 }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The lens glyph and the menu it opens.
#[component]
fn LensMenu(runs: Vec<LensRun>) -> Element {
    let state = use_context::<AppState>();
    let mut open = use_signal(|| false);
    let lines = attention_lines(&runs);
    let attention = !lines.is_empty();
    let title = if attention {
        lines.join("\n")
    } else {
        "Lenses".to_string()
    };
    let (pages, overlays): (Vec<LensRun>, Vec<LensRun>) = runs
        .iter()
        .cloned()
        .partition(|run| run.display == LensDisplay::Page);
    let applied_page = pages.iter().find(|run| run.applied).map(|run| run.token);
    let stale: Vec<LensRun> = runs
        .iter()
        .filter(|run| !run.is_running() && run.stale() > 0)
        .cloned()
        .collect();
    // The menu stays open through what is chosen in it — several lenses
    // shown or hidden one after another — and closes only on a click
    // outside it.
    let act = move |action: fn(AppState, u64), token: u64| action(state, token);

    rsx! {
        div {
            class: "lens-popover-anchor",
            button {
                class: "nav-button lens-glyph",
                class: if attention { "needs-attention" },
                class: if open() { "active" },
                "aria-haspopup": "menu",
                "aria-expanded": open(),
                title: "{title}",
                onclick: move |_| open.set(!open()),
                Icon { name: IconName::Aperture }
            }
            if open() {
                // A click anywhere else puts the menu away.
                div {
                    class: "lens-answer-backdrop",
                    onclick: move |_| open.set(false),
                }
                div {
                    class: "lens-switch-menu",
                    role: "menu",
                    if !pages.is_empty() {
                        div { class: "lens-switch-heading", "Page" }
                        MenuRow {
                            checked: applied_page.is_none(),
                            label: "Original".to_string(),
                            onclick: move |_| {
                                if let Some(token) = applied_page {
                                    crate::lenses::hide_run(state, token);
                                }
                            },
                        }
                        for run in pages {
                            MenuRow {
                                key: "{run.token}",
                                checked: run.applied,
                                working: run.is_running(),
                                label: run.label.clone(),
                                regenerate: move |_| act(crate::lenses::regenerate_all, run.token),
                                onclick: move |_| act(crate::lenses::show, run.token),
                            }
                        }
                    }
                    if !overlays.is_empty() {
                        div { class: "lens-switch-heading", "Over the page" }
                        for run in overlays {
                            MenuRow {
                                key: "{run.token}",
                                checked: run.applied,
                                working: run.is_running(),
                                label: run.label.clone(),
                                regenerate: move |_| act(crate::lenses::regenerate_all, run.token),
                                onclick: move |_| {
                                    let action = if run.applied {
                                        crate::lenses::hide_run
                                    } else {
                                        crate::lenses::show
                                    };
                                    act(action, run.token);
                                },
                            }
                        }
                    }
                    if !stale.is_empty() {
                        div { class: "lens-switch-divider" }
                        for run in stale {
                            MenuRow {
                                key: "update-{run.token}",
                                icon: IconName::Refresh,
                                label: update_label(&run),
                                onclick: move |_| act(crate::lenses::regenerate, run.token),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One row of the lens menu: a choice, checked when it is what shows and
/// turning an hourglass while it is being answered, or an action with an
/// icon in the same column. A lens's row carries, at its end, the way to
/// ask its agent again about everything.
#[component]
fn MenuRow(
    label: String,
    #[props(default)] checked: bool,
    #[props(default)] working: bool,
    #[props(default)] icon: Option<IconName>,
    #[props(default)] regenerate: Option<EventHandler<()>>,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div {
            class: "lens-switch-item",
            class: if checked { "selected" },
            role: if icon.is_some() { "menuitem" } else { "menuitemcheckbox" },
            tabindex: "0",
            "aria-checked": checked,
            onclick: move |event| onclick.call(event),
            span {
                class: "lens-switch-check",
                if let Some(icon) = icon {
                    Icon { name: icon, size: 14 }
                } else if checked {
                    "✓"
                }
            }
            span { "{label}" }
            if working {
                span {
                    class: "lens-switch-note",
                    title: "Being answered",
                    Icon { name: IconName::Hourglass, size: 12, class: "lens-hourglass" }
                }
            }
            if let Some(regenerate) = regenerate {
                button {
                    class: "lens-row-action",
                    class: if !working { "lens-row-action-end" },
                    title: "Regenerate {label} — ask again about everything",
                    onclick: move |event| {
                        event.stop_propagation();
                        regenerate.call(());
                    },
                    Icon { name: IconName::Refresh, size: 12 }
                }
            }
        }
    }
}

/// The summaries opened over the document, under one glyph: each would
/// otherwise take a glyph of its own, and the header is narrow.
///
/// Drawn at the document's zoom, so a summary reads at the size of what
/// it summarizes.
#[component]
fn SummaryAnswers(runs: Vec<LensRun>) -> Element {
    let state = use_context::<AppState>();
    let mut open = use_signal(|| false);
    let zoom = *state.zoom_level.read();
    let outdated = runs.iter().any(|run| run.outdated > 0);
    let title = runs
        .iter()
        .map(|run| run.label.clone())
        .collect::<Vec<_>>()
        .join("\n");

    rsx! {
        div {
            class: "lens-popover-anchor",
            button {
                class: "nav-button lens-glyph",
                class: if open() { "active" },
                class: if outdated { "needs-attention" },
                title: "{title}",
                onclick: move |_| open.set(!open()),
                Icon { name: IconName::Book }
            }
            if open() {
                // A click anywhere else puts the answers away, as it does
                // for a menu.
                div {
                    class: "lens-answer-backdrop",
                    onclick: move |_| open.set(false),
                }
                div {
                    class: "lens-answer",
                    style: "--lens-zoom: {zoom};",
                    for run in runs {
                        div {
                            key: "{run.token}",
                            class: "lens-answer-section",
                            // The same head as a block's popover: the
                            // lens's name, and what is wrong beside it.
                            div {
                                class: "lens-popover-head",
                                span { class: "lens-popover-caption", "{run.label}" }
                                if run.error.is_some() {
                                    span { class: "lens-popover-badge is-danger", "Failed" }
                                } else if run.outdated > 0 {
                                    span { class: "lens-popover-badge is-warning", "Outdated" }
                                }
                            }
                            match (&run.html, &run.error) {
                                (_, Some(reason)) => rsx! {
                                    div { class: "lens-answer-failed", "{reason}" }
                                },
                                (Some(html), None) => rsx! {
                                    div { class: "markdown-body", dangerous_inner_html: "{html}" }
                                },
                                (None, None) if run.is_running() => rsx! {
                                    div { class: "lens-answer-waiting", "working…" }
                                },
                                (None, None) => rsx! {
                                    div { class: "lens-answer-waiting", "Not answered: stopped before the answer came." }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(label: &str, display: LensDisplay) -> LensRun {
        LensRun {
            lens_id: label.to_lowercase(),
            label: label.to_string(),
            display,
            scope: Scope::Document,
            path: PathBuf::from("/a.md"),
            generation: 1,
            token: 1,
            total: 12,
            done: 3,
            failed: 0,
            outdated: 0,
            unanswered: 0,
            task: None,
            html: None,
            error: None,
            applied: true,
        }
    }

    #[test]
    fn a_lens_at_rest_says_nothing_until_it_has_something_to_say() {
        assert!(attention_lines(&[run("Translate", LensDisplay::Page)]).is_empty());

        let outdated = LensRun {
            outdated: 2,
            ..run("Terms", LensDisplay::Annotate)
        };
        let failed = LensRun {
            error: Some("timed out".to_string()),
            ..run("Summary", LensDisplay::Popover)
        };
        assert_eq!(
            attention_lines(&[outdated, failed]),
            ["Terms — 2 outdated", "Summary — failed"]
        );
    }

    #[test]
    fn a_stopped_run_is_left_to_continue_not_taken_for_done() {
        let stopped = LensRun {
            unanswered: 9,
            ..run("Translate", LensDisplay::Page)
        };

        assert_eq!(
            attention_lines(std::slice::from_ref(&stopped)),
            ["Translate — 9 not answered"]
        );
        assert_eq!(update_label(&stopped), "Continue Translate (9 left)");
        assert_eq!(
            update_label(&LensRun {
                outdated: 2,
                ..stopped.clone()
            }),
            "Continue Translate and regenerate what changed (11)"
        );
        assert_eq!(
            update_label(&LensRun {
                unanswered: 0,
                outdated: 2,
                ..stopped
            }),
            "Regenerate what changed in Translate (2)"
        );
    }

    #[test]
    fn places_that_failed_are_asked_again_without_the_rest() {
        let failed = LensRun {
            failed: 2,
            ..run("Terms", LensDisplay::Annotate)
        };

        assert_eq!(failed.stale(), 2);
        assert_eq!(update_label(&failed), "Retry what failed in Terms (2)");
        assert_eq!(
            update_label(&LensRun {
                unanswered: 3,
                ..failed
            }),
            "Continue Terms (5 left)"
        );
    }

    #[test]
    fn progress_counts_places_and_a_popover_is_on_its_way() {
        assert_eq!(progress(&run("Translate", LensDisplay::Page)), "3/12");
        assert_eq!(progress(&run("Summary", LensDisplay::Popover)), "working…");
    }
}
