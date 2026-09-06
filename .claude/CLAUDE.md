# Project-Specific Rules

- **Code comments, log and error messages**: English
- **Test code**: `indoc` for multi-line strings, `tempfile` for files
- **Module system**: Rust 2018+ (no `mod.rs`)
- **Icons**: use the `add-icon` skill
- **Application launch**: do NOT launch the app; the user handles this

## Quality gate

**Before reporting task completion, ALWAYS run:**

```bash
just fmt check test
```

It formats (cargo fmt + oxfmt), lints (clippy + oxlint) and tests every
crate. Do NOT report completion while any of these fail.

## Layout

| Path | What it is |
| --- | --- |
| `crates/arto/` | The desktop app (Dioxus). Windows, tabs, sidebar, menus, IPC client/server |
| `crates/arto-markdown/` | Markdown → HTML pipeline shared by the app, `arto page` and Quick Look |
| `crates/arto-config/` | `config.json` types and file I/O (no globals) |
| `crates/arto-keybindings/` | Keybinding parsing and defaults |
| `crates/arto-ipc/` | Single-instance protocol and socket |
| `crates/arto-page/` | Standalone page renderer: `arto page` CLI and the Quick Look static library |
| `frontend/` | TypeScript and CSS bundled into each crate's `assets/frontend/` by Vite |
| `samples/` | Rendering samples; their HTML is snapshot-tested in `arto-markdown` |
| `platform/` | OS-specific packaging (macOS bundle and Quick Look extension) |

## Where the details live

Architecture notes load on demand from `.claude/rules/` when a matching
file is read or edited (see each rule's `paths` frontmatter): windows and
state, configuration, IPC, menus, the Markdown pipeline, Dioxus async
patterns, testing, UI design. `.claude/TIPS.md` collects longer-form
lessons; read it when a rule points there.

Welcome page text lives in `crates/arto/assets/welcome.md` and the project
description in `README.md`; quote those instead of inventing descriptions.
