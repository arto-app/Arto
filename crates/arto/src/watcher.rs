use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use parking_lot::Mutex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("Failed to send watcher command")]
    CommandFailed,
}

type WatcherResult<T> = Result<T, WatcherError>;

/// Global file watcher that manages file change notifications
pub struct FileWatcher {
    command_tx: Sender<FileWatcherCommand>,
}

enum FileWatcherCommand {
    Watch(PathBuf, Sender<()>),
    Unwatch(PathBuf),
    WatchDirectoryNonRecursive(PathBuf, Sender<Vec<PathBuf>>),
    UnwatchDirectoryNonRecursive(PathBuf),
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

/// Drop one watcher of `path`, and say whether that was the last.
fn remove<T>(senders: &mut Senders<T>, path: &Path) -> bool {
    let Some(watchers) = senders.get_mut(path) else {
        return false;
    };
    watchers.pop();
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

fn run_commands(
    debouncer: &mut WatcherDebouncer,
    watchers: &Mutex<Watchers>,
    mut commands: Receiver<FileWatcherCommand>,
) {
    while let Some(command) = commands.blocking_recv() {
        match command {
            FileWatcherCommand::Watch(path, tx) => {
                if add(&mut watchers.lock().files, &path, tx) {
                    start_watching(debouncer, &path, "file");
                }
            }
            FileWatcherCommand::Unwatch(path) => {
                if remove(&mut watchers.lock().files, &path) {
                    stop_watching(debouncer, &path, "file");
                }
            }
            FileWatcherCommand::WatchDirectoryNonRecursive(path, tx) => {
                if add(&mut watchers.lock().directories, &path, tx) {
                    start_watching(debouncer, &path, "directory");
                }
            }
            FileWatcherCommand::UnwatchDirectoryNonRecursive(path) => {
                if remove(&mut watchers.lock().directories, &path) {
                    stop_watching(debouncer, &path, "directory");
                }
            }
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
        let (command_tx, command_rx) = mpsc::channel::<FileWatcherCommand>(100);

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
    pub async fn watch(&self, path: impl Into<PathBuf>) -> WatcherResult<Receiver<()>> {
        let path = path.into();
        let (tx, rx) = mpsc::channel(100);
        self.command_tx
            .send(FileWatcherCommand::Watch(path, tx))
            .await
            .map_err(|_| WatcherError::CommandFailed)?;
        Ok(rx)
    }

    /// Stop watching a file
    pub async fn unwatch(&self, path: impl Into<PathBuf>) -> WatcherResult<()> {
        let path = path.into();
        self.command_tx
            .send(FileWatcherCommand::Unwatch(path))
            .await
            .map_err(|_| WatcherError::CommandFailed)
    }

    /// Watch a directory non-recursively and receive, for each batch of
    /// changes, the direct children that changed (or the directory itself).
    pub async fn watch_directory_non_recursive(
        &self,
        path: impl Into<PathBuf>,
    ) -> WatcherResult<Receiver<Vec<PathBuf>>> {
        let path = path.into();
        let (tx, rx) = mpsc::channel(100);
        self.command_tx
            .send(FileWatcherCommand::WatchDirectoryNonRecursive(path, tx))
            .await
            .map_err(|_| WatcherError::CommandFailed)?;
        Ok(rx)
    }

    /// [`FileWatcher::watch_directory_non_recursive`] from a plain thread,
    /// outside any async runtime.
    pub fn watch_directory_non_recursive_blocking(
        &self,
        path: impl Into<PathBuf>,
    ) -> WatcherResult<Receiver<Vec<PathBuf>>> {
        let path = path.into();
        let (tx, rx) = mpsc::channel(100);
        self.command_tx
            .blocking_send(FileWatcherCommand::WatchDirectoryNonRecursive(path, tx))
            .map_err(|_| WatcherError::CommandFailed)?;
        Ok(rx)
    }

    /// Stop watching a directory non-recursively
    pub async fn unwatch_directory_non_recursive(
        &self,
        path: impl Into<PathBuf>,
    ) -> WatcherResult<()> {
        let path = path.into();
        self.command_tx
            .send(FileWatcherCommand::UnwatchDirectoryNonRecursive(path))
            .await
            .map_err(|_| WatcherError::CommandFailed)
    }
}

pub static FILE_WATCHER: LazyLock<FileWatcher> = LazyLock::new(FileWatcher::new);

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(add(&mut senders, path, mpsc::channel(1).0));
        assert!(!add(&mut senders, path, mpsc::channel(1).0));

        assert!(!remove(&mut senders, path));
        assert!(remove(&mut senders, path));
        assert!(!remove(&mut senders, path));
    }

    #[test]
    fn what_changed_reaches_the_directory_s_watchers() {
        let mut watchers = Watchers::default();
        let (tx, mut rx) = mpsc::channel(1);
        add(&mut watchers.directories, Path::new("/w"), tx);

        watchers.notify(&paths(&["/w/a.md"]));

        assert_eq!(rx.try_recv().unwrap(), vec![PathBuf::from("/w/a.md")]);
    }
}
