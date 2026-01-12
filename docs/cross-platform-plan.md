# Arto Cross-Platform Support Plan

## Overview

This document outlines the plan to extend Arto from macOS-only to support Windows and Linux.

### Target Platforms

| OS | Architecture | Priority |
|----|--------------|----------|
| macOS | aarch64, x86_64 | Current (maintain) |
| Windows | x86_64 | High |
| Linux | x86_64, aarch64 | Medium |

### Issue Summary

| Category | Count | Impact |
|----------|-------|--------|
| Compilation blockers | 3 | Build fails on Windows/Linux |
| Missing features | 4 | Single instance, file association, etc. |
| UX differences | 3 | Menu labels, window behavior |
| Build/distribution | 4 | Packaging, icons |

---

## Phase 1: Enable Compilation

**Goal**: Make `cargo build` succeed on Windows/Linux

### 1.1 Conditional Compilation for macOS Dependencies

**File**: `desktop/Cargo.toml`

```toml
# Current (lines 43-46)
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6.3"
objc2-app-kit = "0.3.2"
tracing-oslog = "0.3.0"
```

**Action**: Dependencies are already gated with `cfg(target_os = "macos")`. Guard usage sites.

### 1.2 Conditional Compilation for tracing-oslog

**File**: `desktop/src/main.rs:115-118`

```rust
// Current
let registry = registry.with(
    tracing_oslog::OsLogger::new("com.lambdalisue.Arto", "default")
        .with_filter(silence_filter),
);

// Fixed
#[cfg(target_os = "macos")]
let registry = registry.with(
    tracing_oslog::OsLogger::new("com.lambdalisue.Arto", "default")
        .with_filter(silence_filter),
);

#[cfg(not(target_os = "macos"))]
let registry = registry; // no-op
```

### 1.3 Conditional Compilation for NSWindow API

**File**: `desktop/src/menu.rs:472-477`

```rust
// Current
fn disable_automatic_window_tabbing() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindow;
    let marker = MainThreadMarker::new().expect("...");
    NSWindow::setAllowsAutomaticWindowTabbing(false, marker);
}

// Fixed
#[cfg(target_os = "macos")]
fn disable_automatic_window_tabbing() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindow;
    let marker = MainThreadMarker::new().expect("...");
    NSWindow::setAllowsAutomaticWindowTabbing(false, marker);
}

#[cfg(not(target_os = "macos"))]
fn disable_automatic_window_tabbing() {
    // No-op: macOS-specific feature
}
```

### 1.4 Update justfile for Multi-Platform Builds

**File**: `justfile`

```just
# Current
build: setup assets
  @cd desktop && dx bundle --release --macos

# Fixed
build-macos: setup assets
  @cd desktop && dx bundle --release --macos

build-windows: setup assets
  @cd desktop && dx bundle --release --windows

build-linux: setup assets
  @cd desktop && dx bundle --release --linux

build: setup assets
  @cd desktop && dx bundle --release
```

---

## Phase 2: Application Lifecycle

**Goal**: Ensure expected app behavior on Windows/Linux

### 2.1 Single Instance Control

**Issue**: macOS automatically enforces single instance; Windows/Linux allow multiple instances

**Solution**: Use `single-instance` crate

```toml
# Cargo.toml
[dependencies]
single-instance = "0.3"
```

```rust
// main.rs
fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        let instance = single_instance::SingleInstance::new("com.lambdalisue.Arto")
            .expect("Failed to create single instance");
        if !instance.is_single() {
            // Focus existing instance via IPC
            return;
        }
    }
    // ... existing code
}
```

**Additional consideration**: File path forwarding to existing instance via IPC

### 2.2 Command-Line Arguments for File Opening

**Issue**: Windows/Linux receive file paths via command-line arguments on file association launch

**File**: `desktop/src/main.rs`

```rust
fn main() {
    // ...

    // Windows/Linux: Get file/directory from command-line arguments
    #[cfg(not(target_os = "macos"))]
    let initial_open_event = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        args.first().and_then(|arg| {
            let path = std::path::PathBuf::from(arg);
            if path.is_dir() {
                Some(OpenEvent::Directory(path))
            } else if path.is_file() {
                Some(OpenEvent::File(path))
            } else {
                None
            }
        })
    };

    #[cfg(target_os = "macos")]
    let initial_open_event: Option<OpenEvent> = None; // macOS uses Event::Opened
}
```

