# Let the host answer for the files a document names

Status: Accepted

## Context

A render asks about files besides the document: whether a linked document
exists, which file an image names, what that image holds. `arto-markdown`
asked this machine's disk.

A browser, a phone or a server keeping documents in a database has no disk
holding them. Rendered there, every link to another document showed as
missing and no image was drawn, though the parser and the HTML were the same.

## Decision

`arto-markdown` asks about files through a trait the host implements, and the
render functions take one. Answering from the disk is one implementation, and
the functions that take none use it.

## Alternatives considered

- Hand the renderer a list of the paths that exist and the bytes of every
  image up front. A host would have to know before the render which files the
  document names, which only the render finds out.
- Leave links and images unresolved for the host to fix up in the HTML
  afterwards. Every host would rewrite the HTML contract on its own and drift
  from how Arto marks a missing link or draws an image.

## Consequences

- The same document renders to the same HTML wherever its files are kept.
- A host decides how a path is spelled. Two paths naming one file must be
  answered with one spelling, or one image gets two URLs.
- The trait is part of the crate's public API, and changing it is a breaking
  change for hosts.
