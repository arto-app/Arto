---
paths: "crates/arto-lsp/**, crates/arto/src/ipc.rs, crates/arto/src/ipc/**, crates/arto/src/main.rs"
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
| `crates/arto-lsp/src/protocol.rs` | `OpenEvent`, `OpenRequest`, `WindowOptions` |
| `crates/arto-lsp/src/client.rs` | `send_to_existing_instance` |
| `crates/arto-lsp/src/server.rs` | `Server`, `ReadySignal` |
| `crates/arto/src/ipc.rs` | Queues received events, wakes the main thread, opens files in the right window |

`arto-lsp` is a library crate: no globals, debug-level logging only. It does
not know the app's version, so both ends are handed the `PeerInfo` they
report in `initialize` — `Server::serve` the primary's, and
`send_to_existing_instance` the launch's.

`arto.vim` and `arto.el` do not speak this protocol — both exec the `arto`
binary with a path and let the CLI's own handoff do the rest, so a protocol
change never reaches them.
