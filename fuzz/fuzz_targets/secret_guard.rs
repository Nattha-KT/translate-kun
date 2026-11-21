//! Fuzz target: Secret Guard `scan`, then `redact`, on arbitrary UTF-8.
//!
//! Checks that nothing panics and that, after redaction, the text of a
//! finding is gone from the output.
//!
//! The seed corpus uses the marker `<SPLIT>` to cut fake secrets into pieces,
//! so that no file in the repository contains a complete secret-looking string.
//! The marker is removed before scanning.

#![no_main]

use std::sync::OnceLock;

use kun_core::secret_guard::{SecretGuard, redact};
use libfuzzer_sys::fuzz_target;

/// The seed corpus joins its pieces by deleting this marker.
const SPLIT_MARKER: &str = "<SPLIT>";

fn guard() -> &'static SecretGuard {
    // Building the guard compiles every regex, so do it once, not per input.
    static GUARD: OnceLock<SecretGuard> = OnceLock::new();
    GUARD.get_or_init(|| SecretGuard::new(&[]).expect("built-in patterns are valid"))
}

fuzz_target!(|data: &[u8]| {
    let Ok(raw) = std::str::from_utf8(data) else {
        return;
    };
    let text = raw.replace(SPLIT_MARKER, "");

    let findings = guard().scan(&text);
    let redacted = redact(&text, &findings);

    for finding in &findings {
        // A range must be valid for slicing (in bounds, on char boundaries).
        let secret = text
            .get(finding.range.clone())
            .expect("finding range is a valid slice of the scanned text");
        assert!(!secret.is_empty());

        // If the same text also appears elsewhere without being a finding,
        // seeing it in the output is correct, so only check texts that occur
        // exactly once in the input.
        if text.matches(secret).count() == 1 {
            assert!(
                !redacted.contains(secret),
                "redacted output still contains a finding"
            );
        }
    }
});
