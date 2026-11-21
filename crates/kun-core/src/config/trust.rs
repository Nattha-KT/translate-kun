//! Project Config trust rules: keys a Project Config may never contain (ADR 0003).

use std::path::Path;

use super::error::ConfigError;
use super::load::{file_config_from_table, parse_table};
use super::schema::FileConfig;

/// A key name containing one of these looks like a credential.
const CREDENTIAL_WORDS: [&str; 5] = ["key", "token", "secret", "password", "credential"];

/// Legitimate key names that would otherwise match [`CREDENTIAL_WORDS`].
const ALLOWED_NAMES: [&str; 1] = ["secret_guard"];

/// Key names a Project Config may never contain (ADR 0003).
fn is_forbidden_key(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if ALLOWED_NAMES.contains(&name.as_str()) {
        return false;
    }
    name == "endpoint"
        || name == "allow_secrets"
        || CREDENTIAL_WORDS.iter().any(|word| name.contains(word))
}

/// The dotted path of the first forbidden key in `table`, if any.
/// Only key names are looked at, never values.
fn find_forbidden_key(table: &toml::Table, prefix: &str) -> Option<String> {
    for (name, value) in table {
        let key_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if is_forbidden_key(name) {
            return Some(key_path);
        }
        if let Some(found) = find_forbidden_key_in_value(value, &key_path) {
            return Some(found);
        }
    }
    None
}

fn find_forbidden_key_in_value(value: &toml::Value, key_path: &str) -> Option<String> {
    match value {
        toml::Value::Table(table) => find_forbidden_key(table, key_path),
        toml::Value::Array(items) => items
            .iter()
            .find_map(|item| find_forbidden_key_in_value(item, key_path)),
        _ => None,
    }
}

/// Parse Project Config text, rejecting anything the Project Config may not hold.
///
/// The check runs on the generic table, before the typed parse, so a stray
/// `api_key` is reported as a trust violation and not as an unknown field.
pub(super) fn parse_project_config(path: &Path, text: &str) -> Result<FileConfig, ConfigError> {
    let table = parse_table(path, text)?;
    if let Some(key) = find_forbidden_key(&table, "") {
        return Err(ConfigError::ForbiddenInProjectConfig {
            path: path.to_path_buf(),
            key,
        });
    }
    file_config_from_table(path, table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ProviderName;
    use std::path::PathBuf;

    const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

    fn path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    fn forbidden_key(text: &str) -> String {
        match parse_project_config(&path(), text).unwrap_err() {
            ConfigError::ForbiddenInProjectConfig { key, .. } => key,
            other => panic!("{text}: unexpected {other:?}"),
        }
    }

    #[test]
    fn project_config_may_hold_the_allowed_settings() {
        let text = r#"
target = "th"
pair = ["en", "th"]
provider = "anthropic"
protected_terms = ["translate-kun"]
style = "casual"
max_chars = 100
timeout_secs = 5

[secret_guard]
extra_patterns = ["ACME-[0-9]{6}"]
force_redact = true

[providers.anthropic]
model = "claude-haiku-5-5"
"#;
        let config = parse_project_config(&path(), text).unwrap();
        assert_eq!(config.provider, Some(ProviderName::Anthropic));
        assert!(config.secret_guard.force_redact);
        assert_eq!(config.secret_guard.extra_patterns, vec!["ACME-[0-9]{6}"]);
    }

    #[test]
    fn project_config_rejects_allow_secrets_with_any_value() {
        for value in ["true", "false"] {
            let text = format!("[secret_guard]\nallow_secrets = {value}");
            assert_eq!(forbidden_key(&text), "secret_guard.allow_secrets");
        }
    }

    #[test]
    fn project_config_rejects_any_endpoint() {
        let text = "[providers.anthropic]\nendpoint = \"http://evil.example\"";
        assert_eq!(forbidden_key(text), "providers.anthropic.endpoint");
        assert_eq!(
            forbidden_key("endpoint = \"http://evil.example\""),
            "endpoint"
        );
    }

    #[test]
    fn project_config_rejects_credential_like_keys() {
        for (text, key) in [
            ("api_key = \"x\"", "api_key"),
            ("API_KEY = \"x\"", "API_KEY"),
            (
                "[providers.google]\naccessToken = \"x\"",
                "providers.google.accessToken",
            ),
            (
                "[secret_guard]\nclient_secret = \"x\"",
                "secret_guard.client_secret",
            ),
            ("password = \"x\"", "password"),
            (
                "[providers.anthropic]\ncredentials = \"x\"",
                "providers.anthropic.credentials",
            ),
        ] {
            assert_eq!(forbidden_key(text), key);
        }
    }

    #[test]
    fn project_config_finds_credential_keys_inside_arrays_of_tables() {
        let text = "[[providers.extra]]\ntoken = \"x\"";
        assert_eq!(forbidden_key(text), "providers.extra.token");
    }

    #[test]
    fn forbidden_check_runs_before_unknown_key_check() {
        // `api_key` is also an unknown field, but the trust error must win.
        assert_eq!(forbidden_key("api_key = \"x\""), "api_key");
    }

    #[test]
    fn forbidden_error_never_contains_the_value() {
        let secret = "FAKE-SECRET-VALUE-EXAMPLE-12345";
        for text in [
            format!("api_key = \"{secret}\""),
            format!("[secret_guard]\nallow_secrets = \"{secret}\""),
            format!("[providers.anthropic]\nendpoint = \"{secret}\""),
        ] {
            let err = parse_project_config(&path(), &text).unwrap_err();
            assert!(!format!("{err}").contains(secret));
            assert!(!format!("{err:?}").contains(secret));
        }
    }

    #[test]
    fn parse_error_never_contains_the_value() {
        let secret = "FAKE-SECRET-VALUE-EXAMPLE-12345";
        for text in [
            format!("max_chars = \"{secret}\""),
            format!("target = \"{secret}"),
            format!("provider = \"{secret}\""),
            format!("pair = [\"{secret}\", \"en\"]"),
        ] {
            let err = parse_project_config(&path(), &text).unwrap_err();
            assert!(!format!("{err}").contains(secret), "{text}");
            assert!(!format!("{err:?}").contains(secret), "{text}");
        }
    }

    #[test]
    fn forbidden_error_names_the_path_and_escapes_the_key() {
        let err = parse_project_config(Path::new("/repo/.kun.toml"), "\"api_key\\u001b[2J\" = 1")
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("/repo/.kun.toml"));
        assert!(!message.contains('\u{1b}'), "terminal escape leaked");
    }

    #[test]
    fn absent_protected_terms_stay_unset_in_the_file_layer() {
        let config = parse_project_config(&path(), "target = \"th\"").unwrap();
        assert_eq!(config.protected_terms, None);
    }

    #[test]
    fn project_config_rejects_a_credentials_table() {
        for text in [
            format!("[credentials]\ngoogle_api_key = \"{FAKE_KEY}\""),
            "[credentials]".to_string(),
        ] {
            assert_eq!(forbidden_key(&text), "credentials");
            let err = parse_project_config(&path(), &text).unwrap_err();
            assert!(!format!("{err} {err:?}").contains(FAKE_KEY));
        }
    }
}
