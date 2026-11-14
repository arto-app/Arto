# Controlling a running Arto

A running Arto can be asked what it is showing and told where to put the
reader, by any program on the same machine running as the same user. The
`arto` command is one such program: handing a file to an Arto that is already
running goes through the same connection described here.

## The connection

Arto listens on a local socket while it runs: a Unix domain socket named
`com.lambdalisue.arto.sock` in `$XDG_RUNTIME_DIR`, or in `/tmp/arto-<uid>/`
where that is not set, and a named pipe `\\.\pipe\arto-<user>-com.lambdalisue.arto.sock`
on Windows. Only the user Arto runs as can connect.

Messages are JSON-RPC 2.0, each preceded by a `Content-Length` header, as in
the Language Server Protocol:

```text
Content-Length: 58\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
```

The first request on a connection is `initialize`. It is answered with what
this Arto can do, and anything sent before it is refused with error `-32002`.
A capability the answer leaves out, or gives as `false`, is one this Arto
does not have; a client asks only for what was offered.

```json
{"serverInfo":{"name":"arto","version":"…"},
 "capabilities":{"open":true,"reopen":true,"state":true,"goto":true,
                 "selection":true,"stateChanges":true,"waitReady":true}}
```

`shutdown` tells Arto the client is done: it is answered, and any request
after it is refused. `exit` then ends the connection that sent it, not Arto.
Nothing sent over the socket closes the application.

Rust programs can use the `arto-lsp` crate, which holds the message types
and a client.

## Opening and showing

`arto/open` opens files, a directory, or both, as the
[`arto` command](./cli.md) would: `files`, `directory`, `behavior`, `behind` and `window` (`position`, `size`, `theme`) carry what its
paths and flags say, with `behavior` one of `new_window`, `last_focused` and
`current_screen` (left out, `config.json` decides). `arto/reopen` takes the same fields other than the paths,
and brings a window forward or places one without opening anything.

Both answer `{"ready": false}` once the request is applied. Given
`"waitReady": true`, they answer only once the window has drawn, with
`{"ready": true}`, or `{"ready": false}` when it did not draw in time.

## Reading what is open

`arto/state` describes every visible window, or the one named by `window`:

```json
{"windows":[{"window":1,"document":"/home/me/README.md","theme":"dark",
             "scroll":{"line":42,"fraction":0.25},
             "headings":[{"level":2,"text":"Install","id":"install"}]}],
 "focused":1}
```

- `window` is a number Arto gives each window as it opens, counting from
  one. A number is never given to a second window while Arto runs, so a
  number held after its window closed names no window at all.
- `document` is absent for a window that shows no file.
- `scroll` names where the reader is by the source line of the block at the
  top of the window (`0` is the top of the document) and how far into that
  block the top edge sits, from `0` to just under `1`. It names text rather
  than a distance, so it means the same place after diagrams and formulas
  have been drawn and the document has grown.
- `headings` is the table of contents; a heading's `id` is how it is named
  to `arto/goto`.
- `focused` is the window the reader was last in, when it is among those
  described.

A window named by a number that no longer names one is left out of the
answer rather than reported as an error.

## Moving the reader

`arto/goto` puts the reader somewhere in the document a window shows — the
window named by `window`, or the one last focused:

| `target` | Goes to |
| --- | --- |
| `{"heading": "install"}` | the heading with that `id` |
| `{"scroll": {"line": 42, "fraction": 0.25}}` | that place, as `arto/state` reports it |
| `"top"` | the top of the document |
| `"bottom"` | the end of the document |
| `{"search": {"query": "install", "index": 2}}` | the third hit of a search for "install" |

A `scroll` that `arto/state` reported brings the reader back to the same
text, however the document has grown in between.

It is answered with the window that was asked to move, as soon as it has been
asked. The reader arrives once the document has stopped growing around the
destination; `arto/state` asked afterwards reports where they settled. A
window number that names no open window is refused with error `-32602`.

A search runs as if it had been typed into the find panel, so its hits stay
highlighted for the reader. `index` counts hits from zero and may be left
out for the first; an index past the last hit moves nothing.

## Reading the selection

`arto/selection` answers with what the reader has selected in a window — the
one named by `window`, or the one last focused:

```json
{"window":1,"text":"the selected words"}
```

`text` is absent when nothing is selected.

## Following changes

A client that says, in `initialize`, that it wants to follow changes —
`"capabilities": {"stateChanges": true}` — is sent an `arto/didChangeState`
notification whenever a window's state changes, carrying the window's number
and its whole state as `arto/state` reports it. A window that closed is
reported with its number and no state. A change that only moves the reader is
sent at most a few times a second, ending with where the reader stopped.

Nothing is sent to a client that did not ask. Notifications start after the
answer to `initialize`, and only for changes from then on; `arto/state`
reports how things stand at the start.

A connection that has finished `initialize` stays open for as long as the
client holds it, whether or not it has anything to ask.