### 2.3 Platform-Specific Event Handling

**File**: `desktop/src/main.rs:48-82`

```rust
.with_custom_event_handler(move |event, _target| match event {
    #[cfg(target_os = "macos")]
    Event::Opened { urls, .. } => {
        // macOS: File association launch
        for url in urls {
            if let Ok(path) = url.to_file_path() {
                // ... existing handling
            }
        }
    }
    #[cfg(target_os = "macos")]
    Event::Reopen { .. } => {
        // macOS: Dock icon click
        tx.try_send(OpenEvent::Reopen).ok();
    }
    Event::WindowEvent {
        event: WindowEvent::Focused(true),
        window_id,
        ..
    } => {
        // All platforms
        if !window::has_preview_window() {
            window::update_last_focused_window(*window_id);
        }
    }
    _ => {}
})
```

### 2.4 Platform-Specific WindowCloseBehaviour

**File**: `desktop/src/components/main_app.rs:89`

```rust
// Current
window().set_close_behavior(WindowCloseBehaviour::WindowHides);

// Fixed
#[cfg(target_os = "macos")]
window().set_close_behavior(WindowCloseBehaviour::WindowHides);

#[cfg(not(target_os = "macos"))]
window().set_close_behavior(WindowCloseBehaviour::CloseWindow);
```

**Note**: Windows/Linux users expect the app to quit when the last window closes

---

## Phase 3: UX Adjustments

**Goal**: Provide natural UX on each platform

### 3.1 Platform-Specific Menu Labels

**File**: `desktop/src/menu.rs`

```rust
// MenuId::RevealInFinder label
#[cfg(target_os = "macos")]
const REVEAL_LABEL: &str = "Reveal in Finder";

#[cfg(target_os = "windows")]
const REVEAL_LABEL: &str = "Show in Explorer";

#[cfg(target_os = "linux")]
const REVEAL_LABEL: &str = "Show in File Manager";
```

### 3.2 Improved File Reveal

**File**: `desktop/src/utils/file_operations.rs`

```rust
pub fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn().ok();
    }

    #[cfg(target_os = "windows")]
    {
        // Show file selected in Explorer
        Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .ok();
    }

    #[cfg(target_os = "linux")]
    {
        // Use DBus to invoke file manager
        // Fallback: open parent directory
        if let Some(parent) = path.parent() {
            open::that(parent).ok();
        }
    }
}
```

### 3.3 Keyboard Shortcut Display

**Current**: `Modifiers::SUPER` already maps to Cmd/Ctrl correctly

**Additional**: Menu shortcut string display

```rust
#[cfg(target_os = "macos")]
const MOD_KEY_SYMBOL: &str = "⌘";

#[cfg(not(target_os = "macos"))]
const MOD_KEY_SYMBOL: &str = "Ctrl+";
```

---

## Phase 4: Build & Packaging

**Goal**: Generate distribution packages for each platform

### 4.1 Extend Dioxus.toml

**File**: `desktop/Dioxus.toml`

```toml
# Existing
[bundle.macos]
license = "../LICENSE"
provider_short_name = "Alisue"
info_plist_path = "../extras/mac/Info.plist"

# Add
[bundle.windows]
license = "../LICENSE"
icon = "../extras/windows/arto-app.ico"

[bundle.linux]
license = "../LICENSE"
icon = "../extras/linux/arto-app.png"
```

### 4.2 Create Icon Files

**Required files**:

| Platform | Format | Path |
|----------|--------|------|
| macOS | .icns | `extras/mac/arto-app.icns` (existing) |
| Windows | .ico | `extras/windows/arto-app.ico` (new) |
| Linux | .png (256x256) | `extras/linux/arto-app.png` (new) |

