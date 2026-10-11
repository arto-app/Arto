# Show a lens's answer with less trust than the document

Status: Accepted

## Context

A lens's answer is Markdown written by a model, and the document the model
read can talk it into writing anything. The reader chose to open the
document; they did not choose what a model would write about it.

## Decision

An answer is rendered with less trust than the document: raw HTML in it is
shown as written whatever the document's setting allows, and nothing in it
is fetched from another host.

## Alternatives considered

- Render answers like the document. A document could then have a model
  write markup or remote requests the document itself was not allowed.

## Consequences

- Markup the document used as HTML, such as `<kbd>`, shows as the tag in a
  translation.
- An image the answer names from elsewhere shows as its alt text.
