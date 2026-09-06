---
paths: "crates/arto/src/**/*.rs"
---

# Dioxus Async and File Patterns

- `spawn()`: event handlers and one-shot async work.
- `use_effect()`: react to state; reads inside the closure subscribe it. Take
  changing props as `ReadSignal<T>` instead of `use_reactive!`.
- `use_future()`: long-running listeners tied to the component (broadcast
  subscriptions); cancelled when the component drops. It takes
  `FnMut() -> impl Future`, so the closure stays `move || async move { … }`:
  an `async move ||` closure that touches a capture does not implement
  `FnMut` on the current toolchain.
- `use_drop()`: synchronous cleanup only; call blocking `save()` directly.
- Avoid `spawn_forever()` in components: the task outlives the window and
  keeps writing to dropped signals.

Files:

- Canonicalize paths before comparing or storing them (macOS symlinks).
- The directory root of a file is its parent.
- The file watcher is thread-local; keep it off `Send`/`Sync` paths.

Longer discussion and past mistakes: `.claude/TIPS.md` (Dioxus Patterns,
File Operations).
