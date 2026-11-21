//! End-to-end tests for `--dry-run` (S13a). No network, no keychain: the
//! fake Provider is used, and a dry run never builds a Provider anyway.
//! Run with: `cargo test --workspace --features translate-kun/fake-provider`

#![cfg(feature = "fake-provider")]

mod common;

use common::Sandbox;
use predicates::prelude::*;

/// Obviously fake, but shaped like an AWS access key, so the detector fires.
const FAKE_KEY: &str = "AKIAFAKEEXAMPLE12345";

const BEGIN: &str = "----- begin text that would be sent -----";
const END: &str = "----- end text that would be sent -----";

#[test]
fn a_plain_dry_run_reports_the_request_and_sends_nothing() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "google", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run: nothing was sent."))
        .stdout(predicate::str::contains("provider: google"))
        .stdout(predicate::str::contains("host: translation.googleapis.com"))
        .stdout(predicate::str::contains("source language: detect"))
        .stdout(predicate::str::contains("target language: th"))
        .stdout(predicate::str::contains("second call: no"))
        .stdout(predicate::str::contains(format!("{BEGIN}\nhello\n{END}\n")))
        // Google has no model.
        .stdout(predicate::str::contains("model:").not());
}

#[test]
fn a_dry_run_with_no_credentials_still_succeeds_and_says_none() {
    // The Sandbox has no key in the environment, the keychain or the config.
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "google", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("credentials: none"));
}

#[test]
fn the_credential_source_is_named_but_the_key_never_shown() {
    let fake_env_key = "AIzaFAKE-EXAMPLE-KEY-0123456789";
    Sandbox::new()
        .kun()
        .env("KUN_GOOGLE_API_KEY", fake_env_key)
        .args(["--dry-run", "--provider", "google", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("credentials: env"))
        .stdout(predicate::str::contains(fake_env_key).not())
        .stderr(predicate::str::contains(fake_env_key).not());
}

#[test]
fn anthropic_shows_its_host_and_model() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "anthropic", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("host: api.anthropic.com"))
        .stdout(predicate::str::contains("model: claude-haiku-5-5"));
}

#[test]
fn the_model_from_the_config_is_shown() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("[providers.anthropic]\nmodel = \"claude-FAKE-model\"");
    sandbox
        .kun()
        .args(["--dry-run", "--provider", "anthropic", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("model: claude-FAKE-model"));
}

#[test]
fn a_secret_with_redact_shows_the_marker_and_never_the_key() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--redact", "--provider", "google", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("my key is [REDACTED]"))
        .stdout(predicate::str::contains(
            "possible AWS access key at bytes 10..30",
        ))
        .stdout(predicate::str::contains("Secret Guard: redact"))
        .stdout(predicate::str::contains(FAKE_KEY).not())
        .stderr(predicate::str::contains(FAKE_KEY).not())
        // A dry run sends nothing, so the warning is in the conditional.
        .stderr(predicate::str::contains(
            "would be replaced with [REDACTED]",
        ))
        .stderr(predicate::str::contains("before sending").not());
}

#[test]
fn a_secret_from_a_pipe_without_redact_exits_6_like_a_real_run() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "google", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .code(6)
        .stdout("")
        .stderr(predicate::str::contains("AWS access key"))
        .stderr(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn allow_secrets_reports_send_and_shows_the_text_as_it_would_be_sent() {
    Sandbox::new()
        .kun()
        .args([
            "--dry-run",
            "--allow-secrets",
            "--provider",
            "google",
            ":th",
        ])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Secret Guard: send"))
        // The user asked for it to be sent, so this is what would be sent.
        .stdout(predicate::str::contains(FAKE_KEY));
}

#[test]
fn protected_terms_appear_as_placeholders() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("protected_terms = [\"translate-kun\"]");
    sandbox
        .kun()
        .args([
            "--dry-run",
            "--provider",
            "google",
            ":th",
            "use translate-kun now",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "{BEGIN}\nuse KUNPT0X now\n{END}\n"
        )))
        .stdout(predicate::str::contains("translate-kun").not());
}

