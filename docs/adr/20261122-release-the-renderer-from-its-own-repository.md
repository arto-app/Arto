# Release the renderer from its own repository

Status: Accepted. Supersedes
20261122-publish-only-the-renderer-to-crates-io.md.

## Context

Arto's releases published the renderer's crates to crates.io at the app's
version. The renderer now has a repository of its own,
arto-app/arto-markdown, with its own releases and versions. Both publishing
the same crate names would collide on the next version either of them takes.

## Decision

Arto publishes nothing to crates.io. The renderer's crates are published from
arto-app/arto-markdown.

## Alternatives considered

- Keep publishing from Arto until the app depends on the released renderer.
  Arto's next release would take a version the renderer's repository is about
  to publish, and a published version cannot be taken back.

## Consequences

- The renderer's versions follow its own releases, not the app's.
- Until the app depends on the released renderer, the copy in this workspace
  is built but never published.
