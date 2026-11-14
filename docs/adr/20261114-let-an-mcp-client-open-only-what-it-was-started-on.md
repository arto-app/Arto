# Let an MCP client open only what it was started on

Status: Accepted

## Context

An MCP server that can open any local path in a window the user is looking
at puts that power in the hands of whatever drives the agent, including text
the agent read and was misled by. The socket the server talks to cannot
narrow it: any process running as the user can reach the socket, and could
read the same files and drive the window system anyway, so the socket is not
a trust boundary and grants nothing such a process lacks.

What can be narrowed is what one MCP server is willing to ask for. The user
registers the server with their MCP client, once, and says there what it is
for.

## Decision

`arto mcp` opens only files and directories under the roots it was started
with, and under the directory it was started in when it was given none.
Reading what Arto shows, moving the reader and reading the selection are not
restricted.

## Alternatives considered

- Open anything. An agent misled by a document it read could put any file
  the user can read in front of them.
- Ask the user in Arto before opening a path for the first time. The agent's
  work stops on a dialog the user may not be watching, and the user learns
  to accept it without reading.
- Also require a setting in `config.json` before `arto mcp` starts.
  Registering the server is already the user saying they want it, and an
  agent able to run commands could change the setting itself.

## Consequences

- A path outside the roots is refused with a message naming them, and the
  agent cannot widen them from inside the session.
- The roots bound what an agent asks for, not what a process running as the
  user can do; they are a guard against mistakes and manipulation, not
  against a hostile program.
- Reading what is on screen is as open as the socket is.
