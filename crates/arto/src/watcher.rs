use dioxus::core::Task;
use dioxus::prelude::*;
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use parking_lot::Mutex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("Failed to send watcher command")]
    CommandFailed,
}

type WatcherResult<T> = Result<T, WatcherError>;

/// Global file watcher that manages file change notifications
pub struct FileWatcher {
    command_tx: UnboundedSender<FileWatcherCommand>,
}

enum FileWatcherCommand {
    Watch(PathBuf, Sender<()>),
    Release(PathBuf),
    WatchDirectoryNonRecursive(PathBuf, Sender<Vec<PathBuf>>),
    ReleaseDirectoryNonRecursive(PathBuf),
}

type Senders<T> = HashMap<PathBuf, Vec<Sender<T>>>;

/// Who is told about a change: a file's watchers about the file itself, a
/// directory's about its direct children and the directory itself.
#[derive(Default)]
struct Watchers {
    files: Senders<()>,
    directories: Senders<Vec<PathBuf>>,
}

impl Watchers {
    fn notify(&self, changed: &HashSet<PathBuf>) {
        for path in self.changed_files(changed) {
            tracing::debug!(?path, "File changed");
            for sender in &self.files[path] {
                let _ = sender.blocking_send(());
            }
        }
        for (directory, paths) in self.changed_directories(changed) {
            tracing::trace!(?directory, ?paths, "Direct directory content changed");
            for sender in &self.directories[directory] {
                let _ = sender.blocking_send(paths.clone());
            }
        }
    }

    fn changed_files<'a>(&'a self, changed: &'a HashSet<PathBuf>) -> Vec<&'a PathBuf> {
        changed
            .iter()
            .filter(|path| self.files.contains_key(*path))
            .collect()
    }

    /// Each watched directory something changed in, with what changed in it,
    /// in path order. Changes under `.git` are left out: a repository's
    /// bookkeeping churns on every command and is never something a reader
    /// is shown.
    fn changed_directories(&self, changed: &HashSet<PathBuf>) -> Vec<(&PathBuf, Vec<PathBuf>)> {
        let mut found: Vec<(&PathBuf, Vec<PathBuf>)> = self
            .directories
            .keys()
            .filter_map(|directory| {
                let paths: BTreeSet<&PathBuf> = changed
                    .iter()
                    .filter(|path| !path.components().any(|c| c.as_os_str() == ".git"))
                    .filter(|path| *path == directory || path.parent() == Some(directory))
                    .collect();
                (!paths.is_empty()).then(|| (directory, paths.into_iter().cloned().collect()))
            })
            .collect();
        found.sort_by_key(|(directory, _)| *directory);
        found
    }
}

/// Add `tx` to the watchers of `path`, and say whether it is the first.
fn add<T>(senders: &mut Senders<T>, path: &Path, tx: Sender<T>) -> bool {
    let is_first = !senders.contains_key(path);
    senders.entry(path.to_path_buf()).or_default().push(tx);
    is_first
}

/// Forget the watchers of `path` that stopped listening, and say whether that
/// left it with none.
fn prune<T>(senders: &mut Senders<T>, path: &Path) -> bool {
    let Some(watchers) = senders.get_mut(path) else {
        return false;
    };
    watchers.retain(|sender| !sender.is_closed());
    let is_last = watchers.is_empty();
    if is_last {
        senders.remove(path);
    }
    is_last
}

type WatcherDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;

fn route_events(watchers: &Mutex<Watchers>, result: DebounceEventResult) {
    match result {
        Ok(events) => {
            let changed: HashSet<PathBuf> = events
                .into_iter()
                .flat_map(|event| event.event.paths)
                .collect();
            watchers.lock().notify(&changed);
        }
        Err(errors) => {
            for error in errors {
                tracing::error!(?error, "File watcher error");
            }
        }
    }
}

