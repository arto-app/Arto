# Run a lens's agent with nothing but the text unless its lens allows more

Status: Accepted

## Context

A lens hands the document to a command-line agent such as `claude` or
`codex`. A document can be written to talk an agent into anything its tools
can do: read the files beside it and write them into the answer, or send
them out through a search. What an agent loads of its own set-up — MCP
servers, settings, hooks, sessions — also makes each run slower and its
answer less predictable. Some lenses still need a tool: a fact check has to
search the web.

## Decision

A command-line agent runs as a plain text transformer: its tools, MCP
servers, own configuration and sessions are turned off. A lens can allow its
agent more — web search, reading files, running commands — one capability at
a time, and only for that lens.

## Alternatives considered

- Run agents as the user configured them. Every document opened under a
  lens could then steer the agent's tools.
- A global switch for tools. The reader decides per task, and a lens is the
  task.

## Consequences

- A lens that reaches the machine is marked as such, and one that opens
  again by itself does not ask its agent again until the reader regenerates
  it, since the document may have changed into one they would not have
  opened it over.
