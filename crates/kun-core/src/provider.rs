//! The Provider abstraction: one small interface that every translation
//! Provider (Google, Anthropic, the fake one used in tests) implements.

pub mod fake;
pub mod google;

use thiserror::Error;

use crate::language::LanguageCode;

/// What to translate and into which Target Language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub text: String,
    /// The Source Language. `None` means the Provider auto-detects it.
    pub source: Option<LanguageCode>,
    pub target: LanguageCode,
}

/// The result of a successful translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub text: String,
    /// The Source Language the Provider detected, if it reports one.
    pub detected_source: Option<LanguageCode>,
    /// The `name()` of the Provider that produced this translation.
    pub provider: &'static str,
}

/// Why a network call failed. Only fixed categories: never a URL, header or
/// response body, because those can contain API keys or the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NetworkReason {
    #[error("request timed out")]
    Timeout,
    #[error("could not connect")]
    Connect,
    #[error("TLS error")]
    Tls,
    #[error("HTTP status {0}")]
    Status(u16),
}

/// Why a Provider could not translate.
///
/// No variant carries free-form text, so `Display` and `Debug` can never leak
/// credentials or the source text, and the CLI can print them as they are.
/// Variants map to exit codes in the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("no credentials found for this provider")]
    MissingCredentials,
    #[error("the provider rejected the credentials")]
    Authentication,
    #[error("network error: {0}")]
    Network(NetworkReason),
    #[error("the provider is rate limiting requests")]
    RateLimited,
    #[error("the provider does not support language `{0}`")]
    UnsupportedLanguage(LanguageCode),
    #[error("the provider returned a response that could not be understood")]
    InvalidResponse,
}

/// A translation service. Synchronous and object-safe, so the CLI can hold a
/// `Box<dyn Provider>`.
pub trait Provider {
    /// A short stable name such as `"google"`, used for `--provider` and output.
    fn name(&self) -> &'static str;

    /// Translates `request.text` into `request.target`.
    fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            ProviderError::RateLimited.to_string(),
            "the provider is rate limiting requests"
        );
        assert_eq!(
            ProviderError::Network(NetworkReason::Status(503)).to_string(),
            "network error: HTTP status 503"
        );
        assert_eq!(
            ProviderError::UnsupportedLanguage("pt-br".parse().unwrap()).to_string(),
            "the provider does not support language `pt-BR`"
        );
    }

    #[test]
    fn display_and_debug_contain_only_fixed_text() {
        // Every variant, with the exact text allowed in its Display and Debug.
        // A leak would need a free-form payload, which no variant has.
        let cases = [
            (
                ProviderError::MissingCredentials,
                "no credentials found for this provider",
                "MissingCredentials",
            ),
            (
                ProviderError::Authentication,
                "the provider rejected the credentials",
                "Authentication",
            ),
            (
                ProviderError::Network(NetworkReason::Timeout),
                "network error: request timed out",
                "Network(Timeout)",
            ),
            (
                ProviderError::Network(NetworkReason::Connect),
                "network error: could not connect",
                "Network(Connect)",
            ),
            (
                ProviderError::Network(NetworkReason::Tls),
                "network error: TLS error",
                "Network(Tls)",
            ),
            (
                ProviderError::Network(NetworkReason::Status(401)),
                "network error: HTTP status 401",
                "Network(Status(401))",
            ),
            (
                ProviderError::RateLimited,
                "the provider is rate limiting requests",
                "RateLimited",
            ),
            (
                ProviderError::UnsupportedLanguage("th".parse().unwrap()),
                "the provider does not support language `th`",
                "UnsupportedLanguage(LanguageCode(\"th\"))",
            ),
            (
                ProviderError::InvalidResponse,
                "the provider returned a response that could not be understood",
                "InvalidResponse",
            ),
        ];
        for (error, display, debug) in cases {
            assert_eq!(error.to_string(), display);
            assert_eq!(format!("{error:?}"), debug);
        }
    }

    #[test]
    fn unsupported_language_cannot_carry_arbitrary_text() {
        // The only way to build the variant is a validated LanguageCode, so a
        // key or sentence is rejected before it can reach an error message.
        assert!("sk-FAKE-EXAMPLE-KEY".parse::<LanguageCode>().is_err());
        assert!("hello world, this is text".parse::<LanguageCode>().is_err());
    }
}
