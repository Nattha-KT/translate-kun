//! Project Config and User Config: loading, trust rules and merging.
//!
//! No terminal I/O and no credential values are handled here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::language::LanguageCode;

/// A built-in Provider that a config file may select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderName {
    Google,
    Anthropic,
}

impl ProviderName {
    /// Every built-in Provider.
    pub const ALL: [ProviderName; 2] = [ProviderName::Google, ProviderName::Anthropic];
}

/// Two different languages configured together (a Language Pair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePair {
    first: LanguageCode,
    second: LanguageCode,
}

impl LanguagePair {
    /// Returns `None` when both codes are the same language.
    pub fn new(first: LanguageCode, second: LanguageCode) -> Option<Self> {
        if first == second {
            return None;
        }
        Some(LanguagePair { first, second })
    }

    pub fn first(&self) -> &LanguageCode {
        &self.first
    }

    pub fn second(&self) -> &LanguageCode {
        &self.second
    }
}

/// Per-Provider settings from `[providers.<name>]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSettings {
    pub model: Option<String>,
    /// User Config only.
    pub endpoint: Option<String>,
}

/// The `[secret_guard]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SecretGuardSettings {
    pub extra_patterns: Vec<String>,
    pub force_redact: bool,
    /// User Config only.
    pub allow_secrets: bool,
}

/// The validated content of one config file (Project Config or User Config).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileConfig {
    pub target: Option<LanguageCode>,
    pub pair: Option<LanguagePair>,
    pub provider: Option<ProviderName>,
    pub protected_terms: Vec<String>,
    pub style: Option<String>,
    pub max_chars: Option<usize>,
    pub timeout_secs: Option<u64>,
    pub secret_guard: SecretGuardSettings,
    pub providers: HashMap<ProviderName, ProviderSettings>,
}

/// Default input limit in characters (requirements §10).
pub const DEFAULT_MAX_CHARS: usize = 10_000;

/// Default Provider timeout in seconds (requirements §10).
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Settings given on the command line. The CLI Adapter fills this in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub target: Option<LanguageCode>,
    pub provider: Option<ProviderName>,
    pub max_chars: Option<usize>,
    pub timeout_secs: Option<u64>,
    /// `--redact`.
    pub force_redact: bool,
    /// `--allow-secrets`.
    pub allow_secrets: bool,
}

/// The final settings after merging CLI, Project Config, User Config and defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub target: Option<LanguageCode>,
    pub pair: Option<LanguagePair>,
    pub provider: ProviderName,
    pub protected_terms: Vec<String>,
    pub style: Option<String>,
    pub max_chars: usize,
    pub timeout_secs: u64,
    pub secret_guard: SecretGuardSettings,
    pub providers: HashMap<ProviderName, ProviderSettings>,
}

impl EffectiveConfig {
    /// The settings for one Provider (all `None` when nothing is configured).
    pub fn provider_settings(&self, name: ProviderName) -> ProviderSettings {
        self.providers.get(&name).cloned().unwrap_or_default()
    }
}

/// Merge the layers. See the precedence table in the S2 slice spec.
pub fn merge(
    cli: &Overrides,
    project: Option<&FileConfig>,
    user: Option<&FileConfig>,
) -> EffectiveConfig {
    let empty = FileConfig::default();
    let project = project.unwrap_or(&empty);
    let user = user.unwrap_or(&empty);

    // Normal fields: CLI > Project > User > default.
    let target = cli
        .target
        .clone()
        .or_else(|| project.target.clone())
        .or_else(|| user.target.clone());
    let pair = project.pair.clone().or_else(|| user.pair.clone());
    let provider = cli
        .provider
        .or(project.provider)
        .or(user.provider)
        .unwrap_or(ProviderName::Google);
    let style = project.style.clone().or_else(|| user.style.clone());
    let max_chars = cli
        .max_chars
        .or(project.max_chars)
        .or(user.max_chars)
        .unwrap_or(DEFAULT_MAX_CHARS);
    let timeout_secs = cli
        .timeout_secs
        .or(project.timeout_secs)
        .or(user.timeout_secs)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    // Lists add up; the User Config comes first.
    let protected_terms = union(&user.protected_terms, &project.protected_terms);

    // Security fields can only get stricter, or come from a trusted layer.
    let secret_guard = SecretGuardSettings {
        extra_patterns: union(
            &user.secret_guard.extra_patterns,
            &project.secret_guard.extra_patterns,
        ),
        force_redact: cli.force_redact
            || project.secret_guard.force_redact
            || user.secret_guard.force_redact,
        // Never from the Project Config, even if one was built by hand.
        allow_secrets: cli.allow_secrets || user.secret_guard.allow_secrets,
    };

    let mut providers = HashMap::new();
    for name in ProviderName::ALL {
        let from_project = project.providers.get(&name).cloned().unwrap_or_default();
        let from_user = user.providers.get(&name).cloned().unwrap_or_default();
        let settings = ProviderSettings {
            model: from_project.model.or(from_user.model),
            // Endpoints come from the User Config only (ADR 0003).
            endpoint: from_user.endpoint,
        };
        if settings != ProviderSettings::default() {
            providers.insert(name, settings);
        }
    }

    EffectiveConfig {
        target,
        pair,
        provider,
        protected_terms,
        style,
        max_chars,
        timeout_secs,
        secret_guard,
        providers,
    }
}

