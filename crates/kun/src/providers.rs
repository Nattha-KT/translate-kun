//! Choosing and building the Provider.
//!
//! Google and Anthropic are wired here. The fake Provider
//! (behind the `fake-provider` cargo feature) is for tests.

use std::time::Duration;

use kun_core::config::{EffectiveConfig, FileCredentials, ProviderName};
use kun_core::credentials::{CredentialStore, env_var_name, resolve_credential};
use kun_core::provider::Provider;
use kun_core::provider::anthropic::{AnthropicProvider, DEFAULT_MODEL};
use kun_core::provider::google::GoogleProvider;

use crate::error::CliError;

/// The `--provider` / config name of a built-in Provider.
pub fn provider_label(name: ProviderName) -> &'static str {
    match name {
        ProviderName::Google => "google",
        ProviderName::Anthropic => "anthropic",
        // `ProviderName` is `#[non_exhaustive]`.
        _ => "unknown",
    }
}

/// What `--provider <name>` means for the config merge.
///
/// `Ok(Some(name))` is a built-in Provider. `Ok(None)` is the fake Provider,
/// which is not a config value. Unknown names are a usage error.
pub fn parse_provider_flag(name: &str) -> Result<Option<ProviderName>, CliError> {
    if name == "fake" && cfg!(feature = "fake-provider") {
        return Ok(None);
    }
    ProviderName::ALL
        .into_iter()
        .find(|candidate| provider_label(*candidate) == name)
        .map(Some)
        .ok_or(CliError::Usage(
            "unknown provider: use `google` or `anthropic`",
        ))
}

/// Where credentials can be found (requirements §6). The caller passes the
/// real environment and keychain; tests pass fakes.
pub struct CredentialSources<'a> {
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub store: &'a dyn CredentialStore,
    pub user_file: Option<&'a FileCredentials>,
}

/// Builds the Provider called `name`.
pub fn build_provider(
    name: &str,
    config: &EffectiveConfig,
    sources: &CredentialSources,
) -> Result<Box<dyn Provider>, CliError> {
    match name {
        "fake" => build_fake(),
        "google" => {
            let key = resolve_credential(
                ProviderName::Google,
                sources.env,
                sources.store,
                sources.user_file,
            )
            .map_err(|error| CliError::keychain(error, env_var_name(ProviderName::Google)))?
            .ok_or(CliError::NoGoogleKey)?;
            let timeout = Duration::from_secs(config.timeout_secs);
            Ok(Box::new(GoogleProvider::new(key, timeout)))
        }
        "anthropic" => {
            let key = resolve_credential(
                ProviderName::Anthropic,
                sources.env,
                sources.store,
                sources.user_file,
            )
            .map_err(|error| CliError::keychain(error, env_var_name(ProviderName::Anthropic)))?
            .ok_or(CliError::NoAnthropicKey)?;
            let model = config
                .providers
                .get(&ProviderName::Anthropic)
                .and_then(|settings| settings.model.clone())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string());
            let timeout = Duration::from_secs(config.timeout_secs);
            Ok(Box::new(AnthropicProvider::new(key, model, timeout)))
        }
        _ => Err(CliError::Usage(
            "unknown provider: use `google` or `anthropic`",
        )),
    }
}

#[cfg(feature = "fake-provider")]
fn build_fake() -> Result<Box<dyn Provider>, CliError> {
    Ok(Box::new(kun_core::provider::fake::FakeProvider::new()))
}

#[cfg(not(feature = "fake-provider"))]
fn build_fake() -> Result<Box<dyn Provider>, CliError> {
    Err(CliError::Usage(
        "unknown provider: use `google` or `anthropic`",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kun_core::config::{Overrides, merge};
    use kun_core::credentials::{MemoryStore, Secret};

    fn config() -> EffectiveConfig {
        merge(&Overrides::default(), None, None)
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    /// Sources with nothing in them.
    fn build(name: &str, store: &MemoryStore) -> Result<Box<dyn Provider>, CliError> {
        let sources = CredentialSources {
            env: &no_env,
            store,
            user_file: None,
        };
        build_provider(name, &config(), &sources)
    }

    #[test]
    fn google_without_a_credential_is_exit_4() {
        let error = build("google", &MemoryStore::new()).err().unwrap();
        assert_eq!(error.exit_code(), 4);
        assert_eq!(
            error.to_string(),
            "No Google API key found. Run \"kun auth login google\" or set KUN_GOOGLE_API_KEY."
        );
    }

    #[test]
    fn google_is_built_from_a_keychain_key() {
        let mut store = MemoryStore::new();
        store
            .set(
                ProviderName::Google,
                &Secret::new("AIzaFAKE-EXAMPLE".into()),
            )
            .unwrap();
        let provider = build("google", &store).unwrap();
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn google_is_built_from_the_environment() {
        let env = |name: &str| (name == "KUN_GOOGLE_API_KEY").then(|| "AIzaFAKE".to_string());
        let sources = CredentialSources {
            env: &env,
            store: &MemoryStore::new(),
            user_file: None,
        };
        assert!(build_provider("google", &config(), &sources).is_ok());
    }

    #[test]
    fn flag_accepts_the_built_in_names() {
        assert_eq!(
            parse_provider_flag("google").unwrap(),
            Some(ProviderName::Google)
        );
        assert_eq!(
            parse_provider_flag("anthropic").unwrap(),
            Some(ProviderName::Anthropic)
        );
    }

    #[test]
    fn flag_rejects_unknown_names_with_exit_2() {
        let error = parse_provider_flag("deepl").unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn anthropic_without_a_credential_is_exit_4() {
        let error = build("anthropic", &MemoryStore::new()).err().unwrap();
        assert_eq!(error.exit_code(), 4);
        assert_eq!(
            error.to_string(),
            "No Anthropic API key found. Run \"kun auth login anthropic\" or set ANTHROPIC_API_KEY."
        );
    }

    #[test]
    fn anthropic_is_built_from_a_keychain_key() {
        let mut store = MemoryStore::new();
        store
            .set(
                ProviderName::Anthropic,
                &Secret::new("sk-ant-FAKE-EXAMPLE".into()),
            )
            .unwrap();
        let provider = build("anthropic", &store).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn unknown_provider_is_a_usage_error() {
        let error = build("deepl", &MemoryStore::new()).err().unwrap();
        assert_eq!(error.exit_code(), 2);
    }

    #[cfg(feature = "fake-provider")]
    #[test]
    fn fake_provider_is_available_with_the_feature() {
        assert_eq!(parse_provider_flag("fake").unwrap(), None);
        let provider = build("fake", &MemoryStore::new()).unwrap();
        assert_eq!(provider.name(), "fake");
    }

    #[cfg(not(feature = "fake-provider"))]
    #[test]
    fn fake_provider_is_unknown_without_the_feature() {
        assert_eq!(parse_provider_flag("fake").unwrap_err().exit_code(), 2);
        assert_eq!(
            build("fake", &MemoryStore::new())
                .err()
                .unwrap()
                .exit_code(),
            2
        );
    }
}
