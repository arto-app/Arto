# Keep the measure in em

Status: Accepted

## Context

The reader sets the longest a line of text runs. Arto's documents mix
scripts: a line of full-width Japanese and a line of English at the same
width differ by about half in the characters they hold.

## Decision

The measure is set in em.

## Alternatives considered

- `ch`, the width of a "0". A full-width character is about 2ch, so the
  same setting would double a line's length whenever the script changed.
- A width in px. It would not follow the text size.

## Consequences

- One setting gives a line of about the same number of full-width
  characters, or twice as many half-width ones, in either script.
