<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/arto-header-readme-dark.png">
    <img alt="Arto" src="docs/images/arto-header-readme-light.png" width="600">
  </picture>
</p>

<p align="center">
  <strong>Arto — the Art of Reading Markdown.</strong><br>
  A desktop app that renders Markdown the way GitHub does, locally and offline.
</p>

<p align="center">
  <a href="https://arto-app.github.io"><strong>Website</strong></a> ·
  <a href="./docs/installation.md">Install</a> ·
  <a href="./docs/cli.md">CLI</a> ·
  <a href="./docs/keybindings.md">Keybindings</a> ·
  <a href="./CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/images/hero-dark.webp">
    <img src="./docs/images/hero-light.webp" alt="Arto showing a rendered document, with the contents gutter standing in its right-hand margin" width="860">
  </picture>
</p>

> [!WARNING]
> Arto is **beta**. Features may change without regard to backward compatibility. macOS is the platform it is developed and tested on; Linux and Windows builds exist but are **experimental** — see [Platform support](./docs/installation.md#platform-support).

## Why

Most Markdown tools are built for *writing*. Arto is built for **reading**: the name is short for "Art of Reading".

Markdown is where documentation, communication and thinking now live, and reading it deserves more than a preview pane. Arto reproduces GitHub's rendering locally and offline, with typography and whitespace chosen for long reading rather than for editing.

## Features

**Reading** — GitHub's dialect drawn with GitHub's own stylesheet: headings, tables, task lists, footnotes, autolinks and heading slugs; the five alerts (`NOTE`, `TIP`, `IMPORTANT`, `WARNING`, `CAUTION`); code highlighted per language with a copy button; and YAML frontmatter as a table that arrives collapsed, so the document still begins with its title. A file that changes on disk re-renders in place, and the renderer needs nothing from the network — the stylesheet, the highlighter and the diagram and formula code are compiled into the binary, so the only thing ever fetched is an image the document itself names by URL.

**What the renderer reads** — Markdown is a family of dialects, so the extensions are switches rather than assumptions: math, wiki links, superscript and subscript, definition lists, heading attributes and permalinks, smart punctuation, CJK emphasis, bare URLs as links, and whether raw HTML is filtered, passed through, or escaped so the markup itself shows.

**Getting around** — one panel with three faces rather than three panels: a file explorer holding as many folders as you need, the documents you have read grouped by day, and the ones you have starred (`⌘1`, `⌘2`, `⌘3`, and `⌘B` to show the panel at all). Unpinned it comes over the page on hover and leaves again; pinned it takes its own width beside the document. A window with nothing open shows the same three things on its welcome page.

**The gutter** — a ruler stands in the page's own margin and marks every heading: its width is the heading's depth, its colour a pinned search, its thickness where you are. `⌘J` opens it into a list of headings you can walk with the arrow keys. Relative links open the document they name, `⌘[` and `⌘]` move back and forward across the trail, and reopening a document puts you back where you stopped reading.

**Finding** — `⌘K` fuzzy-matches one query the way `fzf` does, across the files under the folder you are in, what you have read, what you have kept, and every command by name — a command showing the keystroke that runs it, where one is bound. `⌘F` puts find in the row the document's name is in, so nothing moves and nothing is covered; `Return` keeps what you typed as a mark in a colour of its own, repeated in the gutter, applied in every window it matches and kept across sessions.

**Windows** — one document to a window, named in its title; hand Arto several files and each gets a window of its own. A link inside a document travels in the window you are already in, unless you middle-click it. A diagram, a formula or an image lifts into a viewer of its own — zoom, pan, fit and copy-as-image, with the formula and the image naming their source in the header. Files dragged onto Arto open, including ones dragged out of an editor, and preferences decide whether a new window reuses the last focused one, appears on the screen the cursor is on, or is always new.

**Rich content** — Mermaid diagrams and KaTeX math where they stand, drawn as they come into view so a long document opens as quickly as a short one. The context menu copies a selection as Markdown, a code block with or without its fence, a table as Markdown, CSV or TSV, and an image as Markdown or as the image itself — as well as the document's path, with the line you are on or a range of them.

**Lenses** — Claude, Codex, a local model through Ollama or any OpenAI-compatible server asked about the document you are reading, the file itself never changed: a translation taking the document's places block by block from the top, each original a point at its right margin away; a summary opened from the header; a note kept beside each paragraph. Nothing is sent anywhere until you open a lens, and then only to what you configured — see [Lenses](./docs/lenses.md).

**Fitting in** — GitHub's own themes, including dimmed, high contrast and the colour-vision ones, with a separate choice for light and dark mode and the system deciding which applies. Keybindings ship as Default, Vim, Emacs and Clear presets, every binding editable and chord sequences supported. Zoom by keyboard or trackpad with the level remembered, `⌘P` to print or save a PDF through a stylesheet made for paper, and — on macOS — `Space` on a Markdown file previews it rendered, in the Finder preview pane too.

<sub>Shortcuts above are the macOS defaults; `⌘` is `Ctrl` on Linux and Windows. All of them are rebindable — see [Keybindings](./docs/keybindings.md).</sub>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/images/gfm-dark.webp">
    <img src="./docs/images/gfm-light.webp" alt="Emphasis, strikethrough, inline code, links, nested and task lists, and a blockquote rendered in Arto" width="410">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/images/contents-dark.webp">
    <img src="./docs/images/contents-light.webp" alt="The gutter opened into a list of the document's headings, beside typeset math" width="410">
  </picture>
  <br>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/images/palette-dark.webp">
    <img src="./docs/images/palette-light.webp" alt="The command palette, one query matching commands and documents at once" width="410">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/images/diagrams-dark.webp">
    <img src="./docs/images/diagrams-light.webp" alt="A Mermaid sequence diagram drawn inline in a document" width="410">
  </picture>
</p>

<p align="center"><em>See it in motion: <a href="https://arto-app.github.io">arto-app.github.io</a></em></p>

## Install

```sh
brew install --cask arto-app/tap/arto
xattr -dr com.apple.quarantine /Applications/Arto.app
```

Linux packages, a single binary for Linux and Windows, Nix, and why that second line is needed: [Installation](./docs/installation.md).

## From the terminal

Arto is a GUI application, and it runs as a single instance: the `arto` command hands files to the process already running rather than starting a second one.

```sh
arto README.md
```

It also renders a Markdown file to a self-contained HTML page — stylesheet, diagrams and math inlined — that opens in any browser without the app:

```sh
arto page README.md > README.html
```

The page follows your configuration, and ships with a Content-Security-Policy that blocks any script embedded in the Markdown. The same renderer is available as a standalone `arto-page` binary, for machines that need the output but not the window.

Full flags and behaviour: [CLI usage](./docs/cli.md).

## Built with

- **[Dioxus]** — the Rust UI framework the whole application is written in. Native windows, menus and state, no Electron.
- **[ox-content]** — the Markdown engine. It renders GitHub's dialect, including autolinks, alerts, heading slugs and the tag filter, so Arto does not carry its own version of any of them.
- **[KaTeX]** and **[Mermaid]** for math and diagrams, drawn in the page as you reach them.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup and guidelines.

## Sponsors

<p align="center">
  <a href="https://blacksmith.sh/">
    <img src="./docs/images/blacksmith-powered.jpg" alt="CI powered by Blacksmith" width="368">
  </a>
</p>

Arto's CI and release builds run on runners provided by **[Blacksmith]** through
their open source program.

## License

See [LICENSE](./LICENSE).

[Blacksmith]: https://blacksmith.sh/
[Dioxus]: https://dioxuslabs.com/
[ox-content]: https://github.com/ubugeeei-prod/ox-content
[KaTeX]: https://katex.org/
[Mermaid]: https://mermaid.js.org/
