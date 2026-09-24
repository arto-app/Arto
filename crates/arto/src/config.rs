//! The live configuration of the running app.
//!
//! The types and the on-disk format live in `arto-config`; this module holds
//! the single loaded instance every window reads and the channel that tells
//! them when it changed.

pub use arto_config::*;

use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;
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

/// How long the files have to settle before they are read again: an editor
/// saving writes, renames and touches in quick succession.
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);

/// Read `config.json` and `mappings.json` again whenever either changes on
/// disk, and tell every window when that changed the configuration.
///
/// Some settings — lenses among them — have no place in the preferences
/// window and are only ever written by hand, so an edit made in another
/// program has to reach the running app as one made in the preferences
/// does. The files are watched where they really are, so a configuration
/// kept elsewhere and linked into place is still seen.
pub fn watch_config_files() {
    let files: Vec<PathBuf> = [Config::path(), Config::mappings_path()]
        .into_iter()
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
        .collect();
    let directories: HashSet<PathBuf> = files
        .iter()
        .filter_map(|file| file.parent().map(Path::to_path_buf))
        .collect();

    std::thread::spawn(move || {
        let watched = files.clone();
        let debouncer = new_debouncer(RELOAD_DEBOUNCE, None, move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let touched = events
                .iter()
                .flat_map(|event| &event.paths)
                .any(|path| watched.contains(path));
            if touched {
                reload();
            }
        });
        let mut debouncer = match debouncer {
            Ok(debouncer) => debouncer,
            Err(error) => {
                tracing::warn!(%error, "config files are not watched");
                return;
            }
        };
        for directory in &directories {
            // On a first launch nothing has been saved yet, and a directory
            // that does not exist cannot be watched; made here, the files
            // the first save writes into it are seen like any later edit.
            if let Err(error) = std::fs::create_dir_all(directory) {
                tracing::warn!(%error, ?directory, "config directory could not be made");
            }
            if let Err(error) = debouncer.watch(directory, RecursiveMode::NonRecursive) {
                tracing::warn!(%error, ?directory, "config directory is not watched");
            }
        }
        // The watcher stops when it is dropped; this thread keeps it for
        // the life of the app.
        loop {
            std::thread::park();
        }
    });
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
