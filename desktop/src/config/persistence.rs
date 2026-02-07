use super::app_config::Config;
use anyhow::Result;
use parking_lot::RwLock;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::broadcast;

impl Config {
    /// Get the configuration file path based on the platform
    pub fn path() -> PathBuf {
        const FILENAME: &str = "config.json";
        if let Some(mut path) = dirs::config_dir() {
            path.push("arto");
            path.push(FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(FILENAME);
            return path;
        }

        PathBuf::from(FILENAME)
    }

    /// Load configuration from file or return default configuration
    pub fn load() -> Result<Self> {
        let path = Self::path();

        if !path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&content)?;

        tracing::debug!(path = %path.display(), "Configuration loaded");

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::path();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self)?;
        fs::write(&path, content)?;

        tracing::debug!(path = %path.display(), "Configuration saved");

        Ok(())
    }
}

/// Global configuration instance
pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| {
    let config = Config::load().unwrap_or_default();
    RwLock::new(config)
});

/// Broadcast channel for config change notifications.
///
/// Sent when Config is saved (from Preferences UI).
/// Each window's App component subscribes to rebuild its KeybindingEngine.
pub static CONFIG_CHANGED_BROADCAST: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);
