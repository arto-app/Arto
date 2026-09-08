use dioxus::prelude::*;

use crate::components::content::PreferencesView;
use crate::hooks::viewer_window::use_theme_dispatch;
use crate::theme::Theme;
use crate::window::preferences::PreferencesSnapshot;

/// Root of the preferences window.
///
/// Thin on purpose: the page itself is [`PreferencesView`], which is the same
/// component whichever window it is drawn in. All this adds is the window's
/// own theme, which has to be dispatched to the document because each window
/// paints its own `data-theme`.
#[component]
pub fn PreferencesWindow(snapshot: PreferencesSnapshot, theme: Theme) -> Element {
    let current_theme = use_signal(|| theme);
    use_theme_dispatch(current_theme);

    rsx! {
        div {
            class: "preferences-window",
            PreferencesView { snapshot }
        }
    }
}
