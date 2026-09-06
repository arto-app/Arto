---
paths: "crates/arto-config/**, crates/arto-keybindings/**, crates/arto-ipc/**, crates/arto/src/state/persistence.rs, crates/arto-markdown/src/options.rs"
---

# Serde Optimization Patterns

Best practices for using serde in Rust applications, especially for configuration and state management.

## Naming Conventions

### Enums - Use snake_case

**Prefer `snake_case` over manual renaming:**

```rust
// Bad - Requires manual #[serde(rename = "...")] for each variant
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    Default,
    #[serde(rename = "last_closed")]  // ← Manual renaming needed
    LastClosed,
}

// Good - Auto-converts to snake_case
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behavior {
    Default,        // → "default"
    LastClosed,     // → "last_closed" (automatic)
    LastFocused,    // → "last_focused" (automatic)
}
```

### Structs - Use camelCase for JSON

**Follow JavaScript/JSON conventions with camelCase:**

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub default_directory: Option<PathBuf>,  // → "defaultDirectory"
    pub on_startup: StartupBehavior,         // → "onStartup"
    pub on_new_window: NewWindowBehavior,    // → "onNewWindow"
}
```

**This produces clean JSON:**

```json
{
  "defaultDirectory": "/path/to/dir",
  "onStartup": "default",
  "onNewWindow": "last_focused"
}
```

## Default Implementation

### Prefer derive(Default) over impl Default

**Bad - Manual implementation:**

```rust
pub struct DirectoryConfig {
    pub default_directory: Option<PathBuf>,
    pub on_startup: StartupBehavior,
    pub on_new_window: NewWindowBehavior,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            default_directory: None,
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
```

**Good - Derive Default:**

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryConfig {
    pub default_directory: Option<PathBuf>,
    pub on_startup: StartupBehavior,
    pub on_new_window: NewWindowBehavior,
}
```

**Benefits:**
- Less boilerplate code
- Auto-updates when adding fields
- Parent struct can use `#[serde(default)]` to provide backward compatibility

### Enum Default Variant

**Mark the default variant explicitly:**

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupBehavior {
    #[default]  // ← Explicit default marker
    Default,
    LastClosed,
}
```

### Custom Default Values

**When you need non-zero defaults in deserialization, use helper functions:**

```rust
fn default_sidebar_width() -> f64 {
    280.0
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarConfig {
    pub default_open: bool,  // → false (Rust default)

    #[serde(default = "default_sidebar_width")]
    pub default_width: f64,  // → 280.0 when field is missing in JSON
}
```

## Field-Level Attributes

### Use #[serde(default)] at the Right Level

**Apply `#[serde(default)]` strategically based on deserialization context:**

#### Top-Level Config Structs

**Add `#[serde(default)]` for optional nested structs:**

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]  // ✓ Needed: allows "directory" field to be missing
    pub directory: DirectoryConfig,

    #[serde(default)]  // ✓ Needed: allows "theme" field to be missing
    pub theme: ThemeConfig,

    #[serde(default)]  // ✓ Needed: allows "sidebar" field to be missing
    pub sidebar: SidebarConfig,
}
```

**Why:** Provides backward compatibility when new top-level sections are added to config files.

#### Nested Structs

**SKIP `#[serde(default)]` on fields if the struct has `#[derive(Default)]`:**

```rust
// DirectoryConfig is already referenced with #[serde(default)] in Config
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryConfig {
    pub default_directory: Option<PathBuf>,  // ✓ No #[serde(default)] needed
    pub on_startup: StartupBehavior,         // ✓ No #[serde(default)] needed
    pub on_new_window: NewWindowBehavior,    // ✓ No #[serde(default)] needed
}
```

**Why:**
- When `Config.directory` is missing → `DirectoryConfig::default()` is used → all fields get defaults
- When `directory.onStartup` is missing → deserialization fails (intentional: catch typos)
- Less attribute noise in code

#### Exception: Custom Default Values

**Use `#[serde(default = "...")]` for non-zero defaults:**

```rust
fn default_sidebar_width() -> f64 {
    280.0
}

#[derive(Default, Serialize, Deserialize)]
pub struct SidebarConfig {
    pub default_open: bool,  // ✓ Uses Rust default (false)

    #[serde(default = "default_sidebar_width")]  // ✓ Custom default
    pub default_width: f64,

    pub show_all_files: bool,  // ✓ Uses Rust default (false)
}
```

**Why:** The custom default function provides a non-zero value when the field is missing.

### Decision Tree

```
Is this field in a top-level config struct (e.g., Config)?
├─ Yes: Use #[serde(default)] (allows entire section to be missing)
└─ No: Is the field type already #[derive(Default)]?
   ├─ Yes: SKIP #[serde(default)] (parent's default covers it)
   └─ No: Does it need a custom default?
      ├─ Yes: Use #[serde(default = "helper_fn")]
      └─ No: Use #[serde(default)]
```

## Common Patterns

### Copy Types in Options

**For Copy types, avoid unnecessary cloning:**

```rust
// Bad - Unnecessary clone for Copy type
if let Some(ref t) = theme {
    state.last_theme = Some(t.clone());
}

// Good - Just copy
if let Some(t) = theme {
    state.last_theme = Some(t);
}
```

### Enum Serialization

**Test your enum serialization format:**

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Auto,   // → "auto"
    Light,  // → "light"
    Dark,   // → "dark"
}

#[test]
fn test_theme_serialization() {
    let theme = Theme::Auto;
    let json = serde_json::to_string(&theme).unwrap();
    assert_eq!(json, r#""auto""#);
}
```

## Migration Strategy

**When changing field names or formats:**

1. Backup existing config files
2. Use `perl` for batch text conversion (see project rules)
3. Convert snake_case → camelCase in one operation

```bash
# Example migration script
config_path="$HOME/Library/Application Support/app/config.json"
cp "$config_path" "$config_path.backup"

perl -i -pe '
s/"default_directory"/"defaultDirectory"/g;
s/"on_startup"/"onStartup"/g;
s/"on_new_window"/"onNewWindow"/g;
' "$config_path"
```

## Summary

1. **Enums**: Use `#[serde(rename_all = "snake_case")]`
2. **Structs**: Use `#[serde(rename_all = "camelCase")]` for JSON
3. **Default**: Prefer `derive(Default)` over manual `impl Default`
4. **Field Attributes**: Use `#[serde(default)]` at top-level config structs, skip on nested struct fields
5. **Enums**: Use type-safe enums instead of String for fixed values
6. **Testing**: Always test JSON serialization format
