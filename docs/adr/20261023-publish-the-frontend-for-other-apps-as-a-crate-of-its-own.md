# Publish the frontend for other apps as a crate of its own

Status: Accepted

## Context

An application that shows Markdown in its own window can render it with
`arto-markdown`, but the diagrams, the formulas and the colours of code are
left to Arto's frontend, which was not available outside Arto.

The frontend Arto ships is built for a document Arto owns whole: its runtime
takes the keyboard, scrolls the page and names the theme on `<html>`, and its
stylesheet styles the root and the body. Placed in another application's
window, it would take over that window.

The library bundles a document may need were embedded by `arto-page`, which
also depends on the configuration and on everything a page writer needs.

## Decision

`arto-frontend`, a crate with no dependencies, publishes a runtime and a
stylesheet built for a document in someone else's page, the library bundles,
and the choice of bundles for a rendered fragment. The runtime acts only on
the elements it is handed, and the stylesheet applies only inside the element
the fragment is placed in. `arto-page` takes its library bundles from it.

## Alternatives considered

- Publish the same items from `arto-page`. Every host would depend on the
  page writer and the configuration to get a few strings.
- Make the page runtime and stylesheet public as they are. They act on the
  whole document, so a host would have to undo what they do to its window.
- Leave each host to draw the placeholders itself against the HTML contract.
  Each would drift from how Arto draws them.

## Consequences

- The crate's first release is published by hand, as every new crate's is,
  before the release workflow can publish it.
- The runtime's script API and the stylesheet's confinement become part of
  the contract `arto-markdown`'s HTML is, and changing them is a breaking
  change for hosts.
- The frontend builds one more runtime and stylesheet, kept to what a rendered
  document needs.
