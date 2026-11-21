//! Reading config files, validating them and checking file permissions.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::ConfigError;
#[cfg(unix)]
use super::owner::is_owned_by;
use super::schema::{
    FileConfig, FileCredentials, LanguagePair, ProviderName, ProviderSettings, SecretGuardSettings,
};
use super::trust::parse_project_config;
use crate::language::LanguageCode;

/// The shape of a config file as written. Converted into [`FileConfig`].
///
/// No `Debug`: it holds the raw free-text values.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawConfig {
    target: Option<String>,
    pair: Option<Vec<String>>,
    provider: Option<ProviderName>,
    protected_terms: Option<Vec<String>>,
    style: Option<String>,
    max_chars: Option<usize>,
    timeout_secs: Option<u64>,
    secret_guard: SecretGuardSettings,
    providers: HashMap<ProviderName, ProviderSettings>,
    credentials: FileCredentials,
}

fn invalid(path: &Path, key: &str) -> ConfigError {
    ConfigError::Invalid {
        path: path.to_path_buf(),
        key: key.to_string(),
    }
}

fn parse_language(path: &Path, key: &str, text: &str) -> Result<LanguageCode, ConfigError> {
    text.parse().map_err(|_| invalid(path, key))
}

fn validate(path: &Path, raw: RawConfig) -> Result<FileConfig, ConfigError> {
    let target = match raw.target {
        Some(text) => Some(parse_language(path, "target", &text)?),
        None => None,
    };

    let pair = match raw.pair {
        Some(codes) => {
            if codes.len() != 2 {
                return Err(invalid(path, "pair"));
            }
            let first = parse_language(path, "pair", &codes[0])?;
            let second = parse_language(path, "pair", &codes[1])?;
            Some(LanguagePair::new(first, second).ok_or_else(|| invalid(path, "pair"))?)
        }
        None => None,
    };

    if raw.max_chars == Some(0) {
        return Err(invalid(path, "max_chars"));
    }
    if raw.timeout_secs == Some(0) {
        return Err(invalid(path, "timeout_secs"));
    }

    Ok(FileConfig {
        target,
        pair,
        provider: raw.provider,
        protected_terms: raw.protected_terms,
        style: raw.style,
        max_chars: raw.max_chars,
        timeout_secs: raw.timeout_secs,
        secret_guard: raw.secret_guard,
        providers: raw.providers,
        credentials: raw.credentials,
        credentials_ignored_insecure: false,
    })
}

fn read_file(path: &Path) -> Result<String, ConfigError> {
    std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and validate a Project Config. Untrusted: see ADR 0003.
pub fn load_project_config(path: &Path) -> Result<FileConfig, ConfigError> {
    parse_project_config(path, &read_file(path)?)
}

/// Like [`load_project_config`], but `Ok(None)` when the file is owned by a
/// different user than the one running `kun` (Unix only; like git's
/// `safe.directory`). The file is opened once and the owner is read from that
/// same handle (`fstat`), so it cannot be swapped between the check and the
/// read. A file whose owner cannot be read counts as not owned.
#[cfg(unix)]
pub fn load_project_config_if_owned(path: &Path) -> Result<Option<FileConfig>, ConfigError> {
    load_project_config_if_owned_by(path, rustix::process::getuid().as_raw())
}

/// Outside Unix there is no ownership check.
#[cfg(not(unix))]
pub fn load_project_config_if_owned(path: &Path) -> Result<Option<FileConfig>, ConfigError> {
    load_project_config(path).map(Some)
}

/// [`load_project_config_if_owned`] with the current uid passed in, for tests.
#[cfg(unix)]
fn load_project_config_if_owned_by(
    path: &Path,
    current_uid: u32,
) -> Result<Option<FileConfig>, ConfigError> {
    use std::os::unix::fs::MetadataExt;

    let io_error = |source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    let owned = match file.metadata() {
        Ok(metadata) => is_owned_by(metadata.uid(), current_uid),
        Err(_) => false,
    };
    if !owned {
        return Ok(None);
    }
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(io_error)?;
    parse_project_config(path, &text).map(Some)
}

/// Read and validate a User Config.
///
/// `[credentials]` values are used only when the file is `0600` or stricter.
/// Otherwise they are dropped and `credentials_ignored_insecure` is set, so
/// the CLI can warn (requirements §6). The permission is checked on the same
/// open file that is read, so the file cannot be swapped between the check
/// and the read.
pub fn load_user_config(path: &Path) -> Result<FileConfig, ConfigError> {
    let io_error = |source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    let private = is_private_handle(&file);
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(io_error)?;

    let mut config = parse_user_config(path, &text)?;
    if !private && !config.credentials.is_empty() {
        config.credentials = FileCredentials::default();
        config.credentials_ignored_insecure = true;
    }
    Ok(config)
}

/// True when a mode grants nothing beyond `0o600`.
#[cfg(unix)]
fn mode_is_private(mode: u32) -> bool {
    // `0o177` = every permission bit that 0600 does not grant.
    mode & 0o177 == 0
}

/// Whether an open file is `0600` or stricter (`fstat` on the handle).
/// A file that cannot be inspected counts as not private.
#[cfg(unix)]
fn is_private_handle(file: &std::fs::File) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match file.metadata() {
        Ok(metadata) => mode_is_private(metadata.permissions().mode()),
        Err(_) => false,
    }
}

