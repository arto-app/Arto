use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::{window, Config, WeakDesktopContext, WindowBuilder};
use dioxus::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;

use crate::assets::{main_stylesheet_head, with_asset_protocol};
use crate::components::image_window::{generate_image_id, ImageWindow, ImageWindowProps};
use crate::components::math_window::{generate_math_id, MathWindow, MathWindowProps};
use crate::components::mermaid_window::{generate_diagram_id, MermaidWindow, MermaidWindowProps};
use crate::theme::Theme;

use super::index::{build_image_window_index, build_math_window_index, build_mermaid_window_index};
use super::main::get_last_focused_window;

struct ChildWindowEntry {
    handle: WeakDesktopContext,
    window_id: WindowId,
    parent_id: WindowId,
}

impl ChildWindowEntry {
    fn is_alive(&self) -> bool {
        self.handle.upgrade().is_some()
    }

    fn focus(&self) -> bool {
        self.handle.upgrade().is_some_and(|ctx| {
            ctx.window.set_focus();
            true
        })
    }

    fn close(&self) {
        if let Some(ctx) = self.handle.upgrade() {
            ctx.close();
        }
    }

    fn is_window(&self, window_id: WindowId) -> bool {
        self.window_id == window_id
    }

    fn is_child_of(&self, parent_id: WindowId) -> bool {
        self.parent_id == parent_id
    }
}

enum ChildWindowState {
    Pending { parent_id: WindowId },
    Created(ChildWindowEntry),
}

thread_local! {
    static CHILD_WINDOWS: RefCell<HashMap<String, ChildWindowState>> = RefCell::new(HashMap::new());
}

pub(crate) fn resolve_to_parent_window(window_id: WindowId) -> WindowId {
    CHILD_WINDOWS.with(|windows| {
        windows
            .borrow()
            .values()
            .find_map(|state| match state {
                ChildWindowState::Created(entry) if entry.is_window(window_id) => {
                    Some(entry.parent_id)
                }
                _ => None,
            })
            .unwrap_or(window_id)
    })
}

pub fn close_child_windows_for_parent(parent_id: WindowId) {
    CHILD_WINDOWS.with(|windows| {
        windows.borrow_mut().retain(|_, state| match state {
            ChildWindowState::Pending {
                parent_id: pending_parent,
            } => *pending_parent != parent_id,
            ChildWindowState::Created(entry) => {
                if entry.is_child_of(parent_id) {
                    entry.close();
                    false
                } else {
                    entry.is_alive()
                }
            }
        });
    });
}

pub fn close_child_windows_for_last_focused() {
    if let Some(window_id) = get_last_focused_window() {
        let parent_id = resolve_to_parent_window(window_id);
        close_child_windows_for_parent(parent_id)
    }
}

pub fn close_all_child_windows() {
    CHILD_WINDOWS.with(|windows| {
        windows.borrow_mut().retain(|_, state| match state {
            ChildWindowState::Pending { .. } => true,
            ChildWindowState::Created(entry) => {
                entry.close();
                false
            }
        });
    });
}

/// Try to focus an existing child window, or mark it as pending for creation.
/// Returns `true` if a new window needs to be created.
pub(super) fn try_focus_or_mark_pending(child_id: &str, parent_id: WindowId) -> bool {
    CHILD_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|_, state| match state {
            ChildWindowState::Pending { .. } => true,
            ChildWindowState::Created(entry) => entry.is_alive(),
        });

        match windows.get(child_id) {
            Some(ChildWindowState::Created(entry)) => !entry.focus(),
            Some(ChildWindowState::Pending { .. }) => false,
            None => {
                windows.insert(
                    child_id.to_string(),
                    ChildWindowState::Pending { parent_id },
                );
                true
            }
        }
    })
}

/// Holds the `Pending` slot for as long as the creation that claimed it is
/// still running.
///
/// The slot is what stops a second click from opening a second window while
/// the first is on its way. That only works while something is still coming:
/// a creation that goes away without registering — its task dropped, or the
/// window refused — would otherwise leave the slot claimed forever, and
/// `try_focus_or_mark_pending` would answer every later click with "still
/// being created" for the rest of the session. Releasing it on drop makes the
/// next click a retry instead.
struct PendingSlot {
    child_id: String,
    /// Set once the window is registered, which is when the slot is somebody
    /// else's to clear.
    handed_over: bool,
}

