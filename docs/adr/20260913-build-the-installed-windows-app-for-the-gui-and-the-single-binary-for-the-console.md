# Build the installed Windows app for the GUI and the single binary for the console

Status: Accepted

## Context

Windows decides at link time whether a program owns a console. Only a
console program is one a shell waits for, which `arto page > page.html`,
`--wait-ready` and a usable exit code all depend on. But Windows gives a
console program a console window of its own whenever no terminal started it,
so a document opened from Explorer or the Start menu left a command prompt
on screen for as long as Arto ran.

## Decision

The app the installer sets up is a GUI program. The single-binary download
is a console program, and it hides the console it was given when a
double-click rather than a shell started it.

## Alternatives considered

- One console program for both. Every launch from Explorer, a file
  association or the Start menu opens a command prompt beside the document.
- One GUI program for both. A shell does not wait for it, so redirects,
  `--wait-ready` and exit codes stop working from a terminal.

## Consequences

- The copy to put on `PATH` on Windows is the single binary, not the
  installed one.
- The application is built twice for Windows.
