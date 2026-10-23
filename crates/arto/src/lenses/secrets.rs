//! The API keys of the servers lenses ask, kept in the system's credential
//! store — the Keychain on macOS, the Credential Manager on Windows, the
//! Secret Service elsewhere — under the server's endpoint.
//!
//! Not in `config.json`, which is often kept in a dotfiles repository, and
//! not in the environment, which an app opened from the Finder or the Dock
//! does not see. Filed under the endpoint rather than the lens, so every
//! lens that asks the same server uses the one key.
//!
//! The store is slow to ask and may ask the reader to unlock it, so a key is
//! read once and kept for the life of the app; storing or forgetting one
//! here replaces what was kept.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The service the keys are filed under in the credential store.
const SERVICE: &str = "Arto lens API key";

/// Why the credential store would not answer.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SecretsError {
    /// There is no store to ask.
    #[error("the system's credential store is not available: {0}")]
    Unavailable(#[source] keyring::Error),
    /// The store was asked and refused.
    #[error(transparent)]
    Store(keyring::Error),
    /// The thread asking it was lost.
    #[error(transparent)]
    Interrupted(tokio::task::JoinError),
}

static KEPT: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// What a key for `endpoint` is filed under: the endpoint as written, less
/// a trailing slash, so `…/v1` and `…/v1/` share a key.
pub(crate) fn account(endpoint: &str) -> String {
    endpoint.trim().trim_end_matches('/').to_string()
}

/// The key filed under `account`, or `None` when there is none.
pub(crate) fn read(account: &str) -> Result<Option<String>, SecretsError> {
    if let Some(kept) = KEPT.lock().get(account) {
        return Ok(kept.clone());
    }
    let key = match entry(account)?.get_password() {
        Ok(key) => Some(key),
        Err(keyring::Error::NoEntry) => None,
        Err(error) => return Err(SecretsError::Store(error)),
    };
    KEPT.lock().insert(account.to_string(), key.clone());
    Ok(key)
}

/// File `key` under `account`, replacing any key filed there.
pub(crate) fn store(account: &str, key: &str) -> Result<(), SecretsError> {
    entry(account)?
        .set_password(key)
        .map_err(SecretsError::Store)?;
    KEPT.lock()
        .insert(account.to_string(), Some(key.to_string()));
    Ok(())
}

/// Remove the key filed under `account`, if there is one.
pub(crate) fn forget(account: &str) -> Result<(), SecretsError> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => return Err(SecretsError::Store(error)),
    }
    KEPT.lock().insert(account.to_string(), None);
    Ok(())
}

/// Whether a key is filed under `account`, asked off the calling thread:
/// the store may block while it asks the reader to unlock it.
pub(crate) async fn is_stored(account: String) -> Result<bool, SecretsError> {
    blocking(move || read(&account).map(|key| key.is_some())).await
}

/// [`store`], off the calling thread.
pub(crate) async fn store_key(account: String, key: String) -> Result<(), SecretsError> {
    blocking(move || store(&account, &key)).await
}

/// [`forget`], off the calling thread.
pub(crate) async fn forget_key(account: String) -> Result<(), SecretsError> {
    blocking(move || forget(&account)).await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, SecretsError> + Send + 'static,
) -> Result<T, SecretsError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(SecretsError::Interrupted)?
}

fn entry(account: &str) -> Result<keyring::Entry, SecretsError> {
    keyring::Entry::new(SERVICE, account).map_err(SecretsError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_endpoint_with_or_without_its_trailing_slash_shares_a_key() {
        assert_eq!(
            account("https://api.openai.com/v1/"),
            account(" https://api.openai.com/v1")
        );
    }
}