/// What a command asks of the file system watcher, once the books are kept.
enum Action {
    Start(PathBuf, &'static str),
    Stop(PathBuf, &'static str),
}

fn apply(watchers: &mut Watchers, command: FileWatcherCommand) -> Option<Action> {
    match command {
        FileWatcherCommand::Watch(path, tx) => {
            add(&mut watchers.files, &path, tx).then_some(Action::Start(path, "file"))
        }
        FileWatcherCommand::Release(path) => {
            prune(&mut watchers.files, &path).then_some(Action::Stop(path, "file"))
        }
        FileWatcherCommand::WatchDirectoryNonRecursive(path, tx) => {
            add(&mut watchers.directories, &path, tx).then_some(Action::Start(path, "directory"))
        }
        FileWatcherCommand::ReleaseDirectoryNonRecursive(path) => {
            prune(&mut watchers.directories, &path).then_some(Action::Stop(path, "directory"))
        }
    }
}

fn run_commands(
    debouncer: &mut WatcherDebouncer,
    watchers: &Mutex<Watchers>,
    mut commands: UnboundedReceiver<FileWatcherCommand>,
) {
    while let Some(command) = commands.blocking_recv() {
        // Unlocked before the watcher is touched, which may wait on the
        // thread that delivers events and takes the same lock.
        let action = apply(&mut watchers.lock(), command);
        match action {
            Some(Action::Start(path, kind)) => start_watching(debouncer, &path, kind),
            Some(Action::Stop(path, kind)) => stop_watching(debouncer, &path, kind),
            None => {}
        }
    }
    tracing::info!("File watcher command channel closed");
}

fn start_watching(debouncer: &mut WatcherDebouncer, path: &Path, kind: &str) {
    match debouncer.watch(path, RecursiveMode::NonRecursive) {
        Ok(()) => tracing::info!(?path, kind, "Started watching"),
        Err(error) => tracing::error!(?path, kind, ?error, "Failed to watch"),
    }
}

fn stop_watching(debouncer: &mut WatcherDebouncer, path: &Path, kind: &str) {
    match debouncer.unwatch(path) {
        Ok(()) => tracing::info!(?path, kind, "Stopped watching"),
        Err(error) => tracing::error!(?path, kind, ?error, "Failed to unwatch"),
    }
}

impl FileWatcher {
    fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel::<FileWatcherCommand>();

        std::thread::spawn(move || {
            let watchers = Arc::new(Mutex::new(Watchers::default()));
            let debouncer = new_debouncer(Duration::from_millis(500), None, {
                let watchers = Arc::clone(&watchers);
                move |result| route_events(&watchers, result)
            });
            let mut debouncer = match debouncer {
                Ok(debouncer) => debouncer,
                Err(error) => {
                    tracing::error!(?error, "Failed to create file watcher");
                    return;
                }
            };
            tracing::info!("Global file watcher started");
            run_commands(&mut debouncer, &watchers, command_rx);
        });

        Self { command_tx }
    }

    /// Watch a file and receive notifications when it changes
    pub fn watch(&self, path: impl Into<PathBuf>) -> WatcherResult<Watch<()>> {
        self.register(
            path.into(),
            FileWatcherCommand::Watch,
            FileWatcherCommand::Release,
        )
    }

    /// Watch a directory non-recursively and receive, for each batch of
    /// changes, the direct children that changed (or the directory itself).
    pub fn watch_directory_non_recursive(
        &self,
        path: impl Into<PathBuf>,
    ) -> WatcherResult<Watch<Vec<PathBuf>>> {
        self.register(
            path.into(),
            FileWatcherCommand::WatchDirectoryNonRecursive,
            FileWatcherCommand::ReleaseDirectoryNonRecursive,
        )
    }

    fn register<T>(
        &self,
        path: PathBuf,
        watch: fn(PathBuf, Sender<T>) -> FileWatcherCommand,
        release: fn(PathBuf) -> FileWatcherCommand,
    ) -> WatcherResult<Watch<T>> {
        let (tx, changes) = mpsc::channel(100);
        self.command_tx
            .send(watch(path.clone(), tx))
            .map_err(|_| WatcherError::CommandFailed)?;
        Ok(Watch {
            path,
            changes,
            commands: self.command_tx.clone(),
            release,
        })
    }
}

/// One listener's hold on a path. The path stays watched for as long as any
/// hold on it lives; dropping this one gives it up.
pub struct Watch<T> {
    path: PathBuf,
    changes: Receiver<T>,
    commands: UnboundedSender<FileWatcherCommand>,
    release: fn(PathBuf) -> FileWatcherCommand,
}

impl<T> Watch<T> {
    pub async fn recv(&mut self) -> Option<T> {
        self.changes.recv().await
    }

