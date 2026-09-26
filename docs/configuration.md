# Configuration

Preferences live in `config.json` in the app config directory:

| Platform | Location |
| --- | --- |
| macOS | `~/Library/Application Support/arto/config.json` |
| Linux | `~/.config/arto/config.json` (or `$XDG_CONFIG_HOME/arto/`) |
| Windows | `%APPDATA%\arto\config.json` |

The Preferences window writes this file, and it can be edited by hand as
well: Arto reads it again when it is saved, so a change takes effect without
a restart. Keybindings are kept apart, in `mappings.json` next to it — see
[Keybindings](./keybindings.md).

## Typography

How a document's text is set lives in `typography`, and the Reading pane of
the Preferences window sets the same keys:

```json
{
  "typography": {
    "measure": 60,
    "lineHeight": 1.5,
    "fontFamily": "sans",
    "customFontFamily": "",
    "fontSize": 16,
    "cjkFontLanguage": "auto"
  }
}
```

| Key | What it sets | Values |
| --- | --- | --- |
| `measure` | The longest a line of text runs, in em | 30–100 |
| `lineHeight` | The height of a line as a multiple of the text size | 1.2–2.2 |
| `fontFamily` | The face the text is set in | `sans`, `serif`, `mono`, `custom` |
| `customFontFamily` | The CSS `font-family` used with `custom` | e.g. `"Source Han Serif", serif` |
| `fontSize` | The size of the text in px; headings and code follow it | 12–24 |
| `cjkFontLanguage` | Whose faces CJK characters are drawn in | `auto`, `ja`, `zh-Hans`, `zh-Hant`, `ko` |

The defaults are the page as GitHub sets it. The measure is in em rather than
in characters because an em is one full-width character: 40em holds about 40
Japanese characters or about 80 Latin ones, whichever script the document is
in. Code keeps its monospace face whatever is chosen.

One Han character takes different glyphs in Japanese, Chinese and Korean
faces, so Chinese text drawn in a Japanese face looks wrong to a Chinese
reader, and the other way round. `cjkFontLanguage` says whose faces Chinese,
Japanese and Korean characters are drawn in: `ja`, `zh-Hans`, `zh-Hant` or
`ko` name that language's faces after the Latin ones in the `sans`, `serif`
and `mono` stacks. `auto` names none and leaves the face to the system, as
GitHub does. It picks faces only — the language documents are read as, which
a screen reader follows, is left alone — and a `custom` face is used as
written.

The system faces (`-apple-system`, `ui-monospace` and their kin) draw CJK
characters in the system's own fallback before a named face is reached, so
they leave the stack whenever a CJK font language is chosen. On macOS that
moves Latin text from SF to Helvetica in `sans` and from SF Mono to Menlo in
`mono`; SF cannot be named directly. Other systems keep their Latin faces.

A value outside its range is drawn at the nearest end of it. A
`customFontFamily` is a comma-separated list that may hold only letters,
digits, spaces, quotes and `_ , - . +`, with any name holding more than
letters, digits, `_` and `-` in quotes; anything else leaves the text in the
`sans` face.

The full-width button in the header still lets one window ignore the measure,
and a printed page always runs the width of the paper. Zoom is separate: it
enlarges the whole page, images included, where `fontSize` sets only the text.
`arto page` and the Quick Look preview set the text the same way.

## Editor support

The file names its JSON Schema on its first line:

```json
{
  "$schema": "https://raw.githubusercontent.com/arto-app/Arto/main/schemas/config.schema.json"
}
```

An editor that reads JSON Schema — VS Code, or any editor running
`vscode-json-languageserver` — then completes every key and value, shows what
each one does on hover, and marks a value Arto would reject. It also marks a
key Arto does not know: Arto ignores such a key and drops it the next time it
saves the file, so a misspelled one is worth fixing while it is still there.

Arto adds the line itself the first time it saves the file. A file written
before that can have it added by hand; a `$schema` that points somewhere else
is kept as it is.

The schema describes Arto as it is on `main`, so a key added since the
release you run is offered before that release reads it. The schema itself
is generated from the configuration types, at
[`schemas/config.schema.json`](../schemas/config.schema.json).
