use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::components::icon::{Icon, IconName};

#[component]
pub fn ContextMenuItem(
    #[props(into)] label: String,
    #[props(default)] shortcut: Option<String>,
    #[props(default)] icon: Option<IconName>,
    #[props(default = false)] disabled: bool,
    on_click: EventHandler<()>,
) -> Element {
    let depth = use_row_depth();
    let mut hover = use_signal(|| 0_u64);

    rsx! {
        div {
            class: if disabled { "context-menu-item disabled" } else { "context-menu-item" },
            // Resting on a plain row puts away a submenu open beside it; only
            // resting, so that a pointer crossing on its way to that submenu
            // does not.
            // The rows a row sits in are not entered again by entering it:
            // passed up to them, the event would read as the pointer coming
            // back to them and put away the very submenu it is in.
            onmouseenter: move |event| {
                event.stop_propagation();
                *hover.write() += 1;
                let entered = *hover.peek();
                if OPEN_SUBMENUS.peek().len() > depth {
                    spawn(async move {
                        tokio::time::sleep(SUBMENU_DWELL).await;
                        if *hover.peek() == entered {
                            settle(&mut OPEN_SUBMENUS.write(), depth, None);
                        }
                    });
                }
            },
            onmouseleave: move |event| {
                event.stop_propagation();
                *hover.write() += 1;
            },
            onclick: move |_| {
                if !disabled {
                    on_click.call(());
                }
            },

            if let Some(icon) = icon {
                Icon {
                    name: icon,
                    size: 14,
                    class: "context-menu-icon",
                }
            }

            span { class: "context-menu-label", "{label}" }

            if let Some(shortcut) = shortcut {
                span { class: "context-menu-shortcut", "{shortcut}" }
            }
        }
    }
}

#[component]
pub fn ContextMenuSeparator() -> Element {
    rsx! {
        div { class: "context-menu-separator" }
    }
}

/// How long the pointer rests on a row before the submenus beside it
/// change: long enough that crossing rows on the way to an open flyout
/// changes nothing, short enough that resting on one reads as choosing it.
const SUBMENU_DWELL: Duration = Duration::from_millis(200);

/// The submenus open in a window's context menu, one per depth: the one at
/// the top level, the one open inside it, and so on.
static OPEN_SUBMENUS: GlobalSignal<Vec<u64>> = Signal::global(Vec::new);

/// How deep in submenus a submenu sits: 0 in the menu itself.
#[derive(Clone, Copy)]
struct SubmenuDepth(usize);

/// How deep the row being drawn sits: 0 in the menu itself, one more inside
/// each submenu. Read once, as the row is made.
fn use_row_depth() -> usize {
    try_use_context::<SubmenuDepth>().map_or(0, |parent| parent.0 + 1)
}

/// The pointer rests on a row at `depth`: the submenu `row` opens there, or,
/// for a plain row, whatever was open there closes — with, either way,
/// whatever was open inside it.
fn settle(open: &mut Vec<u64>, depth: usize, row: Option<u64>) {
    open.truncate(depth);
    open.extend(row);
}

/// Whether entering the submenu row `id` at `depth` opens it at once rather
/// than on resting: when nothing else is open there to be put away.
fn opens_at_once(open: &[u64], depth: usize, id: u64) -> bool {
    open.get(depth).is_none_or(|open| *open == id)
}

