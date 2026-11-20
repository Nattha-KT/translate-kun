//! The one real call to the Anthropic Messages API.
//!
//! Only built with `--features live`, and skipped when `ANTHROPIC_API_KEY`
//! is not set. Never run in CI.
#![cfg(feature = "live")]

use std::time::Duration;

use kun_core::config::ProviderName;
use kun_core::credentials::{Secret, env_var_name};
use kun_core::provider::anthropic::{AnthropicProvider, DEFAULT_MODEL};
use kun_core::provider::{Provider, TranslationRequest};

#[test]
fn translates_english_to_thai() {
    let Ok(key) = std::env::var(env_var_name(ProviderName::Anthropic)) else {
        eprintln!("skipped: ANTHROPIC_API_KEY is not set");
        return;
    };
    let provider = AnthropicProvider::new(
        Secret::new(key),
        DEFAULT_MODEL.to_string(),
        Duration::from_secs(30),
    );
    let request = TranslationRequest {
        text: "Good morning".to_string(),
        source: None,
        target: "th".parse().unwrap(),
    };

    let translation = provider.translate(&request).unwrap();

    assert!(!translation.text.is_empty());
    assert_eq!(translation.provider, "anthropic");
    assert_eq!(translation.detected_source, Some("en".parse().unwrap()));
}
