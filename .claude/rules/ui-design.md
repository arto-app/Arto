---
paths: "renderer/style/**/*.css, crates/arto/src/components/**/*.rs"
---

# UI/UX Design Patterns

This rule provides design guidelines for UI development in Arto.

## CSS Design Tokens

Arto uses a comprehensive design token system defined in `variables.css` for consistency and maintainability.

### Available Design Tokens

**Transitions:**
```css
--transition-fast: 0.15s;     /* Quick state changes */
--transition-normal: 0.2s;    /* Default transitions */
--transition-slow: 0.3s;      /* Slower transitions (sidebar width) */
```

**Border Radius:**
```css
--radius-xs: 2px;    /* Scrollbar, small separators */
--radius-sm: 4px;    /* Buttons, inputs */
--radius-md: 6px;    /* Medium elements */
--radius-lg: 8px;    /* Cards, dropdowns */
--radius-full: 50%;  /* Circular elements */
```

**Font Sizes:**
```css
--font-size-xs: 11px;    /* Very small text */
--font-size-sm: 12px;    /* Small labels, tab text */
--font-size-md: 13px;    /* Small text, sidebar labels */
--font-size-base: 14px;  /* Default text, headers */
--font-size-lg: 15px;    /* Large text */
```

**Opacity:**
```css
--opacity-subtle: 0.3;      /* Missing items, very muted */
--opacity-muted: 0.5;       /* Default icons, placeholders */
--opacity-secondary: 0.6;   /* Secondary text */
--opacity-hover: 0.8;       /* Hover state for icons */
```

**Z-Index Semantic Scale:**
```css
--z-resize-handle: 10;
--z-search-bar: 50;
--z-header: 60;
--z-pinned-chips: 99;
--z-dropdown: 200;
--z-floating-tab: 1000;
--z-tab-actions: 1001;
--z-modal-backdrop: 10000;
--z-context-menu-backdrop: 10001;
--z-context-menu: 10002;
--z-context-submenu: 10003;
```

**Shadows:**
```css
--shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.05);   /* Subtle elevation */
--shadow-sm: 0 2px 6px rgba(0, 0, 0, 0.15);   /* Small dropdowns */
--shadow-md: 0 4px 12px rgba(0, 0, 0, 0.15);  /* Cards, menus (most common) */
--shadow-lg: 0 4px 12px rgba(0, 0, 0, 0.3);   /* High elevation */
```

**Status Colors:**
```css
--success-color: #22c55e;  /* Success states */
--error-color: #dc3545;    /* Error states */
--warning-color: #dc8a2f;  /* Warning states */
```

**Search Highlight (theme-aware):**
```css
--search-highlight: var(--light/dark-search-highlight);  /* Auto-switches with theme */
```

### Variable Naming Convention

**Theme-specific variables follow prefix pattern:**
```
--{theme}-{category}-{property}
```

Examples:
- `--light-bg-color`, `--dark-bg-color`
- `--light-pinned-green`, `--dark-pinned-green`
- `--light-scrollbar-track`, `--dark-scrollbar-track`

**Always use CSS variables instead of hardcoded values** for consistency and theme support.

## Design Principles

### Keep it Subtle (控えめに)

- Avoid competing with the main content area
- Use `transparent` backgrounds where possible
- Use thin borders (`1px`) instead of thick (`2px`)
- Prefer `font-weight: 400-500` over bold for navigation
- Icon opacity: `var(--opacity-muted)` default, `var(--opacity-hover)` on hover/active

### Visual Consistency

- Selected items: `border-color: var(--accent-bg)` + light accent background (`8-10%` opacity)
- All similar buttons must have matching sizes (padding, font-size, border-radius)
- Use `color-mix(in srgb, var(--accent-bg) 8%, transparent)` for subtle selection backgrounds

## In-Page Settings (Browser-Style)

**Prefer in-page settings over modal dialogs for preferences.**

Settings should integrate with the tab system rather than blocking the UI with modals.
This follows browser conventions (Chrome's `chrome://settings`, Firefox's `about:preferences`).

