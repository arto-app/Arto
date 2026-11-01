# Keep the reader's keybindings as changes to a preset

Status: Accepted

## Context

The keybindings file replaced the preset whole. Changing one shortcut meant
keeping every other one of the preset in the file as well, so a reader could
not tell their own changes from what the preset had, and a preset improved in
a later release never reached anyone who had changed a single key.

## Decision

The keybindings file names the preset it starts from and holds only how the
reader's shortcuts differ from it: keys bound to another action, keys added,
and keys taken away. The preferences write the file the same way.

## Alternatives considered

- Keep the whole set in the file, as before. Every reader who changes one key
  carries a copy of the preset that goes stale with each release.
- Ship one set of defaults and drop the presets, offering Vim and Emacs as
  example changes. A reader who wants Vim keys would copy a long list of
  changes into their file, which is the copy this decision removes.

## Consequences

- A file holds only what the reader chose, and changes to the presets reach
  them on upgrade, including bindings they did not expect.
- Taking a key away needs a way to say so in the file.
- A file written before this decision held every binding; it is read as
  changes to the preset it differs least from.