/// Reusable submenu component with hover-to-open behavior, which may hold
/// submenus of its own.
///
/// A submenu does not close as the pointer leaves its row: the way from a
/// row to its flyout runs diagonally across the rows below, however far
/// down the flyout the item being reached for is. What is open beside a row
/// changes only when the pointer rests on another row at the same depth.
#[component]
pub fn ContextMenuSubmenu(
    label: String,
    #[props(default)] icon: Option<IconName>,
    children: Element,
) -> Element {
    let id = use_hook(|| {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        NEXT.fetch_add(1, Ordering::Relaxed)
    });
    // Read before providing: this is the depth of the submenu it sits in.
    let depth = use_row_depth();
    use_context_provider(|| SubmenuDepth(depth));
    let show = OPEN_SUBMENUS.read().get(depth) == Some(&id);
    // Bumped whenever the pointer enters or leaves, so that an opening
    // scheduled while the pointer only crossed the row is dropped.
    let mut hover = use_signal(|| 0_u64);
    // A menu closed with a submenu open leaves nothing open behind it for
    // the next menu to trip over.
    use_drop(move || {
        let mut open = OPEN_SUBMENUS.write();
        if open.get(depth) == Some(&id) {
            open.truncate(depth);
        }
    });

    rsx! {
        div {
            class: "context-menu-item has-submenu",
            // Kept from the rows this one sits in: see `ContextMenuItem`.
            onmouseenter: move |event| {
                event.stop_propagation();
                *hover.write() += 1;
                let entered = *hover.peek();
                if OPEN_SUBMENUS.peek().get(depth) == Some(&id) {
                    // Already open: coming back to it, from its flyout or
                    // across its row, leaves what is open inside it.
                    return;
                }
                if opens_at_once(&OPEN_SUBMENUS.peek(), depth, id) {
                    settle(&mut OPEN_SUBMENUS.write(), depth, Some(id));
                    return;
                }
                spawn(async move {
                    tokio::time::sleep(SUBMENU_DWELL).await;
                    if *hover.peek() == entered {
                        settle(&mut OPEN_SUBMENUS.write(), depth, Some(id));
                    }
                });
            },
            onmouseleave: move |event| {
                event.stop_propagation();
                *hover.write() += 1;
            },

            // A row that opens onto more rows is still a row: it wears its
            // icon in the same column as the items around it, or the column
            // breaks wherever a submenu sits in the list.
            if let Some(icon) = icon {
                Icon {
                    name: icon,
                    size: 14,
                    class: "context-menu-icon",
                }
            }

            span { class: "context-menu-label", "{label}" }
            span { class: "submenu-arrow", "›" }

            if show {
                div {
                    class: "context-submenu",
                    "data-submenu": "{id}",
                    // Where the flyout fits is only known once it is drawn:
                    // a menu opened near the window's right edge, or a
                    // submenu two deep, would otherwise run off screen.
                    onmounted: move |_| {
                        document::eval(&format!("window.Arto?.contextMenu?.fitSubmenu?.({id});"));
                    },
                    {children}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_submenu_opened_inside_another_leaves_it_open() {
        let mut open = Vec::new();
        settle(&mut open, 0, Some(1));
        settle(&mut open, 1, Some(2));

        assert_eq!(open, [1, 2]);
    }

    #[test]
    fn resting_on_another_row_puts_away_what_was_open_beside_it_and_inside() {
        let mut open = vec![1, 2];
        settle(&mut open, 0, Some(3));
        assert_eq!(open, [3]);

        settle(&mut open, 0, None);
        assert!(open.is_empty());
    }

    #[test]
    fn a_row_opens_at_once_only_when_nothing_else_is_open_beside_it() {
        assert!(opens_at_once(&[], 0, 1));
        assert!(opens_at_once(&[1], 0, 1));
        assert!(opens_at_once(&[1], 1, 2));
        assert!(!opens_at_once(&[1, 2], 1, 3));
    }

    thread_local! {
        static DEPTHS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }

    /// A submenu's hold on its depth, with its children always drawn.
    #[component]
    fn Level(children: Element) -> Element {
        let depth = use_row_depth();
        use_context_provider(|| SubmenuDepth(depth));
        DEPTHS.with(|depths| depths.borrow_mut().push(depth));
        rsx! { div { {children} } }
    }

    /// A component between two levels, as a lens's row is.
    #[component]
    fn Between() -> Element {
        rsx! { Level {} }
    }

    #[test]
    fn a_submenu_handed_in_as_a_child_is_one_deeper_than_the_one_it_is_in() {
        fn app() -> Element {
            rsx! { Level { Level { Between {} } } }
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();

        DEPTHS.with(|depths| assert_eq!(*depths.borrow(), [0, 1, 2]));
    }
}
