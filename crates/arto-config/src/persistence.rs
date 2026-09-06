//! Where the configuration files live and how they are read and written.
//!
//! Preferences are `config.json`; keybindings are `mappings.json` next to it.
//! Both sit in the platform configuration directory under `arto/`, falling
//! back to `~/.arto/` when the platform directory is unknown.

use crate::Config;
use arto_keybindings::BindingSet;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.json";
const MAPPINGS_FILENAME: &str = "mappings.json";

/// Why the configuration could not be loaded or saved.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot serialize configuration: {0}")]
    Serialize(#[source] serde_json::Error),
}

impl Config {
    /// Get the configuration file path based on the platform
    pub fn path() -> PathBuf {
        config_file(CONFIG_FILENAME)
    }

    /// Get the keyboard mappings file path based on the platform.
    pub fn mappings_path() -> PathBuf {
        config_file(MAPPINGS_FILENAME)
    }

    /// Every preference at its default plus the default keybinding preset.
    ///
    /// `Config::default()` has no keybindings because they come from
    /// `mappings.json`, not `config.json`; this is what to run with when
    /// loading fails, so the app still has working shortcuts.
    pub fn default_with_keybindings() -> Self {
        Self {
            keybindings: resolve_keybindings(None),
            ..Self::default()
        }
    }

    /// Load configuration from disk, or the defaults when no file exists.
    ///
    /// A missing `mappings.json` yields the default preset, so a fresh
    /// install has working shortcuts without writing anything first.
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::path();
        let mappings_path = Self::mappings_path();

        let mut config = if config_path.exists() {
            read_json(&config_path)?
        } else {
            Config::default()
        };

        config.keybindings = resolve_keybindings(load_mappings(&mappings_path)?);

        tracing::debug!(
            config_path = %config_path.display(),
            mappings_path = %mappings_path.display(),
            "Configuration loaded"
        );

        Ok(config)
    }

    /// Save configuration to disk: preferences to `config.json`, keybindings
    /// to `mappings.json`, creating the directory if needed.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::path();
        let mappings_path = Self::mappings_path();

        write_json(&config_path, self)?;
        write_json(&mappings_path, &self.keybindings)?;

        tracing::debug!(
            config_path = %config_path.display(),
            mappings_path = %mappings_path.display(),
            "Configuration saved"
        );

        Ok(())
    }
}

/// `<platform config dir>/arto/<name>`, or `~/.arto/<name>` when the platform
/// has no configuration directory, or just `<name>` as a last resort.
fn config_file(name: &str) -> PathBuf {
    if let Some(mut path) = dirs::config_dir() {
        path.push("arto");
        path.push(name);
        return path;
    }
    if let Some(mut path) = dirs::home_dir() {
        path.push(".arto");
        path.push(name);
        return path;
    }
    PathBuf::from(name)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let content = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ConfigError> {
    let write_error = |source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(write_error)?;
    }
    let content = serde_json::to_string_pretty(value).map_err(ConfigError::Serialize)?;
    fs::write(path, content).map_err(write_error)
}

fn load_mappings(path: &Path) -> Result<Option<BindingSet>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    read_json(path).map(Some)
}

fn resolve_keybindings(mappings: Option<BindingSet>) -> BindingSet {
    mappings.unwrap_or_else(arto_keybindings::presets::default_bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_keybindings::KeyAction;

    #[test]
    fn resolve_keybindings_uses_mappings_when_present() {
        let mappings = BindingSet {
            global: vec![KeyAction {
                key: "m".to_string(),
                action: "tab.close".to_string(),
            }],
            ..Default::default()
        };

        let resolved = resolve_keybindings(Some(mappings));
        assert_eq!(resolved.global[0].key, "m");
    }

    #[test]
    fn default_with_keybindings_carries_the_preset() {
        let config = Config::default_with_keybindings();
        assert_eq!(
            config.keybindings,
            arto_keybindings::presets::default_bindings()
        );
        assert_eq!(config.theme, Default::default());
    }

    #[test]
    fn resolve_keybindings_uses_defaults_when_mappings_missing() {
        let resolved = resolve_keybindings(None);
        assert_eq!(resolved, arto_keybindings::presets::default_bindings());
    }

    #[test]
    fn read_json_reports_the_offending_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{ not json").unwrap();

        let err = read_json::<Config>(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("config.json"));
    }

    #[test]
    fn write_json_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("mappings.json");

        write_json(&path, &BindingSet::default()).unwrap();
        let back: BindingSet = read_json(&path).unwrap();
        assert_eq!(back, BindingSet::default());
    }
}
