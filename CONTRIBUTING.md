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
│   ├── arto-config/  # config.json / mappings.json types, file locations, load and save
│   ├── arto-ipc/     # Single-instance IPC: JSON Lines protocol and the local socket
│   ├── arto-keybindings/ # Shortcut parsing, binding sets, presets, and the matching engine
│   ├── arto-markdown/ # Markdown → HTML rendering library used by the app and arto-page
│   └── arto-page/    # Self-contained HTML pages: CLI, `arto page`, Quick Look FFI (feature ffi)
├── frontend/         # WebView-side TypeScript and CSS (pnpm + Vite)
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
evaluation, documentation checks, workflow lint, and one bundle per OS —
macOS, Linux and Windows. Only the `ci` job needs to be a required status
check; it fails if anything it depends on failed.

Each bundle leg uploads its installers (and, off macOS, the standalone
executable) as a run artifact named `arto-<leg>`, downloadable from the run
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
- After any change to `frontend/pnpm-lock.yaml`, refresh the `pnpmDeps` hash
  in `flake.nix`: set it to `lib.fakeHash`, run `nix build .#frontend-assets`,
  and copy the hash from the mismatch error.
- The `dioxus` crate version, the `dioxus-cli` pin in
  `.github/workflows/bundle.yml` and the `dioxus-cli` shipped by the pinned
  nixpkgs must agree; check `nix develop -c dx --version` after
  `nix flake update`.

## Publishing to crates.io

Publishing a release takes no action: the `publish` job in
`.github/workflows/release.yml` runs on every published GitHub release and
sends every member of the workspace — `arto` included — to crates.io at the
release version.

No registry token lives in this repository. The job authenticates with
[Trusted Publishing]: crates.io is told which repository and workflow file it
trusts, and `rust-lang/crates-io-auth-action` exchanges GitHub's OIDC claim
for a token that expires when the job ends. That is what `id-token: write` on
the job is for.

### Version stamping

The version in git is always `0.0.0`, and CI stamps the release tag into the
working copy without ever committing it. The stamp replaces **every** `0.0.0`
in the root `Cargo.toml`, not only the `[workspace.package]` one: the members
ask each other for the workspace version, so stamping one line would leave
them requiring `^0.0.0` from crates that had just become `X.Y.Z`, and the
workspace would stop resolving at all.

### Claiming a new name

crates.io only lets a trusted publisher be configured on a crate that already
exists, so a name has to reach the registry once before CI can ever publish
it. That first version is `0.0.0` — the version this repository already
carries in git, so nothing needs stamping and no release has to be spent on
it:

```bash
just frontend::assets   # arto and arto-page carry the bundle in their packages
cargo publish --workspace --allow-dirty
```

`--workspace` orders the members by their dependencies and waits for each to
reach the index before the next one asks for it, so nothing has to be
sequenced by hand. `--allow-dirty` is for the bundle, which git ignores
because it is build output. The verification build is worth the wait: it is
what proves each package carries the files its `include` list names.

`--exclude <name>` leaves out a crate the registry already carries at `0.0.0`
— what crates.io has, not what the working tree says, since every member in
git reads `0.0.0` whether it was ever published or not. There is no
`--skip-existing`, so a run that stopped partway is resumed by excluding the
members that did get through.

Then, on each crate's settings page on crates.io, add a trusted publisher for
this repository naming `release.yml`. From the next release on, the job does
it, and `0.0.0` is superseded by the first real version — which is also when
the registry page starts showing the README, keywords and categories, since
crates.io renders the latest version's metadata.

[Trusted Publishing]: https://crates.io/docs/trusted-publishing

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
