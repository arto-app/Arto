# Project-Specific Rules

> **📖 For detailed best practices and tips:** See [TIPS.md](.claude/TIPS.md)

## Quick Reference

- **Code Comments**: Must be in English
- **Test Code**: Use `indoc` crate for multi-line strings
- **Module System**: Use Rust 2018+ (no `mod.rs`)
- **Icon Management**: Use `add-icon` skill
- **UI/UX Design**: See `.claude/rules/ui-design.md`
- **Quality Check**: Run `just fmt check test` before reporting completion
- **Application Launch**: Do NOT launch the application; the user handles this

## Development Workflow

### Quality Assurance

**CRITICAL: Before reporting task completion, ALWAYS run:**

```bash
just fmt check test
```

This command runs:
- `cargo fmt` + `oxfmt` - Code formatting (Rust + TypeScript/CSS)
- `cargo clippy` + `oxlint` - Linting and best practices
- `cargo test` - All tests

**Do NOT report completion if any of these fail.** Fix all issues first.

## Content Source

**Always check existing content files before writing descriptions:**

- Welcome page content: `crates/arto/assets/welcome.md`
- README: Project description and philosophy
- Use actual project descriptions, not generic placeholders

## Architecture Patterns

### Single-Instance Architecture

**Arto enforces single-instance via IPC (Inter-Process Communication):**

When a new Arto process launches:
1. Check for existing instance via Unix domain socket connection
2. If found → send paths to existing instance via JSON Lines protocol, exit(0)
3. If not found → become primary instance, start IPC server for future launches

**Protocol (JSON Lines):**
```json
{"type":"open","files":["/path/to/file.md"],"directory":null,"behavior":"last_focused"}
{"type":"open","files":[],"directory":"/path/to/dir","behavior":"new_window"}
{"type":"reopen","behavior":"last_focused"}
```

The older `file` and `directory` messages are still accepted from a not-yet-upgraded secondary instance.

**Implementation:** The protocol and the socket live in `crates/arto-ipc/` (`IpcMessage`, `OpenEvent`, `send_to_existing_instance`, `IpcServer`). The app side, `crates/arto/src/ipc.rs`, queues received events, wakes the main thread, and opens files in the right window.

**Why single-instance:** Prevents multiple processes from conflicting over file watches, configuration writes, and persisted state management.

### Window Management & Lifecycle

**Within the single instance, Arto uses a multi-window architecture:**

#### Window Types

1. **Main Windows** (user-visible)
   - First window launched from `main()` uses MainApp component
   - Handles system events: file open, app reopen
   - Uses `WindowCloseBehaviour::WindowHides` (last window hides instead of quitting)
   - Additional windows created on demand (File → New Window)
   - Each has independent tabs and state

2. **Child Windows** (specialized)
   - Mermaid diagram viewer, etc.
   - Owned by a parent main window
   - Auto-close when parent closes

#### Window Creation Pattern

```rust
// First window (synchronous initialization in main())
let is_first_window = true;
let theme = window::settings::get_theme_preference(is_first_window);
let directory = window::settings::get_directory_preference(is_first_window);
let sidebar = window::settings::get_sidebar_preference(is_first_window);

// Launch MainApp with pre-resolved preferences
dioxus::LaunchBuilder::desktop()
    .with_cfg(config)
    .launch(components::main_app::MainApp);

// Additional windows (async creation)
window_manager::create_new_main_window(file, directory, show_welcome);
```

**Key differences:**
- **First window**: Resolved synchronously in `main()` before Dioxus launch (eliminates flash)
- **Additional windows**: Created asynchronously using helper functions
- **Startup**: Uses `PersistedState` from `state.json` (last closed window)
- **New Window**: Accesses last focused window's `AppState` directly via `WINDOW_STATES` mapping

#### Window Lifecycle Hooks

