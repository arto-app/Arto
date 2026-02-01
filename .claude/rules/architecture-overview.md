# Architecture Overview: Config, PersistedState, State

Understanding the relationship between Config, PersistedState, and State modules.

## Single-Instance Enforcement

**Arto runs as a single process enforced via IPC:**

- First launch → Becomes primary instance, starts IPC server
- Subsequent launches → Connect to primary, send paths via JSON Lines, exit(0)
- Primary instance receives paths via IPC server → Opens files/directories in existing windows

**IPC Protocol:** Unix domain socket (`com.lambdalisue.arto.sock`) with JSON Lines messages

**Why:** Prevents multiple processes from conflicting over file watches, config writes, and state persistence.

**Implementation:** See `desktop/src/ipc.rs` for detailed documentation.

**Note:** This document focuses on the state management within the single running instance.

## Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ User Configuration Layer (config/)                          │
│ - File: config.json                                         │
│ - Edited by: User (manual or via Preferences UI)           │
│ - Contains: Default values, behavior settings               │
│ - Example: "default_theme": "auto"                          │
│           "on_startup": "last_closed"                       │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ State Persistence Layer (state/persistence.rs)              │
│ - File: state.json                                          │
│ - Edited by: App (auto-saved on window close)              │
│ - Contains: Last closed window's state (PersistedState)     │
│ - Example: "theme": "dark"                                  │
│           "directory": "/path/to/project"                   │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Runtime State Layer (state.rs)                              │
│ - File: None (memory only)                                  │
│ - Scope: Per-window                                         │
│ - Contains: Current UI state (tabs, zoom, sidebar, etc.)   │
│ - Example: tabs: [Tab1, Tab2], active_tab: 0               │
│           sidebar.is_visible: true                          │
└─────────────────────────────────────────────────────────────┘
```

## Module Responsibilities

### 1. Config Module (`desktop/src/config/`)

**Purpose:** User preferences and default values

**Files:**
- `config.json` - Stored in `~/Library/Application Support/arto/` (macOS)

**Example content:**
```json
{
  "theme": {
    "defaultTheme": "auto",
    "onStartup": "last_closed",
    "onNewWindow": "last_focused"
  },
  "directory": {
    "defaultDirectory": "/Users/alice/Documents",
    "onStartup": "default",
    "onNewWindow": "last_focused"
  },
  "sidebar": {
    "defaultOpen": true,
    "defaultWidth": 280.0,
    "defaultShowAllFiles": false
  }
}
```

**Key type:** `Config`

**When used:**
- On app startup (to determine default behavior)
- When user opens Preferences and saves changes
- As fallback when state.json doesn't exist

### 2. State Persistence (`desktop/src/state/persistence.rs`)

**Purpose:** Remember the last closed window's state for restoration

**Files:**
- `state.json` - Stored in `~/Library/Application Support/arto/` (macOS)

**Example content:**
```json
{
  "directory": "/Users/alice/project/docs",
  "theme": "dark",
  "sidebarOpen": true,
  "sidebarWidth": 320.0,
  "sidebarShowAllFiles": false,
  "rightSidebarOpen": false,
  "rightSidebarWidth": 280.0,
  "rightSidebarTab": "toc",
  "windowPosition": { "x": 100, "y": 100 },
  "windowSize": { "width": 1200, "height": 800 }
}
```

**Key type:** `PersistedState`

**When used:**
- Saved: When any window closes (`use_drop()` in App component)
- Loaded: On app startup (if user configured `on_startup: "last_closed"`)

### 3. State Module (`desktop/src/state.rs`)

**Purpose:** Current UI state for each window instance

**Storage:** Memory only (never saved to disk)

**Key type:** `AppState`

**Contents:**
```rust
pub struct AppState {
    pub tabs: Signal<Vec<Tab>>,              // Open tabs
    pub active_tab: Signal<usize>,           // Which tab is active
    pub current_theme: Signal<Theme>,        // Current theme
    pub zoom_level: Signal<f64>,             // Zoom level
    pub sidebar: Signal<SidebarState>,       // Sidebar state
}
```

**Lifecycle:**
- Created: When window opens
- Updated: During user interaction (opening files, changing theme, etc.)
- Destroyed: When window closes (after saving to state.json)

## Data Flow

### Startup Flow (Primary Instance)

**Single-instance enforcement happens BEFORE initialization:**

```
0. IPC Check (in main(), before any initialization)
   ├─> Try to connect to existing instance via Unix socket
   ├─> If connection succeeds → Send paths via JSON Lines → exit(0)
   └─> If connection fails → Continue as primary instance

