//! Choosing and building the Provider.
//!
//! Real Providers are wired in a later slice. Until then only the fake
//! Provider (behind the `fake-provider` cargo feature) can translate.

use kun_core::config::{EffectiveConfig, ProviderName};
use kun_core::provider::Provider;

use crate::error::CliError;

/// The `--provider` / config name of a built-in Provider.
pub fn provider_label(name: ProviderName) -> &'static str {
    match name {
        ProviderName::Google => "google",
        ProviderName::Anthropic => "anthropic",
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

/// Builds the Provider called `name`.
pub fn build_provider(
    name: &str,
    _config: &EffectiveConfig,
) -> Result<Box<dyn Provider>, CliError> {
    match name {
        "fake" => build_fake(),
        "google" => Err(CliError::ProviderNotWired("google")),
        "anthropic" => Err(CliError::ProviderNotWired("anthropic")),
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

    fn config() -> EffectiveConfig {
        merge(&Overrides::default(), None, None)
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
    fn real_providers_are_not_wired_yet() {
        for name in ["google", "anthropic"] {
            let error = build_provider(name, &config()).err().unwrap();
            assert_eq!(error.exit_code(), 1, "{name}");
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn unknown_provider_is_a_usage_error() {
        let error = build_provider("deepl", &config()).err().unwrap();
        assert_eq!(error.exit_code(), 2);
    }

    #[cfg(feature = "fake-provider")]
    #[test]
    fn fake_provider_is_available_with_the_feature() {
        assert_eq!(parse_provider_flag("fake").unwrap(), None);
        let provider = build_provider("fake", &config()).unwrap();
        assert_eq!(provider.name(), "fake");
    }

    #[cfg(not(feature = "fake-provider"))]
    #[test]
    fn fake_provider_is_unknown_without_the_feature() {
        assert_eq!(parse_provider_flag("fake").unwrap_err().exit_code(), 2);
        assert_eq!(
            build_provider("fake", &config()).err().unwrap().exit_code(),
            2
        );
    }
}
