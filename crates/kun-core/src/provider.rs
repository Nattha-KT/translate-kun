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
    /// The Provider is rate limiting requests. Waiting and retrying may help.
    #[error("the provider is rate limiting requests{}", retry_hint(.retry_after_secs))]
    RateLimited {
        /// The `Retry-After` header in seconds, when the Provider sent one.
        retry_after_secs: Option<u64>,
    },
    /// A quota, credit or billing limit was reached. Retrying does not help
    /// until the user checks the Provider account.
    #[error("the provider quota, credit or billing limit was reached; check your provider account")]
    QuotaExceeded,
    /// The Provider stopped because the reply hit its output limit.
    #[error("the translation was too long for one request; split the text or lower max_chars")]
    OutputTruncated,
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

/// The text added to the rate-limit message when the Provider said how long to
/// wait, such as ", retry in 30 seconds". Empty when it did not.
fn retry_hint(retry_after_secs: &Option<u64>) -> String {
    match retry_after_secs {
        Some(secs) => format!(", retry in {secs} seconds"),
        None => String::new(),
    }
}

/// Reads a `Retry-After` header value. Only the seconds form (`"30"`) is
/// understood; the HTTP-date form and anything else give `None`.
pub(crate) fn parse_retry_after(value: Option<&str>) -> Option<u64> {
    let value = value?.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
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
            ProviderError::RateLimited {
                retry_after_secs: None
            }
            .to_string(),
            "the provider is rate limiting requests"
        );
        assert_eq!(
            ProviderError::RateLimited {
                retry_after_secs: Some(30)
            }
            .to_string(),
            "the provider is rate limiting requests, retry in 30 seconds"
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
                ProviderError::RateLimited {
                    retry_after_secs: None,
                },
                "the provider is rate limiting requests",
                "RateLimited { retry_after_secs: None }",
            ),
            (
                ProviderError::RateLimited {
                    retry_after_secs: Some(30),
                },
                "the provider is rate limiting requests, retry in 30 seconds",
                "RateLimited { retry_after_secs: Some(30) }",
            ),
            (
                ProviderError::QuotaExceeded,
                "the provider quota, credit or billing limit was reached; check your provider account",
                "QuotaExceeded",
            ),
            (
                ProviderError::OutputTruncated,
                "the translation was too long for one request; split the text or lower max_chars",
                "OutputTruncated",
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
    fn retry_after_reads_only_whole_seconds() {
        assert_eq!(parse_retry_after(Some("30")), Some(30));
        assert_eq!(parse_retry_after(Some(" 5 ")), Some(5));
        assert_eq!(parse_retry_after(Some("0")), Some(0));
        for bad in [
            "",
            "abc",
            "-1",
            "1.5",
            "30 seconds",
            "Wed, 21 Oct 2026 07:28:00 GMT",
            "99999999999999999999999",
        ] {
            assert_eq!(parse_retry_after(Some(bad)), None, "{bad}");
        }
        assert_eq!(parse_retry_after(None), None);
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
