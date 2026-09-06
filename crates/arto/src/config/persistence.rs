use super::app_config::Config;
use anyhow::Result;
use parking_lot::RwLock;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::broadcast;

const CONFIG_FILENAME: &str = "config.json";
const MAPPINGS_FILENAME: &str = "mappings.json";

impl Config {
    /// Get the configuration file path based on the platform
    pub fn path() -> PathBuf {
        if let Some(mut path) = dirs::config_dir() {
            path.push("arto");
            path.push(CONFIG_FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(CONFIG_FILENAME);
            return path;
        }

        PathBuf::from(CONFIG_FILENAME)
    }

    /// Get the keyboard mappings file path based on the platform.
    pub fn mappings_path() -> PathBuf {
        if let Some(mut path) = dirs::config_dir() {
            path.push("arto");
            path.push(MAPPINGS_FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(MAPPINGS_FILENAME);
            return path;
        }

        PathBuf::from(MAPPINGS_FILENAME)
    }

    /// Load configuration from file or return default configuration
    pub fn load() -> Result<Self> {
        let config_path = Self::path();
        let mappings_path = Self::mappings_path();

        let mut config = if !config_path.exists() {
            Config::default()
        } else {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        };

        config.keybindings = resolve_keybindings(load_mappings(&mappings_path)?);

        tracing::debug!(
            config_path = %config_path.display(),
            mappings_path = %mappings_path.display(),
            "Configuration loaded"
        );

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::path();
        let mappings_path = Self::mappings_path();

        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if let Some(parent) = mappings_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config_content = serde_json::to_string_pretty(&self)?;
        fs::write(&config_path, config_content)?;

        let mappings_content = serde_json::to_string_pretty(&self.keybindings)?;
        fs::write(&mappings_path, mappings_content)?;

        tracing::debug!(
            config_path = %config_path.display(),
            mappings_path = %mappings_path.display(),
            "Configuration saved"
        );

        Ok(())
    }
}

fn load_mappings(path: &PathBuf) -> Result<Option<crate::config::BindingSet>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let mappings = serde_json::from_str(&content)?;
    Ok(Some(mappings))
}

fn resolve_keybindings(mappings: Option<crate::config::BindingSet>) -> crate::config::BindingSet {
    mappings.unwrap_or_else(crate::keybindings::default_bindings)
}

/// Global configuration instance
pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| {
    let config = Config::load().unwrap_or_default();
    RwLock::new(config)
});

/// Broadcast channel to notify all windows when config changes.
/// Subscribers call `.subscribe()` to get a receiver.
pub static CONFIG_CHANGED_BROADCAST: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_keybindings_uses_mappings_when_present() {
        let mappings = crate::config::BindingSet {
            global: vec![crate::config::KeyAction {
                key: "m".to_string(),
                action: "tab.close".to_string(),
            }],
            ..Default::default()
        };

        let resolved = resolve_keybindings(Some(mappings));
        assert_eq!(resolved.global[0].key, "m");
    }

    #[test]
    fn resolve_keybindings_uses_defaults_when_mappings_missing() {
        let resolved = resolve_keybindings(None);
        assert_eq!(resolved, crate::keybindings::default_bindings());
    }
}
