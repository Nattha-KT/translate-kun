//! Every way `kun` can fail, and the exit code each one maps to (requirements §9).
//!
//! No variant holds user text, so printing or `{:?}`-formatting an error can
//! never leak the input, a config value or a credential.

use std::fmt;

use kun_core::config::ConfigError;
use kun_core::credentials::CredentialError;
use kun_core::provider::ProviderError;
use kun_core::secret_guard::{Finding, SecretGuardError};
use kun_core::translate::TranslateError;

/// Which config file a problem is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFile {
    Project,
    User,
}

impl fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigFile::Project => f.write_str("Project Config (.kun.toml)"),
            ConfigFile::User => f.write_str("User Config (config.toml)"),
        }
    }
}

/// What is wrong with a config file, as a fixed category.
///
/// A `field` is always one of the names in `KNOWN_FIELDS` (`'static`), never
/// text from the file: key names and file paths are chosen by the user (or by
/// whoever wrote the repository) and could hold a pasted secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigProblem {
    /// The file could not be read.
    Unreadable,
    /// The file is not valid TOML, or has a key or value of the wrong shape.
    Unparsable,
    /// A Project Config holds a setting it may not hold (ADR 0003).
    /// `None` = an unknown or credential-like key.
    Forbidden { field: Option<&'static str> },
    /// A setting has a value that is not allowed.
    InvalidValue { field: Option<&'static str> },
}

/// The only key names that may appear in an error message.
const KNOWN_FIELDS: [&str; 12] = [
    "target",
    "pair",
    "provider",
    "max_chars",
    "timeout_secs",
    "style",
    "protected_terms",
    "endpoint",
    "secret_guard.allow_secrets",
    "providers.google.endpoint",
    "providers.anthropic.endpoint",
    "secret_guard.extra_patterns",
];

/// `key` as a `'static` name if it is one of the schema's own keys.
fn known_field(key: &str) -> Option<&'static str> {
    KNOWN_FIELDS.into_iter().find(|known| *known == key)
}

