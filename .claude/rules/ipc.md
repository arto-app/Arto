---
paths: "crates/arto-ipc/**, crates/arto/src/ipc.rs, crates/arto/src/ipc/**, crates/arto/src/main.rs"
---

# Single-Instance Architecture

Arto runs as one process. A newly launched process:

1. tries to connect to the existing instance over the Unix domain socket;
2. if it connects, sends its paths as JSON Lines and exits with `0`;
3. otherwise becomes the primary instance and starts the IPC server.

Protocol (JSON Lines):

```json
{"type":"open","files":["/path/to/file.md"],"directory":null,"behavior":"last_focused","behind":false}
{"type":"open","files":[],"directory":"/path/to/dir","behavior":"new_window","behind":true}
{"type":"reopen","behavior":"last_focused","behind":false}
{"type":"reopen","behavior":"new_window","behind":false,"window":{"position":{"x":120,"y":64},"size":{"width":1400,"height":920},"theme":"light"},"wait_ready":true}
```

`behind` (from `arto --behind`) tells the primary to apply the request
without activating itself or moving the focus; it defaults to `false` when
a message omits it.

`window` (from `--position`, `--size`, `--theme`) says what the request
asks of the window it lands in, rather than what to read there. Each field
is optional and an absent one leaves the running instance's own answer
alone, so the object is omitted entirely when nothing was asked — which is
what keeps the line byte-identical to what a primary from before these
options existed expects. A `reopen` carries it too: naming no path is how a
window is asked for as a window.

`wait_ready` (from `--wait-ready`) is the one thing that is not about the
request at all but about the connection. The primary stops reading at a
message that sets it — such a client will not close the socket, because
that is what it is waiting on — applies what it has, and answers on the
same connection once the target window has drawn:

```json
{"type":"ready"}
{"type":"timeout"}
```

Which window that is depends on what the request did, and
`crates/arto/src/ipc/ready.rs` is the whole rule: a request that reused a
window names it, a request that created one parks its signal for the next
window to mount. A signal dropped rather than fired releases the waiting
launch too, so a window that closes before it draws strands nothing.

The older `file` and `directory` messages are still accepted from a
not-yet-upgraded secondary instance; neither can ask to wait.

Where it lives:

- `crates/arto-ipc/`: `IpcMessage`, `OpenEvent`, `send_to_existing_instance`,
  `IpcServer`. Library crate: no globals, debug-level logging only.
- `crates/arto/src/ipc.rs`: queues received events, wakes the main thread,
  opens files in the right window.

Why: several processes would fight over file watches, `config.json` writes
and `state.json`.
