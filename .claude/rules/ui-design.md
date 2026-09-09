---
paths: "frontend/style/**/*.css, crates/arto/src/components/**/*.rs"
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

/* Quiet controls (see "Quiet controls" below) */
--opacity-rest: 0.3;        /* A chrome control at rest */
--opacity-woken: 0.72;      /* Its band has the pointer in it */
--opacity-active: 0.65;     /* Selected or on, at rest */
--hit-size: 30px;           /* Target size, independent of the glyph */
--transition-quiet: 0.18s;
```

**Z-Index Semantic Scale:**
```css
--z-resize-handle: 10;
--z-header: 60;
--z-pin-popover: 99;
--z-dropdown: 200;
--z-modal-backdrop: 10000;
--z-context-menu-backdrop: 10001;
--z-context-menu: 10002;
--z-context-submenu: 10003;
```

**Shadows, status colors, search highlight** and every other colour are
theme-aware and defined alongside the rest of the palette (below).

### Colours are Primer tokens

Arto paints one of GitHub's themes, chosen in the preferences for light mode
and for dark mode independently. The palette therefore is not written by hand:

- `@primer/primitives` ships a block of design tokens per theme. A Vite plugin
  (`primerThemesPlugin` in `frontend/vite.config.ts`) re-scopes each block onto
  `[data-theme="<name>"]` and writes them to `style/generated/primer-themes.css`.
  The names are GitHub's (`light`, `dark_dimmed`, `light_high_contrast`, …) and
  the Rust `ColorTheme` enum decides which ones are offered.
- `variables.css` maps Arto's semantic variables onto those tokens in a single
  `[data-theme]` block — `--bg-color: var(--bgColor-default)`,
  `--link-color: var(--fgColor-accent)`, and so on. Changing the theme changes
  the tokens, and every component follows without a second palette.
- The rendered Markdown is styled by `@primer/css`'s `markdown.css`, which reads
  the same tokens. Code highlighting maps highlight.js classes onto
  `--color-prettylights-syntax-*` in `style/syntax.css`.

**Use Arto's semantic variables in components** (`--bg-secondary`,
`--border-color`, `--text-secondary`). Reach for a raw Primer token only where
no semantic variable fits, and never write a hex value: a hard-coded colour is
wrong in every theme but the one it was taken from.

## Design Principles

### Keep it Subtle (控えめに)

- Avoid competing with the main content area
- Use `transparent` backgrounds where possible
- Use thin borders (`1px`) instead of thick (`2px`)
- Prefer `font-weight: 400-500` over bold for navigation

### Quiet controls

Chrome controls separate what is drawn from what can be hit, so a band of them
reads as a few faint marks while staying as easy to click as a toolbar:

- 16px glyph, `stroke-width: 1.5`, inside a `var(--hit-size)` (30px) target.
- Nothing painted at rest: no border, no surface. A surface appears only under
  the pointer, and only on the one control being pointed at.
- `--opacity-rest` at rest, `--opacity-woken` when the pointer enters the band
  (`.header:hover .nav-button`, not each button on its own), `1` under the
  pointer. Waking the whole band means a control is already legible by the time
  it is aimed at.
- State is a step along that same scale — `--opacity-active` — not a colour and
  not a filled chip.
- A control that cannot act is not drawn. `disabled` styling leaves something
  in the eye that offers nothing; render the button conditionally instead.

### Giving way to the document

When the window cannot hold everything, the document wins. There is one
setting — `sidebar.minContentWidth` (640px by default, 360–900) — and every
threshold is that number plus the width of whatever is still drawn beside the
page. Things give way from the outside in:

| Order | What folds | Threshold | Default |
| --- | --- | --- | --- |
| 1 | Margin trace (138px) | min + rail + panel + trace + gutter | below 1068px |
| 2 | Panel (its current width) | min + rail + panel + gutter | below 930px |
| 3 | Contents gutter (24px) | min + rail + gutter | below 704px |
| 4 | Rail (40px) | min + rail | below 680px |

`crate::hooks::layout_budget::budget` is the whole rule, and it is pure — the
table above is its test. `AppState::visible_chrome` collects the window's own
numbers for it, measuring width *after zoom* because magnifying the page is
the same as narrowing the window.

The budget is a model, and one thing is placed against the layout it actually
gets: the margin trace, which is drawn in the margin the page leaves over
rather than in a column of its own. The width reserved for it is measured
against the document's *minimum* width, while the page is set to its own width
and centred in whatever is left — so magnifying the page, or widening the
panel beside it, can close that margin while the budget still allows the
trace. `frontend/src/reading-position.ts` measures what is left and the trace
fades out when it can no longer stand clear of the text, which is the same
rule the budget states, applied to the space that is really there.

What comes back is state, never settings: widening the window restores the
panel exactly as configured. The one thing width never overrides is a panel
the reader folded with Cmd+B, because that was intent.

Nothing folds without something behind it. The panel is the clearest case:
`AppState::show_panel` pins it beside the document when the width allows and
peeks it over the document when it does not, so Cmd+B and the rail still open
something at any width — and the pinned choice itself is left untouched, which
is what lets widening restore it. The contents gutter has the same
arrangement: `contents.toggle` opens the same headings as an overlay.

The header's right takes none of the document's width, so it never folds into
an overflow menu; the breadcrumb's trail truncates from the left instead
(`arto / … / README.md`) and absorbs the shrink.

### Visual Consistency

- Selected items: `border-color: var(--accent-bg)` + light accent background (`8-10%` opacity)
- All similar buttons must have matching sizes (padding, font-size, border-radius)
- Use `color-mix(in srgb, var(--accent-bg) 8%, transparent)` for subtle selection backgrounds

## Preferences is its own window

**Preferences is a window, not a tab and not a modal.**

Changing a setting is a short errand; a tab is where a document lives for as
long as it is being read. Giving the errand a document's lifetime is what left
a settings tab sitting open for days, so it gets a window that closes instead.

### Architecture

- `window::preferences::open_or_focus_preferences_window` opens it, reusing the
  child-window machinery in `window/child.rs`: one window at a time, focused
  rather than duplicated, closed with its parent.
- The window holds no `AppState`. What the "Current Settings" section reports
  comes over as a `PreferencesSnapshot` taken when it opens, and what it
  changes goes back through `events::SET_SIDEBAR_ZOOM_IN_WINDOW`, targeted at
  the window that opened it.
- `AppState::open_preferences()` is still the single entry point, so the menu
  item and the keybinding stay unchanged.

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

## Menu Integration with the Preferences Window

**Opening a specific section when a menu item is clicked:**

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

Arto implements context menus in two areas:
1. **Sidebar tree context menu** - Right-click on files/directories
2. **Content context menu** - Right-click in markdown viewer

### Common Patterns

**1. Hoisting the menu out of what it acts on:**
```rust
// The row only records what was asked for; the menu itself is rendered once
// at the app-container root by `SidebarContextMenuHost`, so a watcher-driven
// remount of the tree cannot unmount an open menu.
state.sidebar_context_menu.set(Some(SidebarContextMenuData::new(
    cursor, viewport, path, kind, closable,
)));
```

**2. Submenu hover behavior:**
```rust
// A flyout opens while the pointer rests on its parent item
let mut show_submenu = use_signal(|| false);

div {
    class: "context-menu-item has-submenu",
    onmouseenter: move |_| show_submenu.set(true),
    onmouseleave: move |_| show_submenu.set(false),

    span { "Copy As" }
    span { class: "submenu-arrow", "›" }

    if *show_submenu.read() {
        div { class: "context-submenu", /* items */ }
    }
}
```

**3. Backdrop for outside-click closing:**
```rust
// Invisible backdrop catches clicks outside the menu
div {
    class: "context-menu-backdrop",
    onclick: move |_| on_close.call(()),
}
```

### Event Propagation

**Stop propagation to prevent conflicts:**
- Context menu clicks should `evt.stop_propagation()` to prevent parent handlers
- Sidebar tree clicks use split areas with `stop_propagation()` on chevron

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

**Auto-focus a window an action reached across:**
An action that acts on another window calls `crate::window::main::focus_window(target_id)` after sending its event, so the window it acted on is the one in front.
