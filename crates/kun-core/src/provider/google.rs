//! The Google Cloud Translation Provider (Basic edition, v2 REST API).
//!
//! The HTTP call is thin. The parts worth testing are pure functions:
//! [`request_body`], [`read_response`], [`parse_response`],
//! [`error_for_status`] and [`transport_error`].
//!
//! Logging: `kun-core` caps the `log` crate at DEBUG for the whole build (see
//! its `Cargo.toml`), because ureq-proto dumps raw headers and bodies at TRACE.
//! At DEBUG, ureq redacts header values.

use std::fmt;
use std::io::{ErrorKind, Read};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{NetworkReason, Provider, ProviderError, Translation, TranslationRequest};
use crate::credentials::Secret;
use crate::language::LanguageCode;

/// The Provider name, as used for `--provider` and in output.
const NAME: &str = "google";

/// The v2 `translate` method. No query string: the key goes in a header, so
/// it can never show up in a URL inside an error message or a proxy log.
const ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

/// The header Google reads the API key from.
const KEY_HEADER: &str = "X-goog-api-key";

/// Google Cloud Translation, authenticated with an API key.
pub struct GoogleProvider {
    key: Secret,
    timeout: Duration,
    agent: ureq::Agent,
}

impl GoogleProvider {
    /// `timeout` covers the whole request, including connecting and reading
    /// the response. `HTTPS_PROXY` and `NO_PROXY` are read from the
    /// environment here.
    pub fn new(key: Secret, timeout: Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .proxy(ureq::Proxy::try_from_env())
            // Non-2xx answers come back as responses, so we can read the body
            // and pick the right `ProviderError`.
            .http_status_as_error(false)
            // Never follow a redirect: it would carry the key header elsewhere.
            .max_redirects(0)
            .max_redirects_will_error(false)
            .build();
        GoogleProvider {
            key,
            timeout,
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl fmt::Debug for GoogleProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The agent is left out: its proxy settings may hold a password.
        f.debug_struct("GoogleProvider")
            .field("key", &self.key)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Provider for GoogleProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
        let mut response = self
            .agent
            .post(ENDPOINT)
            .header(KEY_HEADER, self.key.expose())
            .send_json(request_body(request))
            .map_err(transport_error)?;

        let status = response.status().as_u16();
        let mut body = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .reader();
        read_response(status, &mut body, &request.target)
    }
}

/// Largest response body we read. A translation of the 10,000-character
/// input limit is far smaller.
const MAX_BODY_BYTES: u64 = 1024 * 1024;

/// Turn a response into a result, reading `body` only when the status needs
/// it: a 2xx is parsed, a 400 is classified. Every other status is decided by
/// the code alone.
///
/// A failure while reading (timeout, reset connection) goes through
/// [`transport_error`], so it is never mistaken for the HTTP status.
fn read_response(
    status: u16,
    body: &mut dyn Read,
    target: &LanguageCode,
) -> Result<Translation, ProviderError> {
    let is_success = (200..300).contains(&status);
    if !is_success && status != 400 {
        return Err(status_error(status));
    }

    let mut bytes = Vec::new();
    body.read_to_end(&mut bytes)
        // ureq's reader wraps its own errors in `io::Error`; `from` unwraps them.
        .map_err(|error| transport_error(ureq::Error::from(error)))?;

    if is_success {
        let text = String::from_utf8(bytes).map_err(|_| ProviderError::InvalidResponse)?;
        parse_response(&text)
    } else {
        // The body only helps to classify the error; it is never kept.
        let text = String::from_utf8_lossy(&bytes);
        Err(error_for_status(status, &text, target))
    }
}

/// The JSON body for a `translate` call. `format: text` stops Google from
/// HTML-escaping the result.
fn request_body(request: &TranslationRequest) -> Value {
    let mut body = json!({
        "q": request.text,
        "target": request.target.as_str(),
        "format": "text",
    });
    if let Some(source) = &request.source {
        body["source"] = json!(source.as_str());
    }
    body
}

// The response shape. No `Debug`: these hold the translated text.
#[derive(Deserialize)]
struct ResponseBody {
    data: ResponseData,
}

#[derive(Deserialize)]
struct ResponseData {
    translations: Vec<ResponseTranslation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseTranslation {
    translated_text: String,
    detected_source_language: Option<String>,
}

/// Read `data.translations[0]` from a successful response.
///
/// A detected Source Language that is not a valid [`LanguageCode`] is
/// ignored rather than failing the whole translation.
fn parse_response(body: &str) -> Result<Translation, ProviderError> {
    let parsed: ResponseBody =
        serde_json::from_str(body).map_err(|_| ProviderError::InvalidResponse)?;
    let first = parsed
        .data
        .translations
        .into_iter()
        .next()
        .ok_or(ProviderError::InvalidResponse)?;
    let detected_source = first
        .detected_source_language
        .and_then(|code| code.parse::<LanguageCode>().ok());
    Ok(Translation {
        text: first.translated_text,
        detected_source,
        provider: NAME,
    })
}

/// The error for a non-2xx status, without looking at the body.
fn status_error(status: u16) -> ProviderError {
    match status {
        401 | 403 => ProviderError::Authentication,
        429 => ProviderError::RateLimited,
        _ => ProviderError::Network(NetworkReason::Status(status)),
    }
}

// The parts of a Google error body we look at. No `Debug`: the message may
// echo request data.
#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorInfo,
}

#[derive(Deserialize)]
struct ErrorInfo {
    #[serde(default)]
    message: String,
    #[serde(default)]
    details: Vec<ErrorDetail>,
}

#[derive(Deserialize)]
struct ErrorDetail {
    #[serde(default)]
    reason: Option<String>,
}

/// The `reason` Google puts in `error.details[]` for an invalid API key.
const API_KEY_INVALID: &str = "API_KEY_INVALID";

/// True when a parsed error says the API key is invalid.
fn is_invalid_api_key(error: &ErrorInfo) -> bool {
    let by_reason = error
        .details
        .iter()
        .any(|detail| detail.reason.as_deref() == Some(API_KEY_INVALID));
    by_reason || error.message.to_lowercase().contains("api key not valid")
}

/// Phrases in a 400 message that mean the language is not supported.
const UNSUPPORTED_LANGUAGE_PHRASES: [&str; 4] = [
    "bad language pair",
    "language is not supported",
    "unsupported language",
    "invalid language",
];

/// Map a non-2xx response to a [`ProviderError`].
///
/// A 400 is refined by its parsed body: an invalid API key (Google answers
/// 400, not 401) becomes `Authentication`, and an unsupported language becomes
/// `UnsupportedLanguage(target)`. A body that is not a Google error JSON
/// keeps the plain status. The body itself is never put in the error.
fn error_for_status(status: u16, body: &str, target: &LanguageCode) -> ProviderError {
    if status != 400 {
        return status_error(status);
    }
    let Ok(parsed) = serde_json::from_str::<ErrorBody>(body) else {
        return status_error(status);
    };
    if is_invalid_api_key(&parsed.error) {
        return ProviderError::Authentication;
    }
    let message = parsed.error.message.to_lowercase();
    if UNSUPPORTED_LANGUAGE_PHRASES
        .iter()
        .any(|phrase| message.contains(phrase))
    {
        return ProviderError::UnsupportedLanguage(target.clone());
    }
    status_error(status)
}

/// Map a `ureq` error to a fixed [`ProviderError`]. The `ureq` error is
/// dropped: its text can contain URLs or proxy credentials.
///
/// rustls reports handshake and certificate failures as an I/O error of kind
/// `InvalidData`, so that kind counts as a TLS failure.
fn transport_error(error: ureq::Error) -> ProviderError {
    let reason = match error {
        ureq::Error::StatusCode(status) => return status_error(status),
        ureq::Error::Timeout(_) => NetworkReason::Timeout,
        ureq::Error::Io(io) => match io.kind() {
            ErrorKind::TimedOut => NetworkReason::Timeout,
            ErrorKind::InvalidData => NetworkReason::Tls,
            _ => NetworkReason::Connect,
        },
        ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::InvalidProxyUrl => NetworkReason::Connect,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => NetworkReason::Tls,
        _ => return ProviderError::InvalidResponse,
    };
    ProviderError::Network(reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn request(text: &str, source: Option<&str>, target: &str) -> TranslationRequest {
        TranslationRequest {
            text: text.to_string(),
            source: source.map(lang),
            target: lang(target),
        }
    }

    // --- request body -------------------------------------------------------

    #[test]
    fn request_body_without_source_lets_google_detect_it() {
        let body = request_body(&request("Hello", None, "th"));
        assert_eq!(
            body,
            json!({ "q": "Hello", "target": "th", "format": "text" })
        );
    }

    #[test]
    fn request_body_with_source() {
        let body = request_body(&request("Hello", Some("en"), "pt-BR"));
        assert_eq!(
            body,
            json!({ "q": "Hello", "target": "pt-BR", "source": "en", "format": "text" })
        );
    }

    #[test]
    fn endpoint_has_no_query_so_the_key_never_goes_in_the_url() {
        assert_eq!(
            ENDPOINT,
            "https://translation.googleapis.com/language/translate/v2"
        );
        assert!(!ENDPOINT.contains('?'));
        assert_eq!(KEY_HEADER, "X-goog-api-key");
    }

    // --- response parsing -----------------------------------------------------

    #[test]
    fn parses_text_and_detected_source() {
        let body = r#"{"data":{"translations":[{"translatedText":"สวัสดี","detectedSourceLanguage":"en"}]}}"#;
        let translation = parse_response(body).unwrap();
        assert_eq!(translation.text, "สวัสดี");
        assert_eq!(translation.detected_source, Some(lang("en")));
        assert_eq!(translation.provider, "google");
    }

    #[test]
    fn missing_detected_source_is_none() {
        // Google omits it when the request named the Source Language.
        let body = r#"{"data":{"translations":[{"translatedText":"Bonjour"}]}}"#;
        let translation = parse_response(body).unwrap();
        assert_eq!(translation.text, "Bonjour");
        assert_eq!(translation.detected_source, None);
    }

    #[test]
    fn invalid_detected_source_is_ignored() {
        let body = r#"{"data":{"translations":[{"translatedText":"Hi","detectedSourceLanguage":"not a language!"}]}}"#;
        let translation = parse_response(body).unwrap();
        assert_eq!(translation.text, "Hi");
        assert_eq!(translation.detected_source, None);
    }

    #[test]
    fn malformed_responses_are_invalid_response() {
        for body in [
            "",
            "not json",
            "{}",
            r#"{"data":{}}"#,
            r#"{"data":{"translations":[]}}"#,
            r#"{"data":{"translations":[{"detectedSourceLanguage":"en"}]}}"#,
            r#"{"data":{"translations":[{"translatedText":42}]}}"#,
        ] {
            assert_eq!(
                parse_response(body).unwrap_err(),
                ProviderError::InvalidResponse,
                "{body}"
            );
        }
    }

    // --- status mapping -------------------------------------------------------

    #[test]
    fn status_codes_map_to_provider_errors() {
        let target = lang("th");
        for (status, expected) in [
            (401, ProviderError::Authentication),
            (403, ProviderError::Authentication),
            (429, ProviderError::RateLimited),
            (400, ProviderError::Network(NetworkReason::Status(400))),
            (404, ProviderError::Network(NetworkReason::Status(404))),
            (500, ProviderError::Network(NetworkReason::Status(500))),
            (503, ProviderError::Network(NetworkReason::Status(503))),
            (302, ProviderError::Network(NetworkReason::Status(302))),
        ] {
            assert_eq!(error_for_status(status, "", &target), expected, "{status}");
        }
    }

    fn google_error(message: &str, reason: &str) -> String {
        json!({
            "error": {
                "code": 400,
                "message": message,
                "errors": [{ "message": message, "domain": "global", "reason": reason }],
                "status": "INVALID_ARGUMENT"
            }
        })
        .to_string()
    }

    #[test]
    fn bad_request_about_the_language_is_unsupported_language() {
        let target = lang("tlh");
        for message in [
            "Bad language pair: en|tlh",
            "Target language is not supported.",
        ] {
            let body = google_error(message, "badRequest");
            assert_eq!(
                error_for_status(400, &body, &target),
                ProviderError::UnsupportedLanguage(lang("tlh")),
                "{message}"
            );
        }
    }

    #[test]
    fn bad_request_with_an_invalid_api_key_is_authentication() {
        // Google answers an invalid key with 400, not 401.
        let body = google_error(
            "API key not valid. Please pass a valid API key.",
            "badRequest",
        );
        assert_eq!(
            error_for_status(400, &body, &lang("th")),
            ProviderError::Authentication
        );
    }

    #[test]
    fn bad_request_with_api_key_invalid_in_details_is_authentication() {
        let body = json!({
            "error": {
                "code": 400,
                "message": "Request rejected.",
                "status": "INVALID_ARGUMENT",
                "details": [
                    { "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                      "reason": "API_KEY_INVALID",
                      "domain": "googleapis.com" }
                ]
            }
        })
        .to_string();
        assert_eq!(
            error_for_status(400, &body, &lang("th")),
            ProviderError::Authentication
        );
    }

    #[test]
    fn api_key_invalid_outside_details_reason_is_not_authentication() {
        // Only `error.details[].reason` and `error.message` count, never a
        // raw search of the body.
        let elsewhere = [
            json!({ "error": { "code": 400, "message": "Required Text", "status": "API_KEY_INVALID" } }),
            json!({ "error": { "code": 400, "message": "Required Text",
                               "errors": [{ "reason": "API_KEY_INVALID" }] } }),
            json!({ "note": "API key not valid", "error": { "code": 400, "message": "Required Text" } }),
        ];
        for body in elsewhere {
            assert_eq!(
                error_for_status(400, &body.to_string(), &lang("th")),
                ProviderError::Network(NetworkReason::Status(400)),
                "{body}"
            );
        }
    }

    #[test]
    fn malformed_json_on_a_bad_request_keeps_the_status() {
        for body in [
            "API_KEY_INVALID",
            "{\"error\": {\"message\": \"API key not valid\"",
            "<html>Bad language pair</html>",
        ] {
            assert_eq!(
                error_for_status(400, body, &lang("th")),
                ProviderError::Network(NetworkReason::Status(400)),
                "{body}"
            );
        }
    }

    #[test]
    fn other_bad_requests_keep_the_status() {
        for body in [
            google_error("Required Text", "required"),
            "not json".to_string(),
        ] {
            assert_eq!(
                error_for_status(400, &body, &lang("th")),
                ProviderError::Network(NetworkReason::Status(400))
            );
        }
    }

    #[test]
    fn errors_never_contain_the_response_body() {
        let body = google_error(&format!("Bad language pair, key={FAKE_KEY}"), "badRequest");
        for status in [400, 401, 429, 500] {
            let error = error_for_status(status, &body, &lang("th"));
            let shown = format!("{error} {error:?} {error:#?}");
            assert!(!shown.contains(FAKE_KEY), "{shown}");
            assert!(!shown.contains("Bad language pair"), "{shown}");
        }
    }

    // --- transport errors -------------------------------------------------------

    #[test]
    fn transport_errors_map_to_network_reasons() {
        use std::io;
        let cases = [
            (
                ureq::Error::Timeout(ureq::Timeout::Global),
                NetworkReason::Timeout,
            ),
            (
                ureq::Error::Timeout(ureq::Timeout::Connect),
                NetworkReason::Timeout,
            ),
            (
                ureq::Error::Io(io::Error::from(io::ErrorKind::TimedOut)),
                NetworkReason::Timeout,
            ),
            (ureq::Error::HostNotFound, NetworkReason::Connect),
            (ureq::Error::ConnectionFailed, NetworkReason::Connect),
            (
                ureq::Error::Io(io::Error::from(io::ErrorKind::ConnectionRefused)),
                NetworkReason::Connect,
            ),
            (
                ureq::Error::ConnectProxyFailed("proxy said no".to_string()),
                NetworkReason::Connect,
            ),
            (ureq::Error::Tls("bad certificate"), NetworkReason::Tls),
            (
                ureq::Error::Io(io::Error::new(io::ErrorKind::InvalidData, "handshake")),
                NetworkReason::Tls,
            ),
        ];
        for (error, reason) in cases {
            let shown = format!("{error}");
            assert_eq!(
                transport_error(error),
                ProviderError::Network(reason),
                "{shown}"
            );
        }
    }

    #[test]
    fn a_status_error_from_ureq_uses_the_status_mapping() {
        assert_eq!(
            transport_error(ureq::Error::StatusCode(401)),
            ProviderError::Authentication
        );
    }

    #[test]
    fn other_transport_errors_are_invalid_response() {
        assert_eq!(
            transport_error(ureq::Error::BodyExceedsLimit(10)),
            ProviderError::InvalidResponse
        );
    }

    #[test]
    fn transport_errors_never_carry_their_text() {
        let error = transport_error(ureq::Error::ConnectProxyFailed(format!(
            "http://user:{FAKE_KEY}@proxy"
        )));
        assert!(!format!("{error} {error:?}").contains(FAKE_KEY));
    }

    // --- reading the response -----------------------------------------------------

    /// A body that must never be read.
    struct UnreadBody;

    impl Read for UnreadBody {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("the body must not be read for this status");
        }
    }

    /// A body whose every read fails with the error `make` builds.
    struct FailingBody(fn() -> std::io::Error);

    impl Read for FailingBody {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err((self.0)())
        }
    }

