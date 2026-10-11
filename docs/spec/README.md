# Specification

What Arto promises to whoever uses it, one topic per file.

## Who reads it

Users of Arto, and authors of other programs that work with it. A spec
holds what they can rely on, not everything that is true of the code.

## What belongs in a spec

Each layer holds one thing:

| Layer | Holds |
| --- | --- |
| ADR | one decision and why it was made |
| Spec | the behaviour users and other components can rely on |
| Tests | the details that pin the behaviour down: rules, boundaries, tables |
| Code | how it is done |

Tests hold the what in detail, so a spec is narrower than the tests. What
stays in a spec is what tests express poorly: the flow a user goes through, a
promise such as "existing data is never rewritten", the scope of what is
handled.

For every line, ask:

1. **Can a test pin this down (and should one)?** Then it goes in a test, not
   here.
2. **Would a change to this be called a change in behaviour by a user?** If
   not, it is not spec.

These usually fail the questions; when one appears, take it out:

- numbered steps (an algorithm)
- mapping tables
- where another program keeps its files
- exact strings: headers, file names
- ordering rules, screen layout

### The exception: formats Arto defines

A format that Arto defines and that others write — its own data files,
its configuration — is read by outside authors, so the spec describes it in
full. A format someone else defines (another program's files) is not
Arto's to promise; reading it is left to the code and the tests.

## How to write one

1. Write first, briefly, what Arto promises. Leave the details out.
2. Write the details as test names. Tests named as sentences make the list of
   tests a readable specification.
3. When review leads to a fix, do not touch the spec unless the behaviour a
   user sees changes.

Before committing, ask both questions of every line once more.

A spec is rewritten in place, in the same change as the behaviour it
describes. It never carries "to be updated" notes, and it does not say why —
the reasons live in `docs/adr/`.
