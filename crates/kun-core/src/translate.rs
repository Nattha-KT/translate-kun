//! Translation orchestration: choosing the Target Language (including the
//! Language Pair rule) and calling a Provider. No terminal I/O.

use crate::config::LanguagePair;
use crate::language::LanguageCode;
use crate::provider::{Provider, ProviderError, Translation, TranslationRequest};
use thiserror::Error;

/// Where the text should be translated to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPlan {
    /// One Target Language (explicit `:xx` / `-t`, or `target` from config).
    Fixed(LanguageCode),
    /// A Language Pair: the direction depends on the Source Language.
    Pair(LanguagePair),
    /// Nothing says where to translate to (exit code 2 in the CLI).
    Missing,
}

/// Target Language resolution, first match wins (requirements §3):
/// explicit, then Language Pair, then the configured `target`.
pub fn resolve_target(
    explicit: Option<&LanguageCode>,
    pair: Option<&LanguagePair>,
    configured: Option<&LanguageCode>,
) -> TargetPlan {
    if let Some(code) = explicit {
        TargetPlan::Fixed(code.clone())
    } else if let Some(pair) = pair {
        TargetPlan::Pair(pair.clone())
    } else if let Some(code) = configured {
        TargetPlan::Fixed(code.clone())
    } else {
        TargetPlan::Missing
    }
}

/// Why a translation could not be produced.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TranslateError {
    #[error("no Target Language: use `:xx`, `-t xx`, or set `target` or `pair` in a config file")]
    NoTarget,
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// A successful translation plus what the CLI should warn about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translated {
    pub translation: Translation,
    /// The Source Language equals the Target Language, so the input was
    /// returned unchanged (requirements §3). The CLI prints a warning.
    pub same_language: bool,
}

/// Translates `text` following `plan`. See the S5 slice spec for the rules.
pub fn translate(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    plan: TargetPlan,
) -> Result<Translated, TranslateError> {
    match plan {
        TargetPlan::Missing => Err(TranslateError::NoTarget),
        TargetPlan::Fixed(target) => translate_to(provider, text, source, target),
        TargetPlan::Pair(pair) => translate_pair(provider, text, source, &pair),
    }
}

/// One Target Language. Returns the input unchanged when the Source Language
/// is already the Target Language.
fn translate_to(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    target: LanguageCode,
) -> Result<Translated, TranslateError> {
    // An explicit source that equals the target needs no Provider call.
    if source.as_ref() == Some(&target) {
        return Ok(unchanged(provider, text, source));
    }

    let request = TranslationRequest {
        text: text.to_string(),
        source,
        target: target.clone(),
    };
    let translation = provider.translate(&request)?;

    if translation.detected_source.as_ref() == Some(&target) {
        return Ok(unchanged(provider, text, translation.detected_source));
    }
    Ok(Translated {
        translation,
        same_language: false,
    })
}

/// The Language Pair rule (requirements §3): the first language is the
/// default target, and text already in the first language goes to the second.
fn translate_pair(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    pair: &LanguagePair,
) -> Result<Translated, TranslateError> {
    // The source is known, so the direction is known: one call.
    if let Some(source) = source {
        let target = if &source == pair.first() {
            pair.second().clone()
        } else {
            pair.first().clone()
        };
        return translate_to(provider, text, Some(source), target);
    }

    let first = translate_to(provider, text, None, pair.first().clone())?;
    if first.translation.detected_source.as_ref() != Some(pair.first()) {
        return Ok(first);
    }

    // The text is in the first language: translate again to the second one.
    // The source is already known, so the Provider does not detect it again.
    let mut second = translate_to(
        provider,
        text,
        Some(pair.first().clone()),
        pair.second().clone(),
    )?;
    second.translation.detected_source = Some(pair.first().clone());
    Ok(second)
}