1. Initialize primary instance
   ├─> Load .env, init tracing
   ├─> Create OpenEvent channel (tx/rx)
   ├─> Start IPC server (listens for future instances)
   └─> Send CLI paths as OpenEvents

2. Load config.json
   ├─> Config { theme.on_startup: "last_closed" }
   └─> Config { directory.on_startup: "default" }

3. Load state.json
   ├─> PersistedState { theme: "dark" }
   └─> PersistedState { directory: "/path/to/project" }

4. Apply startup behavior using window::settings helpers
   ├─> Theme: "last_closed" → Use persisted.theme ("dark")
   └─> Directory: "default" → Use config.default_directory

5. Create AppState with computed values
   └─> AppState { current_theme: "dark", sidebar.root_directory: "/Users/alice/Documents" }
```

**Secondary Instance Flow:**

```
0. IPC Check (in main())
   ├─> Try to connect to existing instance
   ├─> Connection succeeds!
   ├─> Send paths: [{"type":"file","path":"/doc.md"}]
   └─> exit(0) immediately (no initialization)

Primary instance receives:
   ├─> IPC server accepts connection
   ├─> Parse JSON Lines messages
   ├─> Convert to OpenEvents
   ├─> Send to OPEN_EVENT_RECEIVER channel
   └─> MainApp component processes events (open file/directory)
```

### New Window Flow (Second+ Window)

```
1. Load config.json
   ├─> Config { theme.on_new_window: "last_focused" }
   └─> Config { directory.on_new_window: "last_focused" }

2. Access last focused window's AppState via WINDOW_STATES
   ├─> get_last_focused_window_state() → Some(AppState)
   ├─> Read current_theme from AppState Signal
   └─> Read sidebar.root_directory from AppState Signal

3. Apply new window behavior using window::settings helpers
   ├─> Theme: "last_focused" → Use AppState.current_theme
   └─> Directory: "last_focused" → Use AppState.sidebar.root_directory
   └─> Fallback: If no focused window, use PersistedState::load()

4. Create AppState with computed values + register in WINDOW_STATES
   └─> AppState { current_theme: <from last focused>, ... }
   └─> register_window_state(window_id, app_state)
```

### Window Close Flow

```
1. Unregister from WINDOW_STATES mapping
   └─> unregister_window_state(window_id)

2. Read current AppState
   ├─> current_theme: "dark"
   ├─> sidebar.root_directory: "/path/to/project"
   ├─> sidebar.is_visible: true
   ├─> sidebar.width: 320.0
   └─> sidebar.show_all_files: false

3. Construct PersistedState
   └─> let mut persisted = PersistedState::from(&state);
       persisted.window_position = window_metrics.position;
       persisted.window_size = window_metrics.size;

4. Save to state.json
   └─> persisted.save();
       → ~/Library/Application Support/arto/state.json
```

## Decision Matrix

**When adding new settings, decide:**

| Question | Config | PersistedState | State |
|----------|--------|----------------|-------|
| Should user be able to edit it? | ✓ | ✗ | ✗ |
| Should it persist between app launches? | ✓ | ✓ | ✗ |
| Is it different per window? | ✗ | ✗ | ✓ |
| Should it restore on startup? | ✓ (default) | ✓ (last used) | ✗ |

**Examples:**

- **Default theme** → Config (user sets preference, applies to all windows)
- **Last used theme** → PersistedState (app remembers, restores on startup)
- **Current theme** → State (per-window, might differ during session)
- **Open tabs** → State only (never saved)
- **Sidebar width** → Config (default), PersistedState (last used), State (current)

## Common Patterns

### Reading a value on startup

```rust
// In window/settings.rs
pub fn get_theme_preference(is_first_window: bool) -> ThemePreference {
    let cfg = CONFIG.read();
    let theme = choose_by_behavior(
        is_first_window,
        cfg.theme.on_startup,
        cfg.theme.on_new_window,
        || cfg.theme.default_theme,
        || {
            // Access last focused window's AppState directly
            get_last_focused_window_state()
                .map(|state| *state.current_theme.read())
                .unwrap_or_else(|| PersistedState::load().theme)
        },
    );
    ThemePreference { theme }
}
```

**Priority:**
1. Check config behavior setting (`on_startup` or `on_new_window`)
2. If "default" → use `config.theme.default_theme`
3. If "last_closed"/"last_focused" → access last focused window's `AppState` directly
4. Fallback → `PersistedState::load()` if no focused window exists

### Updating a value during runtime

```rust
// In component (e.g., ThemeSelector)
let mut state = use_context::<AppState>();