    const OK_BODY: &str =
        r#"{"data":{"translations":[{"translatedText":"สวัสดี","detectedSourceLanguage":"en"}]}}"#;

    #[test]
    fn a_200_body_is_read_and_parsed() {
        let translation = read_response(200, &mut OK_BODY.as_bytes(), &lang("th")).unwrap();
        assert_eq!(translation.text, "สวัสดี");
    }

    #[test]
    fn a_400_body_is_read_to_classify_it() {
        let body = google_error("Bad language pair: en|tlh", "badRequest");
        assert_eq!(
            read_response(400, &mut body.as_bytes(), &lang("tlh")).unwrap_err(),
            ProviderError::UnsupportedLanguage(lang("tlh"))
        );
    }

    #[test]
    fn other_statuses_do_not_read_the_body() {
        for (status, expected) in [
            (401, ProviderError::Authentication),
            (403, ProviderError::Authentication),
            (429, ProviderError::RateLimited),
            (404, ProviderError::Network(NetworkReason::Status(404))),
            (500, ProviderError::Network(NetworkReason::Status(500))),
            (302, ProviderError::Network(NetworkReason::Status(302))),
        ] {
            assert_eq!(
                read_response(status, &mut UnreadBody, &lang("th")).unwrap_err(),
                expected,
                "{status}"
            );
        }
    }

