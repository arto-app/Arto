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
| `--behind` | Opens without activating Arto, leaving the keyboard focus where it is. |
| `--directory=DIR` | Sets the file explorer's root for that invocation. A positional directory (`arto docs/`) does the same. |

`--behind` is for the hand-offs you did not stop to make — an editor or a
script putting a file in front of you while you keep typing:

```sh
arto --behind README.md
```

A window that is already open takes the file where it stands, without being
raised; a window that has to be created appears without Arto becoming the
active application. What the flag protects is the application you are working
in — when Arto is already the active application, a window it creates does
take the focus from the Arto window you were reading. Keeping the application
inactive is enforced on macOS; elsewhere a launch may still come forward.

Without any paths, `arto --behind` does nothing when Arto is already running:
bringing the app forward is exactly what the flag declines to do.

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
file — the same theme, and the same choices about what the renderer reads out
of the Markdown. `--theme` and `--no-auto-link-urls` override a setting for one
run, `--config FILE` reads another file, and `--no-config` starts from the
built-in defaults. The macOS Quick Look preview reads the same configuration
when its sandbox allows.

The page ships with a Content-Security-Policy that blocks any script embedded
in the Markdown; pass `--no-csp` only for input you trust.

The same command is available as a standalone `arto-page` binary for machines
without the app.
