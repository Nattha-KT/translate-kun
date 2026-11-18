//! Provider credentials: the [`Secret`] type, where credentials are stored,
//! and the lookup order (env var → OS keychain → User Config).
//!
//! No terminal I/O and no prompting happen here.

use std::collections::HashMap;
use std::fmt;

use thiserror::Error;

pub use crate::config::FileCredentials;
use crate::config::ProviderName;

/// A credential such as an API key.
///
/// `Debug` and `Display` print `<redacted>`, so a `Secret` can sit inside any
/// struct or error without leaking. [`Secret::expose`] is the only way to read
/// the value. Not `Clone`: every copy of a credential should be deliberate.
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Secret(value)
    }

    /// The raw credential. Call this only at the point where the value is
    /// actually sent (an HTTP header, the keychain), never to format it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Why a [`CredentialStore`] could not be used.
///
/// Only fixed messages: the errors of the underlying keychain library can
/// contain the stored bytes, so they are never kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CredentialError {
    /// There is no usable keychain on this machine (e.g. no Secret Service).
    #[error("the OS keychain is not available")]
    StoreUnavailable,
    /// The keychain exists but the read, write or delete failed.
    #[error("the OS keychain could not read or write the credential")]
    StoreFailed,
}

/// Somewhere credentials are kept, one per Provider.
pub trait CredentialStore {
    /// The credential for `provider`, or `None` when none is stored.
    fn get(&self, provider: ProviderName) -> Result<Option<Secret>, CredentialError>;

    /// Store `secret` for `provider`, replacing any earlier one.
    fn set(&mut self, provider: ProviderName, secret: &Secret) -> Result<(), CredentialError>;

    /// Remove the credential for `provider`. Returns `false` if there was none.
    fn delete(&mut self, provider: ProviderName) -> Result<bool, CredentialError>;
}

/// The keychain service name every `kun` credential is stored under.
pub const KEYRING_SERVICE: &str = "translate-kun";

/// The keychain account name for a Provider (`"google"`, `"anthropic"`).
fn account_name(provider: ProviderName) -> &'static str {
    match provider {
        ProviderName::Google => "google",
        ProviderName::Anthropic => "anthropic",
    }
}

/// The OS keychain (macOS Keychain, Secret Service, Windows Credential
/// Manager) via the `keyring` crate. Service `translate-kun`, account = the
/// Provider name.
#[derive(Debug, Default)]
pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        KeyringStore
    }

    fn entry(provider: ProviderName) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(KEYRING_SERVICE, account_name(provider)).map_err(keyring_error)
    }
}

/// Turn a `keyring` error into a fixed category. The original is dropped on
/// purpose: some variants (e.g. `BadEncoding`) carry the stored bytes.
fn keyring_error(error: keyring::Error) -> CredentialError {
    match error {
        keyring::Error::NoDefaultStore | keyring::Error::NoStorageAccess(_) => {
            CredentialError::StoreUnavailable
        }
        _ => CredentialError::StoreFailed,
    }
}

impl CredentialStore for KeyringStore {
    fn get(&self, provider: ProviderName) -> Result<Option<Secret>, CredentialError> {
        match Self::entry(provider)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(keyring_error(other)),
        }
    }

    fn set(&mut self, provider: ProviderName, secret: &Secret) -> Result<(), CredentialError> {
        Self::entry(provider)?
            .set_password(secret.expose())
            .map_err(keyring_error)
    }

    fn delete(&mut self, provider: ProviderName) -> Result<bool, CredentialError> {
        match Self::entry(provider)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(other) => Err(keyring_error(other)),
        }
    }
}

