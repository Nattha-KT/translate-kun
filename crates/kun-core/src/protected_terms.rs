//! Protected Terms: words that must come back exactly as written.
//!
//! The same mechanism works for every Provider. Before a Provider call,
//! [`protect`] swaps each Protected Term for an opaque placeholder token.
//! After the call, [`restore`] swaps the original terms back in.
//!
//! The token looks like `KUNPT0X`, `KUNPT1X`, ... (`KUNPT`, the term's number,
//! and a closing `X`). It was chosen because:
//! - it is plain ASCII letters and digits, so Thai and English translators
//!   treat it as an unknown word (a name) and leave it alone;
//! - it has no spaces, quotes, brackets or punctuation that a translator
//!   might "fix" or re-space;
//! - the closing `X` ends the number, so `KUNPT1X` is never a prefix of
//!   `KUNPT10X`;
//! - it is very unlikely to appear in real text. (If the input does contain
//!   something shaped like a token, `restore` may replace it. That is an
//!   accepted, tiny risk.)

use std::fmt;

use regex::Regex;

/// Starts every placeholder token.
const TOKEN_PREFIX: &str = "KUNPT";
/// Ends every placeholder token.
const TOKEN_SUFFIX: &str = "X";

/// What [`protect`] replaced, so [`restore`] can undo it.
///
/// `Debug` hides the terms: they are config strings and may hold secrets.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Placeholders {
    /// The distinct terms that were used; the index is the token number.
    terms: Vec<String>,
    /// How many times each token was inserted (same index as `terms`).
    inserted: Vec<usize>,
}

impl fmt::Debug for Placeholders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Placeholders")
            .field("terms", &"<redacted>")
            .field("count", &self.terms.len())
            .finish()
    }
}

fn token(index: usize) -> String {
    format!("{TOKEN_PREFIX}{index}{TOKEN_SUFFIX}")
}

/// Replaces every occurrence of every term in `text` with a placeholder token.
///
/// Matching is case-sensitive and exact. At each position the longest term
/// wins, so `translate-kun` is never cut up by the shorter term `kun`. Empty
/// terms are ignored. An empty term list returns the text unchanged.
pub fn protect(text: &str, terms: &[String]) -> (String, Placeholders) {
    // Longest first, so the first match at a position is the longest one.
    let mut candidates: Vec<&String> = terms.iter().filter(|term| !term.is_empty()).collect();
    candidates.sort_by_key(|term| std::cmp::Reverse(term.len()));
    candidates.dedup();

    let mut placeholders = Placeholders::default();
    if candidates.is_empty() {
        return (text.to_string(), placeholders);
    }

    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let found = candidates
            .iter()
            .find(|term| rest.starts_with(term.as_str()));
        if let Some(term) = found {
            let index = match placeholders.terms.iter().position(|t| t == *term) {
                Some(index) => index,
                None => {
                    placeholders.terms.push((*term).clone());
                    placeholders.inserted.push(0);
                    placeholders.terms.len() - 1
                }
            };
            placeholders.inserted[index] += 1;
            output.push_str(&token(index));
            rest = &rest[term.len()..];
        } else {
            // Copy one character (not one byte: Thai is multi-byte).
            let width = rest.chars().next().map_or(1, char::len_utf8);
            output.push_str(&rest[..width]);
            rest = &rest[width..];
        }
    }
    (output, placeholders)
}

