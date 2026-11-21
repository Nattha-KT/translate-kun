//! Fuzz target: parse arbitrary bytes as a Project Config and as a User Config.
//!
//! The loaders read from a path, so the bytes are written to a temporary file
//! first. Bytes that are not UTF-8, or not valid TOML, are an `Err`, never a
//! panic. A Project Config that does load must follow ADR 0003.

#![no_main]

use std::path::PathBuf;

use kun_core::config::{ProviderName, load_project_config, load_user_config};
use libfuzzer_sys::fuzz_target;

/// One file per process, so parallel fuzz workers do not step on each other.
fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kun-fuzz-{}-{name}", std::process::id()))
}

fuzz_target!(|data: &[u8]| {
    let project_path = temp_path("project.toml");
    let user_path = temp_path("user.toml");
    if std::fs::write(&project_path, data).is_err() || std::fs::write(&user_path, data).is_err() {
        return;
    }

    if let Ok(config) = load_project_config(&project_path) {
        // ADR 0003: a Project Config can never carry credentials, an
        // endpoint, or `allow_secrets`.
        assert!(!config.secret_guard.allow_secrets);
        for name in ProviderName::ALL {
            assert!(config.credentials.api_key(name).is_none());
            if let Some(settings) = config.providers.get(&name) {
                assert!(settings.endpoint.is_none());
            }
        }
    }

    // Only "does not panic" is checked for a User Config.
    let _ = load_user_config(&user_path);
});
