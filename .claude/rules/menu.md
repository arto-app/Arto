---
paths: "crates/arto/src/menu.rs, crates/arto/src/menu/**, crates/arto/src/components/app.rs, crates/arto/src/components/main_app.rs"
---

# Menu and Event Handling

## Menu IDs are an enum

Menu items carry hierarchical string ids (`"app.about"`, `"file.new_window"`,
`"view.zoom_in"`) and `MenuId` maps them both ways (`from_str` / `as_str`).
Add a variant and both mappings for every new item; never match on raw
strings elsewhere. Replace `PredefinedMenuItem::about()` with the custom
`MenuId::About` so the About screen navigates in-app.

## Split handlers

Both handlers are plain `use_muda_event_handler` callbacks; muda delivers
every event to every registered handler and there is no channel between
them.

1. **Global handler** (registered once in `MainApp`, no state access):
   `menu::handle_menu_event_global(event)` handles actions that need no
   window state, such as New Window.
2. **State-dependent handler** (registered by every `App`, one per window):
   `menu::handle_menu_event_with_state(event, &mut state)`. It starts with
   `if !window().is_focused() { return false; }`, so only the focused window
   acts on Close Tab, Preferences, zoom and the like.

The closure parameter is already a `&MenuEvent`, so it is passed as is:

```rust
// main_app.rs
#[cfg(target_os = "macos")]
use_muda_event_handler(move |event| {
    crate::menu::handle_menu_event_global(event);
});

// app.rs
#[cfg(target_os = "macos")]
use_muda_event_handler(move |event| {
    menu::handle_menu_event_with_state(event, &mut state);
});
```

Both return `bool` (handled or not). The native menu bar is macOS only —
everywhere else the same commands hang off the glyph at the head of the
header — so check the `cfg` guards before touching registration.

## Dock menu

`menu::dock` builds the menu shown on a right click of the Dock icon. Tao's
delegate has no `applicationDockMenu:`, so `dock::install` (called from
`MainApp`) adds it to the delegate's class at runtime, and AppKit asks for a
fresh menu every time it opens. Its items use `dock.*` ids of their own and
are handled first by the global handler: the menu bar's Open items act on the
focused window, and an app reached from the Dock may have none.
