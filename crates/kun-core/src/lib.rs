//! Core logic for `kun`. No terminal I/O lives here.

pub mod config;
pub mod credentials;
pub mod language;
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