// User changes theme - just update Signal directly
state.current_theme.set(Theme::Dark);

// No synchronization needed!
// WINDOW_STATES mapping holds reference to this AppState
// New windows read directly from last focused window's AppState
```

**Pattern for updating state:**
1. Update `AppState.current_theme` Signal for current window UI
2. No sync code needed - `WINDOW_STATES` mapping provides direct access

**Why no sync needed:** The `WINDOW_STATES` mapping stores `AppState` (which contains `Signal<T>` fields). Signals are Arc-based, so the mapping holds references to the live reactive state. When creating a new window, we read directly from the last focused window's AppState Signals.

### Saving on window close

```rust
// In App component use_drop()

// Unregister from WINDOW_STATES mapping
crate::window::unregister_window_state(window_id);

// Create persisted state from current AppState
let mut persisted = PersistedState::from(&state);

// Capture window metrics from window handle
let window_metrics = crate::window::metrics::capture_window_metrics(&window().window);
persisted.window_position = window_metrics.position;
persisted.window_size = window_metrics.size;

// Save to disk
persisted.save();
```

**What happens:**
1. Unregister from `WINDOW_STATES` mapping (no longer accessible for new windows)
2. Collect values from current `AppState` via `From<&AppState>` trait
3. Capture window metrics from window handle
4. Save to `state.json` (blocking operation)

## WINDOW_STATES Mapping

**Direct access to window state for "last_focused" behavior:**

```rust
// In window/main.rs
thread_local! {
    static WINDOW_STATES: RefCell<HashMap<WindowId, AppState>> = RefCell::new(HashMap::new());
}

pub fn register_window_state(window_id: WindowId, state: AppState) { ... }
pub fn unregister_window_state(window_id: WindowId) { ... }
pub fn get_window_state(window_id: WindowId) -> Option<AppState> { ... }
pub fn get_last_focused_window_state() -> Option<AppState> { ... }
```

**Purpose:** Provide direct access to any window's `AppState` for "new window" behavior

**Key characteristics:**
- Maps `WindowId` → `AppState` for all main windows
- `AppState` is `Copy` (contains `Signal<T>` which are Arc pointers)
- Reading from `AppState` Signals gives current live values
- No synchronization code needed - direct access to reactive state
- Window metrics obtained directly from window handle via `capture_window_metrics()`

**Differences from old LAST_FOCUSED_STATE:**
- **Old design**: Sync state changes to global via `use_effect` hooks
- **New design**: Direct access via `WINDOW_STATES` mapping - no sync needed

**Why this design?**
- Eliminates state duplication (no copy in separate global)
- Removes synchronization overhead (no `use_effect` hooks)
- Simplifies codebase (~100 lines of sync code removed)
- Window metrics read directly from window handle when needed

## Summary

| Aspect | Config | state.json | AppState | WINDOW_STATES |
|--------|--------|-----------|----------|---------------|
| **Type** | `Config` | `PersistedState` | `AppState` | `HashMap<WindowId, AppState>` |
| **Storage** | config.json | state.json | Memory | Memory |
| **Scope** | App-wide | App-wide | Per-window | App-wide mapping |
| **Lifetime** | Permanent | Permanent | Window | App session |
| **Edited by** | User | App | App | App |
| **Updated** | Manual/UI | On close | Real-time | Register/Unregister |
| **Used for** | Defaults | Last closed | Current | Access any window's state |
| **Access pattern** | `CONFIG.read()` | `PersistedState::load()` | `use_context()` | `get_window_state(id)` |