    /// [`Watch::recv`] from a plain thread, outside any async runtime.
    pub fn blocking_recv(&mut self) -> Option<T> {
        self.changes.blocking_recv()
    }
}

impl<T> Drop for Watch<T> {
    fn drop(&mut self) {
        // Closed before the release is sent, so that the watcher can tell
        // this listener from the others on the same path.
        self.changes.close();
        let path = std::mem::take(&mut self.path);
        // Fails only once the watcher thread has gone, taking every watch.
        let _ = self.commands.send((self.release)(path));
    }
}

/// Listen to what `path` names for as long as the calling component shows it.
pub fn use_watch<T: 'static>(
    path: ReadSignal<PathBuf>,
    watch: impl Fn(PathBuf) -> WatcherResult<Watch<T>> + 'static,
    on_change: impl FnMut(T) + Clone + 'static,
) {
    let mut listening = use_hook(|| CopyValue::new(None::<Task>));
    use_effect(move || {
        let path = path();
        // The task holds the watch, so cancelling it is what releases the
        // path shown before. A component going away cancels it unasked.
        if let Some(previous) = listening.take() {
            previous.cancel();
        }
        let mut changes = match watch(path.clone()) {
            Ok(changes) => changes,
            Err(error) => {
                tracing::error!(?path, %error, "Failed to register a watch");
                return;
            }
        };
        let mut on_change = on_change.clone();
        listening.set(Some(spawn(async move {
            while let Some(change) = changes.recv().await {
                on_change(change);
            }
        })));
    });
}

