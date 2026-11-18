//! Every way `kun` can fail, and the exit code each one maps to (requirements §9).
//!
//! No variant holds user text, so printing or `{:?}`-formatting an error can
//! never leak the input, a config value or a credential.

use std::fmt;

use kun_core::config::ConfigError;
use kun_core::provider::ProviderError;
use kun_core::secret_guard::{Finding, SecretGuardError};
use kun_core::translate::TranslateError;

#[derive(Debug)]
pub enum CliError {
    /// Bad arguments or input. The message is fixed text written in `kun`.
    Usage(&'static str),
    /// There is no text to translate. `main` prints the help.
    NoText,
    /// A config file could not be used.
    Config(ConfigError),
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
    /// The Provider exists but is not connected to the CLI yet.
    ProviderNotWired(&'static str),
    /// Something unexpected, such as a failed write to stdout.
    Unexpected(&'static str),
}

impl CliError {
    /// The process exit code. The only place where errors become codes.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Usage(_) | CliError::NoText => 2,
            CliError::Config(_) | CliError::ExtraPattern(_) => 3,
            CliError::Provider(ProviderError::MissingCredentials) => 4,
            CliError::Provider(_) => 5,
            CliError::SecretBlocked(_) | CliError::SecretDeclined => 6,
            CliError::TooLarge { .. } => 7,
            CliError::ProviderNotWired(_) | CliError::Unexpected(_) => 1,
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
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(message) | CliError::Unexpected(message) => f.write_str(message),
            CliError::NoText => f.write_str("no text to translate"),
            CliError::Config(error) => {
                write!(f, "{error}")?;
                // Walk the sources (for example the OS reason for a failed read).
                // Config errors never contain config values.
                let mut source = std::error::Error::source(error);
                while let Some(cause) = source {
                    write!(f, ": {cause}")?;
                    source = cause.source();
                }
                Ok(())
            }
            CliError::ExtraPattern(error) => write!(f, "{error}"),
            CliError::Provider(error) => match error {
                ProviderError::MissingCredentials => f.write_str(
                    "no credentials found for this provider (run `kun auth login <provider>`)",
                ),
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
            CliError::ProviderNotWired(name) => {
                write!(f, "provider `{name}` is not wired up yet in this build")
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
                CliError::Config(ConfigError::Parse {
                    path: "a.toml".into(),
                }),
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
            (CliError::ProviderNotWired("google"), 1),
            (CliError::Unexpected("x"), 1),
        ];
        for (error, code) in cases {
            assert_eq!(error.exit_code(), code, "{error:?}");
        }
    }

    #[test]
    fn translate_errors_map_to_cli_errors() {
        assert_eq!(CliError::from(TranslateError::NoTarget).exit_code(), 2);
        let from_provider =
            CliError::from(TranslateError::Provider(ProviderError::MissingCredentials));
        assert_eq!(from_provider.exit_code(), 4);
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
