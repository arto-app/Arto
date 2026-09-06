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

impl ConfigError {
    /// Whether the file simply does not exist.
    ///
    /// Loaders fall back to defaults only in this case; a file that exists
    /// but cannot be read (permissions, a directory in its place) is an
    /// error the caller must see.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            ConfigError::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        )
    }
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
        let mut config = Self::load_preferences()?;
        let mappings_path = Self::mappings_path();
        config.keybindings = resolve_keybindings(load_mappings(&mappings_path)?);
        tracing::debug!(mappings_path = %mappings_path.display(), "Keybindings loaded");
        Ok(config)
    }

    /// Load `config.json` alone, or the defaults when it does not exist.
    ///
    /// Keybindings are the default preset; `mappings.json` is not read. For
    /// consumers that only render (`arto page`, Quick Look) this keeps a
    /// broken `mappings.json` from getting in the way of settings that have
    /// nothing to do with it.
    pub fn load_preferences() -> Result<Self, ConfigError> {
        // Try the read rather than testing existence first: `Path::exists`
        // answers false for a permission error too, which would silently
        // turn an unreadable config.json into the defaults.
        match Self::load_preferences_from(Self::path()) {
            Err(error) if error.is_not_found() => Ok(Self::default_with_keybindings()),
            result => result,
        }
    }

    /// Load `config.json` from a specific file, which must exist.
    ///
    /// Like [`Config::load_preferences`], keybindings are the default preset.
    pub fn load_preferences_from(config_path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let config_path = config_path.as_ref();
        let mut config: Config = read_json(config_path)?;
        config.keybindings = resolve_keybindings(None);
        tracing::debug!(config_path = %config_path.display(), "Configuration loaded");
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
    match read_json(path) {
        Ok(mappings) => Ok(Some(mappings)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(error),
    }
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
    fn only_a_missing_file_counts_as_not_found() {
        let dir = tempfile::tempdir().unwrap();

        let missing = read_json::<Config>(&dir.path().join("missing.json")).unwrap_err();
        assert!(missing.is_not_found());

        let malformed_path = dir.path().join("config.json");
        fs::write(&malformed_path, "{ not json").unwrap();
        let malformed = read_json::<Config>(&malformed_path).unwrap_err();
        assert!(!malformed.is_not_found());

        // A directory where the file should be exists but cannot be read.
        let in_the_way = read_json::<Config>(dir.path()).unwrap_err();
        assert!(!in_the_way.is_not_found());
    }

    #[test]
    fn load_mappings_distinguishes_missing_from_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_mappings(&dir.path().join("mappings.json")).unwrap(),
            None
        );
        assert!(load_mappings(dir.path()).is_err());
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
    fn load_preferences_from_reads_the_file_and_ignores_sibling_mappings() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{"theme":{"defaultTheme":"dark","onStartup":"default","onNewWindow":"default"}}"#,
        )
        .unwrap();
        // A broken mappings.json next to it must not matter to a consumer
        // that only wants preferences.
        fs::write(dir.path().join("mappings.json"), "{ not json").unwrap();

        let config = Config::load_preferences_from(&config_path).unwrap();
        assert_eq!(config.theme.default_theme, crate::Theme::Dark);
        assert_eq!(
            config.keybindings,
            arto_keybindings::presets::default_bindings()
        );
    }

    #[test]
    fn load_preferences_from_requires_the_file_to_exist() {
        let dir = tempfile::tempdir().unwrap();
        let err = Config::load_preferences_from(dir.path().join("missing.json")).unwrap_err();
        assert!(matches!(err, ConfigError::Read { .. }));
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
