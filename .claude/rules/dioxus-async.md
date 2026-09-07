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
- `utils::task::spawn_detached()`: one-shot work that has to finish even
  though the component that started it is gone. A scope's tasks are dropped
  when it unmounts, so a menu item that closes its menu and then awaits — a
  clipboard copy round-tripping through the WebView — loses the task at its
  first `.await`, silently. The dispatcher spawns every action this way,
  because actions are triggered from menus as well as from the keyboard.
- Avoid raw `spawn_forever()` in components: nothing stops the task, so a
  loop keeps writing to signals whose component is gone. `spawn_detached()`
  is the one-shot form that is safe to reach for.

Files:

- Canonicalize paths before comparing or storing them (macOS symlinks).
- The directory root of a file is its parent.
- The file watcher is thread-local; keep it off `Send`/`Sync` paths.

Longer discussion and past mistakes: `.claude/TIPS.md` (Dioxus Patterns,
File Operations).