/// `first` followed by the items of `second` not already present. Duplicates
/// inside either list are dropped too.
fn union(first: &[String], second: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for item in first.iter().chain(second) {
        if !result.contains(item) {
            result.push(item.clone());
        }
    }
    result
}

/// Why a config file could not be used.
///
/// Messages hold only paths and key names, never config values.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config file {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse config file {}", path.display())]
    Parse { path: PathBuf },
    // `{key:?}` escapes control characters, so a hostile key name cannot
    // inject terminal escape sequences into the message.
    #[error("{key:?} is not allowed in a Project Config ({})", path.display())]
    ForbiddenInProjectConfig { path: PathBuf, key: String },
    #[error("invalid setting {} in {}", key, path.display())]
    Invalid { path: PathBuf, key: String },
}

/// The shape of a config file as written. Converted into [`FileConfig`].
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawConfig {
    target: Option<String>,
    pair: Option<Vec<String>>,
    provider: Option<ProviderName>,
    protected_terms: Vec<String>,
    style: Option<String>,
    max_chars: Option<usize>,
    timeout_secs: Option<u64>,
    secret_guard: SecretGuardSettings,
    providers: HashMap<ProviderName, ProviderSettings>,
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
    })
}

/// File name of a Project Config.
const PROJECT_CONFIG_NAME: &str = ".kun.toml";

/// Walk up from `start` to the filesystem root and return the closest `.kun.toml`.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(PROJECT_CONFIG_NAME))
        .find(|candidate| candidate.is_file())
}

/// `$XDG_CONFIG_HOME/kun/config.toml`, otherwise `~/.config/kun/config.toml`.
///
/// The caller passes the environment values in, so this stays testable.
pub fn user_config_path(home: Option<&Path>, xdg_config_home: Option<&Path>) -> Option<PathBuf> {
    // The XDG spec says a relative `XDG_CONFIG_HOME` must be ignored.
    if let Some(xdg) = xdg_config_home.filter(|path| path.is_absolute()) {
        return Some(xdg.join("kun").join("config.toml"));
    }
    home.map(|home| home.join(".config").join("kun").join("config.toml"))
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

/// Read and validate a User Config.
pub fn load_user_config(path: &Path) -> Result<FileConfig, ConfigError> {
    parse_user_config(path, &read_file(path)?)
}

/// True when the file mode is `0o600` or stricter. Unix only.
///
/// A file that cannot be inspected counts as not private.
#[cfg(unix)]
pub fn is_private_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        // `0o177` = every permission bit that 0600 does not grant.
        Ok(metadata) => metadata.permissions().mode() & 0o177 == 0,
        Err(_) => false,
    }
}

/// Parse TOML text into a generic table.
///
/// The `toml` error is dropped on purpose: its message quotes the source line,
/// which could contain a credential.
fn parse_table(path: &Path, text: &str) -> Result<toml::Table, ConfigError> {
    text.parse::<toml::Table>().map_err(|_| ConfigError::Parse {
        path: path.to_path_buf(),
    })
}

/// Turn a generic table into a validated [`FileConfig`].
fn file_config_from_table(path: &Path, table: toml::Table) -> Result<FileConfig, ConfigError> {
    let raw: RawConfig = table.try_into().map_err(|_| ConfigError::Parse {
        path: path.to_path_buf(),
    })?;
    validate(path, raw)
}

