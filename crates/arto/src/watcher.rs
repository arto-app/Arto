use notify_debouncer_full::{
    new_debouncer, notify::RecursiveMode, DebounceEventResult, Debouncer, RecommendedCache,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
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
    WatchDirectoryNonRecursive(PathBuf, Sender<()>),
    UnwatchDirectoryNonRecursive(PathBuf),
}

impl FileWatcher {
    fn new() -> Self {
        let (command_tx, mut command_rx) = mpsc::channel::<FileWatcherCommand>(100);

        // Spawn a dedicated thread for the file watcher
        std::thread::spawn(move || {
            // Map of file paths to their notification channels
            let file_watchers: Arc<Mutex<HashMap<PathBuf, Vec<Sender<()>>>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let file_watchers_clone = file_watchers.clone();

            // Map of directory paths to their notification channels (non-recursive watching)
            let non_recursive_dir_watchers: Arc<Mutex<HashMap<PathBuf, Vec<Sender<()>>>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let non_recursive_dir_watchers_clone = non_recursive_dir_watchers.clone();

            // Create a debouncer with 500ms delay
            let mut debouncer: Debouncer<
                notify_debouncer_full::notify::RecommendedWatcher,
                RecommendedCache,
            > = match new_debouncer(
                Duration::from_millis(500),
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => {
                        // Collect unique paths that changed
                        let mut changed_paths = std::collections::HashSet::new();
                        for event in events {
                            for path in &event.paths {
                                changed_paths.insert(path.clone());
                            }
                        }

                        // Notify file watchers (exact path match)
                        let file_watchers = file_watchers_clone.lock().unwrap();
                        for path in &changed_paths {
                            if let Some(senders) = file_watchers.get(path) {
                                tracing::debug!("File changed: {:?}", path);
                                for sender in senders {
                                    let _ = sender.blocking_send(());
                                }
                            }
                        }
                        drop(file_watchers);

                        // Notify non-recursive directory watchers (direct children only)
                        let non_recursive_dir_watchers =
                            non_recursive_dir_watchers_clone.lock().unwrap();
                        let mut notified_non_recursive_dirs = std::collections::HashSet::new();
                        for changed_path in &changed_paths {
                            // Skip .git directory changes (too noisy)
                            if changed_path.components().any(|c| c.as_os_str() == ".git") {
                                continue;
                            }

                            for (watched_dir, senders) in non_recursive_dir_watchers.iter() {
                                let is_direct_child = changed_path
                                    .parent()
                                    .is_some_and(|parent| parent == watched_dir.as_path());
                                let is_watched_dir_itself = changed_path == watched_dir;

                                if (is_direct_child || is_watched_dir_itself)
                                    && !notified_non_recursive_dirs.contains(watched_dir)
                                {
                                    tracing::trace!(
                                        ?watched_dir,
                                        ?changed_path,
                                        "Direct directory content changed"
                                    );
                                    for sender in senders {
                                        let _ = sender.blocking_send(());
                                    }
                                    notified_non_recursive_dirs.insert(watched_dir.clone());
                                }
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::error!("File watcher error: {:?}", error);
                        }
                    }
                },
            ) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to create file watcher: {:?}", e);
                    return;
                }
            };

            tracing::info!("Global file watcher started");

            // Process commands
            loop {
                match command_rx.blocking_recv() {
                    Some(FileWatcherCommand::Watch(path, tx)) => {
                        let mut watchers = file_watchers.lock().unwrap();
                        let is_first = !watchers.contains_key(&path);

                        watchers.entry(path.clone()).or_default().push(tx);

                        // Only start watching if this is the first watcher for this file
                        if is_first {
                            if let Err(e) = debouncer.watch(&path, RecursiveMode::NonRecursive) {
                                tracing::error!("Failed to watch file {:?}: {:?}", path, e);
                            } else {
                                tracing::info!("Started watching file: {:?}", path);
                            }
                        }
                    }
                    Some(FileWatcherCommand::Unwatch(path)) => {
                        let mut watchers = file_watchers.lock().unwrap();
                        if let Some(senders) = watchers.get_mut(&path) {
                            senders.pop();
                            // If no more watchers for this file, stop watching
                            if senders.is_empty() {
                                watchers.remove(&path);
                                if let Err(e) = debouncer.unwatch(&path) {
                                    tracing::error!("Failed to unwatch file {:?}: {:?}", path, e);
                                } else {
                                    tracing::info!("Stopped watching file: {:?}", path);
                                }
                            }
                        }
                    }
                    Some(FileWatcherCommand::WatchDirectoryNonRecursive(path, tx)) => {
                        let mut watchers = non_recursive_dir_watchers.lock().unwrap();
                        let is_first = !watchers.contains_key(&path);

                        watchers.entry(path.clone()).or_default().push(tx);

                        // Only start watching if this is the first watcher for this directory
                        if is_first {
                            if let Err(e) = debouncer.watch(&path, RecursiveMode::NonRecursive) {
                                tracing::error!("Failed to watch directory {:?}: {:?}", path, e);
                            } else {
                                tracing::info!(
                                    "Started watching directory (non-recursive): {:?}",
                                    path
                                );
                            }
                        }
                    }
                    Some(FileWatcherCommand::UnwatchDirectoryNonRecursive(path)) => {
                        let mut watchers = non_recursive_dir_watchers.lock().unwrap();
                        if let Some(senders) = watchers.get_mut(&path) {
                            senders.pop();
                            // If no more watchers for this directory, stop watching
                            if senders.is_empty() {
                                watchers.remove(&path);
                                if let Err(e) = debouncer.unwatch(&path) {
                                    tracing::error!(
                                        "Failed to unwatch directory {:?}: {:?}",
                                        path,
                                        e
                                    );
                                } else {
                                    tracing::info!("Stopped watching directory: {:?}", path);
                                }
                            }
                        }
                    }
                    None => {
                        tracing::info!("File watcher command channel closed");
                        break;
                    }
                }
            }
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

    /// Watch a directory non-recursively and receive notifications for direct children.
    pub async fn watch_directory_non_recursive(
        &self,
        path: impl Into<PathBuf>,
    ) -> WatcherResult<Receiver<()>> {
        let path = path.into();
        let (tx, rx) = mpsc::channel(100);
        self.command_tx
            .send(FileWatcherCommand::WatchDirectoryNonRecursive(path, tx))
            .await
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
