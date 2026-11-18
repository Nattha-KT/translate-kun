//! Tests that do not need the fake Provider.

mod common;

use common::Sandbox;
use predicates::prelude::*;

#[test]
fn version_flag_prints_name_and_version() {
    Sandbox::new()
        .kun()
        .arg("--version")
        .assert()
        .success()
        .stdout("kun 0.1.0\n");
}

#[test]
fn no_args_prints_help_to_stderr_and_exits_2() {
    Sandbox::new()
        .kun()
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage"))
        .stdout("");
}

#[test]
fn help_lists_the_options_and_exit_codes() {
    Sandbox::new()
        .kun()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--max-chars"))
        .stdout(predicate::str::contains("--allow-secrets"))
        .stdout(predicate::str::contains("--redact"))
        .stdout(predicate::str::contains("--provider"))
        .stdout(predicate::str::contains("7 input too large"));
}

#[test]
fn json_is_reserved_and_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--json", ":th", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not implemented yet"));
}

#[test]
fn unknown_provider_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--provider", "deepl", ":th", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown provider"));
}

#[test]
fn default_provider_google_is_not_wired_yet_and_exits_1() {
    Sandbox::new()
        .kun()
        .args([":th", "hello"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("google"))
        .stderr(predicate::str::contains("not wired up yet"));
}

#[test]
fn anthropic_is_not_wired_yet_and_exits_1() {
    Sandbox::new()
        .kun()
        .args(["--provider", "anthropic", ":th", "hello"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("anthropic"));
}

#[test]
fn invalid_language_flag_exits_2() {
    Sandbox::new()
        .kun()
        .args(["-t", "english", "hello"])
        .assert()
        .code(2);
}

#[test]
fn zero_max_chars_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--max-chars", "0", ":th", "hello"])
        .assert()
        .code(2);
}

#[test]
fn language_given_twice_exits_2() {
    Sandbox::new()
        .kun()
        .args(["-t", "fr", ":th", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not both"));
}

#[cfg(not(feature = "fake-provider"))]
#[test]
fn the_fake_provider_is_not_available_by_default() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown provider"));
}
