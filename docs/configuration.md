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