impl PendingSlot {
    fn claimed(child_id: String) -> Self {
        Self {
            child_id,
            handed_over: false,
        }
    }

    /// Whether this creation still owns the slot. A `Pending` entry that is
    /// gone means the window was closed or disowned while it was being made.
    fn still_ours(&self) -> bool {
        CHILD_WINDOWS.with(|windows| {
            windows
                .borrow()
                .get(&self.child_id)
                .is_some_and(|state| matches!(state, ChildWindowState::Pending { .. }))
        })
    }

    fn hand_over(mut self, entry: ChildWindowEntry) {
        self.handed_over = true;
        CHILD_WINDOWS.with(|windows| {
            windows
                .borrow_mut()
                .insert(self.child_id.clone(), ChildWindowState::Created(entry));
        });
    }
}

impl Drop for PendingSlot {
    fn drop(&mut self) {
        if self.handed_over {
            return;
        }
        CHILD_WINDOWS.with(|windows| {
            let mut windows = windows.borrow_mut();
            if let Some(ChildWindowState::Pending { .. }) = windows.get(&self.child_id) {
                tracing::debug!(child_id = %self.child_id, "Child window was never created");
                windows.remove(&self.child_id);
            }
        });
    }
}

/// Register a newly created child window, or close it if the pending state was removed.
pub(super) async fn create_and_register_child_window(
    child_id: String,
    dom: VirtualDom,
    config: Config,
    parent_id: WindowId,
) {
    let slot = PendingSlot::claimed(child_id);
    let ctx = window().new_window(dom, config).await;

    if !slot.still_ours() {
        ctx.close();
        return;
    }

    slot.hand_over(ChildWindowEntry {
        handle: std::rc::Rc::downgrade(&ctx),
        window_id: ctx.window.id(),
        parent_id,
    });
}

pub fn open_or_focus_mermaid_window(source: String, theme: Theme) {
    let diagram_id = generate_diagram_id(&source);
    let parent_id = window().id();

    if try_focus_or_mark_pending(&diagram_id, parent_id) {
        let dom = VirtualDom::new_with_props(
            MermaidWindow,
            MermaidWindowProps {
                source,
                diagram_id: diagram_id.clone(),
                theme,
            },
        );
        let config = with_asset_protocol(Config::new())
            .with_menu(None)
            .with_window(super::icon::apply_app_icon(
                WindowBuilder::new().with_title("Mermaid Viewer"),
            ))
            .with_custom_head(main_stylesheet_head())
            .with_custom_index(build_mermaid_window_index(theme));

        crate::utils::task::spawn_detached(create_and_register_child_window(
            diagram_id, dom, config, parent_id,
        ));
    }
}

pub fn open_or_focus_math_window(source: String, theme: Theme) {
    let math_id = generate_math_id(&source);
    let parent_id = window().id();

    if try_focus_or_mark_pending(&math_id, parent_id) {
        let dom = VirtualDom::new_with_props(
            MathWindow,
            MathWindowProps {
                source,
                math_id: math_id.clone(),
                theme,
            },
        );
        let config = with_asset_protocol(Config::new())
            .with_menu(None)
            .with_window(super::icon::apply_app_icon(
                WindowBuilder::new().with_title("Math Viewer"),
            ))
            .with_custom_head(main_stylesheet_head())
            .with_custom_index(build_math_window_index(theme));

        crate::utils::task::spawn_detached(create_and_register_child_window(
            math_id, dom, config, parent_id,
        ));
    }
}

pub fn open_or_focus_image_window(src: String, alt: Option<String>, theme: Theme) {
    let image_id = generate_image_id(&src);
    let parent_id = window().id();

    if try_focus_or_mark_pending(&image_id, parent_id) {
        let dom = VirtualDom::new_with_props(
            ImageWindow,
            ImageWindowProps {
                src,
                alt,
                image_id: image_id.clone(),
                theme,
            },
        );
        let config = with_asset_protocol(Config::new())
            .with_menu(None)
            .with_window(super::icon::apply_app_icon(
                WindowBuilder::new().with_title("Image Viewer"),
            ))
            .with_custom_head(main_stylesheet_head())
            .with_custom_index(build_image_window_index(theme));

        crate::utils::task::spawn_detached(create_and_register_child_window(
            image_id, dom, config, parent_id,
        ));
    }
}
