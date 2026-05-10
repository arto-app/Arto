//! Wrapper around the OS credential store for AI provider secrets.
//!
//! Each `auth_ref` (UUID) in `AiProvider` maps to one keychain entry under
//! `service = "com.lambdalisue.arto.ai"`. The `auth_ref` itself is stored in
//! `config.json` (not sensitive); the secret only ever lives in the OS
//! credential store.
//!
//! On macOS this uses Keychain Services, on Windows the Credential Manager,
//! and on Linux Secret Service / kernel keyring (via the `keyring` crate's
//! native backends).

use anyhow::{Context, Result};

const SERVICE: &str = "com.lambdalisue.arto.ai";

fn entry(auth_ref: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, auth_ref).context("failed to open keychain entry")
}

/// Store a secret under the given `auth_ref`. Overwrites any existing entry.
pub fn set_secret(auth_ref: &str, secret: &str) -> Result<()> {
    entry(auth_ref)?
        .set_password(secret)
        .context("failed to store secret in keychain")
}

/// Retrieve the secret for the given `auth_ref`. Returns `Ok(None)` if no
/// entry exists, propagates other errors.
pub fn get_secret(auth_ref: &str) -> Result<Option<String>> {
    match entry(auth_ref)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err).context("failed to read secret from keychain"),
    }
}

/// Delete the secret for the given `auth_ref`. Treats "no entry" as success
/// so callers can use this idempotently when removing a provider.
pub fn delete_secret(auth_ref: &str) -> Result<()> {
    match entry(auth_ref)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err).context("failed to delete secret from keychain"),
    }
}

/// Generate a new opaque `auth_ref` value. Uses UUIDv4 so values are
/// collision-free without requiring a registry.
pub fn new_auth_ref() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests in this module require an interactive keychain on macOS and are
    /// gated behind `KEYCHAIN_TESTS=1` to keep CI/non-interactive runs green.
    fn keychain_enabled() -> bool {
        std::env::var("KEYCHAIN_TESTS").as_deref() == Ok("1")
    }

    #[test]
    fn new_auth_ref_is_uuid() {
        let r = new_auth_ref();
        assert_eq!(r.len(), 36);
        assert_eq!(uuid::Uuid::parse_str(&r).map(|_| ()), Ok(()));
    }

    #[test]
    fn set_get_delete_roundtrip() {
        if !keychain_enabled() {
            eprintln!("skipping: set KEYCHAIN_TESTS=1 to enable");
            return;
        }
        let auth_ref = new_auth_ref();
        set_secret(&auth_ref, "hunter2").unwrap();
        assert_eq!(get_secret(&auth_ref).unwrap().as_deref(), Some("hunter2"));
        delete_secret(&auth_ref).unwrap();
        assert_eq!(get_secret(&auth_ref).unwrap(), None);
    }

    #[test]
    fn delete_missing_is_idempotent() {
        if !keychain_enabled() {
            eprintln!("skipping: set KEYCHAIN_TESTS=1 to enable");
            return;
        }
        let auth_ref = new_auth_ref();
        delete_secret(&auth_ref).unwrap();
    }
}
