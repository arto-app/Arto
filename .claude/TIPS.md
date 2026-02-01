# Development Tips & Best Practices

Practical lessons learned from developing Arto. These tips complement the architectural documentation in CLAUDE.md.

## Table of Contents

1. [Module Organization](#module-organization)
2. [Serde & Serialization](#serde--serialization)
3. [Dioxus Patterns](#dioxus-patterns)
4. [State Management](#state-management)
5. [File Operations](#file-operations)
6. [Configuration Design](#configuration-design)
7. [Code Quality](#code-quality)

---

## Module Organization

### Use Modern Rust Module System

**❌ Don't:**
```
desktop/src/
  utils/
    mod.rs      # Old style
    helper.rs
```

**✅ Do:**
```
desktop/src/
  utils.rs      # Module declaration
  utils/
    helper.rs   # Submodule
```

**Why:** Better IDE navigation, clearer hierarchy, Rust 2018+ convention.

### Split Large Modules by Responsibility

**When a module grows beyond ~300 lines, split it:**

```
desktop/src/config/
  ├── config.rs       # Entry point (re-exports only)
  ├── types.rs        # Type definitions
  ├── persistence.rs  # File I/O
  └── getters.rs      # Business logic
```

**Pattern for entry point:**
```rust
// config.rs - NO implementation, only re-exports
mod types;
pub use types::*;

mod persistence;
pub use persistence::CONFIG;

mod getters;
pub use getters::{get_startup_theme, get_new_window_theme};
```

### Avoid Over-Abstraction

**❌ Don't create separate modules for closely related functionality:**

```
desktop/src/
  session/        # Separate module for Session
    types.rs
    persistence.rs
  state.rs        # AppState
```

**Problem:** `Session` is just `AppState` persistence → unnecessary abstraction.

**✅ Do group related functionality:**

```
desktop/src/state/
  ├── types.rs        # AppState, Tab, etc.
  ├── globals.rs      # Global variables
  └── persistence.rs  # PersistedState (subset of AppState)
```

**Lesson:** If module B only exists to persist module A's data, merge them.

---

## Serde & Serialization

### Apply #[serde(default)] at the Right Level

**❌ Don't add to every field:**
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct DirectoryConfig {
    #[serde(default)]  // Redundant!
    pub default_directory: Option<PathBuf>,
    #[serde(default)]  // Redundant!
    pub on_startup: StartupBehavior,
}
```

**✅ Do add only at top-level:**
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]  // ✓ Allows missing "directory" section
    pub directory: DirectoryConfig,
}

// DirectoryConfig doesn't need field-level #[serde(default)]
#[derive(Default, Serialize, Deserialize)]
pub struct DirectoryConfig {
    pub default_directory: Option<PathBuf>,  // Gets default from ::default()
    pub on_startup: StartupBehavior,
}
```

**Why:**
- When `Config.directory` is missing → `DirectoryConfig::default()` provides all field defaults
- When `directory.onStartup` is missing → deserialization fails (catches typos!)

**Exception: Custom defaults**
```rust
#[serde(default = "default_sidebar_width")]  // ✓ Non-zero default
pub default_width: f64,
```

### Use snake_case for Enums, camelCase for Structs

**Consistent naming conventions:**

```rust
// Enums: snake_case in JSON
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupBehavior {
    Default,      // → "default"
    LastClosed,   // → "last_closed"
}

// Structs: camelCase in JSON (JavaScript convention)
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub default_directory: Option<PathBuf>,  // → "defaultDirectory"
    pub on_startup: StartupBehavior,         // → "onStartup"
}
```

### Prefer Derive(Default) Over Manual Implementation

**❌ Don't:**
```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            default_directory: None,
            on_startup: StartupBehavior::Default,
        }
    }
}
```

**✅ Do:**
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub default_directory: Option<PathBuf>,
    #[default]
    pub on_startup: StartupBehavior,
}
```

**Why:** Less boilerplate, auto-updates when adding fields.

### Simplify Function Signatures with Structs

**❌ Don't pass many individual parameters:**
```rust
pub fn save_sync(
    directory: Option<PathBuf>,
    theme: Option<Theme>,
    sidebar_visible: Option<bool>,
    sidebar_width: Option<f64>,
    show_all_files: Option<bool>,
)
```

**✅ Do accept a struct:**
```rust
pub fn save_sync(state: &PersistedState)
```

**Benefits:**
- Easier to call: `state.save_sync()` vs `save_sync(a, b, c, d, e)`
- Scalable: adding fields doesn't change signature
- Self-documenting: field names provide context

---

## Dioxus Patterns

### Choose the Right Async Primitive

```rust
// ✓ spawn() - One-time async task (event handlers)
let handle_click = move |_| {
    spawn(async move {
        let data = fetch().await;
        state.set(data);
    });
};

// ✓ use_effect() - React to state changes
use_effect(move || {
    spawn(async move {
        let data = load_file(&path).await;
        content.set(data);
    });
});

// ✓ spawn_forever() - Infinite event loop (NEVER returns)
use_effect(move || {
    let mut rx = BROADCAST.subscribe();
    spawn_forever(async move {
        while let Ok(event) = rx.recv().await {
            handle(event);
        }
    });
});

// ✓ use_drop() - Cleanup (synchronous only!)
use_drop(move || {
    persisted_state.save_sync();  // Blocking is OK here
    close_resources();
});
```

**Critical:** `spawn_forever()` is for infinite loops that should never block. Use for broadcast channel listeners.

### Dynamic Text in RSX

**❌ Don't:**
```rust
rsx! {
    span { "Version {}", env!("CARGO_PKG_VERSION") }  // Doesn't work!
}
```

**✅ Do:**
```rust
let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
rsx! {
    span { "{version}" }
}
```

### Use asset!() for Static Resources

**✅ Do:**
```rust
const ICON: Asset = asset!("/assets/app-icon.png");

rsx! {
    img { src: "{ICON}", alt: "App" }
}
```

**Why:** Assets are bundled at build time, paths are resolved automatically.

---

## State Management

### Three-Tier State Hierarchy

**1. Global Statics** (app-wide, shared across windows):
```rust
pub static CONFIG: LazyLock<RwLock<Config>> = ...;
// WINDOW_STATES mapping in window/main.rs (thread_local)
pub static TRANSFER_TAB_TO_WINDOW: LazyLock<broadcast::Sender<...>> = ...;
```

**2. Context-Provided State** (per-window):
```rust
#[component]
fn App() -> Element {
    let state = use_context_provider(|| {
        let app_state = AppState::new(theme);
        crate::window::register_window_state(window().id(), app_state);
        app_state
    });
    // Children access via use_context::<AppState>()
}
```

**3. Local Component State** (UI-only):
```rust
let mut expanded = use_signal(|| false);
let mut input_value = use_signal(String::new);
```

**Decision tree:**
- Shared across windows? → Global static or WINDOW_STATES mapping
- Shared within window? → Context
- Component-only? → use_signal

### Separate Last Closed vs Last Focused

**Use different access patterns for different behaviors:**

```rust
// Last CLOSED window (persisted to disk) - for startup
let persisted = PersistedState::load();  // From state.json

// Last FOCUSED window (direct access) - for new window
let theme = get_last_focused_window_state()
    .map(|state| *state.current_theme.read())
    .unwrap_or_else(|| PersistedState::load().theme);
```

**Why:**
- Startup: restore state from when app last quit (state.json)
- New window: access last focused window's AppState directly via WINDOW_STATES mapping

---

## File Operations

### Always Canonicalize Paths

**✅ Do:**
```rust
let canonical_path = path.canonicalize()?;
```

**Why:** macOS Finder aliases and symlinks must be resolved to real paths.

### Extract Directory Root Properly

**✅ Do:**
```rust
let root = if path.is_file() {
    path.parent().unwrap_or(&path).to_path_buf()
} else {
    path.clone()
};
```

**Why:** For files, use parent directory as sidebar root.

### Use Thread-Local for File Watcher

**✅ Do:**
```rust
thread_local! {
    pub static FILE_WATCHER: RefCell<Option<FileWatcher>> =
        RefCell::new(Some(FileWatcher::new()));
}

FILE_WATCHER.with(|watcher| {
    watcher.borrow_mut().as_mut().unwrap().watch(path, tx)
});
```

**Why:** Avoids Send/Sync issues with Dioxus.

---

## Configuration Design

### Dual-File Configuration System

**Separate user config from app state:**

```
~/Library/Application Support/arto/
├── config.json   # User preferences (edited manually or via UI)
└── state.json    # Session state (auto-saved on window close)
```

**config.json:**
- Default values
- Behavior settings (startup, new window)
- User-controlled

**state.json:**
- Last used directory
- Last used theme
- Last window settings
- Auto-managed by app

### Config Behavior Pattern

**Two behavior modes for each setting:**

```rust
pub enum StartupBehavior {
    Default,      // Use config.default_theme
    LastClosed,   // Use persisted.theme
}

pub enum NewWindowBehavior {
    Default,      // Use config.default_theme
    LastFocused,  // Use LAST_SELECTED_THEME (in-memory)
}
```

**Implementation pattern:**
```rust
pub fn get_startup_theme() -> Theme {
    let config = CONFIG.read();
    let persisted = PersistedState::load();

    match config.theme.on_startup {
        StartupBehavior::Default => config.theme.default_theme,
        StartupBehavior::LastClosed => persisted.theme,
    }
}
```

### File Naming Consistency

**Match file names to type names:**

| File | Type | Purpose |
|------|------|---------|
| config.json | Config | User preferences |
| state.json | PersistedState | Last window state |

**Why:** Easier to understand what each file contains.

---

## Code Quality

### Avoid Unused Imports

**Remove unused imports immediately:**

```rust
// ❌ Don't leave unused imports
use crate::theme::Theme;  // warning: unused import

// ✓ Remove or use them
```

**Run:** `cargo fix` to auto-remove.

### Write English Comments

**✅ Do:**
```rust
// Convert relative images to data URLs for offline support
element!("img[src]", |el| { ... });
```

**Why:** Maintains consistency with the codebase.

### Use TODO Comments Sparingly

**✅ Do make them specific and actionable:**
```rust
// TODO: Add keyboard shortcut for quick open (Cmd+P)
```

**❌ Don't leave vague TODOs:**
```rust
// TODO: Fix this
// TODO: Improve performance
```

---

## Anti-Patterns to Avoid

### 1. Over-Engineering

**❌ Don't:**
- Add abstractions for one-time operations
- Create helpers for code used only once
- Add error handling for impossible scenarios
- Design for hypothetical future requirements

**✅ Do:**
- Keep it simple
- Only add complexity when needed
- Trust internal code and framework guarantees

### 2. Premature Abstraction

**❌ Don't:**
```rust
// Creating abstraction for 3 similar lines
fn save_to_file(data: &str, path: &Path) -> Result<()> {
    std::fs::write(path, data)?;
    Ok(())
}
```

**✅ Do:**
```rust
// Just write it inline (3 times is fine)
std::fs::write(&path, &data)?;
```

**Rule:** Three instances of similar code is better than a premature abstraction.

### 3. Backwards-Compatibility Hacks

**❌ Don't:**
- Rename unused variables to `_var`
- Re-export removed types
- Add `// removed` comments
- Keep dead code

**✅ Do:**
- Delete unused code completely
- Clean up when refactoring
- Trust version control for history

### 4. Excessive Logging

**❌ Don't:**
```rust
tracing::debug!("Entering function");
tracing::debug!("Processing item: {:?}", item);
tracing::debug!("Exiting function");
```

**✅ Do:**
```rust
tracing::debug!("Processing batch of {} items", items.len());
```

**Use sparingly:** Only log when information would be lost otherwise.

### 5. Over-Engineering Local Communication

**❌ Don't:**
- Add timeout/retry logic for in-process broadcast channels (window-to-window)
- Use request/response patterns for fire-and-forget operations
- Implement acknowledgment systems for desktop app window communication

**Note:** This refers to broadcast channels between windows WITHIN the same process. For inter-process IPC (secondary → primary instance), see `desktop/src/ipc.rs`.

**✅ Do:**
```rust
// Simple fire-and-forget for local tab transfer
crate::events::TRANSFER_TAB_TO_WINDOW
    .send((target_window_id, index, tab))
    .ok();
```

**Why:** Desktop apps have different requirements than distributed systems:
- No network latency (same process, broadcast channel)
- No need for timeout/retry (if window exists, it will receive the event)
- Simpler is better (removed ~150 lines of Two-Phase Commit logic)

**Real example:** Tab transfer previously used `TabTransferRequest`/`TabTransferResponse` with UUID tracking and timeout handling. Simplified to single broadcast channel `TRANSFER_TAB_TO_WINDOW`, reducing complexity without losing functionality (tab history still preserved by sending full `Tab` object).

---

## Quick Reference

### Module Split Decision Tree

```
Is module > 300 lines?
├─ Yes: Split by responsibility
│   ├─ types.rs (type definitions)
│   ├─ persistence.rs (I/O)
│   └─ business logic files
└─ No: Keep as single file
```

### State Storage Decision Tree

```
Where should this state live?
├─ Shared across windows? → Global static
├─ Shared within window? → Context (AppState)
├─ Persisted across sessions? → PersistedState
└─ Component-only? → use_signal
```

### Configuration Decision Tree

```
Adding new setting?
├─ User edits it? → config.json (Config type)
├─ App auto-saves it? → state.json (PersistedState type)
├─ Per-window only? → AppState (not persisted)
└─ Temporary? → use_signal
```

---

## Testing Your Understanding

**Quiz: Where should this state live?**

1. User's preferred default theme → `Config` (config.json)
2. Last opened file in current window → `AppState` (not persisted)
3. Theme of the last closed window → `PersistedState` (state.json)
4. Currently expanded directories in sidebar → `AppState` (not persisted)
5. Last focused directory (for new window) → Access via `WINDOW_STATES` mapping

**Quiz: Should I use #[serde(default)]?**

1. Top-level `Config` fields → Yes (allows missing sections)
2. Nested `DirectoryConfig` fields → No (parent's default covers it)
3. Field with custom default value → Yes (with `= "fn_name"`)
4. Enum variant → No (use `#[default]` on the variant itself)

---

## Lessons Learned

### Session Module Was Unnecessary

**Initial design:**
- Separate `session/` module for `Session` type
- Global `SESSION` variable with locks
- Complex synchronization logic

**Realization:**
- `Session` was just a subset of `AppState`
- No need for separate module
- Can load directly from file instead of maintaining in-memory copy

**Refactoring:**
- Merged into `state/persistence.rs`
- Renamed `Session` → `PersistedState`
- Removed global `SESSION` variable
- Simplified: `PersistedState::load()` instead of `SESSION.lock().await`

**Lesson:** Don't create separate modules for closely related functionality. If module B exists only to persist module A, merge them.

### #[serde(default)] Placement

**Initial approach:**
- Added `#[serde(default)]` to every field
- Assumed it was always needed for backward compatibility

**Realization:**
- Only needed at top-level config structs
- Nested structs get defaults from parent's `::default()`
- Field-level attributes add noise without benefit

**Lesson:** Understand serde's default mechanism before sprinkling attributes everywhere.

### Function Signatures

**Initial approach:**
- 5 individual `Option<T>` parameters
- Caller had to wrap each value in `Some()`

**Realization:**
- Can construct struct directly at call site
- Cleaner, more maintainable
- Easier to add fields later

**Lesson:** When passing >3 related parameters, use a struct.

---

## Session: 2025-12-20 15:00

### State Refactoring Insights

**Moving fields between structs requires careful tracking:**
- When moving `root_directory` from `Sidebar` to `AppState.directory`, update ALL usage sites
- Boolean field renames with logic inversion (`hide_non_markdown` → `show_all_files`) are especially error-prone
- Use grep to find all occurrences: `grep -r "hide_non_markdown" desktop/src/`
- Critical: Inverted logic means `true` → `false` and vice versa in conditions

**Simplifying field names:**
- `PersistedState` fields don't need `last_` prefix when the struct name already indicates "persisted state"
- Better: `directory`, `theme`, `sidebar_open` than `last_directory`, `last_theme`, `last_sidebar_visible`
- Field names should be concise when context is clear from parent struct

### Eliminating Unnecessary Code

**Remove immediately-overwritten values:**
```rust
// ❌ Bad - Sets value that's immediately overwritten
if let Some(parent) = path.parent() {
    *app_state.directory.write() = Some(parent.to_path_buf());
}
// ...
*app_state.directory.write() = Some(initial_directory);  // ← Overwrites above
```

**Remove unnecessary Option wrappers:**
```rust
// ❌ Bad - Unnecessary intermediate validation
let directory_value = get_directory_value(is_first_window);  // Returns PathBuf
let initial_directory = if directory_value.directory.is_dir() {
    Some(directory_value.directory)
} else {
    None
};

// ✅ Good - Use PathBuf directly (resolve_directory already handles fallback)
let directory_value = get_directory_value(is_first_window);
*app_state.directory.write() = Some(directory_value.directory);
```

**Remove intermediate variables for simple field access:**
```rust
// ❌ Bad - Unnecessary intermediate variables
let sidebar_value = get_sidebar_value(is_first_window);
let initial_sidebar_visible = sidebar_value.open;
let initial_sidebar_width = sidebar_value.width;
AppProps { initial_sidebar_visible, initial_sidebar_width, ... }

// ✅ Good - Direct field access
let sidebar_value = get_sidebar_value(is_first_window);
AppProps {
    initial_sidebar_visible: sidebar_value.open,
    initial_sidebar_width: sidebar_value.width,
    ...
}
```

### Serde Optimization

**When #[serde(default)] is unnecessary:**
- Struct has `derive(Default)`
- Deserialization uses `unwrap_or_default()`
- → Field-level `#[serde(default)]` attributes are redundant

```rust
// ❌ Redundant field-level defaults
#[derive(Default, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]  // ← Unnecessary
    pub directory: Option<PathBuf>,
    #[serde(default)]  // ← Unnecessary
    pub theme: Theme,
}

// Usage: serde_json::from_str(&content).unwrap_or_default()
// → Missing fields cause deserialization error → entire struct uses ::default()

// ✅ Clean - Only struct-level Default needed
#[derive(Default, Serialize, Deserialize)]
pub struct PersistedState {
    pub directory: Option<PathBuf>,
    pub theme: Theme,
}
```

### Broadcast Channel Architecture

**Broadcast channels for cross-window coordination (`events.rs`):**

- `TRANSFER_TAB_TO_WINDOW` - Tab drag-and-drop, context menu "Move to Window"
- `ACTIVE_DRAG_UPDATE` - Drag state updates for visual feedback
- `OPEN_FILE_IN_WINDOW` / `OPEN_DIRECTORY_IN_WINDOW` - Context menu "Open in Window"

**OS events (Finder/CLI) always create new windows** - handled via IPC and `main_app.rs`:
- Secondary instances send paths via IPC socket → primary instance receives via `OPEN_EVENT_RECEIVER`
- Primary instance OS events (Finder) handled directly in `main_app.rs`

#### Tab Transfer Between Windows

**Fire-and-forget pattern for tab transfers (drag-and-drop and context menu):**

```rust
// Transfer a tab with full history preservation
crate::events::TRANSFER_TAB_TO_WINDOW
    .send((target_window_id, target_index, tab))
    .ok();

// Auto-focus target window after transfer
crate::window::main::focus_window(target_window_id);
```

**Why fire-and-forget:**
- Desktop app with in-process broadcast channels (no network latency)
- No need for acknowledgment/timeout logic
- Simpler than request/response pattern (~150 lines removed)
- Target window subscribes and handles tab insertion directly

**Pattern usage:**
- Drag-and-drop tab between windows
- Context menu "Move to Window" (tab context menu)
- Context menu "Open in Window" (sidebar file tree)

**Event flow:**
```
Source Window                Target Window
     ├──→ TRANSFER_TAB_TO_WINDOW.send()
     ├──→ focus_window()
     └──→ close_tab()          └──→ insert_tab() + switch_to_tab()
```

**Listener in target window:**
```rust
use_future(move || async move {
    let mut rx = crate::events::TRANSFER_TAB_TO_WINDOW.subscribe();
    let current_window_id = window().id();

    while let Ok((target_window_id, target_index, tab)) = rx.recv().await {
        if target_window_id != current_window_id {
            continue;
        }

        let tabs_len = state.tabs.read().len();
        let insert_index = target_index.unwrap_or(tabs_len);
        let new_tab_index = state.insert_tab(tab, insert_index);
        state.switch_to_tab(new_tab_index);
        window().set_focus();
    }
});
```

### Window State Registration Pattern

**Register AppState in WINDOW_STATES during window creation:**
```rust
// ✅ Good - Register state during context provider setup
let mut state = use_context_provider(|| {
    let app_state = AppState::new(theme);
    crate::window::register_window_state(window().id(), app_state);
    app_state
});

// ✅ Good - Unregister on window close
use_drop(move || {
    crate::window::unregister_window_state(window_id);
    // ... save persisted state ...
});
```

**Why this pattern:**
1. `WINDOW_STATES` provides direct access to any window's live AppState
2. No synchronization code needed - Signals are Arc-based, changes are visible immediately
3. New windows can read from last focused window's AppState directly

### Module Placement by Usage Scope

**Place code where it's actually used, not where it seems conceptually related:**

**Anti-pattern: Grouping by concept**
```
desktop/src/state/globals.rs    # ← Contains event channels because "global"
```

**Good pattern: Grouping by usage scope**
```
desktop/src/events.rs                    # ← Broadcast channels (multiple files)
desktop/src/components/main_app.rs       # ← OpenEvent + mpsc receiver (2 files only)
```

**Decision tree for placement:**

```
How many files use this code?
├─ 2 files only
│  └─ Define in one of the two files
│     Example: OpenEvent in main_app.rs (main.rs ↔ main_app.rs)
│
└─ 3+ files
   └─ Create independent module
      Example: TRANSFER_TAB_TO_WINDOW in events.rs (used by tab_item.rs, tab_bar.rs, app.rs)
```

**Insight**: Don't group by "what it is" (globals, events, state). Group by "where it's used" (2 files vs many files).

### Sidebar Tree Interaction Patterns

**Split click areas for intuitive directory navigation:**

Following browser file-tree conventions (VS Code, Chrome DevTools):
- **Chevron icon**: Click to expand/collapse directory
- **Folder icon + label**: Click to set directory as sidebar root
- **File icon + label**: Click to open file in tab
- **Full row**: Falls back to default action (open file / set root)

**Implementation pattern:**
```rust
// Parent row: default click handler
div {
    class: "sidebar-tree-node-content",
    onclick: move |_| {
        if is_dir {
            state.set_root_directory(&path);
        } else {
            state.open_file(&path);
        }
    },

    // Chevron: toggle expansion (stops propagation)
    if is_dir {
        span {
            class: "sidebar-tree-chevron-wrapper",
            onclick: move |e| {
                e.stop_propagation();  // Don't trigger row click
                state.toggle_directory_expansion(&path);
            },
            Icon { name: if is_expanded { ChevronDown } else { ChevronRight } }
        }
    }

    // Folder/file link (optional override, can be same as row click)
    span {
        class: "sidebar-tree-file-link",
        onclick: move |e| {
            e.stop_propagation();
            state.open_file(&path);
        },
        Icon { name: File }
        span { "{name}" }
    }
}
```

**Why this pattern:**
- Full row is clickable (better UX than small click targets)
- Chevron stops propagation to prevent accidental navigation
- Consistent with user expectations from other file explorers
- Allows clicking on padding/whitespace to trigger default action

**CSS requirements:**
- Fixed row height (`26px`) to prevent layout shift
- Full row hover background for visual feedback
- Separate hover states for chevron vs folder/file areas (optional)

### Layer-Based Architecture Validation

**When designing multi-layer systems, validate each layer independently:**

**Questions to ask:**
1. What are the communication boundaries? (OS/Dioxus, component/component)
2. Who are the senders/receivers? (1:1 vs 1:N)
3. Where should the channel live? (Closer to the unique side)

**Pattern:**
- 1:1 communication → `mpsc` or direct function calls
- 1:N communication → `broadcast` channel
- Don't mix layers in the same module just because they're "event-related"

---

---

## Session: 2025-12-21 16:45

### Theme Selector Slide Dropdown Implementation

**UI Design Patterns:**
- Transform CSS for vertical-only slide: Use `translateX(-50%)` for centering in BOTH initial and expanded states, only animate `translateY(-8px)` → `translateY(0)`
- Faint icons by default: `opacity: 0.5` for inactive state, `opacity: 1` on hover and when expanded (`aria-expanded="true"`)
- Icon choice: `sun-moon` (Tabler Icons) is ideal for Auto theme (system follows) instead of generic `contrast-2`

**Dioxus + JavaScript Integration:**
- **Critical**: In `document::eval()`, JavaScript Promises MUST use `await`: `await new Promise((resolve) => {...})`
- Without `await`, the JavaScript Promise isn't waited for - execution continues without waiting for the event
- Pattern for outside-click detection:
  ```rust
  let _ = document::eval(r#"
      await new Promise((resolve) => {
          const handler = (e) => {
              if (condition) resolve();
          };
          window.addEventListener('event', handler, { once: true });
      })
  "#).await;
  ```

**User Feedback:**
- Don't change specs without permission: When debugging, stick to original requirements (user said "don't change specs arbitrarily" when ESC key close was suggested)
- Avoid over-engineering: Simple solutions preferred over complex abstractions (user said "too much ripple effect" when global signals were added)
- Root cause analysis matters: User correctly identified "simple issue - just missing await" after multiple wrong attempts at fixing

**Component State Management:**
- Use `use_effect()` to setup event listeners once, not on every state change
- Use `postMessage` for JavaScript → Rust communication when dealing with async events
- Keep external click detection simple: one listener setup, one message listener loop

---

## Session: 2025-12-21 16:30

### Eliminating Background Window Workaround

- **Problem Solved**: Removed 1x1 hidden background window pattern that was a workaround for Dioxus v0.6 issues
- **Solution**: MainApp component now renders the first visible window directly, not a background window
- **Key Change**: First window uses MainApp (with system event handling), additional windows use App directly
- **Critical Fix**: Must register first window in `MAIN_WINDOWS` list via `register_main_window()` or `has_any_main_windows()` will always return false

### Initialization Order Bugs

- **Channel Creation**: Must create tokio mpsc channel BEFORE trying to read from receiver (obvious but easy to miss during refactoring)
- **Event Timing**: Initial OS events (file open from Finder) arrive BEFORE Dioxus starts, so must consume them inside component after launch, not in `main()`
- **Window Registration**: First window must be registered in `MAIN_WINDOWS` for window counting logic to work

### Dioxus Signal Lifetime Issues

- **spawn_forever Warning**: Using `spawn_forever` with component-scoped signals causes lifetime warnings because task outlives component
- **Solution**: Use `use_hook` + `spawn` instead - task is automatically cancelled when component drops
- **When spawn_forever IS correct**: Only for app-lifetime components like MainApp that live until app quits
- **Pattern**: `use_effect` + `spawn` for reactive listeners, `use_hook` + `spawn` for one-time infinite loops

### JavaScript Initialization

- **Problem**: `init()` in renderer/src/main.ts only called when opening files, not on window creation
- **Fix**: Call `init()` in App component's `use_hook` to ensure theme listeners are registered in all windows
- **Cleanup**: Remove redundant `init()` calls from FileViewer and InlineViewer (DRY principle)
- **Result**: 90 lines of code removed by centralizing initialization

### Review Process

- **User Expectation**: When rebasing, don't just delete conflicting files - check what changes were in origin and ensure they're migrated
- **Copilot Comments**: GitHub Copilot leaves inline code comments, not just PR summaries - must query API differently to find them
- **Critical vs Nitpicks**: Address critical issues (initialization bugs, missing features) immediately; batch low-priority fixes (typos, alphabetization) with fixup commits

### Code Simplification Philosophy

- **User Feedback**: "複雑化しすぎ" (too complex) - always prefer simple solutions over elaborate architectures
- **Example**: Instead of global broadcast channels + polling + multiple spawns, just use `use_hook` + `spawn` with simple event listener
- **Principle**: Only add complexity when simple approach genuinely doesn't work

---

## Further Reading

- **CLAUDE.md**: Full architectural patterns and guidelines
- **config-module.md**: Configuration module organization patterns
- **serde-patterns.md**: Detailed serde optimization strategies
- **architecture-overview.md**: Understanding config/state/persistence layers
