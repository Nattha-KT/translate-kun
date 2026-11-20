//! End-to-end tests with the fake Provider. Run with:
//! `cargo test --workspace --features kun/fake-provider`

#![cfg(feature = "fake-provider")]

mod common;

use common::Sandbox;
use predicates::prelude::*;

/// Obviously fake, but shaped like an AWS access key, so the detector fires.
const FAKE_KEY: &str = "AKIAFAKEEXAMPLE12345";

#[test]
fn colon_prefix_sets_the_target_language() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n")
        .stderr(predicate::str::contains("provider: fake"))
        .stderr(predicate::str::contains("detected source: en"));
}

#[test]
fn source_and_target_form_and_long_flags_agree() {
    let sandbox = Sandbox::new();
    sandbox
        .kun()
        .args(["--provider", "fake", "en:th", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n");
    sandbox
        .kun()
        .args(["--provider", "fake", "-s", "en", "-t", "th", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n");
}

#[test]
fn several_words_are_joined_with_single_spaces() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "hello", "big", "world"])
        .assert()
        .success()
        .stdout("[th] hello big world\n");
}

#[test]
fn bare_trailing_word_is_text_not_a_language() {
    // `it` must not be read as Italian: the whole line is text, and the
    // Target Language comes from the config.
    let sandbox = Sandbox::new();
    sandbox.write_user_config("target = \"it\"");
    sandbox
        .kun()
        .args(["--provider", "fake", "I", "love", "it"])
        .assert()
        .success()
        .stdout("[it] I love it\n");
}

#[test]
fn explicit_target_beats_the_configured_one() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("target = \"it\"");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n");
}

#[test]
fn no_target_language_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Target Language"))
        .stdout("");
}

#[test]
fn text_is_read_from_stdin() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin("hello\n")
        .assert()
        .success()
        .stdout("[th] hello\n");
}

#[test]
fn multi_line_stdin_is_kept_as_it_is() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin("line one\nline two\n")
        .assert()
        .success()
        .stdout("[th] line one\nline two\n");
}

#[test]
fn arguments_win_over_stdin_with_a_warning() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "from args"])
        .write_stdin("from stdin")
        .assert()
        .success()
        .stdout("[th] from args\n")
        .stderr(predicate::str::contains("using the arguments"));
}

#[test]
fn arguments_with_a_redirected_stdin_always_warn() {
    // The rule is "stdin is not a terminal", not "stdin has data": the test
    // harness gives an empty, non-terminal stdin, so the warning appears.
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n")
        .stderr(predicate::str::contains("using the arguments"));
}

#[test]
fn arguments_win_even_when_stdin_is_a_slow_producer() {
    use std::io::Write;
    use std::process::Stdio;
    use std::time::Duration;

    let sandbox = Sandbox::new();
    let mut child = sandbox
        .std_command()
        .args(["--provider", "fake", ":th", "from args"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // `kun` must not wait for, or depend on, what arrives on stdin.
    let mut stdin = child.stdin.take().unwrap();
    std::thread::sleep(Duration::from_millis(700));
    let _ = stdin.write_all(b"late data");
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "[th] from args\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("using the arguments"));
}

#[test]
fn arguments_never_wait_for_an_open_stdin() {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let sandbox = Sandbox::new();
    let mut child = sandbox
        .std_command()
        .args(["--provider", "fake", ":th", "hello"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Keep stdin open and silent: a program that read it would hang here.
    let _stdin = child.stdin.take().unwrap();

    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if started.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("kun waited for stdin although the text was given as arguments");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn empty_stdin_without_arguments_prints_help_and_exits_2() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin("")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn language_pair_english_goes_to_the_second_language() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("pair = [\"en\", \"th\"]");
    sandbox
        .kun()
        .args(["--provider", "fake", "hello", "there"])
        .assert()
        .success()
        .stdout("[th] hello there\n")
        .stderr(predicate::str::contains("detected source: en"));
}

#[test]
fn language_pair_thai_goes_to_the_first_language() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("pair = [\"en\", \"th\"]");
    sandbox
        .kun()
        .args(["--provider", "fake", "สวัสดี"])
        .assert()
        .success()
        .stdout("[en] สวัสดี\n")
        .stderr(predicate::str::contains("detected source: th"));
}

#[test]
fn explicit_target_beats_a_language_pair() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("pair = [\"en\", \"th\"]");
    sandbox
        .kun()
        .args(["--provider", "fake", ":ja", "hello"])
        .assert()
        .success()
        .stdout("[ja] hello\n");
}

#[test]
fn text_already_in_the_target_language_is_returned_with_a_warning() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":en", "hello"])
        .assert()
        .success()
        .stdout("hello\n")
        .stderr(predicate::str::contains("already in the Target Language"));
}

