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
fn unknown_provider_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--provider", "deepl", ":th", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown provider"));
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

/// Obviously fake, but shaped like an AWS access key, so the detector fires.
const FAKE_KEY: &str = "AKIAFAKEEXAMPLE12345";

#[test]
fn bad_option_values_never_echo_what_was_typed() {
    for args in [
        vec!["-t", FAKE_KEY, "hello"],
        vec!["-s", FAKE_KEY, "hello"],
        vec!["--target", FAKE_KEY, "hello"],
        vec!["--max-chars", FAKE_KEY, "hello"],
        vec!["--max-chars=AKIAFAKEEXAMPLE12345", "hello"],
        vec!["--AKIAFAKEEXAMPLE12345", "hello"],
        vec!["--provider"],
    ] {
        Sandbox::new()
            .kun()
            .args(&args)
            .assert()
            .code(2)
            .stdout("")
            .stderr(predicate::str::contains(FAKE_KEY).not())
            .stderr(predicate::str::contains("error:"));
    }
}

#[test]
fn bad_option_error_names_the_option_but_not_the_value() {
    Sandbox::new()
        .kun()
        .args(["--max-chars", FAKE_KEY, "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--max-chars"))
        .stderr(predicate::str::contains("kun --help"));
}

#[test]
fn a_config_error_never_echoes_key_names_or_paths() {
    // A key name chosen by the user, in the Project Config.
    for text in [
        format!("secret_{FAKE_KEY} = 1"),
        format!("{FAKE_KEY} = 1"),
        format!("[{FAKE_KEY}]\nx = 1"),
        format!("[providers.{FAKE_KEY}]\nmodel = \"x\""),
    ] {
        let sandbox = Sandbox::new();
        sandbox.write_project_config(&text);
        sandbox
            .kun()
            .args([":th", "hello"])
            .assert()
            .code(3)
            .stdout("")
            .stderr(predicate::str::contains(FAKE_KEY).not())
            .stderr(predicate::str::contains("Project Config"));
    }
}

#[test]
fn a_config_error_never_echoes_the_directory_name() {
    // The path of the config file can hold user-chosen text, too.
    let sandbox = Sandbox::new();
    let dir = sandbox.work_dir().join(FAKE_KEY);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".kun.toml"), "api_key = \"x\"").unwrap();
    sandbox
        .kun()
        .current_dir(&dir)
        .args([":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn a_user_config_error_never_echoes_key_names() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!("{FAKE_KEY} = 1"));
    sandbox
        .kun()
        .args([":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(FAKE_KEY).not())
        .stderr(predicate::str::contains("User Config"));
}

#[test]
fn a_forbidden_setting_still_names_the_known_field() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[secret_guard]\nallow_secrets = true");
    sandbox
        .kun()
        .args([":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("secret_guard.allow_secrets"));
}

#[test]
fn a_missing_target_exits_2_before_the_secret_guard() {
    // Nothing says where to translate to, so this is a usage error. The
    // Secret Guard (exit 6) and the Provider (exit 1) are never reached.
    Sandbox::new()
        .kun()
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Target Language"))
        .stderr(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn a_missing_target_exits_2_before_the_provider_is_built() {
    Sandbox::new()
        .kun()
        .arg("hello")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Target Language"));
}

#[test]
fn whitespace_only_text_over_the_limit_exits_7() {
    Sandbox::new()
        .kun()
        .args(["--max-chars", "3", ":th", "          "])
        .assert()
        .code(7);
    Sandbox::new()
        .kun()
        .args(["--max-chars", "3", ":th"])
        .write_stdin("          ")
        .assert()
        .code(7);
}

#[test]
fn whitespace_only_text_within_the_limit_prints_help_and_exits_2() {
    Sandbox::new()
        .kun()
        .args([":th", "   "])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage"));
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
