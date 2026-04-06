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
# Run in development mode (with hot-reload via Vite + Dioxus)
just dev

# Or run the Dioxus dev server directly
cd desktop && dx serve

# Format, lint, and test
just fmt check test
```

> **Windows note:** On Windows, you must use `dx serve` (or `just dev`) instead of `cargo run`.
> WebView2 blocks the `file://` protocol that `cargo run` uses for asset serving.
> `dx serve` works around this by serving assets over HTTP.
> This limitation does not affect production builds (`dx bundle --release`).

## Production Build

```bash
# Build for the current platform
just build

# Install to /Applications (macOS only)
just install
```

The bundled application will be available under `target/dx/arto/bundle/`.

## Project Structure

```
Arto/
├── desktop/          # Main desktop application (Dioxus)
│   ├── src/          # Rust source code
│   └── assets/       # Static assets (CSS, images, welcome.md)
├── extras/           # Additional resources (icons, README images)
└── flake.nix         # Nix flake for reproducible builds
```

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