#[test]
fn a_language_pair_with_a_detected_source_may_call_twice() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("pair = [\"en\", \"th\"]");
    sandbox
        .kun()
        .args(["--dry-run", "--provider", "google", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("language pair: en, th"))
        .stdout(predicate::str::contains("second call: possible"));

    // With a Source Language given, the direction is known: one call.
    sandbox
        .kun()
        .args(["--dry-run", "--provider", "google", "-s", "en", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("second call: no"));
}

#[test]
fn the_same_source_and_target_would_send_nothing() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "google", "en:en", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing would be sent"))
        .stdout(predicate::str::contains(BEGIN).not());
}

#[test]
fn the_size_limit_and_missing_target_apply_in_a_dry_run() {
    Sandbox::new()
        .kun()
        .args([
            "--dry-run",
            "--max-chars",
            "3",
            "--provider",
            "google",
            ":th",
            "hello",
        ])
        .assert()
        .code(7);
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--provider", "google", "hello"])
        .assert()
        .code(2);
}

#[test]
fn a_configured_endpoint_still_fails_like_a_real_run() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("[providers.google]\nendpoint = \"http://localhost:1\"");
    sandbox
        .kun()
        .args(["--dry-run", "--provider", "google", ":th", "hello"])
        .assert()
        .code(3);
}

#[test]
fn json_has_the_documented_shape() {
    let output = Sandbox::new()
        .kun()
        .args(["--dry-run", "--json", "--provider", "anthropic", "en:th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .env("ANTHROPIC_API_KEY", "sk-ant-FAKE-EXAMPLE")
        .arg("--redact")
        .assert()
        .success()
        .stderr(predicate::str::contains(FAKE_KEY).not())
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.contains(FAKE_KEY));
    assert!(!stdout.contains("sk-ant-FAKE-EXAMPLE"));
    assert_eq!(stdout.lines().count(), 1);

    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["provider"], "anthropic");
    assert_eq!(value["host"], "api.anthropic.com");
    assert_eq!(value["model"], "claude-haiku-5-5");
    assert_eq!(value["source"], "en");
    assert_eq!(value["target"], "th");
    assert_eq!(value["pair"], serde_json::Value::Null);
    assert_eq!(value["second_call_possible"], false);
    assert_eq!(value["would_send"], true);
    assert_eq!(value["credentials"], "env");
    assert_eq!(value["secret_guard"]["decision"], "redact");
    assert_eq!(
        value["secret_guard"]["findings"][0]["kind"],
        "AWS access key"
    );
    assert_eq!(value["secret_guard"]["findings"][0]["start"], 10);
    assert_eq!(value["secret_guard"]["findings"][0]["end"], 30);
    assert_eq!(value["text"], "my key is [REDACTED]\n");
    assert!(value["warnings"].is_array());
}

#[test]
fn json_for_a_pair_and_a_detected_source() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("pair = [\"en\", \"th\"]");
    let output = sandbox
        .kun()
        .args(["--dry-run", "--json", "--provider", "google", "hello"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["source"], serde_json::Value::Null);
    assert_eq!(value["target"], serde_json::Value::Null);
    assert_eq!(value["pair"], serde_json::json!(["en", "th"]));
    assert_eq!(value["second_call_possible"], true);
    assert_eq!(value["model"], serde_json::Value::Null);
    assert_eq!(value["credentials"], "none");
    assert_eq!(value["secret_guard"]["decision"], "send");
    assert_eq!(value["secret_guard"]["findings"], serde_json::json!([]));
}

#[test]
fn json_blocked_is_still_an_error_on_stderr() {
    Sandbox::new()
        .kun()
        .args(["--dry-run", "--json", "--provider", "google", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .code(6)
        .stdout("");
}
