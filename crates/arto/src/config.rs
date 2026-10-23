//! The live configuration of the running app.
//!
//! The types and the on-disk format live in `arto-config`; this module holds
//! the single loaded instance every window reads and the channel that tells
//! them when it changed.

pub use arto_config::*;

use crate::watcher::FILE_WATCHER;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::broadcast;

/// Global configuration instance.
///
/// A configuration that cannot be read or parsed is reported once and
/// replaced by the defaults, so a typo in `config.json` never keeps the app
/// from starting. The fallback carries the default keybinding preset;
/// `Config::default()` alone has none, which would leave the app without
/// shortcuts.
pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| {
    let config = Config::load().unwrap_or_else(|error| {
        tracing::warn!(%error, "Falling back to the default configuration");
        Config::default_with_keybindings()
    });
    RwLock::new(config)
});

/// Broadcast channel to notify all windows when config changes.
/// Subscribers call `.subscribe()` to get a receiver.
pub static CONFIG_CHANGED_BROADCAST: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);

/// Read `config.json` and `mappings.json` again whenever either changes on
/// disk, and tell every window when that changed the configuration.
///
/// Some settings — lenses among them — have no place in the preferences
/// window and are only ever written by hand, so an edit made in another
/// program has to reach the running app as one made in the preferences
/// does. The files are watched where they really are, so a configuration
/// kept elsewhere and linked into place is still seen.
///
/// Their directories are watched rather than the files: an editor saving by
/// renaming a new file into place would leave a watch on the old one behind,
/// and on a first launch there is no file yet to watch.
pub fn watch_config_files() {
    let files: Vec<PathBuf> = [Config::path(), Config::mappings_path()]
        .into_iter()
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
        .collect();
    let directories: HashSet<PathBuf> = files
        .iter()
        .filter_map(|file| file.parent().map(Path::to_path_buf))
        .collect();

    for directory in directories {
        let files = files.clone();
        std::thread::spawn(move || {
            // On a first launch nothing has been saved yet, and a directory
            // that does not exist cannot be watched; made here, the files
            // the first save writes into it are seen like any later edit.
            if let Err(error) = std::fs::create_dir_all(&directory) {
                tracing::warn!(%error, ?directory, "config directory could not be made");
            }
            let mut changes = match FILE_WATCHER.watch_directory_non_recursive_blocking(&directory)
            {
                Ok(changes) => changes,
                Err(error) => {
                    tracing::warn!(%error, ?directory, "config directory is not watched");
                    return;
                }
            };
            // The app keeps its own files beside the configuration, and
            // writes them far more often than anyone edits it.
            while let Some(changed) = changes.blocking_recv() {
                if is_config_file_touched(&files, &changed) {
                    reload();
                }
            }
        });
    }
}

fn is_config_file_touched(files: &[PathBuf], changed: &[PathBuf]) -> bool {
    changed.iter().any(|path| files.contains(path))
}

fn reload() {
    let changed = {
        let mut current = CONFIG.write();
        match replacement(&current, Config::load()) {
            Some(loaded) => {
                *current = loaded;
                true
            }
            None => false,
        }
    };
    if changed {
        tracing::info!("configuration reloaded from disk");
        CONFIG_CHANGED_BROADCAST.send(()).ok();
    }
}

/// What the configuration becomes after `loaded` was read from disk: the
/// new one when it differs, nothing when it is the same — the file the
/// preferences window just wrote — or could not be read.
///
/// An unreadable file keeps the configuration the app is running with
/// rather than falling back to the defaults as start-up does: it is most
/// likely half-way through an edit, and dropping every setting until the
/// next save would be worse than waiting for it.
fn replacement(current: &Config, loaded: Result<Config, ConfigError>) -> Option<Config> {
    match loaded {
        Ok(loaded) => (loaded != *current).then_some(loaded),
        Err(error) => {
            tracing::warn!(%error, "config.json not reloaded; keeping the configuration in use");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_change_to_a_config_file_is_read_again() {
        let files = vec![
            PathBuf::from("/c/config.json"),
            PathBuf::from("/c/mappings.json"),
        ];

        assert!(is_config_file_touched(
            &files,
            &[
                PathBuf::from("/c/state.json"),
                PathBuf::from("/c/mappings.json")
            ]
        ));
        assert!(!is_config_file_touched(
            &files,
            &[PathBuf::from("/c/state.json"), PathBuf::from("/c")]
        ));
    }

    #[test]
    fn a_changed_configuration_replaces_the_one_in_use() {
        let current = Config::default_with_keybindings();
        let mut loaded = current.clone();
        loaded.zoom.default_zoom_level = 1.5;

        assert_eq!(replacement(&current, Ok(loaded.clone())), Some(loaded));
    }

    #[test]
    fn the_same_configuration_changes_nothing() {
        let current = Config::default_with_keybindings();

        assert_eq!(replacement(&current, Ok(current.clone())), None);
    }

    #[test]
    fn an_unreadable_file_keeps_the_configuration_in_use() {
        let current = Config::default_with_keybindings();
        let error = ConfigError::Parse {
            path: PathBuf::from("config.json"),
            source: serde_json::from_str::<Config>("{").unwrap_err(),
        };

        assert_eq!(replacement(&current, Err(error)), None);
    }
}
