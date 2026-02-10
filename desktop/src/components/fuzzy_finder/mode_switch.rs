use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::finder::FinderMode;
use crate::state::AppState;

/// Footer bar showing current search mode and shortcut hint.
///
/// Clicking toggles between file and directory mode. The active mode is
/// indicated by accent color; the inactive mode stays muted.
///
/// Mode switching is instant: the FuzzyFinder's use_effect on mode signal
/// re-applies the appropriate result set from the dual Nucleo instances.
#[component]
pub fn FinderModeSwitch() -> Element {
    let mut state = use_context::<AppState>();
    let mode = *state.finder_mode.read();

    let toggle = move |_| {
        state.switch_finder_mode();
    };

    rsx! {
        div {
            class: "finder-footer",
            onclick: toggle,

            // Mode indicator
            div {
                class: "finder-footer-mode",

                span {
                    class: if mode == FinderMode::File { "finder-footer-option active" } else { "finder-footer-option" },
                    Icon { name: IconName::File, size: 12 }
                    "File"
                }

                span { class: "finder-footer-separator", "/" }

                span {
                    class: if mode == FinderMode::Directory { "finder-footer-option active" } else { "finder-footer-option" },
                    Icon { name: IconName::Folder, size: 12 }
                    "Directory"
                }
            }

            // Shortcut hint
            span {
                class: "finder-footer-hint",

                kbd { "⌘M" }
                " to switch"
            }
        }
    }
}
