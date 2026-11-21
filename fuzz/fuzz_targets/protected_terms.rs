//! Fuzz target: `protect` then `restore` must give back the original text.
//!
//! The input is split on `|`: the first piece is the text, every other piece
//! is a Protected Term. For example `hello kun|kun` is the text `hello kun`
//! with the term `kun`.

#![no_main]

use kun_core::protected_terms::{protect, restore};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let mut pieces = input.split('|');
    let text = pieces.next().unwrap_or_default();
    let terms: Vec<String> = pieces.map(str::to_string).collect();

    let (protected, placeholders) = protect(text, &terms);
    let (restored, all_found) = restore(&protected, &placeholders);

    // Known, accepted limit: text that already looks like a placeholder token
    // (`KUNPT0X`, in any letter case) may be replaced on restore. `restore`
    // matches case-insensitively with Unicode rules, so the KELVIN SIGN
    // (U+212A) counts as a `K` too: `to_lowercase` covers that, an ASCII-only
    // check would not.
    if text.to_lowercase().contains("kunpt") {
        return;
    }

    assert_eq!(restored, text);
    assert!(all_found);
});
