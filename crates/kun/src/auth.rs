//! `kun auth login | logout | status` (requirements §6).
//!
//! The functions here take the store and the input as arguments, so tests can
//! use `MemoryStore` and never touch the real keychain.

use std::io::{self, BufRead, IsTerminal, Write};

use kun_core::config::{FileCredentials, ProviderName};
use kun_core::credentials::{
    CredentialStore, MemoryStore, Secret, env_var_name, resolve_credential,
};

use crate::args::AuthAction;
use crate::error::CliError;
use crate::providers::provider_label;
use crate::settings::UserCredentials;

/// The credential store the CLI uses: the OS keychain.
#[cfg(not(feature = "fake-provider"))]
pub fn open_store() -> Box<dyn CredentialStore> {
    Box::new(kun_core::credentials::KeyringStore::new())
}

/// Builds made for tests (the `fake-provider` feature, never a default) use an
/// empty in-memory store, so tests can never read or change the real keychain.
#[cfg(feature = "fake-provider")]
pub fn open_store() -> Box<dyn CredentialStore> {
    Box::new(MemoryStore::new())
}

/// Where a provider's key would come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Env,
    Keychain,
    ConfigFile,
    /// The keychain exists but could not be read.
    KeychainError,
    None,
}

impl KeySource {
    pub fn label(self) -> &'static str {
        match self {
            KeySource::Env => "env",
            KeySource::Keychain => "keychain",
            KeySource::ConfigFile => "config file",
            KeySource::KeychainError => "keychain could not be read",
            KeySource::None => "none",
        }
    }
}

/// The first place that has a key, in the lookup order env → keychain → file.
/// Only the place is returned, never the key.
pub fn key_source(
    provider: ProviderName,
    env: &dyn Fn(&str) -> Option<String>,
    store: &dyn CredentialStore,
    file: Option<&FileCredentials>,
) -> KeySource {
    let no_env = |_: &str| None;
    let empty = MemoryStore::new();
    if found(resolve_credential(provider, env, &empty, None)) {
        return KeySource::Env;
    }
    match resolve_credential(provider, &no_env, store, None) {
        Ok(Some(_)) => return KeySource::Keychain,
        Ok(None) => {}
        Err(_) => return KeySource::KeychainError,
    }
    if found(resolve_credential(provider, &no_env, &empty, file)) {
        return KeySource::ConfigFile;
    }
    KeySource::None
}

fn found<E>(result: Result<Option<Secret>, E>) -> bool {
    matches!(result, Ok(Some(_)))
}

/// One line per provider, e.g. `google: env`. `config_ignored` says the User
/// Config had its `[credentials]` dropped because the file is readable by
/// others; a provider with no other key then says so instead of a bare `none`.
pub fn status_lines(
    env: &dyn Fn(&str) -> Option<String>,
    store: &dyn CredentialStore,
    file: Option<&FileCredentials>,
    config_ignored: bool,
) -> Vec<String> {
    ProviderName::ALL
        .into_iter()
        .map(|provider| {
            let source = key_source(provider, env, store, file);
            let label = if source == KeySource::None && config_ignored {
                "none (config file credentials ignored: file is readable by others)"
            } else {
                source.label()
            };
            format!("{}: {}", provider_label(provider), label)
        })
        .collect()
}

/// Stores `key` (trimmed) for `provider`. An empty key is a usage error.
pub fn login(
    provider: ProviderName,
    key: &str,
    store: &mut dyn CredentialStore,
) -> Result<(), CliError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(CliError::Usage("the API key is empty; nothing was stored"));
    }
    store
        .set(provider, &Secret::new(key.to_string()))
        .map_err(|error| CliError::keychain(error, env_var_name(provider)))
}

/// Deletes the key. Returns `false` when there was none.
pub fn logout(provider: ProviderName, store: &mut dyn CredentialStore) -> Result<bool, CliError> {
    store
        .delete(provider)
        .map_err(|error| CliError::keychain(error, env_var_name(provider)))
}

/// The first line of `reader`, without the line ending.
fn read_line_from(reader: &mut dyn BufRead) -> Result<String, CliError> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|_| CliError::Unexpected("cannot read standard input"))?;
    Ok(line)
}

/// A key typed on a terminal (hidden) or one line from piped stdin.
fn read_key() -> Result<String, CliError> {
    if io::stdin().is_terminal() {
        eprint!("API key (input is hidden): ");
        let _ = io::stderr().flush();
        rpassword::read_password().map_err(|_| CliError::Unexpected("cannot read the API key"))
    } else {
        read_line_from(&mut io::stdin().lock())
    }
}

