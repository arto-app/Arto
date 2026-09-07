//! Spawning for work that has to outlive the widget that started it.

use std::future::Future;

use dioxus::core::spawn_forever;

/// Spawn a one-shot task that keeps running once the component that started
/// it is gone.
///
/// Dioxus drops every task a scope spawned the moment that scope unmounts. A
/// menu item that closes its menu as part of the same click therefore loses
/// whatever it started: the task is cancelled at its first `.await`, so a
/// clipboard copy that round-trips through the WebView never comes back —
/// nothing is copied, no feedback is shown, and not even the error paths run,
/// because the code that would have reported the failure is dropped with it.
///
/// A detached task belongs to the window's root scope instead, so it runs to
/// completion and is cancelled only when the window itself goes away.
///
/// This is for work that should finish regardless of what is on screen —
/// clipboard copies, file dialogs, one JS round-trip. A listener that must
/// stop with its component still belongs in `use_future`.
pub fn spawn_detached(future: impl Future<Output = ()> + 'static) {
    spawn_forever(future);
}

#[cfg(test)]
mod tests {
    use super::*;

    use dioxus::core::NoOpMutations;
    use dioxus::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    /// What the child does on its first render, handed over by the test.
    struct TaskSetup {
        /// Spawn detached instead of on the child's own scope.
        detached: bool,
        /// Released by the test once the child is unmounted.
        gate: tokio::sync::oneshot::Receiver<()>,
        /// Set when the task gets past the gate.
        finished: Rc<Cell<bool>>,
    }

    thread_local! {
        /// Whether the child that spawns the task is still rendered.
        static CHILD_MOUNTED: Cell<bool> = const { Cell::new(true) };
        static TASK_SETUP: RefCell<Option<TaskSetup>> = const { RefCell::new(None) };
    }

    #[component]
    fn Child() -> Element {
        use_hook(|| {
            let TaskSetup {
                detached,
                gate,
                finished,
            } = TASK_SETUP
                .with(|slot| slot.borrow_mut().take())
                .expect("task setup handed over before the first render");
            let body = async move {
                let _ = gate.await;
                finished.set(true);
            };
            if detached {
                spawn_detached(body);
            } else {
                spawn(body);
            }
        });
        rsx! { div {} }
    }

    #[component]
    fn Parent() -> Element {
        rsx! {
            if CHILD_MOUNTED.with(Cell::get) {
                Child {}
            }
        }
    }

    /// Poll the dom until it runs out of work, or the timeout expires.
    async fn settle(dom: &mut VirtualDom) {
        tokio::select! {
            _ = dom.wait_for_work() => {}
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }

    /// Spawn a task from the child, unmount the child, then let the task
    /// finish. Answers whether it ever got there.
    async fn task_finishes_after_unmount(detached: bool) -> bool {
        let finished = Rc::new(Cell::new(false));
        let (release, gate) = tokio::sync::oneshot::channel();
        CHILD_MOUNTED.with(|mounted| mounted.set(true));
        TASK_SETUP.with(|slot| {
            *slot.borrow_mut() = Some(TaskSetup {
                detached,
                gate,
                finished: Rc::clone(&finished),
            });
        });

        let mut dom = VirtualDom::new(Parent);
        dom.rebuild(&mut NoOpMutations);
        settle(&mut dom).await;

        CHILD_MOUNTED.with(|mounted| mounted.set(false));
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut NoOpMutations);

        let _ = release.send(());
        settle(&mut dom).await;

        finished.get()
    }

    #[tokio::test]
    async fn detached_task_survives_the_component_that_spawned_it() {
        assert!(task_finishes_after_unmount(true).await);
    }

    /// The reason [`spawn_detached`] exists: the plain `spawn` a component
    /// would otherwise reach for dies with the component.
    #[tokio::test]
    async fn scope_bound_task_dies_with_the_component_that_spawned_it() {
        assert!(!task_finishes_after_unmount(false).await);
    }
}
