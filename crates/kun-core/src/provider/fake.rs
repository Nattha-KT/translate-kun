//! A deterministic, offline Provider for tests (also usable from the `kun` crate).

use super::{Provider, ProviderError, Translation, TranslationRequest};

/// Fake Provider. Wraps text as `"[<target>] <text>"`, or fails on demand.
#[derive(Debug, Clone, Default)]
pub struct FakeProvider {
    error: Option<ProviderError>,
}

impl FakeProvider {
    /// A Provider that always translates successfully.
    pub fn new() -> Self {
        FakeProvider { error: None }
    }

    /// A Provider whose every `translate` call returns `error`.
    pub fn failing(error: ProviderError) -> Self {
        FakeProvider { error: Some(error) }
    }
}

/// True if `text` contains any character in the Thai block (U+0E00–U+0E7F).
fn contains_thai(text: &str) -> bool {
    text.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c))
}

impl Provider for FakeProvider {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }

        let detected_source = match request.source {
            Some(_) => None,
            None => {
                let code = if contains_thai(&request.text) {
                    "th"
                } else {
                    "en"
                };
                code.parse().ok()
            }
        };

        Ok(Translation {
            text: format!("[{}] {}", request.target, request.text),
            detected_source,
            provider: self.name(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageCode;

    fn lang(s: &str) -> LanguageCode {
        s.parse().unwrap()
    }

    fn request(text: &str, source: Option<&str>, target: &str) -> TranslationRequest {
        TranslationRequest {
            text: text.to_string(),
            source: source.map(lang),
            target: lang(target),
        }
    }

    #[test]
    fn wraps_text_with_target_language() {
        let result = FakeProvider::new()
            .translate(&request("hello", None, "th"))
            .unwrap();
        assert_eq!(result.text, "[th] hello");
        assert_eq!(result.provider, "fake");
    }

    #[test]
    fn detects_english_when_source_is_none() {
        let result = FakeProvider::new()
            .translate(&request("hello", None, "th"))
            .unwrap();
        assert_eq!(result.detected_source, Some(lang("en")));
    }

    #[test]
    fn detects_thai_script_when_source_is_none() {
        let result = FakeProvider::new()
            .translate(&request("สวัสดี hello", None, "en"))
            .unwrap();
        assert_eq!(result.detected_source, Some(lang("th")));
    }

    #[test]
    fn does_not_detect_when_source_is_given() {
        let result = FakeProvider::new()
            .translate(&request("สวัสดี", Some("th"), "en"))
            .unwrap();
        assert_eq!(result.detected_source, None);
    }

    #[test]
    fn failing_provider_returns_the_chosen_error() {
        let provider = FakeProvider::failing(ProviderError::QuotaExceeded);
        let error = provider
            .translate(&request("hello", None, "th"))
            .unwrap_err();
        assert_eq!(error, ProviderError::QuotaExceeded);
    }

    #[test]
    fn works_as_a_boxed_trait_object() {
        let provider: Box<dyn Provider> = Box::new(FakeProvider::new());
        assert_eq!(provider.name(), "fake");
        assert!(provider.translate(&request("hi", None, "th")).is_ok());
    }
}
