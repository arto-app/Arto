---
paths: "crates/arto-lsp/**, crates/arto-mcp/**, crates/arto/src/ipc.rs, crates/arto/src/ipc/**, crates/arto/src/main.rs"
---

# Single-Instance Architecture

Arto runs as one process. A newly launched process:

1. tries to connect to the existing instance over the Unix domain socket;
2. if it connects, hands over what it was asked to open and exits with `0`;
3. otherwise becomes the primary instance and starts the IPC server.

Why: several processes would fight over file watches, `config.json` writes
and `state.json`.

## The protocol is JSON-RPC 2.0 in LSP framing

```text
Content-Length: 58\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
```

LSP's transport, framing and lifecycle, with Arto's own methods — not a
language server in the semantic sense. The framing is borrowed whole
because the reason to speak LSP at all is that editors already have client
machinery for it, and machinery that expects `Content-Length` is not helped
by JSON-RPC over newlines.

Lifecycle: `initialize` must come first and is answered with
`ServerCapabilities`; anything before it is refused with
`ServerNotInitialized` (-32002). Negotiating capabilities is what replaces
the old approach of accumulating message shapes that had to be understood
forever.

Methods, all in `crates/arto-lsp/src/methods.rs`:

| Method | Kind | What it does |
| --- | --- | --- |
| `initialize` | request | Report name, build and capabilities |
| `initialized` | notification | Client is ready; ignored |
| `arto/open` | request | Open files and/or set the root directory |
| `arto/reopen` | request | Bring a window forward, or place and repaint one |
| `arto/state` | request | What is open, and where the reader is in it |
| `arto/goto` | request | Put the reader somewhere in a window's document |
| `arto/selection` | request | What the reader has selected |
| `arto/didChangeState` | notification | **Sent by the instance** — a window changed |
| `shutdown` | request | No more requests; `exit` follows |
| `exit` | notification | **Close this connection** — see below |

**`exit` departs from LSP deliberately.** There it ends the server process;
here it ends only the connection that sent it. A launch handing over a path
is a client for the length of one request, and letting any such client quit
the application the user is reading in would turn a stray message into lost
work. Nothing on this socket can close Arto.

`arto/open` takes `OpenRequest`'s own fields flattened in — `files`,
`directory`, `behavior`, `behind`, `window` — so the document half of the
message is the same object it has always been, read by the same type.

`window` (from `--position`, `--size`, `--theme`) says what the request asks
of the window it lands in rather than what to read there. Each field is
optional and an absent one leaves the running instance's own answer alone,
so the object is omitted entirely when nothing was asked. A `reopen` carries
it too: naming no path is how a window is asked for as a window.

`waitReady` (from `--wait-ready`) is not about the request but about when it
is answered. Without it the response comes as soon as the request is
applied; with it, once the target window has drawn:

```json
{"jsonrpc":"2.0","id":2,"result":{"ready":true}}
```

`ready: false` from a launch that asked to wait means the window did not
draw in time, and the launch **exits non-zero** — `--wait-ready` exists so a
capture script does not photograph an undrawn window, which it cannot do if
failing to draw still reports success. A launch that did not ask to wait is
always answered `ready: false`, because nothing has been drawn at the moment
its request is applied.

The client asks to wait only when `initialize` said the instance can
(`capabilities.waitReady`). That is what the handshake is for: what an
instance says it cannot do, it is not asked to do.

## Reading a window, and moving the reader in it

`arto/state` answers with every open window — the document, the theme, the
headings and where the reader is — and `arto/goto` puts the reader
somewhere else. Both name a window by `WindowKey`, a number the instance
hands out as a window registers, counting from one and never reused: a key
held across a close names nothing rather than whatever took the number.

The scroll position travels as `{line, fraction}`, never as a pixel offset.
A pixel offset only means anything while every block is the height it was
when the offset was taken, and Mermaid and KaTeX draw after the document
appears — so a document full of them is still growing while it is read.
Naming the block is what makes `arto/state`'s answer something
`arto/goto` can be handed straight back.

`arto/state` is a read of the main thread's own state, answered in the turn
that asked; `current_scroll_anchor` is kept up to date by the
reading-position tracker for the header already. `arto/goto` travels to the
target window as an event, because `document::eval` only reaches a WebView
from inside that window's scope. It is answered when the window has been
asked to move, not when the reader has arrived; the destination is held in
the page until the document stops growing under it. A client that needs the
settled position asks `arto/state` for it.

A `search` target runs the panel's own find rather than a search of its
own, so the hits stay highlighted where the reader can see them — a jump
that left no trace would put them somewhere they could not account for.
`index` counts hits from zero and out of range moves nothing.

`arto/selection` is the one method that cannot be answered from the main
thread: a selection lives in the WebView. The responder is parked in
`crates/arto/src/ipc/selection.rs` and the window answers it a round trip
later, which is why it is its own method rather than a field of
`arto/state` — that one is answered in the turn that asked, and every
field of it is already here.

## Ending a session

`shutdown` ends nothing by itself. It marks the connection: a request after
it is refused with `InvalidRequest`, and `exit` closes it. LSP's own
handshake, kept because an editor's client machinery sends it on the way
out and would otherwise read `MethodNotFound` as a fault.

A launch skips it — it sends `exit` alone. `shutdown` costs a round trip
and there is nothing the answer would change for a process that is exiting
anyway.

## `$/cancelRequest` and `$/progress` are deliberately absent

