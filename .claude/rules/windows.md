---
paths: "crates/arto/src/window/**, crates/arto/src/events.rs, crates/arto/src/main.rs, crates/arto/src/components/app.rs, crates/arto/src/components/main_app.rs"
---

# Windows: Lifecycle and Cross-Window Events

## Window types

1. **Main windows**: the first one is launched from `main()` with the
   `MainApp` component and handles system events (file open, app reopen).
   `WindowCloseBehaviour::WindowHides` keeps the last window alive instead of
   quitting. Further windows come from File → New Window and each reads one
   document and owns its own state.
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
    Document::default(),
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

- `SET_SIDEBAR_ZOOM_IN_WINDOW` / `SET_RIGHT_SIDEBAR_ZOOM_IN_WINDOW`: the
  preferences window acting on the window that opened it. Preferences holds no
  `AppState` of its own, so a "Current Settings" slider sends rather than
  writes.

```rust
// Send
crate::events::SET_SIDEBAR_ZOOM_IN_WINDOW.send((target_window_id, zoom)).ok();

// Receive (tied to the component, cancelled when the window drops)
use_future(move || async move {
    let mut rx = crate::events::SET_SIDEBAR_ZOOM_IN_WINDOW.subscribe();
    while let Ok((target_wid, zoom)) = rx.recv().await {
        if target_wid == window().id() {
            state.sidebar.write().zoom_level = zoom;
        }
    }
});
```

Broadcast channels fit because several windows receive the same event,
subscribers come and go at runtime, and there is no network latency to
design around.

What a window does to *itself* does not travel this way. The bookmarks
announce their own changes over a channel of their own, and every window
listens; a document is opened by calling `AppState::open_file` on the window
that should read it, which is the window the command came from.