/// An in-memory [`CredentialStore`] for tests. Never touches the OS keychain.
#[derive(Default)]
pub struct MemoryStore {
    values: HashMap<ProviderName, Secret>,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore::default()
    }
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Secret` already prints `<redacted>`; only the Providers show.
        f.debug_struct("MemoryStore")
            .field("values", &self.values)
            .finish()
    }
}

impl CredentialStore for MemoryStore {
    fn get(&self, provider: ProviderName) -> Result<Option<Secret>, CredentialError> {
        let value = self.values.get(&provider);
        Ok(value.map(|secret| Secret::new(secret.expose().to_string())))
    }

    fn set(&mut self, provider: ProviderName, secret: &Secret) -> Result<(), CredentialError> {
        let copy = Secret::new(secret.expose().to_string());
        self.values.insert(provider, copy);
        Ok(())
    }

    fn delete(&mut self, provider: ProviderName) -> Result<bool, CredentialError> {
        Ok(self.values.remove(&provider).is_some())
    }
}

/// The environment variable that holds the API key for `provider`.
pub fn env_var_name(provider: ProviderName) -> &'static str {
    match provider {
        ProviderName::Google => "KUN_GOOGLE_API_KEY",
        ProviderName::Anthropic => "ANTHROPIC_API_KEY",
    }
}

/// Trim surrounding whitespace; an empty result counts as absent.
fn usable(secret: Option<Secret>) -> Option<Secret> {
    let secret = secret?;
    let trimmed = secret.expose().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Secret::new(trimmed.to_string()))
    }
}

/// Find the credential for `provider` (requirements §6). First found wins:
///
/// 1. the environment variable from [`env_var_name`], read through `env`
///    (pass `|name| std::env::var(name).ok()` in the CLI, a closure in tests);
/// 2. `store` (the OS keychain in the CLI);
/// 3. the User Config `[credentials]` table, if one was loaded.
///
/// Empty or whitespace-only values count as absent; surrounding whitespace is
/// trimmed. A store that is [`CredentialError::StoreUnavailable`] is skipped,
/// so a machine without a keychain can still use the User Config. Any other
/// store failure is returned as an error. Returns `Ok(None)` when nothing is
/// found.
pub fn resolve_credential(
    provider: ProviderName,
    env: &dyn Fn(&str) -> Option<String>,
    store: &dyn CredentialStore,
    user_file: Option<&FileCredentials>,
) -> Result<Option<Secret>, CredentialError> {
    if let Some(secret) = usable(env(env_var_name(provider)).map(Secret::new)) {
        return Ok(Some(secret));
    }

    match store.get(provider) {
        Ok(found) => {
            if let Some(secret) = usable(found) {
                return Ok(Some(secret));
            }
        }
        Err(CredentialError::StoreUnavailable) => {}
        Err(other) => return Err(other),
    }

    Ok(usable(user_file.and_then(|file| file.api_key(provider))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

    #[test]
    fn secret_debug_and_display_never_show_the_value() {
        let secret = Secret::new(FAKE_KEY.to_string());
        for output in [
            format!("{secret:?}"),
            format!("{secret:#?}"),
            format!("{secret}"),
        ] {
            assert!(!output.contains(FAKE_KEY), "leaked in: {output}");
            assert!(output.contains("<redacted>"), "{output}");
        }
    }

    #[test]
    fn secret_expose_returns_the_value() {
        let secret = Secret::new(FAKE_KEY.to_string());
        assert_eq!(secret.expose(), FAKE_KEY);
    }

    // --- helpers for the lookup tests -------------------------------------

    fn no_env(_name: &str) -> Option<String> {
        None
    }

    /// The `[credentials]` table of a User Config with this text.
    fn user_file(text: &str) -> FileCredentials {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, text).unwrap();
        crate::config::load_user_config(&file).unwrap().credentials
    }

    fn store_with(provider: ProviderName, value: &str) -> MemoryStore {
        let mut store = MemoryStore::new();
        store
            .set(provider, &Secret::new(value.to_string()))
            .unwrap();
        store
    }

    fn resolved(
        provider: ProviderName,
        env: &dyn Fn(&str) -> Option<String>,
        store: &dyn CredentialStore,
        file: Option<&FileCredentials>,
    ) -> Option<String> {
        resolve_credential(provider, env, store, file)
            .unwrap()
            .map(|secret| secret.expose().to_string())
    }

    // --- MemoryStore ------------------------------------------------------

    #[test]
    fn memory_store_gets_sets_and_deletes_per_provider() {
        let mut store = MemoryStore::new();
        assert!(store.get(ProviderName::Google).unwrap().is_none());

        store
            .set(
                ProviderName::Google,
                &Secret::new("google-FAKE".to_string()),
            )
            .unwrap();
        assert_eq!(
            store.get(ProviderName::Google).unwrap().unwrap().expose(),
            "google-FAKE"
        );
        assert!(store.get(ProviderName::Anthropic).unwrap().is_none());

        assert!(store.delete(ProviderName::Google).unwrap());
        assert!(store.get(ProviderName::Google).unwrap().is_none());
        // Deleting again is not an error; it reports that nothing was there.
        assert!(!store.delete(ProviderName::Google).unwrap());
    }

    // --- resolve_credential -------------------------------------------------

    #[test]
    fn env_names_per_provider() {
        assert_eq!(env_var_name(ProviderName::Google), "KUN_GOOGLE_API_KEY");
        assert_eq!(env_var_name(ProviderName::Anthropic), "ANTHROPIC_API_KEY");
    }

    #[test]
    fn env_var_wins_over_store_and_file() {
        let env = |name: &str| (name == "KUN_GOOGLE_API_KEY").then(|| "from-env-FAKE".to_string());
        let store = store_with(ProviderName::Google, "from-store-FAKE");
        let file = user_file("[credentials]\ngoogle_api_key = \"from-file-FAKE\"");

        assert_eq!(
            resolved(ProviderName::Google, &env, &store, Some(&file)).as_deref(),
            Some("from-env-FAKE")
        );
    }

    #[test]
    fn store_wins_over_file() {
        let store = store_with(ProviderName::Google, "from-store-FAKE");
        let file = user_file("[credentials]\ngoogle_api_key = \"from-file-FAKE\"");

        assert_eq!(
            resolved(ProviderName::Google, &no_env, &store, Some(&file)).as_deref(),
            Some("from-store-FAKE")
        );
    }

    #[test]
    fn file_is_the_last_resort() {
        let file = user_file("[credentials]\nanthropic_api_key = \"from-file-FAKE\"");
        assert_eq!(
            resolved(
                ProviderName::Anthropic,
                &no_env,
                &MemoryStore::new(),
                Some(&file)
            )
            .as_deref(),
            Some("from-file-FAKE")
        );
    }

    #[test]
    fn nothing_found_is_none() {
        assert_eq!(
            resolved(ProviderName::Google, &no_env, &MemoryStore::new(), None),
            None
        );
    }

    #[test]
    fn each_provider_reads_only_its_own_credential() {
        let env = |name: &str| (name == "ANTHROPIC_API_KEY").then(|| "anthropic-FAKE".to_string());
        let store = store_with(ProviderName::Anthropic, "anthropic-store-FAKE");
        let file = user_file("[credentials]\nanthropic_api_key = \"anthropic-file-FAKE\"");

        assert_eq!(
            resolved(ProviderName::Google, &env, &store, Some(&file)),
            None
        );
    }

    #[test]
    fn empty_or_whitespace_values_count_as_absent() {
        let env = |_: &str| Some("  \t ".to_string());
        let store = store_with(ProviderName::Google, "");
        let file = user_file("[credentials]\ngoogle_api_key = \"from-file-FAKE\"");
        assert_eq!(
            resolved(ProviderName::Google, &env, &store, Some(&file)).as_deref(),
            Some("from-file-FAKE")
        );

        let blank_file = user_file("[credentials]\ngoogle_api_key = \" \"");
        assert_eq!(
            resolved(ProviderName::Google, &env, &store, Some(&blank_file)),
            None
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let env = |_: &str| Some(" key-FAKE\n".to_string());
        assert_eq!(
            resolved(ProviderName::Google, &env, &MemoryStore::new(), None).as_deref(),
            Some("key-FAKE")
        );
    }

    /// A store that always fails in the given way.
    struct FailingStore(CredentialError);

    impl CredentialStore for FailingStore {
        fn get(&self, _: ProviderName) -> Result<Option<Secret>, CredentialError> {
            Err(self.0)
        }
        fn set(&mut self, _: ProviderName, _: &Secret) -> Result<(), CredentialError> {
            Err(self.0)
        }
        fn delete(&mut self, _: ProviderName) -> Result<bool, CredentialError> {
            Err(self.0)
        }
    }

    #[test]
    fn an_unavailable_store_falls_through_to_the_file() {
        // E.g. a headless Linux box without a Secret Service.
        let store = FailingStore(CredentialError::StoreUnavailable);
        let file = user_file("[credentials]\ngoogle_api_key = \"from-file-FAKE\"");
        assert_eq!(
            resolved(ProviderName::Google, &no_env, &store, Some(&file)).as_deref(),
            Some("from-file-FAKE")
        );
    }

    #[test]
    fn other_store_failures_are_errors() {
        let store = FailingStore(CredentialError::StoreFailed);
        let file = user_file("[credentials]\ngoogle_api_key = \"from-file-FAKE\"");
        let err =
            resolve_credential(ProviderName::Google, &no_env, &store, Some(&file)).unwrap_err();
        assert_eq!(err, CredentialError::StoreFailed);
    }

    #[test]
    fn the_store_is_not_asked_when_the_env_var_is_set() {
        let env = |_: &str| Some("from-env-FAKE".to_string());
        let store = FailingStore(CredentialError::StoreFailed);
        assert_eq!(
            resolved(ProviderName::Google, &env, &store, None).as_deref(),
            Some("from-env-FAKE")
        );
    }

    #[test]
    fn credential_errors_hold_only_fixed_text() {
        for (error, display, debug) in [
            (
                CredentialError::StoreUnavailable,
                "the OS keychain is not available",
                "StoreUnavailable",
            ),
            (
                CredentialError::StoreFailed,
                "the OS keychain could not read or write the credential",
                "StoreFailed",
            ),
        ] {
            assert_eq!(error.to_string(), display);
            assert_eq!(format!("{error:?}"), debug);
            assert!(std::error::Error::source(&error).is_none());
        }
    }

    #[test]
    fn memory_store_debug_never_shows_values() {
        let store = store_with(ProviderName::Google, FAKE_KEY);
        for output in [format!("{store:?}"), format!("{store:#?}")] {
            assert!(!output.contains(FAKE_KEY), "leaked in: {output}");
        }
    }
}