#[derive(Debug)]
pub enum CliError {
    /// Bad arguments or input. The message is fixed text written in `kun`.
    Usage(&'static str),
    /// There is no text to translate. `main` prints the help.
    NoText,
    /// A config file could not be used.
    Config {
        file: ConfigFile,
        problem: ConfigProblem,
    },
    /// An extra Secret Guard pattern from a config file is not a valid regex.
    ExtraPattern(SecretGuardError),
    /// The Provider failed (including missing credentials).
    Provider(ProviderError),
    /// The Secret Guard blocked the text.
    SecretBlocked(Vec<Finding>),
    /// The user answered "no" at the Secret Guard prompt (or it could not be asked).
    SecretDeclined,
    /// The input is longer than `max_chars`.
    TooLarge { limit: usize },
    /// No Google API key in the environment, the keychain or the User Config.
    NoGoogleKey,
    /// No Anthropic API key in the environment, the keychain or the User Config.
    NoAnthropicKey,
    /// There is no OS keychain on this machine (`kun auth login`).
    KeychainUnavailable { env_var: &'static str },
    /// The config sets `providers.<provider>.endpoint`, which is not built
    /// yet. `provider` is a built-in Provider name, never config text.
    EndpointNotSupported { provider: &'static str },
    /// The OS keychain exists but a read, write or delete failed.
    KeychainFailed,
    /// Something unexpected, such as a failed write to stdout.
    Unexpected(&'static str),
}

impl CliError {
    /// Turns a `kun-core` config error into a fixed category. The error's own
    /// message (which holds paths and key names) is dropped on purpose.
    pub fn config(file: ConfigFile, error: &ConfigError) -> Self {
        let problem = match error {
            ConfigError::Io { .. } => ConfigProblem::Unreadable,
            ConfigError::Parse { .. } => ConfigProblem::Unparsable,
            ConfigError::ForbiddenInProjectConfig { key, .. } => ConfigProblem::Forbidden {
                field: known_field(key),
            },
            ConfigError::Invalid { key, .. } => ConfigProblem::InvalidValue {
                field: known_field(key),
            },
            // `ConfigError` is `#[non_exhaustive]`.
            _ => ConfigProblem::Unparsable,
        };
        CliError::Config { file, problem }
    }

    /// The process exit code. The only place where errors become codes.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Usage(_) | CliError::NoText => 2,
            CliError::Config { .. }
            | CliError::ExtraPattern(_)
            | CliError::KeychainUnavailable { .. }
            | CliError::EndpointNotSupported { .. } => 3,
            CliError::Provider(ProviderError::MissingCredentials)
            | CliError::NoGoogleKey
            | CliError::NoAnthropicKey => 4,
            CliError::Provider(_) => 5,
            CliError::SecretBlocked(_) | CliError::SecretDeclined => 6,
            CliError::TooLarge { .. } => 7,
            CliError::KeychainFailed | CliError::Unexpected(_) => 1,
        }
    }
}

impl CliError {
    /// A keychain failure for `provider`. `env_var` is the variable the
    /// message suggests when there is no keychain.
    pub fn keychain(error: CredentialError, env_var: &'static str) -> Self {
        match error {
            CredentialError::StoreUnavailable => CliError::KeychainUnavailable { env_var },
            CredentialError::StoreFailed => CliError::KeychainFailed,
            // `CredentialError` is `#[non_exhaustive]`.
            _ => CliError::KeychainFailed,
        }
    }
}

impl From<TranslateError> for CliError {
    fn from(error: TranslateError) -> Self {
        match error {
            TranslateError::NoTarget => CliError::Usage(
                "no Target Language: use `:xx`, `-t xx`, or set `target` or `pair` in a config file",
            ),
            TranslateError::Provider(error) => CliError::Provider(error),
            // `TranslateError` is `#[non_exhaustive]`.
            _ => CliError::Unexpected("translation failed"),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(message) | CliError::Unexpected(message) => f.write_str(message),
            CliError::NoText => f.write_str("no text to translate"),
            CliError::Config { file, problem } => match problem {
                ConfigProblem::Unreadable => write!(f, "cannot read the {file}"),
                ConfigProblem::Unparsable => write!(
                    f,
                    "cannot parse the {file}: it is not valid TOML, or has an unknown key \
                     or a value of the wrong type"
                ),
                ConfigProblem::Forbidden { field: Some(field) } => write!(
                    f,
                    "the {file} sets `{field}`, which is not allowed there \
                     (credentials, endpoints and allow_secrets belong in the User Config)"
                ),
                ConfigProblem::Forbidden { field: None } => write!(
                    f,
                    "the {file} has a key that is not allowed there \
                     (an unknown or credential-like key)"
                ),
                ConfigProblem::InvalidValue { field: Some(field) } => {
                    write!(f, "invalid value for `{field}` in the {file}")
                }
                ConfigProblem::InvalidValue { field: None } => {
                    write!(f, "invalid value for a setting in the {file}")
                }
            },
            CliError::ExtraPattern(error) => write!(f, "{error}"),
            CliError::Provider(error) => match error {
                ProviderError::MissingCredentials => f.write_str(
                    "no credentials found for this provider (run `kun auth login <provider>`)",
                ),
                ProviderError::Refused => f.write_str("The provider declined to translate this text."),
                other => write!(f, "{other}; you can try another one with `--provider`"),
            },
            CliError::SecretBlocked(findings) => {
                // Only kinds and byte positions: never the matched values.
                writeln!(f, "Secret Guard blocked the text; nothing was sent.")?;
                for finding in findings {
                    writeln!(f, "  - {finding}")?;
                }
                write!(
                    f,
                    "Use --redact to mask them, or --allow-secrets to send them anyway."
                )
            }
            CliError::SecretDeclined => {
                f.write_str("Secret Guard: not confirmed, so nothing was sent")
            }
            CliError::TooLarge { limit } => write!(
                f,
                "the input is longer than the limit of {limit} characters \
                 (raise it with --max-chars or `max_chars` in a config file)"
            ),
            CliError::NoGoogleKey => f.write_str(
                "No Google API key found. Run \"kun auth login google\" or set KUN_GOOGLE_API_KEY.",
            ),
            CliError::NoAnthropicKey => f.write_str(
                "No Anthropic API key found. Run \"kun auth login anthropic\" or set ANTHROPIC_API_KEY.",
            ),
            CliError::KeychainUnavailable { env_var } => write!(
                f,
                "no OS keychain is available here, so the key was not saved. \
                 Set {env_var} in the environment instead, or add it to the \
                 [credentials] table of the User Config (and run `chmod 600` on that file)"
            ),
            CliError::EndpointNotSupported { provider } => write!(
                f,
                "providers.{provider}.endpoint is not supported yet; remove it to use the public API."
            ),
            CliError::KeychainFailed => {
                f.write_str("the OS keychain could not read or write the credential")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kun_core::secret_guard::SecretGuard;

    #[test]
    fn exit_codes_follow_the_table() {
        let cases = [
            (CliError::Usage("x"), 2),
            (CliError::NoText, 2),
            (
                CliError::config(
                    ConfigFile::User,
                    &ConfigError::Parse {
                        path: "a.toml".into(),
                    },
                ),
                3,
            ),
            (
                CliError::ExtraPattern(SecretGuardError::InvalidPattern { index: 0 }),
                3,
            ),
            (CliError::Provider(ProviderError::MissingCredentials), 4),
            (CliError::Provider(ProviderError::Authentication), 5),
            (CliError::Provider(ProviderError::RateLimited), 5),
            (CliError::Provider(ProviderError::InvalidResponse), 5),
            (CliError::SecretBlocked(Vec::new()), 6),
            (CliError::SecretDeclined, 6),
            (CliError::TooLarge { limit: 1 }, 7),
            (CliError::Provider(ProviderError::Refused), 5),
            (CliError::NoGoogleKey, 4),
            (CliError::NoAnthropicKey, 4),
            (CliError::KeychainUnavailable { env_var: "X" }, 3),
            (CliError::EndpointNotSupported { provider: "google" }, 3),
            (CliError::KeychainFailed, 1),
            (CliError::Unexpected("x"), 1),
        ];
        for (error, code) in cases {
            assert_eq!(error.exit_code(), code, "{error:?}");
        }
    }

    #[test]
    fn a_refusal_is_exit_5_with_a_clear_message() {
        let error = CliError::from(TranslateError::Provider(ProviderError::Refused));
        assert_eq!(error.exit_code(), 5);
        assert_eq!(
            error.to_string(),
            "The provider declined to translate this text."
        );
    }

    #[test]
    fn translate_errors_map_to_cli_errors() {
        assert_eq!(CliError::from(TranslateError::NoTarget).exit_code(), 2);
        let from_provider =
            CliError::from(TranslateError::Provider(ProviderError::MissingCredentials));
        assert_eq!(from_provider.exit_code(), 4);
    }

    const FAKE: &str = "AKIAFAKEEXAMPLE12345";

    /// Every way to print an error: Display, `{:?}` and `{:#?}`.
    fn all_forms(error: &CliError) -> [String; 3] {
        [
            error.to_string(),
            format!("{error:?}"),
            format!("{error:#?}"),
        ]
    }

    fn hostile_config_errors() -> Vec<ConfigError> {
        let path = std::path::PathBuf::from(format!("/work/{FAKE}/.kun.toml"));
        vec![
            ConfigError::ForbiddenInProjectConfig {
                path: path.clone(),
                key: format!("secret_{FAKE}"),
            },
            ConfigError::ForbiddenInProjectConfig {
                path: path.clone(),
                key: format!("providers.{FAKE}.token"),
            },
            ConfigError::Invalid {
                path: path.clone(),
                key: FAKE.to_string(),
            },
            ConfigError::Parse { path: path.clone() },
            ConfigError::Io {
                path,
                source: std::io::Error::other(FAKE),
            },
        ]
    }

    #[test]
    fn config_errors_never_show_key_names_or_paths() {
        for config_error in hostile_config_errors() {
            for file in [ConfigFile::Project, ConfigFile::User] {
                let error = CliError::config(file, &config_error);
                assert_eq!(error.exit_code(), 3);
                for shown in all_forms(&error) {
                    assert!(!shown.contains(FAKE), "leaked in: {shown}");
                    assert!(!shown.contains("/work/"), "path leaked in: {shown}");
                }
            }
        }
    }

    #[test]
    fn config_errors_name_the_file_and_the_known_field() {
        let known = ConfigError::ForbiddenInProjectConfig {
            path: "x".into(),
            key: "secret_guard.allow_secrets".to_string(),
        };
        let error = CliError::config(ConfigFile::Project, &known);
        assert!(error.to_string().contains("Project Config"));
        assert!(error.to_string().contains("secret_guard.allow_secrets"));

        let unknown = ConfigError::ForbiddenInProjectConfig {
            path: "x".into(),
            key: "api_key".to_string(),
        };
        let error = CliError::config(ConfigFile::Project, &unknown);
        assert!(error.to_string().contains("unknown or credential-like key"));
        assert!(!error.to_string().contains("api_key"));

        let invalid = ConfigError::Invalid {
            path: "x".into(),
            key: "max_chars".to_string(),
        };
        let error = CliError::config(ConfigFile::User, &invalid);
        assert!(error.to_string().contains("User Config"));
        assert!(error.to_string().contains("max_chars"));
    }

    #[test]
    fn secret_blocked_never_shows_the_secret() {
        let key = "AKIAFAKEEXAMPLE12345";
        let findings = SecretGuard::new(&[]).unwrap().scan(&format!("key {key}"));
        assert!(!findings.is_empty(), "the fake key must be detected");
        let error = CliError::SecretBlocked(findings);

        for shown in [
            error.to_string(),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            assert!(!shown.contains(key), "leaked in: {shown}");
        }
        assert!(error.to_string().contains("AWS access key"));
        assert!(error.to_string().contains("bytes 4..24"));
    }
}
