# CLI usage

Arto is a desktop application; the `arto` command is how you hand files to it
from a terminal, not a separate mode.

Homebrew, the `.deb` and Nix put `arto` on your `PATH`. The AppImage does not —
it is a single self-contained file, so run it by its own path, and everything
below works the same:

```sh
./arto_<version>_x86_64.AppImage README.md
```

## Opening files

```sh
arto                     # Launch Arto (shows the welcome screen)
arto README.md           # Open a file
arto docs/               # Open a directory in the file explorer
arto file1.md file2.md   # Open several files in tabs
```

Arto runs as a **single instance**: if it is already running, the command hands
the request to the existing process rather than starting a second one.

| Flag | What it does |
| --- | --- |
| *(none)* | Reuses the last focused visible window. Without arguments, shows or focuses an existing window, or opens one if there is none. |
| `--open=screen` | Opens on — or reuses — a window on the screen the cursor is on. |
| `--open=new` | Always opens a new window. |
| `--directory=DIR` | Sets the file explorer's root for that invocation. A positional directory (`arto docs/`) does the same. |

## Rendering a standalone page

`arto page` renders a Markdown file into a single HTML file carrying Arto's
stylesheet and rendering code inline, so it opens in any browser without the
app — Mermaid diagrams and math included.

```sh
arto page README.md > README.html
arto page --output out.html docs/guide.md
arto page --theme dark notes.md
```

The page follows your `config.json`, so it looks the way the app shows the
file. `--theme`, `--no-auto-link-urls` and friends override individual
settings, `--config FILE` reads another file, and `--no-config` starts from the
built-in defaults. The macOS Quick Look preview reads the same configuration
when its sandbox allows.

The page ships with a Content-Security-Policy that blocks any script embedded
in the Markdown; pass `--no-csp` only for input you trust.

The same command is available as a standalone `arto-page` binary for machines
without the app.
