---
paths: "docs/spec/**"
---

# Writing a spec

Before writing, fixing or reviewing a file in `docs/spec/`, read
`docs/spec/README.md` and follow "How to write one". It is the whole rule;
this file only makes sure it is read at the moment of writing.

The checks people skip:

- Ask of every line: can a test pin this down? Then it belongs in a test.
  Would a user call a change to it a change in behaviour? If not, it is not
  spec.
- Numbered steps, mapping tables, other programs' file locations, exact
  strings and ordering rules are candidates to take out — unless they
  describe a format this project defines for others to write.
- A review fix that does not change what a user sees does not touch the spec.
- Do not copy the level of detail of neighbouring specs; they may be too
  detailed themselves.