/// Outside Unix there are no mode bits to check; the file is trusted.
#[cfg(not(unix))]
fn is_private_handle(_file: &std::fs::File) -> bool {
    true
}

/// True when the file mode is `0o600` or stricter. Unix only.
///
/// A file that cannot be inspected counts as not private.
#[cfg(unix)]
pub fn is_private_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(metadata) => mode_is_private(metadata.permissions().mode()),
        Err(_) => false,
    }
}

/// Parse TOML text into a generic table.
///
/// The `toml` error is dropped on purpose: its message quotes the source line,
/// which could contain a credential.
pub(super) fn parse_table(path: &Path, text: &str) -> Result<toml::Table, ConfigError> {
    text.parse::<toml::Table>().map_err(|_| ConfigError::Parse {
        path: path.to_path_buf(),
    })
}

/// Turn a generic table into a validated [`FileConfig`].
pub(super) fn file_config_from_table(
    path: &Path,
    table: toml::Table,
) -> Result<FileConfig, ConfigError> {
    let raw: RawConfig = table.try_into().map_err(|_| ConfigError::Parse {
        path: path.to_path_buf(),
    })?;
    validate(path, raw)
}

/// Parse User Config text. `path` is only used in error messages.
fn parse_user_config(path: &Path, text: &str) -> Result<FileConfig, ConfigError> {
    let table = parse_table(path, text)?;
    file_config_from_table(path, table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FAKE_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

    fn path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_the_full_file_shape() {
        let text = r#"
target = "th"
pair = ["en", "th"]
provider = "google"
protected_terms = ["translate-kun"]
style = "casual, developer tone"
max_chars = 5000
timeout_secs = 30

[secret_guard]
extra_patterns = ["ACME-[0-9]{6}"]
force_redact = true
allow_secrets = true

[providers.anthropic]
model = "claude-haiku-5-5"
endpoint = "http://localhost:11434"
"#;
        let config = parse_user_config(&path(), text).unwrap();

        assert_eq!(config.target, Some(lang("th")));
        let pair = config.pair.unwrap();
        assert_eq!(pair.first(), &lang("en"));
        assert_eq!(pair.second(), &lang("th"));
        assert_eq!(config.provider, Some(ProviderName::Google));
        assert_eq!(config.protected_terms, Some(strings(&["translate-kun"])));
        assert_eq!(config.style.as_deref(), Some("casual, developer tone"));
        assert_eq!(config.max_chars, Some(5000));
        assert_eq!(config.timeout_secs, Some(30));
        assert_eq!(config.secret_guard.extra_patterns, vec!["ACME-[0-9]{6}"]);
        assert!(config.secret_guard.force_redact);
        assert!(config.secret_guard.allow_secrets);
        let anthropic = &config.providers[&ProviderName::Anthropic];
        assert_eq!(anthropic.model.as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(
            anthropic.endpoint.as_deref(),
            Some("http://localhost:11434")
        );
    }

    #[test]
    fn empty_file_is_all_defaults() {
        let config = parse_user_config(&path(), "").unwrap();
        assert_eq!(config, FileConfig::default());
    }

    #[test]
    fn unknown_keys_are_parse_errors() {
        for text in [
            "colour = 1",
            "[secret_guard]\nnope = true",
            "[providers.google]\nx = 1",
        ] {
            let err = parse_user_config(&path(), text).unwrap_err();
            assert!(matches!(err, ConfigError::Parse { .. }), "{text}");
        }
    }

    #[test]
    fn unknown_provider_is_a_parse_error() {
        for text in ["provider = \"ollama\"", "[providers.ollama]\nmodel = \"x\""] {
            let err = parse_user_config(&path(), text).unwrap_err();
            assert!(matches!(err, ConfigError::Parse { .. }), "{text}");
        }
    }

    #[test]
    fn invalid_toml_is_a_parse_error() {
        let err = parse_user_config(&path(), "target = ").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn pair_must_be_two_different_valid_codes() {
        for text in [
            r#"pair = ["en"]"#,
            r#"pair = ["en", "th", "ja"]"#,
            r#"pair = ["en", "EN"]"#,
            r#"pair = ["en", "1"]"#,
        ] {
            let err = parse_user_config(&path(), text).unwrap_err();
            match err {
                ConfigError::Invalid { key, .. } => assert_eq!(key, "pair", "{text}"),
                other => panic!("{text}: unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn user_config_may_hold_what_project_config_may_not() {
        let text = "[secret_guard]\nallow_secrets = true\n[providers.anthropic]\nendpoint = \"http://localhost:11434\"";
        assert!(parse_user_config(&path(), text).is_ok());
    }

    #[test]
    fn load_project_config_reads_a_file_and_enforces_trust() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.toml");
        std::fs::write(&good, "target = \"th\"").unwrap();
        assert_eq!(load_project_config(&good).unwrap().target, Some(lang("th")));

        let bad = dir.path().join("bad.toml");
        std::fs::write(&bad, "api_key = \"FAKE-EXAMPLE\"").unwrap();
        match load_project_config(&bad).unwrap_err() {
            ConfigError::ForbiddenInProjectConfig { path, key } => {
                assert_eq!(path, bad);
                assert_eq!(key, "api_key");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn load_user_config_reads_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[secret_guard]\nallow_secrets = true").unwrap();
        assert!(load_user_config(&file).unwrap().secret_guard.allow_secrets);
    }

    /// Write a User Config with this text and file mode.
    #[cfg(unix)]
    fn user_config_with_mode(dir: &Path, text: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let file = dir.join("config.toml");
        std::fs::write(&file, text).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
        file
    }

    #[cfg(unix)]
    #[test]
    fn credentials_in_a_readable_user_config_are_ignored_and_flagged() {
        let text = format!("target = \"th\"\n[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"");
        for mode in [0o644, 0o640, 0o604, 0o660] {
            let dir = tempfile::tempdir().unwrap();
            let file = user_config_with_mode(dir.path(), &text, mode);

            let config = load_user_config(&file).unwrap();

            assert!(
                config.credentials.api_key(ProviderName::Google).is_none(),
                "mode {mode:o}"
            );
            assert!(config.credentials_ignored_insecure, "mode {mode:o}");
            // The rest of the file still applies.
            assert_eq!(config.target, Some(lang("th")));
            assert!(!format!("{config:?}").contains(FAKE_KEY));
        }
    }

    #[cfg(unix)]
    #[test]
    fn credentials_in_a_private_user_config_are_used() {
        let text = format!("[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"");
        for mode in [0o600, 0o400] {
            let dir = tempfile::tempdir().unwrap();
            let file = user_config_with_mode(dir.path(), &text, mode);

            let config = load_user_config(&file).unwrap();

            let key = config.credentials.api_key(ProviderName::Google).unwrap();
            assert_eq!(key.expose(), FAKE_KEY, "mode {mode:o}");
            assert!(!config.credentials_ignored_insecure, "mode {mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_readable_user_config_without_credentials_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let file = user_config_with_mode(dir.path(), "target = \"th\"", 0o644);
        let config = load_user_config(&file).unwrap();
        assert!(!config.credentials_ignored_insecure);
        assert_eq!(config.target, Some(lang("th")));
    }

    #[test]
    fn missing_file_is_an_io_error_naming_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.toml");
        for err in [
            load_project_config(&missing).unwrap_err(),
            load_user_config(&missing).unwrap_err(),
        ] {
            match &err {
                ConfigError::Io { path, .. } => assert_eq!(path, &missing),
                other => panic!("unexpected {other:?}"),
            }
            assert!(err.to_string().contains("nope.toml"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_project_config_owned_by_the_current_user_is_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".kun.toml");
        std::fs::write(&file, "target = \"th\"").unwrap();

        let loaded = load_project_config_if_owned(&file).unwrap().unwrap();
        assert_eq!(loaded.target, Some(lang("th")));
    }

    #[cfg(unix)]
    #[test]
    fn a_project_config_owned_by_another_user_is_skipped_unread() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".kun.toml");
        // Not valid TOML: skipping must happen before any parsing.
        std::fs::write(&file, "this is = = not toml").unwrap();
        let file_uid = std::fs::metadata(&file).unwrap().uid();

        // Pretend we are somebody else, so no `chown` or root is needed.
        let loaded = load_project_config_if_owned_by(&file, file_uid + 1).unwrap();
        assert!(loaded.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_project_config_that_cannot_be_opened_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.toml");
        let err = load_project_config_if_owned(&missing).unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn is_private_file_accepts_0600_or_stricter_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "").unwrap();

        for (mode, expected) in [
            (0o600, true),
            (0o400, true),
            (0o200, true),
            (0o000, true),
            (0o640, false),
            (0o604, false),
            (0o660, false),
            (0o644, false),
            (0o700, false),
            (0o777, false),
        ] {
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(is_private_file(&file), expected, "mode {mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn is_private_file_is_false_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_private_file(&dir.path().join("nope.toml")));
    }

    #[test]
    fn user_config_accepts_a_credentials_table() {
        let text = format!(
            "[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"\nanthropic_api_key = \"sk-ant-FAKE-EXAMPLE\""
        );
        let config = parse_user_config(&path(), &text).unwrap();
        let google = config.credentials.api_key(ProviderName::Google).unwrap();
        assert_eq!(google.expose(), FAKE_KEY);
        let anthropic = config.credentials.api_key(ProviderName::Anthropic).unwrap();
        assert_eq!(anthropic.expose(), "sk-ant-FAKE-EXAMPLE");
    }

    #[test]
    fn credentials_table_is_optional_and_may_hold_one_key() {
        let empty = parse_user_config(&path(), "").unwrap();
        assert!(empty.credentials.api_key(ProviderName::Google).is_none());

        let text = "[credentials]\nanthropic_api_key = \"sk-ant-FAKE-EXAMPLE\"";
        let config = parse_user_config(&path(), text).unwrap();
        assert!(config.credentials.api_key(ProviderName::Google).is_none());
        assert!(
            config
                .credentials
                .api_key(ProviderName::Anthropic)
                .is_some()
        );
    }

    #[test]
    fn credentials_table_rejects_unknown_keys() {
        let err = parse_user_config(&path(), "[credentials]\nopenai_api_key = \"x\"").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn debug_output_redacts_credentials() {
        let text = format!("[credentials]\ngoogle_api_key = \"{FAKE_KEY}\"");
        let config = parse_user_config(&path(), &text).unwrap();
        for output in [
            format!("{config:?}"),
            format!("{config:#?}"),
            format!("{:?}", config.credentials),
            format!("{:#?}", config.credentials),
        ] {
            assert!(!output.contains(FAKE_KEY), "leaked in: {output}");
        }
        assert!(format!("{config:?}").contains("credentials: <redacted>"));
    }

    #[test]
    fn invalid_target_and_zero_limits_are_rejected() {
        for (text, key) in [
            ("target = \"english\"", "target"),
            ("max_chars = 0", "max_chars"),
            ("timeout_secs = 0", "timeout_secs"),
        ] {
            match parse_user_config(&path(), text).unwrap_err() {
                ConfigError::Invalid { key: got, .. } => assert_eq!(got, key),
                other => panic!("{text}: unexpected {other:?}"),
            }
        }
    }
}
