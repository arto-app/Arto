//! The preferences window.
//!
//! Preferences are a short errand — change a thing, close it again — while a
//! tab is where a document lives for as long as you are reading it. Giving
//! the errand a document's lifetime is what left a settings tab sitting open
//! for days, so it gets a window of its own instead: opened from the menu or
//! `⌘,`, focused if already open, and gone when closed.
//!
//! The window carries no `AppState`. What the "Current Settings" sliders need
//! from the window that opened them travels here as a [`PreferencesSnapshot`],
//! taken at open time; what they change travels back as an event
//! ([`crate::events::SET_SIDEBAR_ZOOM_IN_WINDOW`]).

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::assets::{main_stylesheet_head, with_asset_protocol};
use crate::components::preferences_window::{PreferencesWindow, PreferencesWindowProps};
use crate::theme::Theme;

use super::child::{create_and_register_child_window, try_focus_or_mark_pending};
use super::index::build_preferences_window_index;

/// There is only ever one preferences window, so it needs no generated key.
const PREFERENCES_CHILD_ID: &str = "preferences";

const PREFERENCES_WIDTH: f64 = 880.0;
const PREFERENCES_HEIGHT: f64 = 640.0;

/// What the preferences window knows about the window that opened it.
///
/// Only the values the "Current Settings" section reports back — everything
/// else it edits is the configuration, which it reads for itself.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesSnapshot {
    /// The window these settings act on, so changes can be sent back to it.
    pub window_id: WindowId,
    pub sidebar_width: f64,
    pub sidebar_zoom_level: f64,
    pub directory: Option<PathBuf>,
}

/// Open the preferences window, or focus it if it is already open.
pub fn open_or_focus_preferences_window(snapshot: PreferencesSnapshot, theme: Theme) {
    let parent_id = window().id();

    if try_focus_or_mark_pending(PREFERENCES_CHILD_ID, parent_id) {
        let dom = VirtualDom::new_with_props(
            PreferencesWindow,
            PreferencesWindowProps { snapshot, theme },
        );
        let config = with_asset_protocol(Config::new())
            .with_menu(None)
            .with_window(super::icon::apply_app_icon(
                WindowBuilder::new()
                    .with_title("Preferences")
                    .with_inner_size(LogicalSize::new(PREFERENCES_WIDTH, PREFERENCES_HEIGHT)),
            ))
            .with_custom_head(main_stylesheet_head())
            .with_custom_index(build_preferences_window_index(theme));

        dioxus_core::spawn(create_and_register_child_window(
            PREFERENCES_CHILD_ID.to_string(),
            dom,
            config,
            parent_id,
        ));
    }
}
