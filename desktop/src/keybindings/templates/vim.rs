/// Vim-style keybinding template definitions.
///
/// These bindings are independent of menu shortcuts (Cmd+Key).
/// They provide Vim-like navigation for the markdown viewer.
///
/// Each entry is (key, action, context):
/// - `None` context = global binding (active in any panel)
/// - `Some("sidebar")` etc. = only active when that panel has focus
pub fn bindings() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    let mut b = vec![
        // Scroll (global)
        ("j", "scroll.down", None),
        ("k", "scroll.up", None),
        ("Ctrl+d", "scroll.half_page_down", None),
        ("Ctrl+u", "scroll.half_page_up", None),
        ("Ctrl+f", "scroll.page_down", None),
        ("Ctrl+b", "scroll.page_up", None),
        ("g g", "scroll.top", None),
        ("Shift+g", "scroll.bottom", None),
        // History (global)
        ("Shift+h", "history.back", None),
        ("Shift+l", "history.forward", None),
        // Search (global)
        ("/", "search.open", None),
        ("n", "search.next", None),
        ("Shift+n", "search.prev", None),
        // Tab (global)
        ("g t", "tab.next", None),
        ("g Shift+t", "tab.prev", None),
        ("u", "tab.undo_close", None),
        // Clipboard (global)
        ("y y", "clipboard.copy_url", None),
        // Zoom (global)
        ("z i", "zoom.in", None),
        ("z o", "zoom.out", None),
        ("z z", "zoom.reset", None),
        // Window (global)
        ("Shift+b", "window.toggle_sidebar", None),
        ("Shift+r", "window.toggle_right_sidebar", None),
        // Focus (global)
        ("Ctrl+h", "focus.left_sidebar", None),
        ("Ctrl+l", "focus.right_sidebar", None),
        ("Ctrl+q", "focus.quick_access", None),
        // General (global)
        ("Escape", "action.cancel", None),
    ];

    // Panel cursor bindings (shared across sidebar, right_sidebar, quick_access)
    for ctx in ["sidebar", "right_sidebar", "quick_access"] {
        b.extend([
            ("j", "cursor.down", Some(ctx)),
            ("k", "cursor.up", Some(ctx)),
            ("Ctrl+n", "cursor.down", Some(ctx)),
            ("Ctrl+p", "cursor.up", Some(ctx)),
            ("l", "cursor.enter", Some(ctx)),
            ("Enter", "cursor.enter", Some(ctx)),
        ]);
    }

    // Sidebar-only bindings
    b.extend([
        ("h", "cursor.collapse", Some("sidebar")),
        ("Minus", "directory.parent", Some("sidebar")),
        ("Shift+h", "directory.back", Some("sidebar")),
        ("Shift+l", "directory.forward", Some("sidebar")),
    ]);

    b
}
