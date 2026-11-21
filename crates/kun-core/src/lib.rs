//! Core logic for `kun`, the translate-kun CLI translator. No terminal I/O
//! lives here.
//!
//! The crate holds the Providers, the Secret Guard, Protected Terms, config
//! loading and the translation flow. The `kun` binary is a thin CLI Adapter
//! on top of it.
//!
//! # Example
//!
//! Translate a text with the offline `FakeProvider`:
//!
//! ```
//! use kun_core::provider::fake::FakeProvider;
//! use kun_core::translate::{translate, TargetPlan};
//!
//! let provider = FakeProvider::new();
//! let target = "th".parse().unwrap();
//! let result = translate(&provider, "hello", None, TargetPlan::Fixed(target)).unwrap();
//!
//! assert_eq!(result.translation.text, "[th] hello");
//! ```

#![warn(missing_docs)]

pub mod config;
pub mod credentials;
pub mod language;
pub mod protected_terms;
pub mod provider;
pub mod secret_guard;
pub mod translate;

/// Returns the version of the `kun-core` crate (shared by the whole workspace).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
        assert_eq!(version(), "0.1.0");
    }
}
