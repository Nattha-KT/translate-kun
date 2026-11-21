//! The Provider abstraction: one small interface that every translation
//! Provider (Google, Anthropic, the fake one used in tests) implements.

pub mod anthropic;
pub mod fake;
pub mod google;

use std::fmt;

use thiserror::Error;

use crate::language::LanguageCode;

/// What to translate and into which Target Language.
#[derive(Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    /// The text to translate.
    pub text: String,
    /// The Source Language. `None` means the Provider auto-detects it.
    pub source: Option<LanguageCode>,
    /// The Target Language.
    pub target: LanguageCode,
}

/// `Debug` hides the text: it is user input and may hold a secret.
impl fmt::Debug for TranslationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationRequest")
            .field("text", &"<redacted>")
            .field("source", &self.source)
            .field("target", &self.target)
            .finish()
    }
}

/// The result of a successful translation.
#[derive(Clone, PartialEq, Eq)]
pub struct Translation {
    /// The translated text.
    pub text: String,
    /// The Source Language the Provider detected, if it reports one.
    pub detected_source: Option<LanguageCode>,
    /// The `name()` of the Provider that produced this translation.
    pub provider: &'static str,
}

/// `Debug` hides the text: it is a translation of user input.
impl fmt::Debug for Translation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Translation")
            .field("text", &"<redacted>")
            .field("detected_source", &self.detected_source)
            .field("provider", &self.provider)
            .finish()
    }
}

/// Why a network call failed. Only fixed categories: never a URL, header or
/// response body, because those can contain API keys or the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NetworkReason {
    /// The request took longer than the timeout.
    #[error("request timed out")]
    Timeout,
    /// No connection to the server could be made.
    #[error("could not connect")]
    Connect,
    /// The TLS handshake failed.
    #[error("TLS error")]
    Tls,
    /// The server answered with this HTTP status code.
    #[error("HTTP status {0}")]
    Status(u16),
}

/// Why a Provider could not translate.
///
/// No variant carries free-form text, so `Display` and `Debug` can never leak
/// credentials or the source text, and the CLI can print them as they are.
/// Variants map to exit codes in the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// No credentials were found for this Provider.
    #[error("no credentials found for this provider")]
    MissingCredentials,
    /// The Provider rejected the credentials.
    #[error("the provider rejected the credentials")]
    Authentication,
    /// The network call failed.
    #[error("network error: {0}")]
    Network(NetworkReason),
    /// The Provider is rate limiting requests.
    #[error("the provider is rate limiting requests")]
    RateLimited,
    /// The Provider does not support this language.
    #[error("the provider does not support language `{0}`")]
    UnsupportedLanguage(LanguageCode),
    /// The response could not be parsed.
    #[error("the provider returned a response that could not be understood")]
    InvalidResponse,
    /// The model declined to translate the text (Anthropic `refusal`).
    #[error("the provider refused to translate this text")]
    Refused,
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
            (
                ProviderError::Refused,
                "the provider refused to translate this text",
                "Refused",
            ),
        ];
        for (error, display, debug) in cases {
            assert_eq!(error.to_string(), display);
            assert_eq!(format!("{error:?}"), debug);
        }
    }

    #[test]
    fn request_and_translation_debug_hide_the_text() {
        let fake = "FAKE-SECRET-TEXT-EXAMPLE-12345";
        let request = TranslationRequest {
            text: fake.to_string(),
            source: Some("en".parse().unwrap()),
            target: "th".parse().unwrap(),
        };
        let translation = Translation {
            text: fake.to_string(),
            detected_source: Some("en".parse().unwrap()),
            provider: "fake",
        };
        for shown in [
            format!("{request:?}"),
            format!("{request:#?}"),
            format!("{translation:?}"),
            format!("{translation:#?}"),
        ] {
            assert!(!shown.contains(fake), "leaked in: {shown}");
            assert!(shown.contains("<redacted>"));
        }
        assert!(format!("{request:?}").contains("th"));
        assert!(format!("{translation:?}").contains("fake"));
    }

    #[test]
    fn unsupported_language_cannot_carry_arbitrary_text() {
        // The only way to build the variant is a validated LanguageCode, so a
        // key or sentence is rejected before it can reach an error message.
        assert!("sk-FAKE-EXAMPLE-KEY".parse::<LanguageCode>().is_err());
        assert!("hello world, this is text".parse::<LanguageCode>().is_err());
    }
}
