//! Credential lookup through the public API, from a real User Config file.
//! Uses `MemoryStore` only: never the OS keychain.

use kun_core::config::{ProviderName, load_project_config, load_user_config};
use kun_core::credentials::{CredentialStore, MemoryStore, Secret, resolve_credential};

const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

fn no_env(_name: &str) -> Option<String> {
    None
}

#[test]
fn a_key_in_the_user_config_is_found_when_env_and_store_are_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        format!("target = \"th\"\n[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"\n"),
    )
    .unwrap();
    let user = load_user_config(&path).unwrap();

    let found = resolve_credential(
        ProviderName::Google,
        &no_env,
        &MemoryStore::new(),
        Some(&user.credentials),
    )
    .unwrap()
    .unwrap();

    assert_eq!(found.expose(), FAKE_KEY);
    for output in [
        format!("{found:?}"),
        format!("{found}"),
        format!("{user:#?}"),
    ] {
        assert!(!output.contains(FAKE_KEY), "leaked in: {output}");
    }
}

#[test]
fn the_store_beats_the_user_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[credentials]\nanthropic_api_key = \"file-FAKE\"\n").unwrap();
    let user = load_user_config(&path).unwrap();

    let mut store = MemoryStore::new();
    store
        .set(
            ProviderName::Anthropic,
            &Secret::new("store-FAKE".to_string()),
        )
        .unwrap();

    let found = resolve_credential(
        ProviderName::Anthropic,
        &no_env,
        &store,
        Some(&user.credentials),
    )
    .unwrap()
    .unwrap();
    assert_eq!(found.expose(), "store-FAKE");
}

#[test]
fn a_project_config_with_credentials_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".kun.toml");
    std::fs::write(
        &path,
        format!("[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"\n"),
    )
    .unwrap();

    let err = load_project_config(&path).unwrap_err();
    let shown = format!("{err} {err:?}");
    assert!(shown.contains("not allowed in a Project Config"), "{shown}");
    assert!(!shown.contains(FAKE_KEY), "{shown}");
}
