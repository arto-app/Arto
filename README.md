<p align="center">
  <img src="./extras/arto-header-readme.png" alt="Arto" />
</p>

**Arto — the Art of Reading Markdown.**

A local app that faithfully recreates GitHub-style Markdown rendering for a beautiful reading experience.

## Philosophy

Markdown has become more than a lightweight markup language — it's the medium for documentation, communication, and thinking in the developer's world. While most tools focus on _writing_ Markdown, **Arto is designed for _reading_ it beautifully**.

The name "Arto" comes from "Art of Reading" — reflecting the philosophy that reading Markdown is not just a utility task, but a quiet, deliberate act of understanding and appreciation.

Arto faithfully reproduces GitHub's Markdown rendering in a local, offline environment, offering a calm and precise reading experience with thoughtful typography and balanced whitespace.

> [!WARNING]
> **Beta Software Notice**
>
> - This application is still in **beta** and may contain bugs or unstable behavior. Features may change without regard to backward compatibility.
> - **macOS Only**: This application is currently designed exclusively for macOS and does not support other platforms. However, cross-platform support is a long-term goal, and **PRs are welcome**.

## Features

### Core Reading Experience

- **GitHub-Style Rendering** — Accurate reproduction of GitHub's Markdown styling with full support for extended syntax
- **Native Performance** — Built with Rust for fast, responsive rendering
- **Auto-Reload** — Automatically updates when the file changes on disk
- **Offline First** — No internet connection required — read your docs anytime, anywhere

### Navigation & Organization

- **File Explorer** — Built-in sidebar with file tree navigation for browsing local directories
- **Quick Access** — Bookmark frequently used files and directories for instant access
- **Directory History** — Back/forward navigation within the sidebar file explorer
- **Table of Contents** — Automatic TOC panel for easy document navigation
- **Live Navigation** — Navigate between linked markdown documents with history support (back/forward)

### Search & Discovery

- **Find in Page** — Search within documents with `Cmd+F`
- **Pinned Search** — Pin search queries with persistent multi-color highlighting across sessions

### Window & Tab Management

- **Tab Support** — Open and manage multiple documents in tabs within a single window
- **Multi-Window** — Create multiple windows and open child windows for diagrams
- **Cross-Window Tabs** — Drag and drop tabs between windows
- **Drag & Drop** — Simply drag markdown files onto the window to open them

### Advanced Rendering

- **Mermaid Diagrams** — Interactive diagram viewer with zoom, pan, and copy-as-image
- **Math Expressions** — Beautiful KaTeX rendering for mathematical notation
- **Code Highlighting** — Syntax highlighting with copy button for code blocks
- **Frontmatter** — Renders YAML frontmatter as a styled, collapsible table
- **GitHub Alerts** — Full support for NOTE, TIP, IMPORTANT, WARNING, and CAUTION alerts

### Customization

- **Dark Mode** — Manual and automatic theme switching based on system preferences
- **Zoom Controls** — Keyboard shortcuts and trackpad gestures for zoom
- **Preferences** — Configurable settings for sidebar, TOC, and more
- **Context Menus** — Right-click menus for quick actions on files and content

## Installation

Use [Homebrew] tap to install. Since the application is not signed or notarized with an Apple Developer ID, you'll need to remove the quarantine attribute after installation.
See [homebrew-tap] for more information.

```
brew install --cask arto-app/tap/arto
xattr -dr com.apple.quarantine /Applications/Arto.app
```

Alternatively, [Nix] is also supported.
To try it without a permanent installation:

```
nix run github:arto-app/Arto
```

For a permanent installation, use [nix-darwin] or [home-manager].
Add the following to your flake inputs:

```nix
arto.url = "github:arto-app/Arto";
```

Then add it to `environment.systemPackages` (nix-darwin) or `home.packages` (home-manager):

```nix
environment.systemPackages = [ inputs.arto.packages.${system}.default ];
```

Launch the application to see the welcome screen with keyboard shortcuts and usage instructions.

## Usage

After installation, the `arto` command becomes available in your terminal:

```
arto                     # Launch Arto (shows welcome screen)
arto README.md           # Open a specific file
arto docs/               # Open a directory in the file explorer
arto file1.md file2.md   # Open multiple files in tabs
```

Arto runs as a **single instance** — if Arto is already running, the command sends paths to the existing window and activates it instead of launching a new process.

[Homebrew]: https://brew.sh/
[homebrew-tap]: https://github.com/arto-app/homebrew-tap
[Nix]: https://nixos.org/
[nix-darwin]: https://github.com/nix-darwin/nix-darwin
[home-manager]: https://github.com/nix-community/home-manager

## Official Website

Visit [arto-app.github.io](https://arto-app.github.io) for screenshots, feature highlights, and more.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

See [LICENSE](LICENSE) file for details.
