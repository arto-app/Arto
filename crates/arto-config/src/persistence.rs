//! Where the configuration files live and how they are read and written.
//!
//! Preferences are `config.json`; keybindings are `keys.toml` next to it.
//! Both sit in the platform configuration directory under `arto/`, falling
//! back to `~/.arto/` when the platform directory is unknown.

use crate::{write_atomically, Access, Config};
use arto_keybindings::{BindingSet, Keymap, KeymapError, Preset};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.json";
const KEYS_FILENAME: &str = "keys.toml";
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
    #[error("cannot parse {path}: {source}")]
    Keys {
        path: PathBuf,
        #[source]
        source: KeymapError,
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

    /// Get the keybindings file path based on the platform.
    pub fn keys_path() -> PathBuf {
        config_file(KEYS_FILENAME)
    }

    /// The file keybindings were kept in before `keys.toml`, read only while
    /// `keys.toml` does not exist.
    pub fn mappings_path() -> PathBuf {
        config_file(MAPPINGS_FILENAME)
    }

    /// Every preference at its default plus the default keybinding preset.
    ///
    /// `Config::default()` has no keybindings because they come from
    /// `keys.toml`, not `config.json`; this is what to run with when
    /// loading fails, so the app still has working shortcuts.
    pub fn default_with_keybindings() -> Self {
        Self {
            keybindings: Preset::default().bindings(),
            ..Self::default()
        }
    }

    /// Load configuration from disk, or the defaults when no file exists.
    ///
    /// A missing `keys.toml` yields the default preset, so a fresh install
    /// has working shortcuts without writing anything first.
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Self::load_preferences()?;
        let keys_path = Self::keys_path();
        (config.keybinding_preset, config.keybindings) =
            load_keybindings(&keys_path, &Self::mappings_path())?;
        tracing::debug!(keys_path = %keys_path.display(), "Keybindings loaded");
        Ok(config)
    }

    /// Load `config.json` alone, or the defaults when it does not exist.
    ///
    /// Keybindings are the default preset; `keys.toml` is not read. For
    /// consumers that only render (`arto page`, Quick Look) this keeps a
    /// broken `keys.toml` from getting in the way of settings that have
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
        config.keybindings = Preset::default().bindings();
        tracing::debug!(config_path = %config_path.display(), "Configuration loaded");
        Ok(config)
    }

    /// Save configuration to disk: preferences to `config.json`, keybindings
    /// to `keys.toml` as changes to their preset, creating the directory if
    /// needed.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::path();
        let keys_path = Self::keys_path();

        write_json(&config_path, self)?;
        write_keys(&keys_path, self.keybinding_preset, &self.keybindings)?;

        tracing::debug!(
            config_path = %config_path.display(),
            keys_path = %keys_path.display(),
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
    let content = serde_json::to_string_pretty(value).map_err(ConfigError::Serialize)?;
    write_atomically(path, content.as_bytes(), Access::Kept).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn load_mappings(path: &Path) -> Result<Option<BindingSet>, ConfigError> {
    match read_json(path) {
        Ok(mappings) => Ok(Some(mappings)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_existing(path: &Path) -> Result<Option<String>, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// The keybindings in force and the preset they are written against.
///
/// `keys.toml` when it exists; otherwise a `mappings.json` from before it,
/// which held every binding and so is taken against the preset it differs
/// least from; otherwise the default preset.
fn load_keybindings(
    keys_path: &Path,
    mappings_path: &Path,
) -> Result<(Preset, BindingSet), ConfigError> {
    if let Some(text) = read_existing(keys_path)? {
        let keymap = Keymap::parse(text).map_err(|source| ConfigError::Keys {
            path: keys_path.to_path_buf(),
            source,
        })?;
        return Ok((keymap.preset, keymap.resolve()));
    }
    match load_mappings(mappings_path)? {
        Some(bindings) => Ok((Preset::closest_to(&bindings), bindings)),
        None => Ok((Preset::default(), Preset::default().bindings())),
    }
}

/// Write `bindings` to `keys.toml` as changes to `preset`, keeping what
/// still holds of the file as it was written.
///
/// A file that cannot be read as a keymap is left alone: the reader is
/// mid-edit, and writing over it would lose what they typed.
fn write_keys(path: &Path, preset: Preset, bindings: &BindingSet) -> Result<(), ConfigError> {
    let existing = read_existing(path)?;
    let text = Keymap::between(preset, bindings)
        .write(existing.as_deref())
        .map_err(|source| ConfigError::Keys {
            path: path.to_path_buf(),
            source,
        })?;
    write_atomically(path, text.as_bytes(), Access::Kept).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_keybindings::KeyAction;
    use indoc::indoc;

    fn binding(key: &str, action: &str) -> KeyAction {
        KeyAction {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    #[test]
    fn default_with_keybindings_carries_the_preset() {
        let config = Config::default_with_keybindings();
        assert_eq!(config.keybindings, Preset::Default.bindings());
        assert_eq!(config.keybinding_preset, Preset::Default);
        assert_eq!(config.theme, Default::default());
    }

    #[test]
    fn without_either_file_the_keybindings_are_the_default_preset() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_keybindings(
            &dir.path().join("keys.toml"),
            &dir.path().join("mappings.json"),
        )
        .unwrap();
        assert_eq!(loaded, (Preset::Default, Preset::Default.bindings()));
    }

    #[test]
    fn keys_toml_is_its_preset_with_its_changes() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys.toml");
        fs::write(
            &keys,
            indoc! {r#"
                preset = "none"

                [global]
                "j" = "scroll.down"
            "#},
        )
        .unwrap();
        let loaded = load_keybindings(&keys, &dir.path().join("mappings.json")).unwrap();
        assert_eq!(
            loaded,
            (
                Preset::None,
                BindingSet {
                    global: vec![binding("j", "scroll.down")],
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn keys_toml_wins_over_an_older_mappings_json() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys.toml");
        let mappings = dir.path().join("mappings.json");
        fs::write(&keys, "").unwrap();
        fs::write(&mappings, "{}").unwrap();
        let loaded = load_keybindings(&keys, &mappings).unwrap();
        assert_eq!(loaded, (Preset::Default, Preset::Default.bindings()));
    }

    #[test]
    fn an_older_mappings_json_is_read_against_the_preset_it_is_closest_to() {
        let dir = tempfile::tempdir().unwrap();
        let mappings = dir.path().join("mappings.json");
        let mut bindings = Preset::Vim.bindings();
        bindings.global.push(binding("Ctrl+z", "zoom.in"));
        write_json(&mappings, &bindings).unwrap();
        let loaded = load_keybindings(&dir.path().join("keys.toml"), &mappings).unwrap();
        assert_eq!(loaded, (Preset::Vim, bindings));
    }

    #[test]
    fn a_keys_toml_that_cannot_be_read_as_a_keymap_is_an_error_naming_it() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys.toml");
        fs::write(&keys, "[sidbar]").unwrap();
        let error = load_keybindings(&keys, &dir.path().join("mappings.json")).unwrap_err();
        assert!(matches!(error, ConfigError::Keys { .. }));
        assert!(error.to_string().contains("keys.toml"));
    }

    #[test]
    fn keys_are_written_as_changes_to_their_preset() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys.toml");
        let mut bindings = Preset::Emacs.bindings();
        bindings.global.push(binding("Ctrl+z", "zoom.in"));
        write_keys(&keys, Preset::Emacs, &bindings).unwrap();
        assert_eq!(
            fs::read_to_string(&keys).unwrap(),
            indoc! {r#"
                preset = "emacs"

                [global]
                "Ctrl+z" = "zoom.in"
            "#}
        );
    }

    #[test]
    fn keys_are_not_written_over_a_file_mid_edit() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys.toml");
        fs::write(&keys, "[global").unwrap();
        let error = write_keys(&keys, Preset::Default, &Preset::Default.bindings()).unwrap_err();
        assert!(matches!(error, ConfigError::Keys { .. }));
        assert_eq!(fs::read_to_string(&keys).unwrap(), "[global");
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
    fn load_preferences_from_reads_the_file_and_ignores_sibling_keys() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{"theme":{"defaultTheme":"dark","onStartup":"default","onNewWindow":"default"}}"#,
        )
        .unwrap();
        // A broken keys.toml next to it must not matter to a consumer
        // that only wants preferences.
        fs::write(dir.path().join("keys.toml"), "[global").unwrap();

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
        let path = dir.path().join("nested").join("config.json");

        write_json(&path, &BindingSet::default()).unwrap();
        let back: BindingSet = read_json(&path).unwrap();
        assert_eq!(back, BindingSet::default());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_config_is_saved_where_it_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let kept = dir.path().join("dotfiles.json");
        fs::write(&kept, "{}").unwrap();
        let link = dir.path().join("config.json");
        std::os::unix::fs::symlink(&kept, &link).unwrap();

        write_json(&link, &BindingSet::default()).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let back: BindingSet = read_json(&kept).unwrap();
        assert_eq!(back, BindingSet::default());
    }
}
