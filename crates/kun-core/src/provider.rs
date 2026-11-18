//! The Provider abstraction: one small interface that every translation
//! Provider (Google, Anthropic, the fake one used in tests) implements.

pub mod fake;

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

/// Why a Provider could not translate.
///
/// Messages must never contain credentials or the full source text, so the
/// CLI can print them as they are. Variants map to exit codes in the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("no credentials found for this provider")]
    MissingCredentials,
    #[error("the provider rejected the credentials")]
    Authentication,
    #[error("network error: {0}")]
    Network(String),
    #[error("the provider is rate limiting requests")]
    RateLimited,
    #[error("the provider does not support language `{0}`")]
    UnsupportedLanguage(String),
    #[error("the provider returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("provider error: {0}")]
    Other(String),
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
            ProviderError::Network("timed out".into()).to_string(),
            "network error: timed out"
        );
    }
}