**Generation**:
```bash
# Extract PNG from macOS .icns
sips -s format png extras/mac/arto-app.icns --out extras/linux/arto-app.png

# Generate ICO from PNG (ImageMagick)
convert extras/linux/arto-app.png extras/windows/arto-app.ico
```

### 4.3 Windows Manifest

**File**: `extras/windows/app.manifest` (new)

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="Arto"/>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
```

### 4.4 Linux .desktop File

**File**: `extras/linux/arto.desktop` (new)

```ini
[Desktop Entry]
Name=Arto
Comment=Markdown Viewer
Exec=arto %F
Icon=arto
Terminal=false
Type=Application
Categories=Office;Viewer;
MimeType=text/markdown;text/x-markdown;
```

### 4.5 Extend flake.nix

**File**: `flake.nix`

```nix
# Add Linux to systems
systems = [
  "aarch64-darwin"
  "x86_64-darwin"
  "x86_64-linux"
  "aarch64-linux"
];

# Platform-specific build
buildPhaseCargoCommand = ''
  dx bundle --release --platform desktop ${
    if pkgs.stdenv.isDarwin then "--package-types macos"
    else if pkgs.stdenv.isLinux then "--package-types appimage"
    else ""
  }
'';
```

---

## Phase 5: CI/CD

**Goal**: Automated builds and releases for all platforms

### 5.1 GitHub Actions Workflow

**File**: `.github/workflows/release.yml` (new/extend)

```yaml
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: Arto.app
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: Arto.app
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: Arto.exe
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: Arto.AppImage

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install dioxus-cli
      - run: dx bundle --release
      - uses: actions/upload-artifact@v4
        with:
          name: arto-${{ matrix.target }}
          path: target/release/${{ matrix.artifact }}
```

---

## Implementation Order

```
Phase 1 (Compilation)
├── 1.1 Verify dependencies ✓ (already cfg-gated)
├── 1.2 Gate tracing-oslog
├── 1.3 Gate NSWindow API
└── 1.4 Update justfile

Phase 2 (Lifecycle)
├── 2.1 Single instance
├── 2.2 Command-line arguments
├── 2.3 Event handling
└── 2.4 WindowCloseBehaviour

Phase 3 (UX)
├── 3.1 Menu labels
├── 3.2 File reveal
└── 3.3 Shortcut display

Phase 4 (Packaging)
├── 4.1 Dioxus.toml
├── 4.2 Create icons
├── 4.3 Windows manifest
├── 4.4 Linux .desktop
└── 4.5 flake.nix

Phase 5 (CI/CD)
└── 5.1 GitHub Actions
```

---

## Risks and Challenges

### High Risk

| Issue | Impact | Mitigation |
|-------|--------|------------|
| Dioxus Windows/Linux bugs | Build failures, runtime errors | Monitor upstream issues, find workarounds |
| WebView dependency (wry) | Rendering differences across platforms | E2E testing |

### Medium Risk

| Issue | Impact | Mitigation |
|-------|--------|------------|
| Font rendering differences | Visual inconsistencies | Use system-ui fonts |
| File path format | Path resolution errors | Use PathBuf properly |

### Low Risk

| Issue | Impact | Mitigation |
|-------|--------|------------|
| Keyboard layout | Shortcut conflicts | Use standard key bindings |

---

## Test Checklist

### Functional Tests

- [ ] App launch
- [ ] Launch via file association
- [ ] Single instance behavior
- [ ] File/directory picker dialogs
- [ ] "Show in Explorer/Finder" feature
- [ ] Window resize/move
- [ ] Multi-window
- [ ] Tab operations
- [ ] Markdown rendering
- [ ] Settings save/load

### Platform-Specific Tests

**Windows**:
- [ ] High DPI support
- [ ] Windows 10/11 compatibility
- [ ] Installer (MSI/Setup)

**Linux**:
- [ ] GNOME/KDE compatibility
- [ ] Wayland/X11 support
- [ ] AppImage functionality

---

## References

- [Dioxus Desktop Documentation](https://dioxuslabs.com/learn/0.6/guides/desktop/)
- [single-instance crate](https://crates.io/crates/single-instance)
- [Dioxus GitHub Issues](https://github.com/DioxusLabs/dioxus/issues)
