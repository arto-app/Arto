# Arto

<!-- Quoted from README.md; do not rephrase it here. -->
Arto — the Art of Reading Markdown. A desktop app that renders Markdown the
way GitHub does, locally and offline.

## How it is built

- **Test first.** Red → green → refactor. Write the failing test before the
  code that makes it pass.
- **Each layer holds one thing.** Put a fact in the one layer that owns it,
  and nowhere else.

  | Layer | Holds |
  | --- | --- |
  | `docs/concept.md` | what this is and for whom |
  | `docs/adr/` | one decision and why it was made |
  | `docs/spec/` | the behaviour users and other components can rely on |
  | Tests | the details that pin the behaviour down: rules, boundaries, tables |
  | Code | how it is done |
  | Commits | why a change was made |
  | Comments | why it is not done another way |

  Before writing, fixing or reviewing a record, read `docs/adr/README.md`;
  before writing, fixing or reviewing a spec, read `docs/spec/README.md`.
  Follow their writing order.
- **Dependencies are current.** Before adding or bumping one, look up its
  latest release in the registry. Do not copy a version from memory or from
  another repository.

## Project rules

- **Test code**: `indoc` for multi-line strings, `tempfile` for files
- **Module system**: Rust 2018+ (no `mod.rs`)
- **Icons**: use the `add-icon` skill
- **Application launch**: do NOT launch the app; the user handles this

## Quality gate

Run `just ci` before reporting a change as done. It checks formatting
(rustfmt, oxfmt, nixfmt), lints (clippy, oxlint), runs every test, and checks
the rustdoc, feature combinations, advisories, unused dependencies, the
documentation's links and the workflow files. CI runs the same recipes.

`just fmt` formats everything `just ci` checks.

## Layout

| Path | What lives there |
| --- | --- |
| `crates/arto/` | The desktop app (Dioxus). Windows, the panel and its faces, menus, IPC client/server |
| `crates/arto-markdown/` | Markdown → HTML pipeline shared by the app, `arto page` and Quick Look |
| `crates/arto-config/` | `config.json` types and file I/O (no globals) |
| `crates/arto-keybindings/` | Keybinding parsing and defaults |
| `crates/arto-lsp/` | The wire protocol: JSON-RPC 2.0 in LSP framing, and the socket it runs on |
| `crates/arto-frontend/` | The frontend's bundles as Rust strings: the library bundles `arto-page` embeds, and the runtime and stylesheet for another app's page |
| `crates/arto-page/` | Standalone page renderer: `arto page` CLI and the Quick Look static library |
| `crates/arto-types/` | Values `config.json` and the wire protocol both carry (`Theme`, `FileOpenBehavior`) |
| `frontend/` | TypeScript and CSS bundled into each crate's `assets/frontend/` by Vite |
| `samples/` | Rendering samples; their HTML is snapshot-tested in `arto-markdown` |
| `schemas/` | JSON Schema of `config.json`, generated from `arto-config` and kept in step by its tests |
| `platform/` | OS-specific packaging (macOS bundle and Quick Look extension) |
| `docs/` | concept, ADRs, and specs — the pages the README links to are the specs |
| `nix/` | Wrappers the Nix build puts in front of macOS tools |

## Where the details live

Architecture notes load on demand from `.claude/rules/` when a matching file
is read or edited (see each rule's `paths` frontmatter): windows and state,
configuration, IPC, menus, the Markdown pipeline, Dioxus async patterns,
testing, UI design, interface translation, ADRs and specs. `.claude/TIPS.md`
collects longer-form lessons; read it when a rule points there.

The project description lives in `README.md`; quote it instead of inventing
descriptions.

## Language

- README, docs and commit messages: English
- Code comments, log and error messages: English

## Commits

Scoped Commits (`.gitmessage` is the template):

```
<scope>: <description>

<why this change is needed>
```

- The scope names where the change is, from the table below. Several scopes
  are joined with `, `; a change across the whole tree uses `treewide`.
- The description starts in lower case, has no trailing period, and stays
  within 72 characters.
- The body explains why. The code shows how; the tests show what.
- A breaking change says so in the body and carries a `BREAKING CHANGE:`
  trailer.

Commits before the switch use Conventional Commits (`feat(sidebar): …`);
they stay as they are.

| Scope | Covers |
| --- | --- |
| `arto` | `crates/arto/` |
| `markdown` | `crates/arto-markdown/` |
| `config` | `crates/arto-config/` |
| `keybindings` | `crates/arto-keybindings/` |
| `lsp` | `crates/arto-lsp/` |
| `page` | `crates/arto-page/` |
| `types` | `crates/arto-types/` |
| `frontend` | `frontend/`, `crates/arto-frontend/` |
| `samples` | `samples/` |
| `schemas` | `schemas/` |
| `platform` | `platform/` |
| `docs` | `README.md`, `CONTRIBUTING.md`, `docs/concept.md`, `docs/images/` |
| `adr` | `docs/adr/` |
| `spec` | `docs/spec/` |
| `agents` | `AGENTS.md`, `.claude/` |
| `build` | `justfile`s, the workspace `Cargo.toml`, `rust-toolchain.toml`, `.cargo/`, lint and deny settings |
| `ci` | `.github/` |
| `nix` | `flake.nix`, `flake.lock`, `nix/` |
| `deps` | dependency updates |

Add a row when a new area appears.
