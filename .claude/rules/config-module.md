# Configuration Module Patterns

Design patterns and best practices for structuring configuration modules in Rust/Dioxus applications.

## Module Organization

**Organize configuration and state into focused modules:**

```
crates/arto/src/
├── config/
│   ├── app_config.rs        # Submodule declarations, Config struct, tests
│   ├── app_config/          # Config type definitions and enums
│   │   ├── behavior.rs
│   │   ├── directory_config.rs
│   │   ├── sidebar_config.rs
│   │   └── theme_config.rs
│   └── persistence.rs       # File I/O and global CONFIG instance
├── state/
│   ├── app_state.rs         # Module entry point (re-exports only)
│   ├── app_state/           # Per-window state types
│   │   ├── sidebar.rs
│   │   └── tabs.rs
│   └── persistence.rs       # PersistedState (disk persistence)
└── window/
    ├── main.rs              # WINDOW_STATES mapping (WindowId → AppState)
    └── settings.rs          # Startup/new window preference resolution
```

### Module Entry Point Pattern

**Entry point files typically declare modules and re-export public APIs:**

```rust
// config/app_config.rs
mod behavior;
mod directory_config;
mod sidebar_config;
mod theme_config;

pub use behavior::{NewWindowBehavior, StartupBehavior};
pub use directory_config::DirectoryConfig;
pub use sidebar_config::SidebarConfig;
pub use theme_config::ThemeConfig;

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub theme: ThemeConfig,
    pub sidebar: SidebarConfig,
}

// Tests can live here too
#[cfg(test)]
mod tests { ... }
```

**Note:** In Arto's case, `app_config.rs` also contains the `Config` struct definition and tests. This is acceptable for configuration entry points. The key principle is to avoid complex business logic in entry point modules.

## Configuration vs State Separation

**Separate user configuration from application state:**

- **config.json** - User preferences (manually edited or via UI)
  - Default values
  - Behavior settings (startup, new window)
  - User-controlled configuration

- **state.json** - Session state (auto-saved on window close)
  - Last used directory
  - Last used theme
  - Last window settings
  - Runtime state

### File Locations

```rust
// Config directory (macOS)
if let Some(mut path) = dirs::config_local_dir() {
    path.push("app-name");
    path.push("config.json");
    return path;
}
```

## Startup vs New Window Pattern

**Use value resolution helpers in window/settings.rs:**

```rust
// window/settings.rs provides unified preference resolution
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

**Usage in window creation:**

```rust
// First window (startup)
let theme = window::settings::get_theme_preference(true);
let directory = window::settings::get_directory_preference(true);
let sidebar = window::settings::get_sidebar_preference(true);

// Subsequent windows
let theme = window::settings::get_theme_preference(false);
let directory = window::settings::get_directory_preference(false);
let sidebar = window::settings::get_sidebar_preference(false);
```

**Key differences:**
- **Startup** (`is_first_window: true`): Uses `PersistedState::load()` (from state.json)
- **New Window** (`is_first_window: false`): Accesses last focused window's `AppState` directly via `WINDOW_STATES` mapping, with fallback to `PersistedState::load()`

## Avoid Duplicate Enums

**Bad - Multiple enums for same concept:**

```rust
pub enum DirectoryStartupBehavior {
    Default,
    LastClosed,
}

pub enum ThemeStartupBehavior {
    Default,
    LastClosed,
}

pub enum SidebarStartupBehavior {
    Default,
    LastClosed,
}
```

**Good - Unified enums:**

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupBehavior {
    #[default]
    Default,
    LastClosed,  // Auto-converted to "last_closed"
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewWindowBehavior {
    #[default]
    Default,
    LastFocused,  // Auto-converted to "last_focused"
}
```

**Use the same enum across all config structs:**

```rust
pub struct DirectoryConfig {
    pub on_startup: StartupBehavior,
    pub on_new_window: NewWindowBehavior,
}

pub struct ThemeConfig {
    pub on_startup: StartupBehavior,      // ✓ Same enum
    pub on_new_window: NewWindowBehavior, // ✓ Same enum
}
```

## Enum vs String

**Use enums for fixed sets of values:**

```rust
// Bad - String allows typos ("ligt", "autoo", etc.)
pub struct ThemeConfig {
    pub default_theme: String,
}

// Good - Type-safe enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,   // → "auto"
    Light,  // → "light"
    Dark,   // → "dark"
}

pub struct ThemeConfig {
    pub default_theme: Theme,
}
```

**Benefits:**
- Type safety (prevents typos)
- Better IDE support
- Self-documenting code
- Easy to refactor
