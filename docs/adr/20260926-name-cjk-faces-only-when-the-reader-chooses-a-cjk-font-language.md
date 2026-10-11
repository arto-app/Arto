# Name CJK faces only when the reader chooses a CJK font language

Status: Accepted

## Context

One Han character takes different glyphs in Japanese, Chinese and Korean
faces, so a font stack that always named one language's faces would draw
the others' text in the wrong glyphs. Marking documents with a `lang` would
choose the faces, but would also change the language a screen reader reads
them in.

## Decision

The font stacks name Latin faces and end in a generic family. A CJK face is
named only when the reader chooses a CJK font language, and the choice adds
faces without changing the document's language.

## Alternatives considered

- Always name a Japanese face. Chinese and Korean readers would see their
  text in Japanese glyphs.
- Set the document's `lang`. It would change what a screen reader does.

## Consequences

- With no CJK font language chosen, the system picks the CJK faces, as on
  GitHub.
- The system faces draw CJK characters in the system's own fallback before a
  named face is reached, so they leave the stack whenever a CJK font language
  is chosen, and Latin text may change face with them.