```rust
// In App component - register state on creation
let mut state = use_context_provider(|| {
    let mut app_state = AppState::new(theme);
    // ... initialization ...
    crate::window::register_window_state(window().id(), app_state);
    app_state
});

// Cleanup on window close
use_drop(move || {
    // Unregister from WINDOW_STATES mapping
    crate::window::unregister_window_state(window_id);

    // Save state on window close
    let mut persisted = PersistedState::from(&state);

    // Capture window metrics for restoration
    let window_metrics = crate::window::metrics::capture_window_metrics(&window().window);
    persisted.window_position = window_metrics.position;
    persisted.window_size = window_metrics.size;

    // Save to disk (synchronous, blocking)
    persisted.save();

    // Close child windows owned by this window
    crate::window::close_child_windows_for_parent(window_id);
});
```

**IMPORTANT:** The `save()` method is synchronous and blocking, suitable for `use_drop()` context.

### State Management Hierarchy

**Three-tier system (see TIPS.md and architecture-overview.md for details):**

1. **Global Statics** - Shared across windows (CONFIG, WINDOW_STATES mapping, broadcast channels)
2. **Context (AppState)** - Per-window state (tabs, active tab, zoom)
3. **Local (use_signal)** - Component-only UI state

**Startup priority:**
1. `PersistedState` from `state.json` (last closed window)
2. Fallback to `Config` defaults

**New window priority:**
1. Last focused window's `AppState` via `WINDOW_STATES` mapping
2. Fallback to `PersistedState::load()` then `Config` defaults

### Configuration System

**Dual-file system (see TIPS.md and architecture-overview.md for details):**

```
~/Library/Application Support/arto/
├── config.json   # User preferences (Config type)
└── state.json    # Last window state (PersistedState type)
```

**Hot reload:** File changes broadcast to all windows via `CONFIG_CHANGED_BROADCAST`.

### Async Patterns in Dioxus

**Key patterns (see TIPS.md for details):**

- `spawn()` - Event handlers, one-time async
- `use_effect()` - React to state changes (reads inside the closure subscribe it; take changing props as `ReadSignal<T>` instead of `use_reactive!`)
- `use_future()` - Long-running listeners tied to the component (broadcast subscriptions); cancelled when the component drops
- `use_drop()` - Cleanup (synchronous only!)

**Critical:** avoid `spawn_forever()` in components: the task outlives the window and keeps writing to dropped signals. `use_drop()` is synchronous - use `persisted.save()` for blocking operations.

### Markdown Rendering Pipeline

**Markdown rendering follows a specific order to handle special syntax:**

```
Input Markdown
    ↓
1. Pre-process GitHub Alerts
   (Convert blockquote-based alerts to custom HTML)
    ↓
2. Parse with pulldown-cmark
   (GitHub Flavored Markdown options)
    ↓
3. Process Special Code Blocks
   - Mermaid diagrams → custom renderer
   - Math expressions → KaTeX
    ↓
4. Render to HTML
    ↓
5. Post-process with lol_html
   - Convert relative image paths to data URLs
   - Convert local .md links to clickable spans
   - Preserve HTTP/HTTPS URLs
    ↓
Output HTML
```

#### Key Implementation Details

**1. GitHub Alerts** (`markdown.rs`):
```rust
// Convert blockquote alerts BEFORE parsing
fn preprocess_github_alerts(markdown: &str) -> String {
    // [!NOTE] → <div class="markdown-alert markdown-alert-note">
}
```

**2. Special Code Blocks** (during HTML generation):
```rust
Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
    match lang.as_ref() {
        "mermaid" => {
            // Generate mermaid diagram container
        }
        "math" => {
            // Generate KaTeX container
        }
        _ => {
            // Regular syntax highlighting
        }
    }
}
```

**3. Post-processing** (`lol_html` element handler):
```rust
// Convert relative images to data URLs (offline support)
element!("img[src]", |el| {
    if let Some(src) = el.get_attribute("src") {
        if !src.starts_with("http") && !src.starts_with("data:") {
            let data_url = image_to_data_url(&base_path.join(&src))?;
            el.set_attribute("src", &data_url)?;
        }
    }
});

// Convert local .md links to custom click handlers
element!("a[href]", |el| {
    if let Some(href) = el.get_attribute("href") {
        if href.ends_with(".md") && !href.starts_with("http") {
            el.remove_attribute("href");
            el.set_attribute("class", "markdown-link")?;
            el.set_attribute("data-path", &href)?;
        }
    }
});
```

**IMPORTANT:** Always follow this order. Pre-processing must happen before parsing to avoid conflicts.

