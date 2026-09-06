---
paths: "crates/arto/src/window/**, crates/arto/src/events.rs, crates/arto/src/main.rs, crates/arto/src/components/app.rs, crates/arto/src/components/main_app.rs"
---

# Windows: Lifecycle and Cross-Window Events

## Window types

1. **Main windows**: the first one is launched from `main()` with the
   `MainApp` component and handles system events (file open, app reopen).
   `WindowCloseBehaviour::WindowHides` keeps the last window alive instead of
   quitting. Further windows come from File → New Window and each owns its
   tabs and state.
2. **Child windows** (Mermaid, math, image viewers) are owned by a main
   window and close with it.

## Creating windows

```rust
// First window: preferences resolved synchronously in main() so the first
// frame already has the right theme, directory and sidebar (no flash).
let is_first_window = true;
let theme = window::settings::get_theme_preference(is_first_window);
let directory = window::settings::get_directory_preference(is_first_window);
let sidebar = window::settings::get_sidebar_preference(is_first_window);
dioxus::LaunchBuilder::desktop()
    .with_cfg(config)
    .launch(components::main_app::MainApp);

// Additional windows: fire-and-forget, created by the event loop on its
// next iteration. Must run on the main thread.
window::create_main_window_sync(
    &window(),
    Tab::default(),
    CreateMainWindowConfigParams::default(),
);
```

Startup reads `PersistedState` (`state.json`, the last closed window); New
Window reads the last focused window's `AppState` through `WINDOW_STATES`.
The resolution rules are in `architecture-overview.md`.

## Lifecycle hooks in `App`

```rust
let mut state = use_context_provider(|| {
    let mut app_state = AppState::new(theme);
    crate::window::register_window_state(window().id(), app_state);
    app_state
});

use_drop(move || {
    crate::window::unregister_window_state(window_id);
    let mut persisted = PersistedState::from(&state);
    let metrics = crate::window::metrics::capture_window_metrics(&window().window);
    persisted.window_position = metrics.position;
    persisted.window_size = metrics.size;
    persisted.save(); // synchronous and blocking: fine inside use_drop
    crate::window::close_child_windows_for_parent(window_id);
});
```

`use_drop` is synchronous; never spawn from it.

## Cross-window communication

Windows coordinate through the broadcast channels in
`crates/arto/src/events.rs`; the module doc there is the reference.

- `TRANSFER_TAB_TO_WINDOW`: fire-and-forget tab move (drag-and-drop and the
  "Move to Window" context menu). The whole tab, including its navigation
  history, is sent; the target window inserts it and is focused.
- `ACTIVE_DRAG_UPDATE`: drag state changes, so every window can draw the
  floating tab and drop indicators.
- `OPEN_FILE_IN_WINDOW` / `OPEN_DIRECTORY_IN_WINDOW`: open a path in a
  specific window.

```rust
// Send
crate::events::TRANSFER_TAB_TO_WINDOW.send((target_window_id, target_index, tab)).ok();
crate::window::main::focus_window(target_window_id);

// Receive (tied to the component, cancelled when the window drops)
use_future(move || async move {
    let mut rx = crate::events::TRANSFER_TAB_TO_WINDOW.subscribe();
    while let Ok((target_wid, index, tab)) = rx.recv().await {
        if target_wid == window().id() {
            state.insert_tab(tab, index.unwrap_or(tabs_len));
        }
    }
});
```

Broadcast channels fit because several windows receive the same event,
subscribers come and go at runtime, and there is no network latency to
design around.