/// Parse Project Config text, rejecting anything the Project Config may not hold.
///
/// The check runs on the generic table, before the typed parse, so a stray
/// `api_key` is reported as a trust violation and not as an unknown field.
fn parse_project_config(path: &Path, text: &str) -> Result<FileConfig, ConfigError> {
    let table = parse_table(path, text)?;
    if let Some(key) = find_forbidden_key(&table, "") {
        return Err(ConfigError::ForbiddenInProjectConfig {
            path: path.to_path_buf(),
            key,
        });
    }
    file_config_from_table(path, table)
}

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

/// Parse User Config text. `path` is only used in error messages.
fn parse_user_config(path: &Path, text: &str) -> Result<FileConfig, ConfigError> {
    let table = parse_table(path, text)?;
    file_config_from_table(path, table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
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
        assert_eq!(config.protected_terms, vec!["translate-kun"]);
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
    fn user_config_may_hold_what_project_config_may_not() {
        let text = "[secret_guard]\nallow_secrets = true\n[providers.anthropic]\nendpoint = \"http://localhost:11434\"";
        assert!(parse_user_config(&path(), text).is_ok());
    }

    fn pair(a: &str, b: &str) -> LanguagePair {
        LanguagePair::new(lang(a), lang(b)).unwrap()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_apply_when_nothing_is_set() {
        let config = merge(&Overrides::default(), None, None);
        assert_eq!(config.max_chars, 10_000);
        assert_eq!(config.timeout_secs, 15);
        assert_eq!(config.provider, ProviderName::Google);
        assert_eq!(config.target, None);
        assert_eq!(config.pair, None);
        assert!(config.protected_terms.is_empty());
        assert_eq!(config.style, None);
        assert_eq!(config.secret_guard, SecretGuardSettings::default());
    }

    #[test]
    fn normal_fields_prefer_cli_then_project_then_user() {
        let user = FileConfig {
            target: Some(lang("fr")),
            pair: Some(pair("en", "fr")),
            provider: Some(ProviderName::Google),
            style: Some("user style".into()),
            max_chars: Some(300),
            timeout_secs: Some(30),
            ..FileConfig::default()
        };
        let project = FileConfig {
            target: Some(lang("de")),
            pair: Some(pair("en", "de")),
            provider: Some(ProviderName::Anthropic),
            style: Some("project style".into()),
            max_chars: Some(200),
            timeout_secs: Some(20),
            ..FileConfig::default()
        };
        let cli = Overrides {
            target: Some(lang("th")),
            provider: Some(ProviderName::Google),
            max_chars: Some(100),
            timeout_secs: Some(10),
            ..Overrides::default()
        };

        let all = merge(&cli, Some(&project), Some(&user));
        assert_eq!(all.target, Some(lang("th")));
        assert_eq!(all.provider, ProviderName::Google);
        assert_eq!(all.max_chars, 100);
        assert_eq!(all.timeout_secs, 10);
        // `pair` and `style` have no CLI override: Project wins.
        assert_eq!(all.pair, Some(pair("en", "de")));
        assert_eq!(all.style.as_deref(), Some("project style"));

        let no_cli = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(no_cli.target, Some(lang("de")));
        assert_eq!(no_cli.provider, ProviderName::Anthropic);
        assert_eq!(no_cli.max_chars, 200);
        assert_eq!(no_cli.timeout_secs, 20);

        let user_only = merge(&Overrides::default(), None, Some(&user));
        assert_eq!(user_only.target, Some(lang("fr")));
        assert_eq!(user_only.provider, ProviderName::Google);
        assert_eq!(user_only.max_chars, 300);
        assert_eq!(user_only.timeout_secs, 30);
        assert_eq!(user_only.pair, Some(pair("en", "fr")));
    }

    #[test]
    fn project_may_only_tighten_force_redact() {
        let on = FileConfig {
            secret_guard: SecretGuardSettings {
                force_redact: true,
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let off = FileConfig::default();

        assert!(
            merge(&Overrides::default(), Some(&on), Some(&off))
                .secret_guard
                .force_redact
        );
        assert!(
            merge(&Overrides::default(), Some(&off), Some(&on))
                .secret_guard
                .force_redact
        );
        let cli = Overrides {
            force_redact: true,
            ..Overrides::default()
        };
        assert!(
            merge(&cli, Some(&off), Some(&off))
                .secret_guard
                .force_redact
        );
        assert!(
            !merge(&Overrides::default(), Some(&off), Some(&off))
                .secret_guard
                .force_redact
        );
    }

    #[test]
    fn extra_patterns_are_a_deduplicated_union_in_order() {
        let user = FileConfig {
            secret_guard: SecretGuardSettings {
                extra_patterns: strings(&["A", "B"]),
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let project = FileConfig {
            secret_guard: SecretGuardSettings {
                extra_patterns: strings(&["C", "B", "C"]),
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(
            config.secret_guard.extra_patterns,
            strings(&["A", "B", "C"])
        );
    }

    #[test]
    fn protected_terms_are_a_deduplicated_union_in_order() {
        let user = FileConfig {
            protected_terms: strings(&["kun", "Rust"]),
            ..FileConfig::default()
        };
        let project = FileConfig {
            protected_terms: strings(&["translate-kun", "kun"]),
            ..FileConfig::default()
        };
        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(
            config.protected_terms,
            strings(&["kun", "Rust", "translate-kun"])
        );
    }

    #[test]
    fn allow_secrets_comes_from_cli_or_user_only() {
        let allowing = FileConfig {
            secret_guard: SecretGuardSettings {
                allow_secrets: true,
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let plain = FileConfig::default();

        // A Project Config value is ignored even if one got into the struct.
        let from_project = merge(&Overrides::default(), Some(&allowing), Some(&plain));
        assert!(!from_project.secret_guard.allow_secrets);

        let from_user = merge(&Overrides::default(), Some(&plain), Some(&allowing));
        assert!(from_user.secret_guard.allow_secrets);

        let cli = Overrides {
            allow_secrets: true,
            ..Overrides::default()
        };
        assert!(merge(&cli, None, None).secret_guard.allow_secrets);
    }

    #[test]
    fn endpoint_comes_from_user_only_and_model_prefers_project() {
        let mut user = FileConfig::default();
        user.providers.insert(
            ProviderName::Anthropic,
            ProviderSettings {
                model: Some("user-model".into()),
                endpoint: Some("http://localhost:11434".into()),
            },
        );
        let mut project = FileConfig::default();
        project.providers.insert(
            ProviderName::Anthropic,
            ProviderSettings {
                model: Some("project-model".into()),
                endpoint: Some("http://evil.example".into()),
            },
        );

        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        let anthropic = config.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.model.as_deref(), Some("project-model"));
        assert_eq!(
            anthropic.endpoint.as_deref(),
            Some("http://localhost:11434")
        );

        let project_only = merge(&Overrides::default(), Some(&project), None);
        let anthropic = project_only.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.endpoint, None);

        let user_only = merge(&Overrides::default(), None, Some(&user));
        let anthropic = user_only.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.model.as_deref(), Some("user-model"));

        assert_eq!(
            config.provider_settings(ProviderName::Google),
            ProviderSettings::default()
        );
    }

    #[test]
    fn find_project_config_returns_the_closest_one() {
        let root = tempfile::tempdir().unwrap();
        let inner = root.path().join("a/b");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(root.path().join(".kun.toml"), "").unwrap();
        std::fs::write(root.path().join("a/.kun.toml"), "").unwrap();

        assert_eq!(
            find_project_config(&inner),
            Some(root.path().join("a/.kun.toml"))
        );
        assert_eq!(
            find_project_config(root.path()),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn find_project_config_walks_up_several_levels() {
        let root = tempfile::tempdir().unwrap();
        let deep = root.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(root.path().join(".kun.toml"), "").unwrap();

        assert_eq!(
            find_project_config(&deep),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn find_project_config_ignores_a_directory_named_like_the_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".kun.toml")).unwrap();
        // May find a real one above the temp dir on an unusual machine, but never the directory.
        assert_ne!(
            find_project_config(root.path()),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn user_config_path_prefers_xdg_then_home() {
        let home = Path::new("/home/me");
        let xdg = Path::new("/custom/xdg");
        assert_eq!(
            user_config_path(Some(home), Some(xdg)),
            Some(PathBuf::from("/custom/xdg/kun/config.toml"))
        );
        assert_eq!(
            user_config_path(Some(home), None),
            Some(PathBuf::from("/home/me/.config/kun/config.toml"))
        );
        assert_eq!(user_config_path(None, None), None);
        assert_eq!(
            user_config_path(None, Some(xdg)),
            Some(PathBuf::from("/custom/xdg/kun/config.toml"))
        );
    }

    #[test]
    fn user_config_path_ignores_empty_or_relative_xdg() {
        let home = Path::new("/home/me");
        for xdg in ["", "relative/dir"] {
            assert_eq!(
                user_config_path(Some(home), Some(Path::new(xdg))),
                Some(PathBuf::from("/home/me/.config/kun/config.toml")),
                "{xdg:?}"
            );
        }
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
