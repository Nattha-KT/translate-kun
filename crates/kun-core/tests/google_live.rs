//! The one real call to Google Cloud Translation.
//!
//! Only built with `--features live`, and skipped when `KUN_GOOGLE_API_KEY`
//! is not set. Never run in CI.
#![cfg(feature = "live")]

use std::time::Duration;

use kun_core::config::ProviderName;
use kun_core::credentials::{Secret, env_var_name};
use kun_core::provider::google::GoogleProvider;
use kun_core::provider::{Provider, TranslationRequest};

#[test]
fn translates_english_to_thai() {
    let Ok(key) = std::env::var(env_var_name(ProviderName::Google)) else {
        eprintln!("skipped: KUN_GOOGLE_API_KEY is not set");
        return;
    };
    let provider = GoogleProvider::new(Secret::new(key), Duration::from_secs(15));
    let request = TranslationRequest {
        text: "Good morning".to_string(),
        source: None,
        target: "th".parse().unwrap(),
    };

    let translation = provider.translate(&request).unwrap();

    assert!(!translation.text.is_empty());
    assert_eq!(translation.provider, "google");
    assert_eq!(translation.detected_source, Some("en".parse().unwrap()));
}