#[test]
fn over_long_argument_text_exits_7() {
    Sandbox::new()
        .kun()
        .args([
            "--provider",
            "fake",
            "--max-chars",
            "5",
            ":th",
            "hello world",
        ])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("limit of 5 characters"))
        .stdout("");
}

#[test]
fn over_long_stdin_exits_7() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--max-chars", "5", ":th"])
        .write_stdin("hello world")
        .assert()
        .code(7);
}

#[test]
fn the_limit_counts_characters_not_bytes() {
    // "ไทย" is 3 characters but 9 bytes: within a limit of 3 characters.
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--max-chars", "3", ":en", "ไทย"])
        .assert()
        .success();
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--max-chars", "3", ":en"])
        .write_stdin("ไทย")
        .assert()
        .success();
    // One character more is over the limit.
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--max-chars", "3", ":en", "ไทยx"])
        .assert()
        .code(7);
}

#[test]
fn max_chars_from_the_project_config_applies() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("max_chars = 3");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(7);
}

#[test]
fn a_secret_in_piped_input_is_blocked_and_never_echoed() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .code(6)
        .stdout("")
        .stderr(predicate::str::contains("AWS access key"))
        .stderr(predicate::str::contains("bytes 10..30"))
        .stderr(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn a_secret_in_arguments_is_blocked_when_stdin_is_not_a_terminal() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", FAKE_KEY])
        .assert()
        .code(6)
        .stderr(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn redact_replaces_the_secret_and_succeeds() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--redact", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("[REDACTED]"))
        .stdout(predicate::str::contains(FAKE_KEY).not())
        .stderr(predicate::str::contains(FAKE_KEY).not())
        .stderr(predicate::str::contains("[REDACTED]"));
}

#[test]
fn allow_secrets_sends_the_text() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", "--allow-secrets", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success()
        .stdout(format!("[th] my key is {FAKE_KEY}\n"));
}

#[test]
fn allow_secrets_in_the_user_config_is_honoured() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("[secret_guard]\nallow_secrets = true");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success();
}

#[test]
fn force_redact_in_the_project_config_applies() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[secret_guard]\nforce_redact = true");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin(format!("my key is {FAKE_KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("[REDACTED]"))
        .stdout(predicate::str::contains(FAKE_KEY).not());
}

#[test]
fn extra_secret_pattern_from_the_project_config_blocks() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[secret_guard]\nextra_patterns = [\"ACME-[0-9]{6}\"]");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th"])
        .write_stdin("code ACME-123456\n")
        .assert()
        .code(6)
        .stderr(predicate::str::contains("ACME-123456").not());
}

#[test]
fn allow_secrets_in_the_project_config_exits_3() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[secret_guard]\nallow_secrets = true");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("secret_guard.allow_secrets"))
        .stdout("");
}

#[test]
fn a_credential_in_the_project_config_exits_3_without_echoing_it() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("api_key = \"FAKE-EXAMPLE-KEY-0123456789\"");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("FAKE-EXAMPLE-KEY").not());
}

#[test]
fn an_unparsable_user_config_exits_3() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("target = ");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(3);
}

#[test]
fn an_invalid_extra_pattern_exits_3() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[secret_guard]\nextra_patterns = [\"(unclosed\"]");
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("unclosed").not());
}

#[cfg(unix)]
#[test]
fn a_world_readable_user_config_warns_but_works() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    sandbox.write_user_config("target = \"fr\"");
    // Loosen the mode that `write_user_config` set.
    std::fs::set_permissions(
        sandbox.user_config_path(),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    sandbox
        .kun()
        .args(["--provider", "fake", "hello"])
        .assert()
        .success()
        .stdout("[fr] hello\n")
        .stderr(predicate::str::contains("readable by other users"));
}

#[test]
fn project_config_is_found_from_a_subdirectory() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("target = \"de\"");
    let sub = sandbox.work_dir().join("a/b");
    std::fs::create_dir_all(&sub).unwrap();
    sandbox
        .kun()
        .current_dir(&sub)
        .args(["--provider", "fake", "hello"])
        .assert()
        .success()
        .stdout("[de] hello\n");
}
