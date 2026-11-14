# Serve MCP from the arto command over the socket protocol

Status: Accepted

## Context

A running Arto can already be asked what it shows, and told to open a file
or move the reader, over the local socket a second launch hands its request
to. Agents do not speak that protocol; they speak the Model Context Protocol,
and an MCP client starts each server it is configured with as a program
that talks over its standard input and output.

## Decision

`arto mcp` is an MCP server over standard input and output. Each tool is one
request of the socket protocol, sent to the running Arto and answered with
what Arto answered; the server holds no state of its own. It lives in its own
crate, `arto-mcp`, which the `arto` command runs as a subcommand.

## Alternatives considered

- Speak MCP from the running app itself. An MCP client has to start the
  server it talks to, and the app is started by the user, not by the client;
  the app would also carry a second protocol beside the socket's.
- Ship the server as a binary of its own. It could be installed apart from
  the app and drift from the version it talks to, and it is one more program
  to put on `PATH`.
- Build it inside the `arto` crate. The MCP library would become a
  dependency of the app, and the adapter could not be tested without it.

## Consequences

- Whatever an MCP tool can do is what the socket protocol can do; a new tool
  means a new method there first.
- `arto mcp` is the same program, and the same version, as the app it talks
  to.
- The crate's first release is published by hand, as every new crate's is,
  before the release workflow can publish it.