### File Operations

**Key patterns (see TIPS.md for details):**

- Always canonicalize paths (macOS symlinks)
- Extract directory root: use parent for files
- File watcher is thread-local (avoid Send/Sync issues)

### Menu & Event Handling

**Menu system follows platform-specific patterns with type-safe IDs:**

#### Menu ID Pattern

```rust
enum MenuId {
    // App menu
    About,

    // File menu
    NewWindow,
    OpenFile,
    OpenDirectory,
    CloseTab,

    // Edit menu
    Copy,
    SelectAll,

    // View menu
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // Window menu
    Preferences,
}

impl MenuId {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "app.about" => Some(Self::About),
            "file.new_window" => Some(Self::NewWindow),
            "file.open_file" => Some(Self::OpenFile),
            // ... other mappings
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::About => "app.about",
            Self::NewWindow => "file.new_window",
            // ... other mappings
        }
    }
}
```

**Why enum over strings:** Type safety, autocomplete, refactoring support. Menu IDs use hierarchical string format (e.g., `"app.about"`, `"file.new_window"`).

#### Split Handler Pattern

Menu events are handled in two places:

**1. Global Handler** (no state access):
```rust
// In main_app.rs
use_muda_event_handler(move |event| {
    crate::menu::handle_menu_event_global(event);
});

// In menu.rs
pub fn handle_menu_event_global(event: MenuEvent) {
    match event.id.as_ref().parse::<MenuId>() {
        MenuId::NewWindow => {
            window_manager::create_new_main_window(None, None, false);
        }
        // Other global actions...
        _ => {}
    }
}
```

**2. State-Dependent Handler** (in App component, one per window):
```rust
// In app.rs
#[cfg(not(target_os = "windows"))]
use_muda_event_handler(move |event| {
    menu::handle_menu_event_with_state(&event, &mut state);
});

// In menu.rs
pub fn handle_menu_event_with_state(event: &MenuEvent, state: &mut AppState) -> bool {
    // Every window registers this handler; only the focused one acts.
    if !window().is_focused() {
        return false;
    }
    match event.id.as_ref().parse::<MenuId>() {
        MenuId::CloseTab => {
            state.close_current_tab();
            true
        }
        MenuId::Preferences => {
            state.open_preferences();
            true
        }
        // Other state actions...
        _ => false,
    }
}
```

Both handlers are plain `use_muda_event_handler` callbacks; there is no
channel between them. Every window registers the state-dependent one, muda
delivers each event to every registered handler, and the focus check keeps
only the focused window from acting on it.

**Why split:** Some actions don't need state (new window), others do (close tab, preferences).

**IMPORTANT:** Replace `PredefinedMenuItem::about()` with custom `MenuId::About` to control navigation.

### Cross-Window Communication

**Event-based coordination between windows using broadcast channels:**

Arto uses broadcast channels for multi-window features. See `crates/arto/src/events.rs` for detailed architecture documentation.

#### 1. Tab Transfers

**TRANSFER_TAB_TO_WINDOW:**
- Fire-and-forget pattern for moving tabs between windows
- Used by drag-and-drop and context menu "Move to Window"
- Preserves full tab history including navigation stack
- Auto-focuses target window after transfer

```rust
// Send tab to another window (preserves history)
crate::events::TRANSFER_TAB_TO_WINDOW
    .send((target_window_id, target_index, tab))
    .ok();
crate::window::main::focus_window(target_window_id);

// Receive in target window
use_future(move || async move {
    let mut rx = crate::events::TRANSFER_TAB_TO_WINDOW.subscribe();
    while let Ok((target_wid, index, tab)) = rx.recv().await {
        if target_wid == window().id() {
            state.insert_tab(tab, index.unwrap_or(tabs_len));
        }
    }
});
```

#### 2. Drag State Updates

**ACTIVE_DRAG_UPDATE:**
- Notifies all windows when drag state changes
- Enables visual feedback (floating tab, drop indicators)
- Bridges global event handlers with Dioxus reactivity

**Why broadcast channels:**
- Multiple windows need to receive the same event
- Dynamic subscribers (windows created/destroyed at runtime)
- Simple fire-and-forget pattern for desktop app (no network latency)
