# Concept

This document describes what Arto is for and the ideas every design decision
rests on. It is rewritten in place as the concept evolves; decisions built on
it are recorded as ADRs.

## What it is

A desktop app for reading Markdown, locally and offline. It renders what
GitHub renders the way GitHub does, and draws more where plain Markdown
cannot show what a document means — data, charts, diagrams.
The name is short for "Art of Reading".

## Who it is for

People who spend their day reading Markdown — documentation, notes, specs,
the output of other tools — and want it to read as well as it does on GitHub,
without a browser, a server or an editor in the way.

More and more of that Markdown is written by AI agents. When an agent needs a
chart or a table of data, it reaches for HTML, which reads well once and is
hard to diff, search or keep. Arto lets the agent stay in Markdown and still
give the reader the richer page.

## Principles

- **Reading, not writing.** Typography, whitespace and navigation are chosen
  for long reading. The document on disk is never changed by reading it.
- **GitHub as the floor.** What GitHub renders, Arto renders the same way,
  with GitHub's stylesheet. Beyond it, Arto draws fenced code blocks that
  GitHub shows as code, so a document written for Arto still reads on GitHub
  as plain Markdown would. Every extension can be switched off, and every
  drawn block can be shown as the text it was written in.
- **A document is data.** Nothing a document carries runs as code. An
  extension draws what a block declares; it does not execute it.
- **Offline by default.** Everything the renderer needs is compiled into the
  binary. Nothing leaves the machine unless the document names it by URL or
  the reader opens a lens they configured.
- **Out of the way.** One document to a window, a panel that comes and goes,
  and every keystroke rebindable.

## What it does not do

- Edit Markdown. Arto is a reader; editing belongs to the reader's editor.
- Publish or sync documents, or keep them anywhere but on disk.
- Render other markup languages. Data inside a Markdown block is part of the
  Markdown document, not a document of its own.