/// The input returned as it is, for when no translation is needed.
fn unchanged(
    provider: &dyn Provider,
    text: &str,
    detected_source: Option<LanguageCode>,
) -> Translated {
    Translated {
        translation: Translation {
            text: text.to_string(),
            detected_source,
            provider: provider.name(),
        },
        same_language: true,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::provider::fake::FakeProvider;

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn pair(a: &str, b: &str) -> LanguagePair {
        LanguagePair::new(lang(a), lang(b)).unwrap()
    }

    #[test]
    fn explicit_target_wins_over_pair_and_config() {
        let plan = resolve_target(
            Some(&lang("ja")),
            Some(&pair("en", "th")),
            Some(&lang("fr")),
        );
        assert_eq!(plan, TargetPlan::Fixed(lang("ja")));
    }

    #[test]
    fn pair_wins_over_configured_target() {
        let plan = resolve_target(None, Some(&pair("en", "th")), Some(&lang("fr")));
        assert_eq!(plan, TargetPlan::Pair(pair("en", "th")));
    }

    #[test]
    fn configured_target_is_used_without_explicit_or_pair() {
        let plan = resolve_target(None, None, Some(&lang("fr")));
        assert_eq!(plan, TargetPlan::Fixed(lang("fr")));
    }

    #[test]
    fn nothing_given_is_missing() {
        assert_eq!(resolve_target(None, None, None), TargetPlan::Missing);
    }

    /// Wraps the fake Provider and counts how many times it is called.
    struct CountingProvider {
        inner: FakeProvider,
        calls: Cell<usize>,
    }

    impl CountingProvider {
        fn new() -> Self {
            CountingProvider {
                inner: FakeProvider::new(),
                calls: Cell::new(0),
            }
        }
    }

    impl Provider for CountingProvider {
        fn name(&self) -> &'static str {
            self.inner.name()
        }

        fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
            self.calls.set(self.calls.get() + 1);
            self.inner.translate(request)
        }
    }

    const THAI: &str = "สวัสดี";

    fn run(text: &str, source: Option<&str>, plan: TargetPlan) -> (Translated, usize) {
        let provider = CountingProvider::new();
        let result = translate(&provider, text, source.map(lang), plan).unwrap();
        (result, provider.calls.get())
    }

    #[test]
    fn fixed_target_translates_once() {
        let (result, calls) = run("hello", None, TargetPlan::Fixed(lang("th")));
        assert_eq!(result.translation.text, "[th] hello");
        assert_eq!(result.translation.detected_source, Some(lang("en")));
        assert!(!result.same_language);
        assert_eq!(calls, 1);
    }

    #[test]
    fn missing_plan_is_an_error() {
        let provider = CountingProvider::new();
        let error = translate(&provider, "hello", None, TargetPlan::Missing).unwrap_err();
        assert_eq!(error, TranslateError::NoTarget);
        assert_eq!(provider.calls.get(), 0);
    }

    #[test]
    fn provider_errors_are_passed_on() {
        let provider = FakeProvider::failing(ProviderError::RateLimited);
        let error = translate(&provider, "hello", None, TargetPlan::Fixed(lang("th"))).unwrap_err();
        assert_eq!(error, TranslateError::Provider(ProviderError::RateLimited));
    }

    #[test]
    fn same_language_returns_input_unchanged_with_a_flag() {
        let (result, _) = run("hello", None, TargetPlan::Fixed(lang("en")));
        assert_eq!(result.translation.text, "hello");
        assert_eq!(result.translation.detected_source, Some(lang("en")));
        assert_eq!(result.translation.provider, "fake");
        assert!(result.same_language);
    }

    #[test]
    fn explicit_source_equal_to_target_needs_no_provider_call() {
        let (result, calls) = run("hello", Some("en"), TargetPlan::Fixed(lang("en")));
        assert_eq!(result.translation.text, "hello");
        assert!(result.same_language);
        assert_eq!(calls, 0);
    }

    #[test]
    fn pair_english_input_goes_to_the_second_language() {
        // detected en == first, so a second call translates to `th`.
        let (result, calls) = run("hello", None, TargetPlan::Pair(pair("en", "th")));
        assert_eq!(result.translation.text, "[th] hello");
        assert_eq!(result.translation.detected_source, Some(lang("en")));
        assert!(!result.same_language);
        assert_eq!(calls, 2);
    }

    #[test]
    fn pair_thai_input_goes_to_the_first_language() {
        // detected th != first (en), so the first result stands.
        let (result, calls) = run(THAI, None, TargetPlan::Pair(pair("en", "th")));
        assert_eq!(result.translation.text, format!("[en] {THAI}"));
        assert_eq!(result.translation.detected_source, Some(lang("th")));
        assert_eq!(calls, 1);
    }

    #[test]
    fn pair_works_for_any_languages_not_just_en_th() {
        // Pair th/ja: Thai is the first language, so it goes to `ja`.
        let (result, calls) = run(THAI, None, TargetPlan::Pair(pair("th", "ja")));
        assert_eq!(result.translation.text, format!("[ja] {THAI}"));
        assert_eq!(calls, 2);

        // English is neither language: translate to the first (`th`).
        let (result, calls) = run("hello", None, TargetPlan::Pair(pair("th", "ja")));
        assert_eq!(result.translation.text, "[th] hello");
        assert_eq!(calls, 1);
    }

    #[test]
    fn pair_with_explicit_source_decides_direction_in_one_call() {
        let plan = TargetPlan::Pair(pair("en", "th"));

        let (result, calls) = run("hello", Some("en"), plan.clone());
        assert_eq!(result.translation.text, "[th] hello");
        assert_eq!(calls, 1);

        let (result, calls) = run(THAI, Some("th"), plan.clone());
        assert_eq!(result.translation.text, format!("[en] {THAI}"));
        assert_eq!(calls, 1);

        // A source outside the pair goes to the first language.
        let (result, calls) = run("bonjour", Some("fr"), plan);
        assert_eq!(result.translation.text, "[en] bonjour");
        assert_eq!(calls, 1);
    }
}