### Architecture

- Add a `TabContent::Preferences` variant to the content enum
- Implement `open_preferences()` method with tab deduplication (reuse existing preferences tab)
- Use state-based navigation instead of broadcast channels for window-specific features

### Layout Structure

```
preferences-page (全体: flex column, min-width: 600px)
│
└─ preferences-page-body (flex row, 両方スクロール可能)
   │
   ├─ preferences-nav (左: width: 180px, 縦並びボタン)
   │  ├─ Theme
   │  ├─ Sidebar
   │  ├─ Directory
   │  ├─ (spacer)
   │  └─ About
   │
   └─ preferences-settings (右: flex: 1)
      ├─ preferences-settings-header
      │  └─ save-status (右寄せ: [Save Changes] or Saving... or Saved!)
      │
      └─ preferences-pane (選択されたタブのコンテンツ)
         ├─ preference-section-title (h3, uppercase)
         └─ preference-item (各設定項目)
            ├─ preference-item-header (label + description)
            └─ Controls (option-cards, theme-selector, slider, etc.)
```

### Key CSS Properties

- Page: `min-width: 600px; overflow-x: auto` (allow horizontal scroll below minimum)
- Navigation: `width: 180px; background: transparent` (don't compete with sidebar)
- Settings header: `min-height: 36px` (prevent layout shift when Save button appears/disappears)

## Form Controls

### Custom Radio Button Styles

**1. Option Cards** - For binary/multiple choices with descriptions:
- Hide native `<input type="radio">` with `opacity: 0; position: absolute`
- Style the `<label>` as a card with `border: 1px solid var(--border-color)`
- Selected: `border-color: var(--accent-bg)` + accent-tinted background
- Separate cards with `gap: 12px` (not connected)

**2. Theme/Icon Selector** - For icon-based choices:
- Same card style as Option Cards (separated, not segmented)
- Icon + label vertically stacked with `gap: 6px`

### Directory/Path Inputs

- Make text input editable (not readonly) for direct path entry
- Use icon button for browse (`FolderOpen` icon, 40x40px square)
- Include "Use Current" button to grab value from current app state

### Slider Inputs

- Combine with value display (`{value}px`) and "Use Current" button
- Keep all related controls on the same row with `gap: 16px`

## Button Sizing Consistency

**All buttons in the same context must match:**

| Button Type | Padding | Font Size | Border Radius |
|-------------|---------|-----------|---------------|
| Primary action (Save) | 8px 16px | var(--font-size-md) | var(--radius-md) |
| Secondary (Browse, Use Current) | 10px 18px | var(--font-size-base) | var(--radius-lg) |
| Icon button | 0 (40x40px) | - | var(--radius-lg) |

## Typography & Spacing

### Recommended Sizes for Settings Pages

| Element | Font Size | Font Weight |
|---------|-----------|-------------|
| Navigation tab | var(--font-size-md) | 400 (500 when active) |
| Section title | var(--font-size-sm) | 600, uppercase |
| Item label | var(--font-size-lg) | 600 |
| Description | var(--font-size-md) | 400 |
| Button/Input | var(--font-size-base) | 500 |

### Spacing Guidelines

- Page padding: 24-32px
- Navigation padding: 24px 12px
- Settings content padding: 24px 48px
- Item padding: 20px vertical
- Gap between elements: 12-16px
- Border radius: var(--radius-lg) for cards/inputs, var(--radius-sm/md) for small elements

## Full-Page Content Sections (About, Welcome)

For pages like About, Welcome that fill the entire content area, follow the pattern in `no-file.css`:

```css
.page-container {
  display: flex;
  flex-direction: column;
  align-items: center;      /* Horizontal center */
  /* No justify-content: center - content starts from top */
  width: 100%;
  height: 100%;
  padding: 4rem 2rem;
  text-align: center;
  box-sizing: border-box;
}

.page-content {
  max-width: 500px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  animation: fadeInUp 0.6s ease-out;
}
```

### Key Patterns

- Use `fadeInUp` animation for smooth entry
- Cards for links/hints: `background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-lg); padding: 1.5rem`
- Opacity-based text hierarchy: title `0.9`, description `0.6`, footer `0.4`
- Link items: `display: flex; gap: 0.75rem; opacity: 0.7` with hover → `opacity: 1`

### Disable Parent Scroll

When content should fit without scrolling:

```css
.parent-container:has(.full-page) {
  overflow: hidden;
  padding: 0;
}
```

## Menu Integration with Tab Content

**Opening a specific tab when menu item is clicked:**

1. Create a static function to set the tab state before opening:
   ```rust
   // In preferences_view.rs
   static LAST_TAB: LazyLock<Mutex<Tab>> = LazyLock::new(|| Mutex::new(Tab::default()));

   pub fn set_tab_to_about() {
       *LAST_TAB.lock().unwrap() = Tab::About;
   }
   ```

2. Re-export from parent module:
   ```rust
   // In content.rs
   pub use preferences_view::set_tab_to_about;
   ```

3. Call before opening preferences in menu handler:
   ```rust
   // In menu.rs
   MenuId::About => {
       set_preferences_tab_to_about();
       state.open_preferences();
   }
   ```

**Note:** Replace predefined menu items (`PredefinedMenuItem::about`) with custom ones to control navigation.

## Context Menu Patterns

**Unified context menu behavior across components:**

Arto implements context menus in three areas:
1. **Tab context menu** - Right-click on tab items
2. **Sidebar tree context menu** - Right-click on files/directories
3. **Content context menu** - Right-click in markdown viewer (future)

### Common Patterns

**1. Window targeting with submenus:**
```rust
// Refresh window list on menu open
let handle_context_menu = move |evt: Event<MouseData>| {
    let windows = crate::window::main::list_visible_main_windows();
    let current_id = window().id();
    other_windows.set(
        windows
            .iter()
            .filter(|w| w.window.id() != current_id)
            .map(|w| (w.window.id(), w.window.title()))
            .collect(),
    );
    show_context_menu.set(true);
};

// Handler uses fire-and-forget transfer + auto-focus
let handle_move_to_window = move |target_id: WindowId| {
    crate::events::TRANSFER_TAB_TO_WINDOW.send((target_id, None, tab));
    crate::window::main::focus_window(target_id);
    show_context_menu.set(false);
};
```

**2. Submenu hover behavior:**
```rust
// Track submenu state to show/hide "Open in Window" options
let mut show_submenu = use_signal(|| false);

div {
    class: "context-menu-item has-submenu",
    onmouseenter: move |_| show_submenu.set(true),
    onmouseleave: move |_| show_submenu.set(false),

    span { "Open in Window" }
    span { class: "submenu-arrow", "›" }

    if *show_submenu.read() {
        div {
            class: "context-submenu",
            // Window list...
        }
    }
}
```

**3. Backdrop for outside-click closing:**
```rust
// Invisible backdrop catches clicks outside menu
div {
    class: "context-menu-backdrop",
    onclick: move |_| on_close.call(()),
}
```

### Event Propagation

**Stop propagation to prevent conflicts:**
- Context menu clicks should `evt.stop_propagation()` to prevent parent handlers
- Sidebar tree clicks use split areas with `stop_propagation()` on chevron
- Tab context menu should not interfere with drag events

### Menu Positioning

**Prevent overflow at viewport edges:**
```css
.context-menu {
  position: fixed;
  left: var(--menu-x);
  top: var(--menu-y);
  max-width: calc(100vw - 20px);
  max-height: calc(100vh - 20px);
  z-index: var(--z-context-menu);
}

.context-submenu {
  position: absolute;
  left: 100%; /* Right of parent */
  top: 0;
}
```

**Auto-focus target window after operations:**
All "Open in Window" / "Move to Window" operations should call `crate::window::main::focus_window(target_id)` after sending the event, providing immediate visual feedback to the user.