/// Puts the original terms back in place of the placeholder tokens.
///
/// The `bool` is `true` when every inserted token was found in `translated`.
/// A token the Provider dropped or mangled only makes it `false`: the
/// translation is still returned, never an error.
pub fn restore(translated: &str, placeholders: &Placeholders) -> (String, bool) {
    if placeholders.terms.is_empty() {
        return (translated.to_string(), true);
    }

    // Tokens are matched case-insensitively in case a translator changed case.
    let pattern = format!("(?i){TOKEN_PREFIX}([0-9]+){TOKEN_SUFFIX}");
    let Ok(regex) = Regex::new(&pattern) else {
        // The pattern is a constant, so this cannot happen.
        return (translated.to_string(), false);
    };

    let mut found = vec![0usize; placeholders.terms.len()];
    let restored = regex.replace_all(translated, |caps: &regex::Captures| {
        let index = caps[1].parse::<usize>().ok();
        match index.and_then(|i| placeholders.terms.get(i).map(|term| (i, term))) {
            Some((i, term)) => {
                found[i] += 1;
                term.clone()
            }
            // Not one of ours: leave it as it is.
            None => caps[0].to_string(),
        }
    });

    let all_found = found
        .iter()
        .zip(&placeholders.inserted)
        .all(|(found, inserted)| found >= inserted);
    (restored.into_owned(), all_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn empty_term_list_is_a_no_op() {
        let (text, placeholders) = protect("hello translate-kun", &[]);
        assert_eq!(text, "hello translate-kun");
        assert_eq!(restore(&text, &placeholders), (text.clone(), true));
    }

    #[test]
    fn empty_terms_are_ignored() {
        let (text, _) = protect("abc", &terms(&[""]));
        assert_eq!(text, "abc");
    }

    #[test]
    fn terms_are_replaced_by_tokens_and_restored() {
        let list = terms(&["translate-kun", "Orca"]);
        let (text, placeholders) = protect("Use translate-kun with Orca.", &list);
        assert_eq!(text, "Use KUNPT0X with KUNPT1X.");
        assert!(!text.contains("Orca"));
        let (restored, all_found) = restore(&text, &placeholders);
        assert_eq!(restored, "Use translate-kun with Orca.");
        assert!(all_found);
    }

    #[test]
    fn the_longest_term_wins_when_terms_overlap() {
        let list = terms(&["kun", "translate-kun"]);
        let (text, placeholders) = protect("translate-kun and kun", &list);
        // `translate-kun` is one token; the bare `kun` is another.
        assert_eq!(text, "KUNPT0X and KUNPT1X");
        assert_eq!(
            restore(&text, &placeholders),
            ("translate-kun and kun".to_string(), true)
        );
    }

    #[test]
    fn matching_is_case_sensitive() {
        let (text, _) = protect("orca Orca ORCA", &terms(&["Orca"]));
        assert_eq!(text, "orca KUNPT0X ORCA");
    }

    #[test]
    fn repeated_and_duplicate_terms_share_a_token() {
        let (text, placeholders) = protect("Orca Orca", &terms(&["Orca", "Orca"]));
        assert_eq!(text, "KUNPT0X KUNPT0X");
        assert_eq!(
            restore(&text, &placeholders),
            ("Orca Orca".to_string(), true)
        );
    }

    #[test]
    fn thai_text_around_terms_is_kept() {
        let list = terms(&["translate-kun"]);
        let (text, placeholders) = protect("ใช้translate-kunแปลภาษา", &list);
        assert_eq!(text, "ใช้KUNPT0Xแปลภาษา");
        assert_eq!(
            restore(&text, &placeholders),
            ("ใช้translate-kunแปลภาษา".to_string(), true)
        );
    }

    #[test]
    fn tokens_survive_a_translation_that_moves_them() {
        let (text, placeholders) = protect("Orca is fast", &terms(&["Orca"]));
        let translated = format!("{} เร็วมาก", text.split(' ').next().unwrap());
        let (restored, all_found) = restore(&translated, &placeholders);
        assert_eq!(restored, "Orca เร็วมาก");
        assert!(all_found);
    }

    #[test]
    fn tokens_with_two_digits_do_not_clash_with_one_digit() {
        let list: Vec<String> = (0..12).map(|i| format!("term{i:02}")).collect();
        let input = list.join(" ");
        let (text, placeholders) = protect(&input, &list);
        let (restored, all_found) = restore(&text, &placeholders);
        assert_eq!(restored, input);
        assert!(all_found);
    }

    #[test]
    fn a_missing_token_is_reported_not_fatal() {
        let (_, placeholders) =
            protect("Orca and translate-kun", &terms(&["Orca", "translate-kun"]));
        let (restored, all_found) = restore("KUNPT0X only", &placeholders);
        assert_eq!(restored, "Orca only");
        assert!(!all_found);
    }

    #[test]
    fn a_term_used_twice_but_returned_once_is_reported() {
        let (_, placeholders) = protect("Orca Orca", &terms(&["Orca"]));
        let (_, all_found) = restore("KUNPT0X", &placeholders);
        assert!(!all_found);
    }

    #[test]
    fn a_lowercased_token_is_still_restored() {
        let (_, placeholders) = protect("Orca", &terms(&["Orca"]));
        assert_eq!(
            restore("kunpt0x", &placeholders),
            ("Orca".to_string(), true)
        );
    }

    #[test]
    fn an_unknown_token_is_left_alone() {
        let (_, placeholders) = protect("Orca", &terms(&["Orca"]));
        let (restored, all_found) = restore("KUNPT0X KUNPT9X", &placeholders);
        assert_eq!(restored, "Orca KUNPT9X");
        assert!(all_found);
    }

    #[test]
    fn debug_never_shows_the_terms() {
        let secret = "FAKE-SECRET-TERM-EXAMPLE";
        let (_, placeholders) = protect(secret, &terms(&[secret]));
        for shown in [format!("{placeholders:?}"), format!("{placeholders:#?}")] {
            assert!(!shown.contains(secret), "{shown}");
            assert!(shown.contains("<redacted>"));
        }
    }
}
