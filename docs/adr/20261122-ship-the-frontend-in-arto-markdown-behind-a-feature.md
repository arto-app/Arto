# Ship the frontend in arto-markdown behind a feature

Status: Accepted. Supersedes
20261023-publish-the-frontend-for-other-apps-as-a-crate-of-its-own.md.

## Context

A host that shows a document needs both the HTML `arto-markdown` writes and
the frontend that draws what that HTML leaves to script. The frontend was a
crate of its own, `arto-frontend`, kept in step with `arto-markdown` only by
publishing both at one version. Nothing in the crate names said that one was
useless without the other, and every crate published is one more to keep.

## Decision

The frontend's bundles and the choice among them are a module of
`arto-markdown`, compiled in only when its `frontend` feature is on.
`arto-frontend` re-exports that module under its old name and is no longer
developed.

## Alternatives considered

- Keep two crates and explain the pairing in their descriptions. The pairing
  would still rest on a convention, and both would go on being published.
- Fold the bundles into `arto-markdown` with no feature. A host that only
  writes HTML, such as the configuration crate, would compile several
  megabytes of script it never uses.

## Consequences

- A host depends on one crate, and the renderer it runs and the frontend it
  loads come from the same version.
- Every download of `arto-markdown` carries the bundles, since a feature
  decides what is compiled, not what is packaged.
- A host that only wanted the bundles now compiles the renderer to get them.
