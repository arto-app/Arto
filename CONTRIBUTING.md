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
- Rust toolchain with required targets
- pnpm for frontend dependencies
- just command runner
- dioxus-cli for development
- All other required tools

[Nix]: https://nixos.org/

### Alternative: Manual Setup

If you prefer not to use Nix, install these prerequisites manually:

- [Rust](https://rust-lang.org/) (stable toolchain)
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

# Format, lint, and test
just fmt check test
```

## Production Build

```bash
# Build for macOS
just build

# Install to /Applications (macOS)
just install
```

The binary will be available at `target/release/arto` or `target/dx/arto/bundle/macos/bundle/`.

## Project Structure

```
Arto/
├── Cargo.toml        # Cargo workspace (members = crates/*)
├── crates/
│   ├── arto/         # Desktop application (Dioxus): src/, assets/, Dioxus.toml
│   ├── arto-markdown/ # Markdown → HTML rendering library shared by the app and Quick Look
│   └── arto-ql/      # Quick Look FFI static library (macOS)
├── renderer/         # WebView-side TypeScript and CSS (pnpm + Vite)
├── platform/         # OS-specific files, one directory per OS
│   ├── macos/        #   bundle/ (Info.plist, icon), quicklook/ (Swift shim), verify-bundle.sh
│   ├── windows/      #   NSIS installer hook (file associations)
│   └── linux/        #   verify-bundle.sh
├── nix/              # Command wrappers used only by the Nix build
├── docs/images/      # Brand images (README header, logo)
├── samples/          # Sample Markdown files for manual testing
└── flake.nix         # Nix flake for reproducible builds
```

## Continuous Integration

`.github/workflows/ci.yml` runs on every pull request and finishes in a few
minutes: the frontend job (lint, type-check, tests, production bundle), one
Rust job per OS (format, clippy, tests via nextest; the Linux leg adds
rustdoc, feature combinations, `cargo deny` and `cargo machete`), flake
evaluation, documentation checks, workflow lint, and a macOS bundle as the
release-critical smoke test. Only the `ci` job needs to be a required status
check; it fails if anything it depends on failed.

The expensive jobs run when they can find something: the full five-leg
bundle (`bundle.yml`) and the from-scratch Nix build (`nix.yml`) run on every
push to `main`, on a pull request that touches packaging inputs or the flake,
or on a pull request labelled `ci:bundle`. A new push to a pull request
cancels the run in flight.

Every check has a matching recipe so a CI failure can be reproduced locally
inside the devShell:

| CI step | Recipe |
| --- | --- |
| Rust format, clippy | `cargo fmt --all --check` (`just arto::fmt` rewrites instead), `just arto::check` |
| Rust tests | `just arto::test-ci` (nextest) or `just arto::test` |
| rustdoc, feature combinations | `just arto::doc`, `just arto::features` |
| advisories and licenses, unused dependencies | `just arto::deny`, `just arto::machete` |
| frontend | `just renderer::check`, `just renderer::test` |
| documentation links and paths | `just docs` |
| workflow files | `just workflows` |
| flake evaluation | `nix flake check --no-build --all-systems` |

Rendering is covered by snapshots: `crates/arto-markdown/tests/samples.rs`
renders every numbered file under `samples/` and compares it with
`crates/arto-markdown/tests/snapshots/`. When a change to the output is
intended, run the test, inspect the `.snap.new` files with
`cargo insta review`, and commit the accepted snapshots.

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
- After any change to `renderer/pnpm-lock.yaml`, refresh the `pnpmDeps` hash
  in `flake.nix`: set it to `lib.fakeHash`, run `nix build .#renderer-assets`,
  and copy the hash from the mismatch error.
- The `dioxus` crate version, the `dioxus-cli` pin in
  `.github/workflows/bundle.yml` and the `dioxus-cli` shipped by the pinned
  nixpkgs must agree; check `nix develop -c dx --version` after
  `nix flake update`.

## Code Style

- **Rust**: Follow standard Rust formatting (`cargo fmt`)
- **Comments**: Must be in English
- **Tests**: Use `indoc` crate for multi-line test strings
- **Module System**: Use Rust 2018+ style (no `mod.rs`)

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/amazing-feature`)
3. Make your changes
4. Run `just fmt check test` to ensure code quality
5. Commit with [Conventional Commits](https://www.conventionalcommits.org/) format
6. Push and create a Pull Request

## License

By contributing, you agree that your contributions will be licensed under the same license as the project. See [LICENSE](LICENSE) for details.
