//! Translation orchestration: choosing the Target Language (including the
//! Language Pair rule) and calling a Provider. No terminal I/O.

use std::fmt;

use crate::config::LanguagePair;
use crate::language::LanguageCode;
use crate::protected_terms::{protect, restore};
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
#[derive(Clone, PartialEq, Eq)]
pub struct Translated {
    pub translation: Translation,
    /// The Source Language equals the Target Language, so the input was
    /// returned unchanged (requirements §3). The CLI prints a warning.
    pub same_language: bool,
    /// A Protected Term was lost by the Provider, so it could not be put back
    /// (requirements §8). The translation is still returned; the CLI warns.
    pub protected_terms_missing: bool,
}

/// `Debug` hides the text: it is user input (or a translation of it) and may
/// hold a secret. The other fields are not sensitive.
impl fmt::Debug for Translated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Translated")
            .field("text", &"<redacted>")
            .field("detected_source", &self.translation.detected_source)
            .field("provider", &self.translation.provider)
            .field("same_language", &self.same_language)
            .field("protected_terms_missing", &self.protected_terms_missing)
            .finish()
    }
}

/// Translates `text` following `plan`, with no Protected Terms.
/// See [`translate_with_terms`].
pub fn translate(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    plan: TargetPlan,
) -> Result<Translated, TranslateError> {
    translate_with_terms(provider, text, source, plan, &[])
}

/// Translates `text` following `plan`. See the S5 slice spec for the rules.
///
/// Every Provider call is wrapped in Protected Terms: the terms are swapped
/// for placeholders before the call and put back after it. If the Provider
/// loses a placeholder, the translation is still returned, with
/// `protected_terms_missing` set.
pub fn translate_with_terms(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    plan: TargetPlan,
    protected_terms: &[String],
) -> Result<Translated, TranslateError> {
    match plan {
        TargetPlan::Missing => Err(TranslateError::NoTarget),
        TargetPlan::Fixed(target) => translate_to(provider, text, source, target, protected_terms),
        TargetPlan::Pair(pair) => translate_pair(provider, text, source, &pair, protected_terms),
    }
}

/// One Target Language. Returns the input unchanged when the Source Language
/// is already the Target Language.
fn translate_to(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    target: LanguageCode,
    terms: &[String],
) -> Result<Translated, TranslateError> {
    // An explicit source that equals the target needs no Provider call.
    if source.as_ref() == Some(&target) {
        return Ok(unchanged(provider, text, source));
    }

    let (translation, all_terms_found) =
        call_provider(provider, text, source, target.clone(), terms)?;

    if translation.detected_source.as_ref() == Some(&target) {
        return Ok(unchanged(provider, text, translation.detected_source));
    }
    Ok(Translated {
        translation,
        same_language: false,
        protected_terms_missing: !all_terms_found,
    })
}

/// One Provider call, wrapped in Protected Terms. The `bool` is `true` when
/// every Protected Term came back.
fn call_provider(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    target: LanguageCode,
    terms: &[String],
) -> Result<(Translation, bool), ProviderError> {
    let (protected_text, placeholders) = protect(text, terms);
    let mut translation = provider.translate(&TranslationRequest {
        text: protected_text,
        source,
        target,
    })?;
    let (restored, all_found) = restore(&translation.text, &placeholders);
    translation.text = restored;
    Ok((translation, all_found))
}

