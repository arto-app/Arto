# Publish only the renderer to crates.io

Status: Superseded by
20261122-release-the-renderer-from-its-own-repository.md

## Context

Every release published every member of the workspace to crates.io, the app
and its internal crates included. Only the renderer is something other
programs build on: the embedding specification promises it to hosts. The
other crates are parts of the app, which is installed from its own packages,
and each one published is a name to claim, a trusted publisher to keep and a
package to verify on every release.

## Decision

Only the renderer's crates are published to crates.io. The app's crates are
not published; their versions already on the registry stay there.

## Alternatives considered

- Keep publishing every crate. It keeps the cost of a published crate for
  crates nobody builds on.
- Yank the app's crates as well. Yanking marks a version broken; these are
  not, and a project still depending on one would lose it on its next update.

## Consequences

- `cargo install arto` stays at the last version published. The app is
  installed from its release packages.
- The crates published are named explicitly, so a crate made public has to
  be added where they are named.
- The release that stops publishing them is preceded by one whose crates say
  in their descriptions that it is the last.
