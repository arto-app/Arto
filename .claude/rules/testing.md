# Dioxus Unit Testing Patterns

Best practices for testing Dioxus applications, from pure logic to component rendering.

Reference: https://dioxuslabs.com/learn/0.7/guides/testing/

## Testing Layers

Dioxus applications can be tested at three layers:

| Layer | Dioxus Dependency | Difficulty | Examples |
|-------|-------------------|------------|----------|
| Pure Logic | None | Easy | `HistoryManager`, `markdown::render_to_html` |
| State Structs | Partial | Medium | `Tab`, `TabContent`, `Config` |
| Hooks / Components | Full | Hard | Custom hooks, `App`, `Sidebar` |

**Priority:** Pure Logic > State Structs > Hooks / Components

## Pure Logic Testing (Recommended)

Pure logic that doesn't depend on Dioxus can be tested with standard Rust tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_navigation() {
        let mut manager = HistoryManager::new();
        manager.push("/file1.md");
        manager.push("/file2.md");

        assert!(manager.can_go_back());
        assert_eq!(manager.go_back(), Some(Path::new("/file1.md")));
    }
}
```

### File System Testing

Use `tempfile` crate to create temporary directories:

```rust
use tempfile::TempDir;
use std::fs;

#[test]
fn test_image_processing() {
    let temp_dir = TempDir::new().unwrap();
    let image_path = temp_dir.path().join("test.png");

    // Create test file
    fs::write(&image_path, vec![0x89, 0x50, 0x4E, 0x47]).unwrap();

    // Test your function
    let result = process_image(&image_path);
    assert!(result.is_ok());
}
```

### Multiline Strings with indoc

Use `indoc` crate for readable test data:

```rust
use indoc::indoc;

