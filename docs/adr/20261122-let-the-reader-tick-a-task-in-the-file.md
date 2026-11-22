# Let the reader tick a task in the file

Status: Accepted

## Context

Arto never changes the document on disk; editing belongs to the reader's
editor. A task list is the one place where that costs the reader more than
it protects: ticking a task off is part of reading a checklist, and doing it
in an editor means leaving the document, finding the line and changing one
character, then coming back.

## Decision

In the app, a task list checkbox can be clicked, and the click changes that
task's `[ ]` or `[x]` in the file and nothing else. The file is read again
before it is written, and nothing is written unless the mark is still where
the page was drawn from and still says what the page showed. A setting turns
this off, and it is on unless the reader turns it off.

## Alternatives considered

- Keep the checkboxes inert. Every checklist read in Arto has to be finished
  in an editor.
- Remember the ticks with the app's data and leave the file alone. The
  editor, the repository and every other reader of the file never see them,
  and the two disagree as soon as the file is ticked elsewhere.
- Let the reader edit text in place. That makes Arto an editor, which it
  does not set out to be.
- Leave it off until the reader turns it on. Few readers would find the
  setting, and a reader who wants the file left alone can turn it off.

## Consequences

- Reading no longer guarantees the file is untouched: a click on a checkbox
  writes it, while the setting is on.
- A tick that cannot be placed — the file changed under the page — writes
  nothing, and the checkbox goes back to what the file says.
- Pages Arto writes for other programs, `arto page` and the Quick Look
  preview keep their checkboxes inert; they have no file to write to.
