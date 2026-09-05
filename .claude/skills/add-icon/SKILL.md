---
name: add-icon
description: Add a new Tabler icon to the project. Use when adding icons to the UI.
---

# Add Icon Skill

This skill guides you through adding a new icon from Tabler Icons to the project.

## Process

**Icons are managed via `icons.json` and automatically built by Vite plugin.**

### Steps

1. **Choose Icon**
   - Browse available icons: https://tabler.io/icons
   - Note the icon name (e.g., `folder-open`, `info-circle`)

2. **Add to icons.json**
   - Edit `renderer/icons.json`
   - Add the icon name to the JSON array (in alphabetical order)

3. **Build Icon Sprite (Automatic)**
   - Icon sprite is automatically generated when Vite builds
   - Just run `just dev` or `cargo build`

4. **Add Rust Enum Variant**
   - Edit `crates/arto/src/components/icon.rs`
   - Add variant to `IconName` enum (use PascalCase)
   - Add case to `Display` implementation

### Example

**renderer/icons.json:**
```json
[
  "folder",
  "folder-open",
  "file"
]
```

**crates/arto/src/components/icon.rs:**
```rust
pub enum IconName {
    Folder,
    FolderOpen,
    File,
}

impl std::fmt::Display for IconName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IconName::Folder => write!(f, "folder"),
            IconName::FolderOpen => write!(f, "folder-open"),
            IconName::File => write!(f, "file"),
        }
    }
}
```

### File Locations

| File | Purpose | Git Tracked |
|------|---------|-------------|
| `renderer/icons.json` | Icon list configuration | ✅ Yes |
| `renderer/vite.config.ts` | Icon sprite plugin | ✅ Yes |
| `renderer/public/icons/tabler-sprite.svg` | Generated sprite (Vite source) | ❌ No |
| `assets/dist/icons/tabler-sprite.svg` | Build output (Dioxus asset) | ❌ No |

### Important

- **NEVER** edit generated SVG files directly
- The `assets/dist/` directory is `.gitignore`d as build output
- Icon sprite generation is automatic via Vite plugin (`buildStart` hook)
- Rust code references icons via `asset!("/assets/dist/icons/tabler-sprite.svg")`
- Icons come from `@tabler/icons` npm package (outline style only)

### Build Process

```
icons.json
    ↓
Vite plugin (buildStart hook)
    ↓
renderer/public/icons/tabler-sprite.svg
    ↓
Vite build
    ↓
assets/dist/icons/tabler-sprite.svg
    ↓
Rust asset!() macro
```
