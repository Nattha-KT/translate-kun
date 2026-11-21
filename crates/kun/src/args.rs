//! Command-line arguments (requirements §3).
//!
//! `Cli` holds the user's text, so it deliberately has no `Debug`.

use clap::{Parser, Subcommand};
use kun_core::config::ProviderName;
use kun_core::language::LanguageCode;

const EXAMPLES: &str = "\
Examples:
  kun :th \"hello\"              translate to Thai (Source Language is detected)
  kun en:th \"hello\"            Source Language en, Target Language th
  kun -s en -t th \"hello\"      the same, long form for scripts
  echo \"hello\" | kun :th       read the text from stdin
  kun I love it                translates \"I love it\" (`it` is never a language)

A language is only recognised as the first argument in the form `:xx` or `xx:yy`.
Text that starts with `auth`, `init` or `completions` needs `--` first: kun :th -- auth is great
Set up: `kun init` (Project Config), `kun auth login google` (API key),
`kun completions <shell>` (shell completions).
Exit codes: 0 ok, 1 unexpected, 2 usage, 3 config, 4 missing credentials,
5 network or provider, 6 blocked by the Secret Guard, 7 input too large,
8 rate limited or quota exceeded (retrying later may help).";

/// A command-line translator for developers.
///
/// `kun auth ...`, `kun init` and `kun completions` are subcommands and win
/// over the text. To translate text that starts with one of those words, put
/// `--` before it.
#[derive(Parser)]
#[command(
    name = "kun",
    version = kun_core::version(),
    after_help = EXAMPLES,
    subcommand_precedence_over_arg = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Source Language, e.g. `en` (default: detected)
    #[arg(short, long, value_name = "LANG")]
    pub source: Option<LanguageCode>,

    /// Target Language, e.g. `th`
    #[arg(short, long, value_name = "LANG")]
    pub target: Option<LanguageCode>,

    /// Provider to use: google (default) or anthropic
    #[arg(long, value_name = "NAME")]
    pub provider: Option<String>,

    /// Print one JSON object on stdout (see docs/json-output.md)
    #[arg(long)]
    pub json: bool,

    /// Show exactly what would be sent (host, text after the Secret Guard), and send nothing
    #[arg(long)]
    pub dry_run: bool,

    /// Longest accepted input in characters (default 10000)
    #[arg(long, value_name = "N", value_parser = parse_max_chars)]
    pub max_chars: Option<usize>,

    /// Send the text even if the Secret Guard finds a possible secret
    #[arg(long)]
    pub allow_secrets: bool,

    /// Replace possible secrets with [REDACTED] and continue
    // The doc comment is also the `--help` text, so the brackets must stay.
    #[allow(rustdoc::broken_intra_doc_links)]
    #[arg(long)]
    pub redact: bool,

    /// Optional `:xx` or `xx:yy` first, then the text. Without text, stdin is read.
    #[arg(value_name = "[:xx | xx:yy] TEXT")]
    pub words: Vec<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Store, remove or check API keys
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Create a `.kun.toml` Project Config in the current directory
    Init {
        /// Overwrite an existing `.kun.toml`
        #[arg(long)]
        force: bool,
    },
    /// Print a shell completion script on stdout
    Completions {
        /// bash, zsh, fish, powershell or elvish
        #[arg(value_name = "SHELL")]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum AuthAction {
    /// Read an API key (hidden prompt, or one line from stdin) and store it in the OS keychain
    Login {
        #[arg(value_name = "PROVIDER", value_parser = parse_provider_name)]
        provider: ProviderName,
    },
    /// Delete the API key of a provider from the OS keychain
    Logout {
        #[arg(value_name = "PROVIDER", value_parser = parse_provider_name)]
        provider: ProviderName,
    },
    /// Show where each provider's key comes from (never the key itself)
    Status,
}

/// `google` or `anthropic`, for the `auth` subcommands.
fn parse_provider_name(value: &str) -> Result<ProviderName, String> {
    ProviderName::ALL
        .into_iter()
        .find(|candidate| crate::providers::provider_label(*candidate) == value)
        .ok_or_else(|| "use `google` or `anthropic`".to_string())
}

/// A fixed message for a command-line parsing error.
///
/// clap's own message repeats the offending value (`invalid value 'xyz' for
/// '--target'`), and a user may have pasted a secret there. This text is
/// built from the error kind and the option's name only, never from a value.
pub fn describe_parse_error(error: &clap::Error) -> String {
    use clap::error::{ContextKind, ContextValue, ErrorKind};

    let option_name = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(name)) => Some(name.as_str()),
        _ => None,
    };
    let problem = match (error.kind(), option_name) {
        // For these kinds the context holds the option as declared
        // (for example `--target <LANG>`), not what the user typed.
        (ErrorKind::InvalidValue | ErrorKind::ValueValidation, Some(name)) => {
            format!("invalid or missing value for `{name}`")
        }
        (ErrorKind::InvalidValue | ErrorKind::ValueValidation, None) => {
            "invalid or missing value for an option".to_string()
        }
        // The context would hold the unknown argument as typed: leave it out.
        (ErrorKind::UnknownArgument, _) => {
            "unknown option (to translate text that starts with `-`, put `--` before it)"
                .to_string()
        }
        (ErrorKind::InvalidUtf8, _) => "arguments must be valid UTF-8".to_string(),
        _ => "invalid arguments".to_string(),
    };
    format!("{problem}\n\nFor more information, try 'kun --help'.")
}