/// The Language Pair rule (requirements §3): the first language is the
/// default target, and text already in the first language goes to the second.
///
/// The Provider's results are returned as they are. The "already in the
/// Target Language" rule is for a single Target Language only: with a pair,
/// text in the first language is simply sent on to the second.
fn translate_pair(
    provider: &dyn Provider,
    text: &str,
    source: Option<LanguageCode>,
    pair: &LanguagePair,
    terms: &[String],
) -> Result<Translated, TranslateError> {
    // The source is known, so the direction is known: one call.
    if let Some(source) = source {
        let target = if &source == pair.first() {
            pair.second().clone()
        } else {
            pair.first().clone()
        };
        let (translation, all_found) = call_provider(provider, text, Some(source), target, terms)?;
        return Ok(Translated {
            translation,
            same_language: false,
            protected_terms_missing: !all_found,
        });
    }

    let (first, first_all_found) =
        call_provider(provider, text, None, pair.first().clone(), terms)?;
    if first.detected_source.as_ref() != Some(pair.first()) {
        return Ok(Translated {
            translation: first,
            same_language: false,
            protected_terms_missing: !first_all_found,
        });
    }

    // The text is in the first language: translate again to the second one.
    // The source is already known, so the Provider does not detect it again.
    // Only this second result is returned, so only its terms count.
    let (mut second, all_found) = call_provider(
        provider,
        text,
        Some(pair.first().clone()),
        pair.second().clone(),
        terms,
    )?;
    // A Provider that was told the source may not report it: fill it in.
    if second.detected_source.is_none() {
        second.detected_source = Some(pair.first().clone());
    }
    Ok(Translated {
        translation: second,
        same_language: false,
        protected_terms_missing: !all_found,
    })
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
        protected_terms_missing: false,
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

    /// Always reports the Target Language it was asked for as the detected
    /// source: the worst case for the same-language rule.
    struct EchoTargetProvider;

    impl Provider for EchoTargetProvider {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
            Ok(Translation {
                text: format!("[{}] {}", request.target, request.text),
                detected_source: Some(request.target.clone()),
                provider: "echo",
            })
        }
    }

    #[test]
    fn pair_returns_the_provider_results_directly() {
        // The same-language rule is for a Fixed target only. On a Pair, what
        // the Provider returns is what the user gets, with no "unchanged" flag.
        let plan = TargetPlan::Pair(pair("en", "th"));

        let result = translate(&EchoTargetProvider, "hello", None, plan.clone()).unwrap();
        assert_eq!(result.translation.text, "[th] hello");
        assert!(!result.same_language);

        let result = translate(&EchoTargetProvider, "hello", Some(lang("en")), plan).unwrap();
        assert_eq!(result.translation.text, "[th] hello");
        assert!(!result.same_language);
    }

    #[test]
    fn fixed_target_still_applies_the_same_language_rule() {
        let result = translate(
            &EchoTargetProvider,
            "hello",
            None,
            TargetPlan::Fixed(lang("en")),
        )
        .unwrap();
        assert_eq!(result.translation.text, "hello");
        assert!(result.same_language);
    }

    #[test]
    fn debug_output_never_shows_the_text() {
        let secret = "FAKE-SECRET-TEXT-EXAMPLE-12345";
        let translated = Translated {
            translation: Translation {
                text: secret.to_string(),
                detected_source: Some(lang("en")),
                provider: "fake",
            },
            same_language: true,
            protected_terms_missing: false,
        };
        for shown in [format!("{translated:?}"), format!("{translated:#?}")] {
            assert!(!shown.contains(secret), "leaked in: {shown}");
            assert!(shown.contains("<redacted>"));
            // The non-sensitive fields stay visible.
            assert!(shown.contains("fake"));
            assert!(shown.contains("same_language: true"));
        }

        let result = translate(
            &FakeProvider::new(),
            secret,
            None,
            TargetPlan::Fixed(lang("th")),
        )
        .unwrap();
        assert!(!format!("{result:?}").contains(secret));
        assert!(!format!("{result:#?}").contains(secret));
    }

    // --- Protected Terms ------------------------------------------------------

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    /// Remembers every text it was asked to translate, then behaves like the
    /// fake Provider.
    struct RecordingProvider {
        seen: std::cell::RefCell<Vec<String>>,
    }

    impl Provider for RecordingProvider {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
            self.seen.borrow_mut().push(request.text.clone());
            FakeProvider::new().translate(request)
        }
    }

    /// Drops every word it does not know, like a translator that eats a name.
    struct EatingProvider;

    impl Provider for EatingProvider {
        fn name(&self) -> &'static str {
            "eating"
        }

        fn translate(&self, _: &TranslationRequest) -> Result<Translation, ProviderError> {
            Ok(Translation {
                text: "gone".to_string(),
                detected_source: None,
                provider: "eating",
            })
        }
    }

    #[test]
    fn protected_terms_survive_a_fake_provider_round_trip() {
        let provider = RecordingProvider {
            seen: Default::default(),
        };
        let list = terms(&["translate-kun", "Orca"]);
        let result = translate_with_terms(
            &provider,
            "translate-kun runs in Orca",
            None,
            TargetPlan::Fixed(lang("th")),
            &list,
        )
        .unwrap();
        assert_eq!(result.translation.text, "[th] translate-kun runs in Orca");
        assert!(!result.protected_terms_missing);
        // The Provider only ever saw the placeholders.
        assert_eq!(
            provider.seen.borrow().as_slice(),
            ["KUNPT0X runs in KUNPT1X"]
        );
    }

    #[test]
    fn overlapping_terms_are_handled() {
        let result = translate_with_terms(
            &FakeProvider::new(),
            "translate-kun is kun",
            None,
            TargetPlan::Fixed(lang("th")),
            &terms(&["kun", "translate-kun"]),
        )
        .unwrap();
        assert_eq!(result.translation.text, "[th] translate-kun is kun");
    }

    #[test]
    fn no_terms_sends_the_text_as_it_is() {
        let provider = RecordingProvider {
            seen: Default::default(),
        };
        let result =
            translate(&provider, "hello Orca", None, TargetPlan::Fixed(lang("th"))).unwrap();
        assert_eq!(result.translation.text, "[th] hello Orca");
        assert_eq!(provider.seen.borrow().as_slice(), ["hello Orca"]);
    }

    #[test]
    fn both_pair_calls_protect_the_terms() {
        let provider = RecordingProvider {
            seen: Default::default(),
        };
        let result = translate_with_terms(
            &provider,
            "hello Orca",
            None,
            TargetPlan::Pair(pair("en", "th")),
            &terms(&["Orca"]),
        )
        .unwrap();
        // en is the first language, so there are two calls.
        assert_eq!(
            provider.seen.borrow().as_slice(),
            ["hello KUNPT0X", "hello KUNPT0X"]
        );
        assert_eq!(result.translation.text, "[th] hello Orca");
        assert!(!result.protected_terms_missing);
    }

    #[test]
    fn a_lost_term_is_a_warning_flag_not_an_error() {
        let result = translate_with_terms(
            &EatingProvider,
            "hello Orca",
            None,
            TargetPlan::Fixed(lang("th")),
            &terms(&["Orca"]),
        )
        .unwrap();
        assert_eq!(result.translation.text, "gone");
        assert!(result.protected_terms_missing);
    }

    #[test]
    fn same_language_returns_the_original_text_with_its_terms() {
        let result = translate_with_terms(
            &FakeProvider::new(),
            "hello Orca",
            Some(lang("en")),
            TargetPlan::Fixed(lang("en")),
            &terms(&["Orca"]),
        )
        .unwrap();
        assert_eq!(result.translation.text, "hello Orca");
        assert!(result.same_language);
        assert!(!result.protected_terms_missing);
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