pub static FILE_WATCHER: LazyLock<FileWatcher> = LazyLock::new(FileWatcher::new);

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::core::{NoOpMutations, ScopeId};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A watcher with no file system behind it: what it was asked is applied
    /// to the books when they are read.
    struct Books {
        commands: UnboundedReceiver<FileWatcherCommand>,
        watchers: Watchers,
    }

    impl Books {
        fn open() -> (&'static FileWatcher, Self) {
            let (command_tx, commands) = mpsc::unbounded_channel();
            let watcher = Box::leak(Box::new(FileWatcher { command_tx }));
            let books = Self {
                commands,
                watchers: Watchers::default(),
            };
            (watcher, books)
        }

        fn settle(&mut self) {
            while let Ok(command) = self.commands.try_recv() {
                apply(&mut self.watchers, command);
            }
        }

        fn files(&mut self) -> Vec<PathBuf> {
            self.settle();
            let mut held: Vec<PathBuf> = self.watchers.files.keys().cloned().collect();
            held.sort();
            held
        }

        fn directories(&mut self) -> Vec<PathBuf> {
            self.settle();
            let mut held: Vec<PathBuf> = self.watchers.directories.keys().cloned().collect();
            held.sort();
            held
        }

        fn change(&mut self, path: &str) {
            self.settle();
            let changed = paths(&[path]);
            // Delivery blocks, which an async test's thread may not.
            std::thread::scope(|scope| {
                scope.spawn(|| self.watchers.notify(&changed));
            });
        }
    }

    fn held(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    /// What the window under test shows, and what its views were told.
    #[derive(Clone)]
    struct Window {
        watcher: &'static FileWatcher,
        showing: Rc<RefCell<Option<PathBuf>>>,
        heard: Rc<RefCell<Vec<PathBuf>>>,
    }

    #[component]
    fn View(path: ReadSignal<PathBuf>) -> Element {
        let window = use_context::<Window>();
        let watcher = window.watcher;
        let heard = window.heard.clone();
        use_watch(
            path,
            move |path| watcher.watch(path),
            move |()| heard.borrow_mut().push(path.peek().clone()),
        );
        rsx! {}
    }

    fn window_root(window: Window) -> Element {
        let showing = window.showing.borrow().clone();
        use_context_provider(|| window);
        rsx! {
            if let Some(path) = showing {
                View { path }
            }
        }
    }

    async fn run(dom: &mut VirtualDom) {
        for _ in 0..3 {
            tokio::select! {
                _ = dom.wait_for_work() => {}
                _ = tokio::time::sleep(Duration::from_millis(20)) => {}
            }
            dom.render_immediate(&mut NoOpMutations);
        }
    }

    async fn open_window(watcher: &'static FileWatcher, path: &str) -> (VirtualDom, Window) {
        let window = Window {
            watcher,
            showing: Rc::new(RefCell::new(Some(PathBuf::from(path)))),
            heard: Rc::default(),
        };
        let mut dom = VirtualDom::new_with_props(window_root, window.clone());
        dom.rebuild(&mut NoOpMutations);
        run(&mut dom).await;
        (dom, window)
    }

    async fn show(dom: &mut VirtualDom, window: &Window, path: Option<&str>) {
        *window.showing.borrow_mut() = path.map(PathBuf::from);
        dom.mark_dirty(ScopeId::APP);
        run(dom).await;
    }

    #[test]
    fn dropping_a_watch_releases_what_it_watched() {
        let (watcher, mut books) = Books::open();
        let file = watcher.watch("/w/a.md").unwrap();
        let directory = watcher.watch_directory_non_recursive("/w").unwrap();
        assert_eq!(books.files(), held(&["/w/a.md"]));
        assert_eq!(books.directories(), held(&["/w"]));

        drop(file);
        drop(directory);

        assert_eq!(books.files(), held(&[]));
        assert_eq!(books.directories(), held(&[]));
    }

    /// Whichever of two holders lets go, the other goes on hearing.
    #[test]
    fn a_path_held_twice_stays_watched_for_the_holder_that_stays() {
        for first_leaves in [true, false] {
            let (watcher, mut books) = Books::open();
            let first = watcher.watch("/w/a.md").unwrap();
            let second = watcher.watch("/w/a.md").unwrap();
            let (leaving, mut staying) = if first_leaves {
                (first, second)
            } else {
                (second, first)
            };

            drop(leaving);
            books.change("/w/a.md");

            assert_eq!(books.files(), held(&["/w/a.md"]), "{first_leaves}");
            assert_eq!(staying.changes.try_recv(), Ok(()), "{first_leaves}");
        }
    }

    #[tokio::test]
    async fn showing_another_document_releases_the_one_before() {
        let (watcher, mut books) = Books::open();
        let (mut dom, window) = open_window(watcher, "/w/a.md").await;
        assert_eq!(books.files(), held(&["/w/a.md"]));

        show(&mut dom, &window, Some("/w/b.md")).await;

        assert_eq!(books.files(), held(&["/w/b.md"]));
        books.change("/w/a.md");
        run(&mut dom).await;
        assert!(window.heard.borrow().is_empty());
        books.change("/w/b.md");
        run(&mut dom).await;
        assert_eq!(*window.heard.borrow(), held(&["/w/b.md"]));
    }

    #[tokio::test]
    async fn a_view_that_goes_away_releases_its_document() {
        let (watcher, mut books) = Books::open();
        let (mut dom, window) = open_window(watcher, "/w/a.md").await;

        show(&mut dom, &window, None).await;

        assert_eq!(books.files(), held(&[]));
    }

    #[tokio::test]
    async fn closing_one_of_two_windows_on_a_document_keeps_it_watched() {
        let (watcher, mut books) = Books::open();
        let (first, _) = open_window(watcher, "/w/a.md").await;
        let (mut second, window) = open_window(watcher, "/w/a.md").await;

        drop(first);
        books.change("/w/a.md");
        run(&mut second).await;

        assert_eq!(books.files(), held(&["/w/a.md"]));
        assert_eq!(*window.heard.borrow(), held(&["/w/a.md"]));

        drop(second);
        assert_eq!(books.files(), held(&[]));
    }

    // Three times the debounce, so what the watcher holds has been handed over.
    const SETTLED: Duration = Duration::from_millis(1500);

    fn paths(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    fn watching(files: &[&str], directories: &[&str]) -> Watchers {
        let mut watchers = Watchers::default();
        for file in files {
            add(&mut watchers.files, Path::new(file), mpsc::channel(1).0);
        }
        for directory in directories {
            add(
                &mut watchers.directories,
                Path::new(directory),
                mpsc::channel(1).0,
            );
        }
        watchers
    }

    /// A file under the real watcher, with what its creation reports gone by.
    fn watched_file() -> (tempfile::TempDir, PathBuf, Receiver<()>) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().canonicalize().unwrap().join("a.md");
        std::fs::write(&file, "# Before\n").unwrap();

        let (tx, mut rx) = mpsc::channel(100);
        FILE_WATCHER
            .command_tx
            .send(FileWatcherCommand::Watch(file.clone(), tx))
            .unwrap();
        std::thread::sleep(SETTLED);
        while rx.try_recv().is_ok() {}

        (dir, file, rx)
    }

    fn told_within(rx: &mut Receiver<()>, limit: Duration) -> bool {
        let deadline = std::time::Instant::now() + limit;
        while std::time::Instant::now() < deadline {
            if rx.try_recv().is_ok() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn a_file_is_told_about_itself_alone() {
        let watchers = watching(&["/w/a.md"], &[]);

        let changed = paths(&["/w/a.md", "/w/b.md"]);

        assert_eq!(
            watchers.changed_files(&changed),
            vec![&PathBuf::from("/w/a.md")]
        );
    }

    #[test]
    fn a_directory_is_told_which_of_its_children_changed() {
        let watchers = watching(&[], &["/w"]);

        let changed = paths(&["/w/b.md", "/w/a.md", "/w/sub/deep.md", "/elsewhere/c.md"]);

        assert_eq!(
            watchers.changed_directories(&changed),
            vec![(
                &PathBuf::from("/w"),
                vec![PathBuf::from("/w/a.md"), PathBuf::from("/w/b.md")]
            )]
        );
    }

    #[test]
    fn a_directory_is_told_about_itself() {
        let watchers = watching(&[], &["/w"]);

        let changed = paths(&["/w"]);

        assert_eq!(
            watchers.changed_directories(&changed),
            vec![(&PathBuf::from("/w"), vec![PathBuf::from("/w")])]
        );
    }

    #[test]
    fn a_repository_s_bookkeeping_is_not_a_change() {
        let watchers = watching(&[], &["/w", "/w/.git"]);

        let changed = paths(&["/w/.git", "/w/.git/index"]);

        assert!(watchers.changed_directories(&changed).is_empty());
    }

    #[test]
    fn only_the_first_watcher_starts_a_watch_and_only_the_last_stops_it() {
        let mut senders: Senders<()> = HashMap::new();
        let path = Path::new("/w/a.md");

        let (first, first_listens) = mpsc::channel(1);
        let (second, second_listens) = mpsc::channel(1);
        assert!(add(&mut senders, path, first));
        assert!(!add(&mut senders, path, second));

        drop(first_listens);
        assert!(!prune(&mut senders, path));
        drop(second_listens);
        assert!(prune(&mut senders, path));
        assert!(!prune(&mut senders, path));
    }

    #[test]
    fn what_changed_reaches_the_directory_s_watchers() {
        let mut watchers = Watchers::default();
        let (tx, mut rx) = mpsc::channel(1);
        add(&mut watchers.directories, Path::new("/w"), tx);

        watchers.notify(&paths(&["/w/a.md"]));

        assert_eq!(rx.try_recv().unwrap(), vec![PathBuf::from("/w/a.md")]);
    }

    #[test]
    fn a_watched_file_being_read_tells_its_watcher_nothing() {
        let (_dir, file, mut rx) = watched_file();

        std::fs::read_to_string(&file).unwrap();

        assert!(!told_within(&mut rx, SETTLED));
    }

    #[test]
    fn a_watched_file_being_written_tells_its_watcher() {
        let (_dir, file, mut rx) = watched_file();

        std::fs::write(&file, "# After\n").unwrap();

        assert!(told_within(&mut rx, SETTLED * 4));
    }
}
