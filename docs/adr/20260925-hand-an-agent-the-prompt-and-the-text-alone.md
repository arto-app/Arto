# Hand an agent the prompt and the text alone

Status: Accepted

## Context

Lenses are answered by general models and by models trained for a single
task, such as translation. A single-task model reads any framing around the
text as more text to work on.

## Decision

An agent is asked the lens's prompt, a blank line, and the text, with nothing
else around them. Only where several texts have to be told apart — a block
and its neighbours — are they set apart in tags.

## Alternatives considered

- Wrap the text in tags or a template. General models would follow it, but
  a single-task model would translate or transform the framing too.

## Consequences

- A lens without a prompt hands the model the text alone, so a single-task
  model gets exactly what it expects; what it needs to know goes in the
  system prompt.