fn parse_max_chars(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(number) if number >= 1 => Ok(number),
        _ => Err("must be a whole number of at least 1".to_string()),
    }
}

/// The languages given as the first argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSpec {
    pub source: Option<LanguageCode>,
    pub target: LanguageCode,
}

/// The first argument as a language spec, if it is one.
///
/// Only `:xx` and `xx:yy` count, where both parts are valid language codes.
/// Anything else (including `to: me`, which has a space) is text.
pub fn parse_language_spec(argument: &str) -> Option<LanguageSpec> {
    if argument.chars().any(char::is_whitespace) {
        return None;
    }
    let (source, target) = argument.split_once(':')?;
    let target: LanguageCode = target.parse().ok()?;
    if source.is_empty() {
        return Some(LanguageSpec {
            source: None,
            target,
        });
    }
    let source: LanguageCode = source.parse().ok()?;
    Some(LanguageSpec {
        source: Some(source),
        target,
    })
}

/// The positional arguments, split into the language spec and the text.
pub struct Positionals {
    pub spec: Option<LanguageSpec>,
    /// The remaining arguments joined with single spaces. `None` = no text given.
    pub text: Option<String>,
}

/// Splits the positional arguments: an optional language spec, then the text.
pub fn split_positionals(words: &[String]) -> Positionals {
    let spec = words.first().and_then(|first| parse_language_spec(first));
    let text_words = if spec.is_some() { &words[1..] } else { words };
    let text = if text_words.is_empty() {
        None
    } else {
        Some(text_words.join(" "))
    };
    Positionals { spec, text }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn colon_prefix_is_a_target() {
        let spec = parse_language_spec(":th").unwrap();
        assert_eq!(spec.source, None);
        assert_eq!(spec.target, lang("th"));
    }

    #[test]
    fn source_colon_target_gives_both() {
        let spec = parse_language_spec("en:th").unwrap();
        assert_eq!(spec.source, Some(lang("en")));
        assert_eq!(spec.target, lang("th"));
    }

    #[test]
    fn regional_codes_and_case_are_normalised() {
        let spec = parse_language_spec("zh-tw:EN").unwrap();
        assert_eq!(spec.source, Some(lang("zh-TW")));
        assert_eq!(spec.target, lang("en"));
    }

    #[test]
    fn things_that_are_not_specs() {
        for text in [
            "it", "I", "hello", "th:", ":", "::th", "en:th:ja", ":english", "1:2", "http://x",
            "to: me", ": th", "note:", ":th ",
        ] {
            assert_eq!(parse_language_spec(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_bare_word_is_never_a_language() {
        let parsed = split_positionals(&words(&["I", "love", "it"]));
        assert_eq!(parsed.spec, None);
        assert_eq!(parsed.text.as_deref(), Some("I love it"));
    }

    #[test]
    fn spec_is_taken_from_the_first_argument_only() {
        let parsed = split_positionals(&words(&[":th", "hello", "world"]));
        assert_eq!(parsed.spec.unwrap().target, lang("th"));
        assert_eq!(parsed.text.as_deref(), Some("hello world"));

        let later = split_positionals(&words(&["hello", ":th"]));
        assert_eq!(later.spec, None);
        assert_eq!(later.text.as_deref(), Some("hello :th"));
    }

    #[test]
    fn spec_without_text_leaves_the_text_empty() {
        let parsed = split_positionals(&words(&["en:th"]));
        assert!(parsed.spec.is_some());
        assert_eq!(parsed.text, None);
        assert_eq!(split_positionals(&[]).text, None);
    }

    #[test]
    fn parse_error_descriptions_never_repeat_typed_values() {
        use clap::Parser;

        let fake = "AKIAFAKEEXAMPLE12345";
        let cases: Vec<Vec<String>> = vec![
            vec!["kun".into(), "-t".into(), fake.into()],
            vec!["kun".into(), "-s".into(), fake.into()],
            vec!["kun".into(), format!("--max-chars={fake}")],
            vec!["kun".into(), format!("--{fake}")],
            vec!["kun".into(), format!("-{fake}")],
        ];
        for args in cases {
            let error = Cli::try_parse_from(&args)
                .err()
                .expect("these arguments must be rejected");
            let message = describe_parse_error(&error);
            assert!(!message.contains(fake), "leaked: {message}");
            assert!(message.contains("kun --help"));
        }
    }

    #[test]
    fn parse_error_description_names_the_option() {
        use clap::Parser;

        let error = Cli::try_parse_from(["kun", "--max-chars", "x"])
            .err()
            .unwrap();
        assert!(describe_parse_error(&error).contains("--max-chars"));
    }

    #[test]
    fn max_chars_must_be_a_positive_number() {
        assert_eq!(parse_max_chars("5"), Ok(5));
        assert!(parse_max_chars("0").is_err());
        assert!(parse_max_chars("-1").is_err());
        assert!(parse_max_chars("many").is_err());
    }
}
