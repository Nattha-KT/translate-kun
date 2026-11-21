//! Language codes used for Source Language and Target Language.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// Longest accepted input, e.g. `zh-TW` is 5 characters.
const MAX_LEN: usize = 8;

/// Why a string was rejected as a [`LanguageCode`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LanguageCodeError {
    /// The input was an empty string.
    #[error("language code is empty")]
    Empty,
    /// The input was longer than any valid code.
    #[error("language code is too long")]
    TooLong,
    /// The input is not `xx` or `xx-YY` made of letters.
    #[error("invalid language code: expected `xx` or `xx-YY` (letters only)")]
    Invalid,
}

/// A validated language code such as `en`, `th` or `zh-TW`.
///
/// The language part is stored in lowercase and the region part in uppercase,
/// so `ZH-tw` and `zh-TW` are the same code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageCode(String);

impl LanguageCode {
    /// The normalised code, e.g. `"pt-BR"`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_letters(part: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&part.len()) && part.chars().all(|c| c.is_ascii_alphabetic())
}

impl FromStr for LanguageCode {
    type Err = LanguageCodeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // Check the raw length first, so padded or huge input is rejected
        // without being processed further.
        if input.chars().count() > MAX_LEN {
            return Err(LanguageCodeError::TooLong);
        }
        let input = input.trim();
        if input.is_empty() {
            return Err(LanguageCodeError::Empty);
        }

        let mut parts = input.split('-');
        let language = parts.next().unwrap_or_default();
        let region = parts.next();
        if parts.next().is_some() || !is_letters(language, 2, 3) {
            return Err(LanguageCodeError::Invalid);
        }

        let mut code = language.to_ascii_lowercase();
        if let Some(region) = region {
            if !is_letters(region, 2, 2) {
                return Err(LanguageCodeError::Invalid);
            }
            code.push('-');
            code.push_str(&region.to_ascii_uppercase());
        }
        Ok(LanguageCode(code))
    }
}

impl fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<LanguageCode, LanguageCodeError> {
        s.parse()
    }

    #[test]
    fn accepts_plain_and_regional_codes() {
        for code in ["en", "th", "ja", "zh-TW", "pt-BR"] {
            assert_eq!(parse(code).unwrap().to_string(), code);
        }
    }

    #[test]
    fn normalises_case() {
        assert_eq!(parse("EN").unwrap().as_str(), "en");
        assert_eq!(parse("zh-tw").unwrap().as_str(), "zh-TW");
        assert_eq!(parse("PT-br").unwrap().as_str(), "pt-BR");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(parse(""), Err(LanguageCodeError::Empty));
        assert_eq!(parse("   "), Err(LanguageCodeError::Empty));
    }

    #[test]
    fn rejects_digits_and_symbols() {
        for bad in ["1", "e1", "12", "en-1", "en-12", "en_US", "e n", "ไทย"] {
            assert_eq!(parse(bad), Err(LanguageCodeError::Invalid), "{bad}");
        }
    }

    #[test]
    fn rejects_bad_shapes() {
        for bad in ["e", "english", "en-", "-en", "en-US-x", "en-USA"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_overlong_input() {
        assert_eq!(parse("abcdefghijkl"), Err(LanguageCodeError::TooLong));
    }

    #[test]
    fn rejects_padded_overlong_input() {
        // Trims to "en", but the raw input is far longer than any real code.
        let padded = format!("{}en{}", " ".repeat(1000), " ".repeat(1000));
        assert_eq!(parse(&padded), Err(LanguageCodeError::TooLong));
    }

    #[test]
    fn still_accepts_short_surrounding_whitespace() {
        assert_eq!(parse(" en ").unwrap().as_str(), "en");
    }
}
