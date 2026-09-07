# Keybindings

Shortcuts live in `mappings.json`, next to `config.json` in the app config
directory. Preferences ships Default, Vim and Emacs presets; this document is
for editing the file directly.

## Two kinds of shortcut

**Menu shortcuts** (`menuShortcuts`) are native OS menu accelerators. They are
single chords (`Cmd+o`), appear in the menu bar, and are dispatched by the
system — so they work even when no window has keyboard focus, which is how
`Cmd+n` opens a window with everything closed. Each is keyed by a menu action
such as `file.open`.

**Keybindings** (`global` and the per-context sections) are handled by the
in-window engine. They support chord sequences (vim's `g g`) and per-context
behaviour, but only fire while a document window has focus.

The same action may appear in both: `file.open` can be a native `Cmd+o` menu
shortcut *and* have an in-window keybinding.

## Migrating an older `mappings.json`

Files written before `menuShortcuts` existed still load unchanged.
Menu-backed shortcuts under `global` (`Cmd+o`, `Cmd+n`) keep working through
the engine, but to get native menu accelerators, move those entries into a
`menuShortcuts` section — or re-apply a preset from Preferences.
