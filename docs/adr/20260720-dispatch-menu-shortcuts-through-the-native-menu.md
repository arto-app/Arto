# Dispatch menu shortcuts through the native menu

Status: Accepted

## Context

Every shortcut was handled by the in-window keybinding engine, which reads
keys from the WebView. With no window focused it hears nothing, so `Cmd+N`
did nothing once every window was closed, and the accelerators shown in the
menu bar were only hints.

## Decision

Shortcuts for menu actions are native menu accelerators, dispatched by the
operating system. The in-window engine keeps the shortcuts that need what
only it offers: chord sequences and per-context behaviour. Both share one
vocabulary of actions.

## Alternatives considered

- Keep one engine and open a hidden window to receive keys. A window that
  exists only to listen still needs focus, and the menu bar would still show
  accelerators it does not handle.

## Consequences

- Menu shortcuts work without a focused window, and the menu bar shows the
  shortcuts that actually run.
- A menu shortcut is a single chord; a sequence has to be an in-window
  keybinding.
- `mappings.json` holds two kinds of shortcut.
