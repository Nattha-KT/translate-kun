//! Command-line arguments (requirements §3).
//!
//! `Cli` holds the user's text, so it deliberately has no `Debug`.

use clap::Parser;
use kun_core::language::LanguageCode;

const EXAMPLES: &str = "\
Examples:
  kun :th \"hello\"              translate to Thai (Source Language is detected)
  kun en:th \"hello\"            Source Language en, Target Language th
  kun -s en -t th \"hello\"      the same, long form for scripts
  echo \"hello\" | kun :th       read the text from stdin
  kun I love it                translates \"I love it\" (`it` is never a language)

A language is only recognised as the first argument in the form `:xx` or `xx:yy`.
Exit codes: 0 ok, 1 unexpected, 2 usage, 3 config, 4 missing credentials,
5 network or provider, 6 blocked by the Secret Guard, 7 input too large.";

/// A command-line translator for developers.
#[derive(Parser)]
#[command(name = "kun", version = kun_core::version(), after_help = EXAMPLES)]
pub struct Cli {
    /// Source Language, e.g. `en` (default: detected)
    #[arg(short, long, value_name = "LANG")]
    pub source: Option<LanguageCode>,

    /// Target Language, e.g. `th`
    #[arg(short, long, value_name = "LANG")]
    pub target: Option<LanguageCode>,

    /// Provider to use: google (default) or anthropic
    #[arg(long, value_name = "NAME")]
    pub provider: Option<String>,

    /// Structured output (not implemented yet)
    #[arg(long)]
    pub json: bool,

    /// Longest accepted input in characters (default 10000)
    #[arg(long, value_name = "N", value_parser = parse_max_chars)]
    pub max_chars: Option<usize>,

    /// Send the text even if the Secret Guard finds a possible secret
    #[arg(long)]
    pub allow_secrets: bool,

    /// Replace possible secrets with [REDACTED] and continue
    #[arg(long)]
    pub redact: bool,

    /// Optional `:xx` or `xx:yy` first, then the text. Without text, stdin is read.
    #[arg(value_name = "[:xx | xx:yy] TEXT")]
    pub words: Vec<String>,
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
        items.iter().map(|s| s.to_string()).collect()
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
    fn max_chars_must_be_a_positive_number() {
        assert_eq!(parse_max_chars("5"), Ok(5));
        assert!(parse_max_chars("0").is_err());
        assert!(parse_max_chars("-1").is_err());
        assert!(parse_max_chars("many").is_err());
    }
}
