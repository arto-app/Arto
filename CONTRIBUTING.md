# Contributing to Arto

Thank you for your interest in contributing to Arto!

## Development Setup

### Recommended: Using Nix (Reproducible Environment)

[Nix] provides a fully reproducible development environment with all dependencies pre-configured. This is the recommended approach as it ensures consistent tooling across all contributors.

```bash
git clone https://github.com/arto-app/Arto.git
cd Arto
cachix use arto   # Enable binary cache (speeds up builds)
nix develop
```

This automatically provides:
- The Rust toolchain `rust-toolchain.toml` pins
- pnpm for frontend dependencies
- just command runner
- dioxus-cli for development
- All other required tools

[Nix]: https://nixos.org/

### Alternative: Manual Setup

If you prefer not to use Nix, install these prerequisites manually:

- [rustup](https://rustup.rs/), which installs the toolchain `rust-toolchain.toml` pins
- [pnpm](https://pnpm.io/)
- [just](https://github.com/casey/just)
- [dioxus-cli](https://crates.io/crates/dioxus-cli)

Then run:

```bash
git clone https://github.com/arto-app/Arto.git
cd Arto
just setup
```

## Development Commands

```bash
# Run in development mode
cargo run --release

# Run with hot-reload (requires dioxus-cli)
dx serve --platform desktop

# Format everything
just fmt

# Everything CI checks: formatting, lints, tests, docs and workflows
just ci
```

## Production Build

```bash
# Build for macOS
just build

# Install to /Applications (macOS)
just install
```

The binary will be available at `target/release/arto` or `target/dx/arto/bundle/macos/bundle/`.

## Trying Windows in a VM

Code behind `cfg(windows)` never compiles on macOS or Linux. [vitro] runs it
in a throwaway Windows VM from a golden image of your own, built once with
`vitro build windows` and provisioned with Git, the MSVC build tools, LLVM and
Rust (vitro's `examples/provision/windows-devtools.ps1` does that).
`platform/windows/vitro.ps1` adds the rest — Node with pnpm, just, and Git's
bash — to every VM before running a command.

```bash
# The tests, in a VM that is destroyed afterwards (see vitro.toml)
vitro test

# The app itself, in a VM that stays up
vitro run windows
vitro exec --sync . windows powershell -ExecutionPolicy Bypass \
  -File platform/windows/vitro.ps1 just standalone
vitro wait --desktop windows
vitro launch windows 'C:\Users\dev\Arto\target\aarch64-pc-windows-msvc\release\arto.exe'
vitro screenshot windows
vitro destroy windows
```

`--sync` copies the checkout to a directory of the same name in the guest's
home, so `dev` above is your image's `ssh_user` and `Arto` the name of your
checkout. `launch` does not start in that home, so the path has to be
absolute. On an x86_64 host the binary is under `target\x86_64-pc-windows-msvc`
instead.

[vitro]: https://github.com/lambdalisue/vitro

## Project Structure

```
Arto/
├── Cargo.toml        # Cargo workspace (members = crates/*)
├── crates/
│   ├── arto/         # Desktop application (Dioxus): src/, assets/, Dioxus.toml
│   ├── arto-config/  # config.json / keys.toml types, file locations, load and save
│   ├── arto-lsp/     # The wire protocol: JSON-RPC 2.0 in LSP framing, and the local socket
│   ├── arto-mcp/     # `arto mcp`: an MCP server over the wire protocol, for agents
│   ├── arto-keybindings/ # Shortcut parsing, binding sets, presets, and the matching engine
│   ├── arto-page/    # Self-contained HTML pages: CLI, `arto page`, Quick Look FFI (feature ffi)
│   └── arto-types/   # Values config.json and the wire protocol both carry (Theme, FileOpenBehavior)
├── frontend/         # WebView-side TypeScript and CSS (pnpm + Vite), drawing with @arto-app/markdown
├── platform/         # OS-specific files, one directory per OS
│   ├── macos/        #   bundle/ (Info.plist, icon), quicklook/ (Swift shim), verify-bundle.sh
│   ├── windows/      #   NSIS installer hook (file associations)
│   └── linux/        #   verify-bundle.sh
├── nix/              # Command wrappers used only by the Nix build
├── docs/images/      # Brand images (README header, logo) and README screenshots
├── samples/          # Sample Markdown files for manual testing
├── schemas/          # JSON Schema of config.json, generated from arto-config
└── flake.nix         # Nix flake for reproducible builds
```

## Continuous Integration

`.github/workflows/ci.yml` runs on every pull request and finishes in a few
minutes: the frontend job (lint, type-check, tests, production bundle), one
Rust job per OS (format, clippy, tests via nextest; the Linux leg adds
rustdoc, feature combinations, `cargo deny` and `cargo machete`), flake
evaluation, documentation checks, workflow lint, and one bundle per OS —
macOS, Linux and Windows — which runs alongside the Rust jobs rather than
after them. Only the `ci` job needs to be a required status check; it fails if
anything it depends on failed.

Each bundle leg uploads its installers (and, off macOS and outside pull
requests, the standalone executable) as a run artifact named `arto-<leg>`,
downloadable from the run
summary page for three days — long enough to try a pull request on the
machine it matters on. Release bundles are kept for ninety days instead.

The expensive legs run when they can find something: the arm64 bundles
(`bundle.yml`) and the from-scratch Nix build (`nix.yml`) run on every push
to `main`, on a pull request that touches packaging inputs or the flake, or
on a pull request labelled `ci:bundle`. A new push to a pull request cancels
the run in flight.

Every check has a matching recipe so a CI failure can be reproduced locally
inside the devShell:

| CI step | Recipe |
| --- | --- |
| Rust format, clippy | `cargo fmt --all --check` (`just arto::fmt` rewrites instead), `just arto::check` |
| Rust tests | `just arto::test-ci` (nextest) or `just arto::test` |
| rustdoc, feature combinations | `just arto::doc`, `just arto::features` |
| advisories and licenses, unused dependencies | `just arto::deny`, `just arto::machete` |
| frontend | `just frontend::check`, `just frontend::test` |
| documentation links and paths | `just docs` |
| workflow files | `just workflows` |
| flake evaluation | `nix flake check --no-build --all-systems` |

The renderer, `arto-markdown` and `@arto-app/markdown`, is developed and
released in [arto-app/arto-markdown], which pins its own output in snapshots.
Arto keeps a record of what the version it depends on draws:
`crates/arto-page/tests/renderer.rs` renders every numbered file under
`samples/` together with the library bundles each one needs, and compares them
with `crates/arto-page/tests/snapshots/`. When a renderer bump changes the
output, inspect the `.snap.new` files with `cargo insta review` and commit the
accepted snapshots.

## Dependency Updates

Dependabot opens weekly pull requests for Cargo, pnpm and GitHub Actions
(`.github/dependabot.yml`); a scheduled workflow does the same for the Nix
flake inputs (`.github/workflows/update-flake-lock.yml`). Minor and patch
bumps arrive grouped; major bumps arrive one per PR because each one needs its
own API check. The flake.lock PR is opened with the workflow's own token, so
CI does not start on it automatically: close and reopen the PR to run the
CI workflow.

When updating by hand:

- Versions shared by more than one crate live in `[workspace.dependencies]`
  in the root `Cargo.toml`; bump them there, then run `cargo update`.
- The `arto-markdown` crate and the `@arto-app/markdown` package are one
  release: bump both to the same version together.
- After any change to `frontend/pnpm-lock.yaml`, refresh the `pnpmDeps` hash
  in `flake.nix`: set it to `lib.fakeHash`, run `nix build .#frontend-assets`,
  and copy the hash from the mismatch error.
- The `dioxus` crate version, the `dioxus-cli` pin in
  `.github/workflows/bundle.yml` and the `dioxus-cli` shipped by the pinned
  nixpkgs must agree; check `nix develop -c dx --version` after
  `nix flake update`.

## Publishing to crates.io

Arto publishes nothing to crates.io; every crate in this workspace is
`publish = false`. The renderer, `arto-markdown`, is released from
[arto-app/arto-markdown] and Arto depends on that release. `arto-frontend`, which
re-exported its frontend under the old name, is no longer released. The versions already on crates.io stay
there: the app's crates last at 0.47.0, and the renderer's at 0.47.1 before
its own repository took them over.

[arto-app/arto-markdown]: https://github.com/arto-app/arto-markdown

## Code Style

- **Rust**: Follow standard Rust formatting (`cargo fmt`)
- **Comments**: Must be in English
- **Tests**: Use `indoc` crate for multi-line test strings
- **Module System**: Use Rust 2018+ style (no `mod.rs`)

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/amazing-feature`)
3. Make your changes
4. Run `just ci` to ensure code quality
5. Commit with the Scoped Commits format described in [AGENTS.md](./AGENTS.md#commits)
6. Push and create a Pull Request

## License

By contributing, you agree that your contributions will be licensed under the same license as the project. See [LICENSE](LICENSE) for details.
