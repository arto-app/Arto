/// Emacs-style keybinding template definitions.
///
/// These bindings are independent of menu shortcuts (Cmd+Key).
/// They provide Emacs-like navigation for the markdown viewer.
///
/// Each entry is (key, action, context):
/// - `None` context = global binding (active in any panel)
/// - `Some("sidebar")` etc. = only active when that panel has focus
pub fn bindings() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    let mut b = vec![
        // Scroll (global)
        ("Ctrl+n", "scroll.down", None),
        ("Ctrl+p", "scroll.up", None),
        ("Ctrl+v", "scroll.page_down", None),
        ("Alt+v", "scroll.page_up", None),
        ("Alt+Shift+,", "scroll.top", None),
        ("Alt+Shift+.", "scroll.bottom", None),
        // Search (global)
        ("Ctrl+s", "search.open", None),
        // Window (global)
        ("Ctrl+x b", "window.toggle_sidebar", None),
        ("Ctrl+x r", "window.toggle_right_sidebar", None),
        // General (global)
        ("Ctrl+g", "action.cancel", None),
        ("Escape", "action.cancel", None),
    ];

    // Panel cursor bindings (shared across sidebar, right_sidebar, quick_access)
    for ctx in ["sidebar", "right_sidebar", "quick_access"] {
        b.extend([
            ("Ctrl+n", "cursor.down", Some(ctx)),
            ("Ctrl+p", "cursor.up", Some(ctx)),
            ("Enter", "cursor.enter", Some(ctx)),
        ]);
    }

    // Sidebar-only bindings
    b.extend([
        ("Ctrl+b", "cursor.collapse", Some("sidebar")),
        ("Minus", "directory.parent", Some("sidebar")),
    ]);

    b
}