    #[test]
    fn body_read_failures_use_the_transport_mapping_not_the_status() {
        fn timed_out() -> std::io::Error {
            std::io::Error::from(ErrorKind::TimedOut)
        }
        fn ureq_timeout() -> std::io::Error {
            // How ureq's body reader reports its own timeout.
            ureq::Error::Timeout(ureq::Timeout::Global).into_io()
        }
        fn reset() -> std::io::Error {
            std::io::Error::from(ErrorKind::ConnectionReset)
        }
        for status in [200, 400] {
            for (make, reason) in [
                (timed_out as fn() -> std::io::Error, NetworkReason::Timeout),
                (ureq_timeout, NetworkReason::Timeout),
                (reset, NetworkReason::Connect),
            ] {
                assert_eq!(
                    read_response(status, &mut FailingBody(make), &lang("th")).unwrap_err(),
                    ProviderError::Network(reason),
                    "{status}"
                );
            }
        }
    }

    #[test]
    fn a_200_body_that_is_not_utf8_is_invalid_response() {
        let bytes: &[u8] = &[0xff, 0xfe, 0xfd];
        assert_eq!(
            read_response(200, &mut &bytes[..], &lang("th")).unwrap_err(),
            ProviderError::InvalidResponse
        );
    }

    // --- logging ----------------------------------------------------------------------

    #[test]
    fn trace_logging_is_compiled_out() {
        // ureq-proto logs raw request headers and bodies at TRACE, which
        // would leak the API key and the source text to any TRACE logger.
        assert!(log::STATIC_MAX_LEVEL <= log::LevelFilter::Debug);
    }

    // --- the Provider itself ------------------------------------------------------

    #[test]
    fn provider_name_is_google() {
        let provider =
            GoogleProvider::new(Secret::new(FAKE_KEY.to_string()), Duration::from_secs(15));
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn provider_debug_never_shows_the_key() {
        let provider =
            GoogleProvider::new(Secret::new(FAKE_KEY.to_string()), Duration::from_secs(15));
        for output in [format!("{provider:?}"), format!("{provider:#?}")] {
            assert!(!output.contains(FAKE_KEY), "leaked in: {output}");
            assert!(output.contains("<redacted>"), "{output}");
        }
    }
}
