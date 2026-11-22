//! Property-based tests (proptest) for the Secret Guard and Protected Terms.
//!
//! A normal test checks one example. A property test checks a rule against
//! many random inputs, and shrinks a failing input to a small one.
//!
//! The case count is kept small so that `cargo test` stays fast. To run more
//! cases locally: `PROPTEST_CASES=2000 cargo test -p translate-kun-core --test properties`.
//!
//! Every secret-looking fixture is built from parts and contains `EXAMPLE` or
//! `FAKE`, like the other tests.

use std::sync::LazyLock;

use kun_core::protected_terms::{protect, restore};
use kun_core::secret_guard::{SecretGuard, redact};
use proptest::prelude::*;

/// The built-in patterns, compiled once for all cases.
static GUARD: LazyLock<SecretGuard> =
    LazyLock::new(|| SecretGuard::new(&[]).expect("built-in patterns compile"));

/// One fake sample of each kind of secret.
fn fake_keys() -> Vec<String> {
    let dashes = "-----";
    vec![
        format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE"),
        format!("{}{}", "ghp_", "FAKEexample0123456789abcdefghijklmno"),
        format!("{}{}", "sk-ant-api03-", "FAKE_EXAMPLE-0123456789abcdefghij"),
        format!(
            "{}{}",
            "sk-", "FAKEEXAMPLE0123456789abcdefghijklmnopqrstuvwxyzAB"
        ),
        format!("{}{}", "AIza", "SyFAKE-EXAMPLE_0123456789abcdefghij"),
        format!("{}{}", "xoxb-", "0000000000-FAKE-EXAMPLE"),
        format!("{}{}", "sk_live_", "FAKEEXAMPLE0123456789abcd"),
        format!("{}{}", "glpat-", "FAKE-EXAMPLE_0123456"),
        format!("{}{}", "npm_", "FAKEEXAMPLE0123456789abcdefghijklmno"),
        format!("{}{}", "hf_", "FAKEEXAMPLE0123456789abcdefghijklm"),
        format!(
            "{}.{}.{}",
            "eyJhbGciOiJIUzI1NiJ9", "e30", "FAKE_EXAMPLE-signature"
        ),
        format!(
            "{dashes}BEGIN RSA PRIVATE KEY{dashes}\nFAKEEXAMPLEFAKEEXAMPLE\nNOTAREALKEY==\n{dashes}END RSA PRIVATE KEY{dashes}"
        ),
    ]
}

/// A piece of text that is often near the edge of a pattern: random text, or
/// a prefix, a marker or a line break the Secret Guard looks for.
fn fragment() -> impl Strategy<Value = String> {
    let special = prop::sample::select(vec![
        "-----BEGIN PRIVATE KEY-----",
        "-----END PRIVATE KEY-----",
        "Proc-Type: 4,ENCRYPTED",
        "QUJDREVGRw==",
        "\\n",
        "\n",
        "\r\n",
        "AKIA",
        "ghp_",
        "eyJ",
        "sk-",
        "aws_secret_access_key=",
        "กุญแจ",
    ])
    .prop_map(String::from);
    prop_oneof![any::<String>(), special]
}

/// Text built from many fragments.
fn tricky_text() -> impl Strategy<Value = String> {
    prop::collection::vec(fragment(), 0..12).prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// A fake key put anywhere in arbitrary text never survives redaction.
    /// (The key sits on its own line: a key glued to letters is, by design,
    /// not a key, because the prefix must follow a separator.)
    #[test]
    fn a_fake_key_never_survives_redaction(
        before in any::<String>(),
        after in any::<String>(),
        which in 0usize..12,
    ) {
        let keys = fake_keys();
        let key = &keys[which % keys.len()];
        let text = format!("{before}\n{key}\n{after}");

        let findings = GUARD.scan(&text);
        let redacted = redact(&text, &findings);

        prop_assert!(!redacted.contains(key.as_str()), "key survived in {redacted:?}");
    }

    /// Scanning and redacting never panic, every finding is a valid slice,
    /// and the result is valid UTF-8.
    #[test]
    fn scan_and_redact_never_panic_and_stay_valid_utf8(text in tricky_text()) {
        let findings = GUARD.scan(&text);
        for finding in &findings {
            prop_assert!(finding.range.start <= finding.range.end);
            prop_assert!(finding.range.end <= text.len());
            prop_assert!(text.is_char_boundary(finding.range.start));
            prop_assert!(text.is_char_boundary(finding.range.end));
        }

        let redacted = redact(&text, &findings);

        prop_assert!(std::str::from_utf8(redacted.as_bytes()).is_ok());
    }

    /// Protect then restore gives back the original text, for terms that
    /// really occur in it: the text is built by putting every term between
    /// filler pieces. So `protect` must replace them (an identity `protect`
    /// fails the checks before the round trip).
    #[test]
    fn protect_then_restore_round_trips_with_matching_terms(
        terms in prop::collection::vec("[abcก-]{1,3}", 1..4),
        fillers in prop::collection::vec("[abc กข-]{0,6}", 4..=4),
    ) {
        let mut text = fillers[0].clone();
        for (term, filler) in terms.iter().zip(fillers.iter().skip(1)) {
            text.push_str(term);
            text.push_str(filler);
        }

        let (protected, placeholders) = protect(&text, &terms);

        // The terms are gone and a placeholder took their place. (A term
        // never contains the capital letters of a placeholder.)
        for term in &terms {
            prop_assert!(!protected.contains(term.as_str()), "term left in {protected:?}");
        }
        prop_assert!(protected.contains("KUNPT0X"), "no placeholder in {protected:?}");

        let (restored, all_found) = restore(&protected, &placeholders);

        prop_assert_eq!(restored, text);
        prop_assert!(all_found);
    }

    /// The same, for arbitrary text and terms. Inputs that already contain
    /// the `KUNPT` placeholder shape are skipped: that collision is an
    /// accepted, documented limit (audit nit N-6).
    #[test]
    fn protect_then_restore_round_trips_for_arbitrary_text(
        text in any::<String>(),
        terms in prop::collection::vec(any::<String>(), 0..4),
    ) {
        let has_placeholder_shape = |s: &String| s.to_lowercase().contains("kunpt");
        prop_assume!(!has_placeholder_shape(&text));
        prop_assume!(!terms.iter().any(has_placeholder_shape));

        let (protected, placeholders) = protect(&text, &terms);
        let (restored, all_found) = restore(&protected, &placeholders);

        prop_assert_eq!(restored, text);
        prop_assert!(all_found);
    }
}