#[test]
fn test_markdown_alert() {
    let input = indoc! {"
        > [!NOTE]
        > This is a note
    "};

    let result = process_github_alerts(input);
    assert!(result.contains("markdown-alert-note"));
}
```

## State Struct Testing

Structs without Dioxus Signals can be tested directly:

```rust
#[test]
fn test_tab_content() {
    let tab = Tab::new("/test/file.md");

    assert_eq!(tab.content, TabContent::File(PathBuf::from("/test/file.md")));
    assert!(!tab.is_no_file());
    assert_eq!(tab.display_name(), "file.md");
}
```

### Serialization Roundtrip

Test config structs with JSON serialization roundtrip:

```rust
#[test]
fn test_config_roundtrip() {
    let config = Config {
        theme: ThemeConfig {
            default_theme: Theme::Dark,
            on_startup: StartupBehavior::LastClosed,
            ..Default::default()
        },
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.theme.default_theme, Theme::Dark);
}
```

## Hook Testing (Advanced)

Dioxus does not have a full hook testing library, but you can manually drive the `VirtualDom`.

Reference: https://dioxuslabs.com/learn/0.7/guides/testing/web#hook-testing

### VirtualDom Methods for Testing

| Method | Purpose |
|--------|---------|
| `rebuild(&mut NoOpMutations)` | Initial render, discard DOM mutations |
| `render_immediate_to_vec()` | Render and get mutations as vector |
| `mark_dirty(ScopeId::APP)` | Mark root scope for re-render |
| `wait_for_work()` | Poll pending futures/tasks |
| `wait_for_suspense()` | Wait for suspense boundaries |

### Testing Async Hooks (spawn, use_effect)

```rust
use dioxus::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_spawn_hook() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    fn test_app(counter: Arc<AtomicUsize>) -> Element {
        use_hook(|| {
            let counter = counter.clone();
            spawn(async move {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        });
        rsx! { div {} }
    }

    let mut dom = VirtualDom::new_with_props(
        move || test_app(counter_clone.clone()),
        (),
    );

    // Initial render
    dom.rebuild(&mut dioxus_core::NoOpMutations);

    // Wait for async tasks to complete
    tokio::select! {
        _ = dom.wait_for_work() => {}
        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
    }

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
```

### Testing use_memo

```rust
use dioxus::prelude::*;

#[tokio::test]
async fn test_memo_updates() {
    fn test_app() -> Element {
        let items = use_signal(|| vec![1, 2, 3]);
        let count = use_memo(move || items.read().len());

        rsx! {
            div { "Count: {count}" }
        }
    }

    let mut dom = VirtualDom::new(test_app);
    dom.rebuild(&mut dioxus_core::NoOpMutations);

    // Render and check mutations
    let mutations = dom.render_immediate_to_vec();
    // Assert on mutations...
}
```

### Testing Pattern Summary

1. Create `VirtualDom` with test component
2. Call `dom.rebuild(&mut NoOpMutations)` for initial render
3. Trigger state changes or events
4. Advance state with `mark_dirty()`, `render_immediate_to_vec()`, or `wait_for_work()`
5. Assert expected outcomes

## Component Testing with SSR

Use `dioxus-ssr` to test component HTML output:

Reference: https://dioxuslabs.com/learn/0.7/guides/testing/web#component-testing

```rust
use dioxus::prelude::*;
use dioxus_ssr::render_to_string;
use pretty_assertions::assert_eq;

#[component]
fn Greeting(name: String) -> Element {
    rsx! {
        h1 { "Hello, {name}!" }
    }
}

#[test]
fn test_greeting_component() {
    let html = render_to_string(|| rsx! { Greeting { name: "World" } });
    assert!(html.contains("Hello, World!"));
}
```

**Limitations:**

- Components using `use_context` require mocking
- Platform-specific code (`dioxus::desktop::*`) won't work
- Signal reactivity cannot be tested with SSR alone

## End-to-End Testing

For complex UI interactions, use Playwright:

Reference: https://dioxuslabs.com/learn/0.7/guides/testing/web#end-to-end-testing

- [Web example](https://github.com/DioxusLabs/dioxus/tree/main/playwright-tests/web)
- [Fullstack example](https://github.com/DioxusLabs/dioxus/tree/main/playwright-tests/fullstack)

## Alternative: Extract Logic

Extract testable logic from components:

```rust
// Before: Logic inside component
#[component]
fn FileList(files: Vec<PathBuf>) -> Element {
    let filtered: Vec<_> = files
        .iter()
        .filter(|f| f.extension().map(|e| e == "md").unwrap_or(false))
        .collect();
    rsx! { /* ... */ }
}

// After: Extract testable function
fn filter_markdown_files(files: &[PathBuf]) -> Vec<&PathBuf> {
    files
        .iter()
        .filter(|f| f.extension().map(|e| e == "md").unwrap_or(false))
        .collect()
}

#[test]
fn test_filter_markdown() {
    let files = vec![
        PathBuf::from("doc.md"),
        PathBuf::from("image.png"),
    ];
    let result = filter_markdown_files(&files);
    assert_eq!(result.len(), 1);
}
```

## Test-Only Methods

Restrict test-only methods with `#[cfg(test)]`:

```rust
impl HistoryManager {
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.history.len()
    }
}
```

## Best Practices

### DO

- Extract pure functions from components and test them
- Use `tempfile` for safe temporary file creation
- Use `indoc` for readable multiline test strings
- Test Config with serialization roundtrip
- Use `#[cfg(test)]` for test-only code
- Use `tokio::test` for async hook testing
- Use `NoOpMutations` to discard DOM mutations in tests

### DON'T

- Test components with `use_context` directly (hard to mock)
- Test platform-specific code (`dioxus::desktop::*`)
- Forget timeout in `wait_for_work()` (can hang indefinitely)
- Use `#[allow(dead_code)]` for test-only code (use `#[cfg(test)]` instead)

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_history_navigation

# Run tests with output
cargo test -- --nocapture

# Run tests in specific module
cargo test markdown::tests
```
