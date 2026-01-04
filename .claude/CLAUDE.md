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

- Welcome page content: `assets/welcome.md`
- README: Project description and philosophy
- Use actual project descriptions, not generic placeholders

## Architecture Patterns

### Window Management & Lifecycle

**Arto uses a multi-window architecture:**

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
let theme_value = window::helpers::get_theme_value(is_first_window);
let directory_value = window::helpers::get_directory_value(is_first_window, file.as_ref(), directory);
let sidebar_value = window::helpers::get_sidebar_value(is_first_window);

// Launch MainApp with pre-resolved values
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
- **New Window**: Uses in-memory globals (last focused window)

#### Window Lifecycle Hooks

```rust
// In App component
use_drop(move || {
    // Save state on window close
    config::save_session_sync(
        Some(current_dir),
        Some(current_theme),
        Some(sidebar_visible),
        Some(sidebar_width),
        Some(show_all_files),
    );

    // Close child windows
    window::close_child_windows();
});
```

**IMPORTANT:** Use `persisted.save_sync()` in `use_drop()` context (synchronous, blocking).

### State Management Hierarchy

**Three-tier system (see TIPS.md and architecture-overview.md for details):**

1. **Global Statics** - Shared across windows (CONFIG, LAST_SELECTED_THEME, broadcast channels)
2. **Context (AppState)** - Per-window state (tabs, active tab, zoom)
3. **Local (use_signal)** - Component-only UI state

**Startup priority:**
1. `PersistedState` from `state.json` (last closed window)
2. Fallback to `Config` defaults

**New window priority:**
1. In-memory globals (last focused window)
2. Fallback to `Config` defaults

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
- `use_effect()` - React to state changes
- `spawn_forever()` - Infinite loops (broadcast listeners)
- `use_drop()` - Cleanup (synchronous only!)

**Critical:** `spawn_forever()` never returns. `use_drop()` is synchronous - use `persisted.save_sync()` for blocking operations.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuId {
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

    // Custom items (replace predefined)
    About,
    Preferences,
}

impl From<MenuId> for MenuItemId {
    fn from(id: MenuId) -> Self {
        MenuItemId::new(format!("{:?}", id))
    }
}
```

**Why enum over strings:** Type safety, autocomplete, refactoring support.

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

**2. State-Dependent Handler** (in App component):
```rust
// In app.rs
use_effect(move || {
    spawn_forever(async move {
        while let Ok(event) = rx.recv().await {
            match event.id.as_ref().parse::<MenuId>() {
                MenuId::CloseTab => {
                    state.close_current_tab();
                }
                MenuId::Preferences => {
                    state.open_preferences();
                }
                // Other state actions...
                _ => {}
            }
        }
    });
});
```

**Why split:** Some actions don't need state (new window), others do (close tab, preferences).

**IMPORTANT:** Replace `PredefinedMenuItem::about()` with custom `MenuId::About` to control navigation.
