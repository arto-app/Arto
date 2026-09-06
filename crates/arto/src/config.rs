//! The live configuration of the running app.
//!
//! The types and the on-disk format live in `arto-config`; this module holds
//! the single loaded instance every window reads and the channel that tells
//! them when it changed.

pub use arto_config::*;

use parking_lot::RwLock;
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
