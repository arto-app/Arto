# Letting an agent use Arto

`arto mcp` is a [Model Context Protocol](https://modelcontextprotocol.io)
server. Registered with an agent's MCP client, it lets the agent put a file
in front of you in Arto, see what you are reading and where, move you to the
part that matters, and read what you have selected.

## Registering it

An MCP client starts the server itself, as a command that talks over its
standard input and output. Give it `arto mcp` and the directories the agent
may open files in:

```sh
claude mcp add arto -- arto mcp --allow ~/notes --allow ~/src/project
```

```json
{"mcpServers": {"arto": {"command": "arto", "args": ["mcp", "--allow", "/Users/me/notes"]}}}
```

The command is the `arto` from [the CLI](./cli.md); on Windows, the single
binary rather than the installed copy.

A client that cannot change the command line can name the directories in
`ARTO_MCP_ALLOW` instead, separated as in `PATH` (`:`, or `;` on Windows).
`--allow` takes precedence over it.

In Claude Code, the `arto` plugin from the marketplace in Arto's repository
registers the server for you:

```
/plugin marketplace add arto-app/Arto
/plugin install arto@arto
```

The plugin asks for the directories to allow, in the form `ARTO_MCP_ALLOW`
takes; `/plugin configure arto@arto` changes them later. Left empty, the
agent opens files under the project Claude Code was started in.

## What it may open

`open` opens only files and directories under the `--allow` directories, or
the ones `ARTO_MCP_ALLOW` lists, or, when neither names any, under the
directory `arto mcp` was started in. A path is
compared after links are followed, so neither a link nor `..` reaches
outside. A path outside them is refused with a message that names them, and
nothing reaches Arto. The agent cannot widen them during a session.

Reading what Arto shows, moving the reader, reading the selection and
picturing what is drawn are not limited to these directories.

## The tools

Each tool is one request to the running Arto, described in
[Controlling a running Arto](./control.md), and answers with Arto's answer
as JSON text. When Arto refuses, or is not running, the tool answers with an
error saying so.

| Tool | Takes | Does |
| --- | --- | --- |
| `open` | `paths`, `newWindow`, `activate` | Opens the files, and the one directory as the folder the file explorer shows |
| `state` | `window` | Describes every window, or the one numbered `window` |
| `goto` | `window`, `target` | Moves the reader, as `arto/goto` does |
| `selection` | `window` | Reads what is selected, as `arto/selection` does |
| `capture` | `window`, `target` | Pictures a block or what you see, as `arto/capture` does |

`open` answers once the window has drawn the document. It opens where Arto's
settings say, or in a new window with `newWindow`, and leaves the keyboard
with the application you are working in unless `activate` is set. Relative
paths are read against the directory `arto mcp` was started in.

When Arto is not running, `open` starts it on the same files and waits for
it to show them, answering `{"ready": true, "launched": true}`, or
`"ready": false` if it did not within thirty seconds. Arto keeps running
after the agent's session ends. No other tool starts Arto.

`goto` takes a `target` as `arto/goto` does — `{"heading": id}`,
`{"scroll": {"line": …, "fraction": …}}`, `"top"`, `"bottom"` or
`{"search": {"query": …, "index": …}}` — so the `scroll` that `state`
reported can be handed straight back. `window` is a number from `state`;
left out, it is the window you were last in.

`capture` answers with the picture as an image, and beside it, as JSON, its
size and the source lines of the block. Its `target` is `{"block": line}`,
`{"heading": id}` or `"viewport"`; the `line` of a `scroll` from `state`
names the block at the top of the window. Taking a picture never moves you.