A connection is read one request at a time, each answered before the next
is read. So a `$/cancelRequest` naming an outstanding request cannot arrive
while it is outstanding — the loop is not reading — and one naming any
other request has nothing to cancel. Implementing it would mean answering
requests concurrently, which would buy nothing else: the only slow request
is a `waitReady` launch, and a launch that gives up closes its socket.

`$/progress` has nothing to report progress on for the same reason. The one
long operation says what happened in its own answer (`ready`), and a
progress notification between "asked" and "drew" would carry no information
the client does not already have.

**Do not add either without first making request handling concurrent**, and
do not make it concurrent for their sake alone.

## The instance speaks first

A client that declares `capabilities.stateChanges` in `initialize` is sent
`arto/didChangeState` whenever a window changes, carrying that window's
whole `WindowState` — or no state at all, which is how a close is reported.

Nothing is pushed at a client that did not ask: a launch handing over a path
declares nothing and is written to only in answer. That is what
`ClientCapabilities` is for, and why every field of it defaults to `false`.

Scrolling changes a window's state on every frame, so a change that only
moves the scroll is held for 150ms — `crates/arto/src/ipc/subscribers.rs`
releases the held one afterwards, because the last move of a scroll is the
one a client actually wants. Everything else goes out at once.

A connection's reads are bounded by `IPC_TIMEOUT` only until `initialize`
succeeds; after that the read timeout is lifted and only writes stay
bounded. A subscriber, or an MCP server holding one connection for a whole
session, has nothing to ask for long stretches, and a bound on its reads
would cut it off at the first quiet moment.

## The socket is not a trust boundary

It is reachable by any process running as the user, and the directory is
the only thing that narrows that (`0o700`, created in `server.rs`). There
is no authentication and nothing to add one to: a peer that can connect can
open files, read what is on screen and move the reader.

That is the same authority any process running as the user already has —
it can read those files directly and drive the app through the window
system. So the socket grants nothing new, and the rule that follows is that
**no capability may be added here that a process running as the user does
not already have.** Writing files, running commands, reaching the network
on the app's behalf: none of those belong on this socket.

## What the launch reports back

`SendResult` sorts the outcomes by one question — was the request already
delivered? — because that decides whether this process may go on to become
primary. Starting a second app after the request is already with a live
instance either opens the document twice or fails to bind and exits saying
single-instance enforcement is broken.

| Outcome | Delivered? | The launch |
| --- | --- | --- |
| `Applied { ready }` | yes | exits 0, or 1 when it waited and `ready` is false |
| `Refused(error)` | yes | prints the refusal, exits 1 |
| `Unanswered` | unknown | says so, exits 1 |
| `NoExistingInstance` | no | becomes primary |
| `Failed(error)` | no — nothing was written | becomes primary |

Only the last two become primary. Everything from the request being written
onward is reported rather than retried, because a retry is a second open.

Which window that is depends on what the request did, and
`crates/arto/src/ipc/ready.rs` is the whole rule: a request that reused a
window names it, a request that created one parks its signal for the next
window to mount. A signal dropped rather than fired releases the waiting
launch too, so a window that closes before it draws strands nothing.

## There is no second protocol

The bespoke line-delimited protocol this replaced is **gone**, not kept as
a fallback. The consequence is one upgrade window: a copy of Arto started
before the change does not understand a launch from after it, so the launch
reports no instance and then cannot become primary either, because the
socket is held. Nothing opens until the user quits and reopens Arto, and
then never again.

That was chosen over carrying the old protocol, which would have meant
keeping ~300 lines plus a rule for when they could go — indefinitely, to
smooth a window that closes the first time the app restarts.

**Do not add a compatibility path back.** If a launch cannot reach the
running instance, the right answer is a clear failure, not a second wire
format to keep in step with the first.

## Where it lives

| Path | What it is |
| --- | --- |
| `crates/arto-lsp/src/framing.rs` | `Content-Length` framing |
| `crates/arto-lsp/src/jsonrpc.rs` | The envelope: requests, responses, notifications, error codes |
| `crates/arto-lsp/src/methods.rs` | Arto's own methods and their parameters |
| `crates/arto-lsp/src/protocol.rs` | `OpenEvent`, `OpenRequest`, `WindowOptions`, `WindowKey` |
| `crates/arto-lsp/src/client.rs` | `send_to_existing_instance` |
| `crates/arto-lsp/src/server.rs` | `Server`, `ReadySignal` |
| `crates/arto-lsp/src/writer.rs` | `Writer` — a connection's write half, shared |
| `crates/arto/src/ipc.rs` | Queues received events, wakes the main thread, opens files in the right window |
| `crates/arto/src/ipc/state.rs` | Answers `arto/state` and routes `arto/goto` and `arto/selection` |
| `crates/arto/src/ipc/selection.rs` | Requests parked while a window is asked what is selected |
| `crates/arto/src/ipc/subscribers.rs` | Who gets `arto/didChangeState`, and how often |

`arto-lsp` is a library crate: no globals, debug-level logging only. It does
not know the app's version, so both ends are handed the `PeerInfo` they
report in `initialize` — `Server::serve` the primary's, and
`send_to_existing_instance` the launch's.

`arto.vim` and `arto.el` do not speak this protocol — both exec the `arto`
binary with a path and let the CLI's own handoff do the rest, so a protocol
change never reaches them.

`arto mcp` (`crates/arto-mcp/`) is a client of this protocol and nothing
more: each MCP tool is one request sent with `ask_existing_instance`, on a
connection of its own. A tool that needs something the socket cannot do
needs a method here first. The roots it limits `open` to are its own guard
against an agent's mistakes, not a trust boundary — see `docs/adr/20261114-let-an-mcp-client-open-only-what-it-was-started-on.md`.