/// Runs an `auth` subcommand. `user` is the User Config, used by `status`.
pub fn run(action: AuthAction, user: Option<&UserCredentials>) -> Result<(), CliError> {
    let mut store = open_store();
    match action {
        AuthAction::Login { provider } => {
            let key = read_key()?;
            login(provider, &key, &mut *store)?;
            println!(
                "Stored the {} key in the OS keychain.",
                provider_label(provider)
            );
        }
        AuthAction::Logout { provider } => {
            if logout(provider, &mut *store)? {
                println!(
                    "Deleted the {} key from the OS keychain.",
                    provider_label(provider)
                );
            } else {
                println!(
                    "No {} key was stored in the OS keychain.",
                    provider_label(provider)
                );
            }
        }
        AuthAction::Status => {
            let env = |name: &str| std::env::var(name).ok();
            let file = user.and_then(|user| user.credentials.as_ref());
            let ignored = user.is_some_and(|user| user.ignored_insecure);
            for line in status_lines(&env, &*store, file, ignored) {
                println!("{line}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kun_core::credentials::CredentialError;

    const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn login_stores_the_trimmed_key() {
        let mut store = MemoryStore::new();
        login(ProviderName::Google, &format!("  {FAKE_KEY}\n"), &mut store).unwrap();
        let stored = store.get(ProviderName::Google).unwrap().unwrap();
        assert_eq!(stored.expose(), FAKE_KEY);
    }

    #[test]
    fn login_rejects_an_empty_key_with_exit_2() {
        let mut store = MemoryStore::new();
        let error = login(ProviderName::Google, " \n", &mut store).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(store.get(ProviderName::Google).unwrap().is_none());
    }

    #[test]
    fn login_reads_one_line_from_piped_input() {
        let data = format!("{FAKE_KEY}\nsecond line\n");
        let mut input = data.as_bytes();
        let line = read_line_from(&mut input).unwrap();
        assert_eq!(line.trim(), FAKE_KEY);
    }

    struct NoKeychain;

    impl CredentialStore for NoKeychain {
        fn get(&self, _: ProviderName) -> Result<Option<Secret>, CredentialError> {
            Err(CredentialError::StoreUnavailable)
        }
        fn set(&mut self, _: ProviderName, _: &Secret) -> Result<(), CredentialError> {
            Err(CredentialError::StoreUnavailable)
        }
        fn delete(&mut self, _: ProviderName) -> Result<bool, CredentialError> {
            Err(CredentialError::StoreUnavailable)
        }
    }

    #[test]
    fn login_without_a_keychain_exits_3_and_suggests_the_env_var() {
        let error = login(ProviderName::Google, FAKE_KEY, &mut NoKeychain).unwrap_err();
        assert_eq!(error.exit_code(), 3);
        let message = error.to_string();
        assert!(message.contains("KUN_GOOGLE_API_KEY"));
        assert!(message.contains("[credentials]"));
        assert!(!message.contains(FAKE_KEY));
    }

    #[test]
    fn logout_deletes_and_reports_whether_a_key_was_there() {
        let mut store = MemoryStore::new();
        login(ProviderName::Google, FAKE_KEY, &mut store).unwrap();
        assert!(logout(ProviderName::Google, &mut store).unwrap());
        assert!(!logout(ProviderName::Google, &mut store).unwrap());
    }

    fn file_with(key_line: &str) -> FileCredentials {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, format!("[credentials]\n{key_line}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        kun_core::config::load_user_config(&path)
            .unwrap()
            .credentials
    }

    #[test]
    fn status_names_each_source_and_never_shows_a_key() {
        let env = |name: &str| (name == "KUN_GOOGLE_API_KEY").then(|| FAKE_KEY.to_string());
        let mut store = MemoryStore::new();
        login(ProviderName::Anthropic, FAKE_KEY, &mut store).unwrap();
        let file = file_with(&format!("google_api_key = \"{FAKE_KEY}\""));

        let lines = status_lines(&env, &store, Some(&file), false);
        assert_eq!(lines, ["google: env", "anthropic: keychain"]);

        let lines = status_lines(&no_env, &MemoryStore::new(), Some(&file), false);
        assert_eq!(lines, ["google: config file", "anthropic: none"]);

        for line in lines {
            assert!(!line.contains("EXAMPLE"), "{line}");
        }
    }

    #[test]
    fn status_says_when_the_config_credentials_were_ignored() {
        let env = |name: &str| (name == "KUN_GOOGLE_API_KEY").then(|| FAKE_KEY.to_string());
        let lines = status_lines(&env, &MemoryStore::new(), None, true);
        // A key that is found elsewhere is still reported as found.
        assert_eq!(lines[0], "google: env");
        assert_eq!(
            lines[1],
            "anthropic: none (config file credentials ignored: file is readable by others)"
        );
    }

    #[test]
    fn status_survives_a_missing_keychain() {
        let lines = status_lines(&no_env, &NoKeychain, None, false);
        assert_eq!(lines.len(), 2);
        let file = file_with("google_api_key = \"x-FAKE\"");
        // An unavailable keychain is skipped, like in the lookup.
        assert_eq!(
            key_source(ProviderName::Google, &no_env, &NoKeychain, Some(&file)),
            KeySource::ConfigFile
        );
    }
}
