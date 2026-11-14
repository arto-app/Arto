# Concept

This document describes what Arto is for and the ideas every design decision
rests on. It is rewritten in place as the concept evolves; decisions built on
it are recorded as ADRs.

## What it is

A desktop app for reading Markdown, locally and offline.
The name is short for "Art of Reading".

## Who it is for

People who spend their day reading Markdown — documentation, notes, specs,
the output of other tools — and want it to read well, without a browser, a
server or an editor in the way.

## Principles

- **Reading, not writing.** Typography, whitespace and navigation are chosen
  for long reading. The document on disk is never changed by reading it.
- **Offline by default.** Everything the renderer needs is compiled into the
  binary. Nothing leaves the machine unless the document names it by URL or
  the reader opens a lens they configured.
- **Out of the way.** One document to a window, a panel that comes and goes,
  and every keystroke rebindable.

## What it does not do

- Edit Markdown. Arto is a reader; editing belongs to the reader's editor.
- Publish or sync documents, or keep them anywhere but on disk.
- Render other markup languages.
