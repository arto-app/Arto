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
```

`behind` (from `arto --behind`) tells the primary to apply the request
without activating itself or moving the focus; it defaults to `false` when
a message omits it.

The older `file` and `directory` messages are still accepted from a
not-yet-upgraded secondary instance.

Where it lives:

- `crates/arto-ipc/`: `IpcMessage`, `OpenEvent`, `send_to_existing_instance`,
  `IpcServer`. Library crate: no globals, debug-level logging only.
- `crates/arto/src/ipc.rs`: queues received events, wakes the main thread,
  opens files in the right window.

Why: several processes would fight over file watches, `config.json` writes
and `state.json`.
