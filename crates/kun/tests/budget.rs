//! End-to-end tests for the Usage Budget (S17), with the fake Provider. Run with:
//! `cargo test --workspace --features translate-kun/fake-provider`
//!
//! The fake Provider is counted against the Google budget (the default
//! Provider), so these tests set `google_chars_per_month`. Every test runs in
//! a `Sandbox`: its own HOME, XDG_CONFIG_HOME and XDG_STATE_HOME.

#![cfg(feature = "fake-provider")]

mod common;

use common::Sandbox;
use predicates::prelude::*;

/// Obviously fake, but shaped like an AWS access key, so the detector fires.
const FAKE_KEY: &str = "AKIAFAKEEXAMPLE12345";

fn limit(chars: u64) -> String {
    format!("[budget]\ngoogle_chars_per_month = {chars}\n")
}

/// `kun --provider fake :th hello` (5 characters).
fn translate(sandbox: &Sandbox) -> assert_cmd::assert::Assert {
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "hello"])
        .assert()
}

/// The current UTC month, as `kun budget status --json` reports it.
fn current_month(sandbox: &Sandbox) -> String {
    let output = sandbox
        .kun()
        .args(["budget", "status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["month"].as_str().unwrap().to_string()
}

fn usage_json(sandbox: &Sandbox) -> serde_json::Value {
    let text = std::fs::read_to_string(sandbox.usage_path()).unwrap();
    serde_json::from_str(&text).unwrap()
}

// --- kun budget set / unset / status -------------------------------------

#[test]
fn set_creates_the_user_config_and_status_shows_it() {
    let sandbox = Sandbox::new();
    assert!(!sandbox.user_config_path().exists());

    sandbox
        .kun()
        .args(["budget", "set", "google", "450000"])
        .assert()
        .success();

    let text = std::fs::read_to_string(sandbox.user_config_path()).unwrap();
    assert!(text.contains("[budget]"), "{text}");
    assert!(text.contains("google_chars_per_month = 450000"), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(sandbox.user_config_path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    sandbox
        .kun()
        .args(["budget", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "google: 0 of 450000 characters used, 450000 left",
        ))
        .stdout(predicate::str::contains(
            "anthropic: no limit, 0 characters used",
        ))
        .stdout(predicate::str::contains("resets on"));
}

#[test]
fn a_bare_budget_command_is_status() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox
        .kun()
        .arg("budget")
        .assert()
        .success()
        .stdout(predicate::str::contains("google: 0 of 100 characters used"));
}

#[test]
fn status_json_is_one_object() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    let output = sandbox
        .kun()
        .args(["--json", "budget", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["providers"]["google"]["limit"], 100);
    assert_eq!(json["providers"]["google"]["used"], 0);
    assert_eq!(json["providers"]["google"]["remaining"], 100);
    assert!(json["providers"]["anthropic"]["limit"].is_null());
    assert!(json["providers"]["anthropic"]["remaining"].is_null());
    assert!(json["resets_on"].as_str().unwrap().ends_with("-01"));
    let month = json["month"].as_str().unwrap();
    assert_eq!(month.len(), 7, "{month}");
}

#[test]
fn set_keeps_the_comments_and_other_settings_of_an_existing_file() {
    let sandbox = Sandbox::new();
    let original = "\
# my own notes, please keep
target = \"th\"   # inline note

[secret_guard]
force_redact = true
";
    sandbox.write_user_config(original);

    sandbox
        .kun()
        .args(["budget", "set", "anthropic", "50000"])
        .assert()
        .success();

    let text = std::fs::read_to_string(sandbox.user_config_path()).unwrap();
    for kept in [
        "# my own notes, please keep",
        "target = \"th\"   # inline note",
        "[secret_guard]",
        "force_redact = true",
    ] {
        assert!(text.contains(kept), "lost {kept:?} in:\n{text}");
    }
    assert!(text.contains("anthropic_chars_per_month = 50000"), "{text}");

    // Changing a value keeps the other key, and `unset` removes only its key.
    sandbox
        .kun()
        .args(["budget", "set", "google", "10"])
        .assert()
        .success();
    sandbox
        .kun()
        .args(["budget", "unset", "anthropic"])
        .assert()
        .success();
    let text = std::fs::read_to_string(sandbox.user_config_path()).unwrap();
    assert!(text.contains("google_chars_per_month = 10"), "{text}");
    assert!(!text.contains("anthropic_chars_per_month"), "{text}");
    assert!(text.contains("# my own notes, please keep"), "{text}");
}

#[test]
fn unset_without_a_file_or_a_key_is_not_an_error_and_creates_nothing() {
    let sandbox = Sandbox::new();
    sandbox
        .kun()
        .args(["budget", "unset", "google"])
        .assert()
        .success();
    assert!(!sandbox.user_config_path().exists());
}

#[test]
fn set_rejects_a_bad_provider_or_amount_with_exit_2() {
    let sandbox = Sandbox::new();
    for args in [
        ["budget", "set", "deepl", "5"],
        ["budget", "set", "google", "0"],
        ["budget", "set", "google", "-5"],
        ["budget", "set", "google", "lots"],
    ] {
        sandbox.kun().args(args).assert().code(2);
    }
    assert!(!sandbox.user_config_path().exists());
}

#[cfg(unix)]
#[test]
fn set_replaces_a_symlinked_config_instead_of_writing_through_it() {
    let sandbox = Sandbox::new();
    let other = sandbox.work_dir().join("dotfiles-original.toml");
    std::fs::write(&other, "target = \"th\"\n").unwrap();
    let config = sandbox.user_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&other, &config).unwrap();

    sandbox
        .kun()
        .args(["budget", "set", "google", "10"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(&other).unwrap(),
        "target = \"th\"\n"
    );
    assert!(!config.symlink_metadata().unwrap().file_type().is_symlink());
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains("target = \"th\""), "{text}");
    assert!(text.contains("google_chars_per_month = 10"), "{text}");
}

#[test]
fn budget_set_refuses_to_touch_a_file_that_is_not_valid_toml() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("this is = = not toml");
    sandbox
        .kun()
        .args(["budget", "set", "google", "10"])
        .assert()
        .code(3);
    assert_eq!(
        std::fs::read_to_string(sandbox.user_config_path()).unwrap(),
        "this is = = not toml"
    );
}

#[test]
fn text_that_starts_with_budget_needs_a_double_dash() {
    Sandbox::new()
        .kun()
        .args(["--provider", "fake", ":th", "--", "budget", "is", "tight"])
        .assert()
        .success()
        .stdout("[th] budget is tight\n");
}

// --- counting ------------------------------------------------------------

#[test]
fn a_translation_adds_its_characters_to_the_counter() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));

    translate(&sandbox).success().stdout("[th] hello\n");
    translate(&sandbox).success();

    let month = current_month(&sandbox);
    let usage = usage_json(&sandbox);
    assert_eq!(usage["version"], 1);
    assert_eq!(usage["month"], month.as_str());
    assert_eq!(usage["google"], 10);
    assert_eq!(usage["anthropic"], 0);

    sandbox
        .kun()
        .args(["budget", "status"])
        .assert()
        .stdout(predicate::str::contains(
            "google: 10 of 100 characters used, 90 left",
        ));
}

#[test]
fn the_state_file_holds_counters_only_and_is_private() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "private words here"])
        .assert()
        .success();

    let text = std::fs::read_to_string(sandbox.usage_path()).unwrap();
    assert!(!text.contains("private"), "{text}");
    assert!(!text.contains("words"), "{text}");
    assert!(!text.contains("[th]"), "{text}");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys.len(), 4, "{keys:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(sandbox.usage_path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn what_counts_is_what_is_sent_after_redaction() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(1000));
    sandbox
        .kun()
        .args(["--provider", "fake", "--redact", ":th"])
        .write_stdin(format!("key {FAKE_KEY}"))
        .assert()
        .success()
        .stdout("[th] key [REDACTED]\n");
    // "key [REDACTED]" is 14 characters; the 20-character key is not counted.
    assert_eq!(usage_json(&sandbox)["google"], 14);
}

#[test]
fn what_counts_is_what_is_sent_after_protected_terms() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!(
        "protected_terms = [\"translate-kun\"]\n{}",
        limit(1000)
    ));
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", "translate-kun"])
        .assert()
        .success()
        .stdout("[th] translate-kun\n");
    // The Provider only sees the placeholder `KUNPT0X` (7 characters).
    assert_eq!(usage_json(&sandbox)["google"], 7);
}

#[test]
fn characters_are_counted_not_bytes() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(1000));
    sandbox
        .kun()
        .args(["--provider", "fake", ":en", "สวัสดี"])
        .assert()
        .success();
    assert_eq!(usage_json(&sandbox)["google"], 6);
}

#[test]
fn without_a_budget_the_state_file_is_never_created() {
    let sandbox = Sandbox::new();
    translate(&sandbox).success();
    assert!(!sandbox.usage_path().exists());
}

#[test]
fn text_that_is_not_sent_is_not_counted() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(1000));

    // The Source Language equals the Target Language: no request.
    sandbox
        .kun()
        .args(["--provider", "fake", "en:en", "hello"])
        .assert()
        .success();
    // Secret Guard block (exit 6) and the size limit (exit 7).
    sandbox
        .kun()
        .args(["--provider", "fake", ":th", FAKE_KEY])
        .assert()
        .code(6);
    sandbox
        .kun()
        .args(["--provider", "fake", "--max-chars", "3", ":th", "hello"])
        .assert()
        .code(7);

    assert!(!sandbox.usage_path().exists());
}

#[test]
fn a_new_month_starts_from_zero() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox.write_usage(r#"{"version":1,"month":"1999-01","google":99,"anthropic":7}"#);

    translate(&sandbox).success();

    let usage = usage_json(&sandbox);
    assert_eq!(usage["month"], current_month(&sandbox).as_str());
    assert_eq!(usage["google"], 5);
    assert_eq!(usage["anthropic"], 0);
}

// --- refusing ------------------------------------------------------------

#[test]
fn a_translation_over_the_budget_exits_8_and_sends_nothing() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(4));

    let assert = translate(&sandbox).code(8).stdout("");
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains(
            "monthly usage budget for google reached: 0 of 4 characters used; \
             this text needs 5. It resets on "
        ),
        "{stderr}"
    );
    assert!(stderr.contains("(UTC). Raise it with: kun budget set google <chars>"));
    assert!(!stderr.contains("hello"), "the message repeats the text");
    // Nothing was sent, so nothing was counted or even created.
    assert!(!sandbox.usage_path().exists());
}

#[test]
fn the_limit_may_be_reached_exactly_but_not_passed() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(10));

    translate(&sandbox).success();
    translate(&sandbox).success();
    translate(&sandbox)
        .code(8)
        .stderr(predicate::str::contains("10 of 10 characters used"));
    assert_eq!(usage_json(&sandbox)["google"], 10);
}

#[test]
fn raising_the_limit_lets_the_next_translation_through() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(5));
    translate(&sandbox).success();
    translate(&sandbox).code(8);

    sandbox
        .kun()
        .args(["budget", "set", "google", "50"])
        .assert()
        .success();
    translate(&sandbox).success();
    assert_eq!(usage_json(&sandbox)["google"], 10);
}

#[test]
fn a_language_pair_counts_both_calls() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!("pair = [\"en\", \"th\"]\n{}", limit(100)));

    // English is the first language of the pair, so there are two calls.
    sandbox
        .kun()
        .args(["--provider", "fake", "hello"])
        .assert()
        .success()
        .stdout("[th] hello\n");
    assert_eq!(usage_json(&sandbox)["google"], 10);
}

#[test]
fn the_second_call_of_a_language_pair_is_checked_again() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!("pair = [\"en\", \"th\"]\n{}", limit(7)));

    // The first call (5 of 7) fits, the second (10 of 7) does not.
    sandbox
        .kun()
        .args(["--provider", "fake", "hello"])
        .assert()
        .code(8)
        .stdout("")
        .stderr(predicate::str::contains(
            "5 of 7 characters used; this text needs 5.",
        ));
    // The first call already happened and stays counted.
    assert_eq!(usage_json(&sandbox)["google"], 5);
}

// --- the state file ------------------------------------------------------

#[test]
fn a_corrupt_state_file_with_a_budget_fails_closed_with_exit_3() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox.write_usage("{ not json");

    translate(&sandbox)
        .code(3)
        .stdout("")
        .stderr(predicate::str::contains("Delete it"))
        .stderr(predicate::str::contains("usage.json"));
    assert_eq!(
        std::fs::read_to_string(sandbox.usage_path()).unwrap(),
        "{ not json"
    );
    sandbox.kun().args(["budget", "status"]).assert().code(3);
}

/// Makes the state directory unusable: a dangling symlink where `kun/` should
/// be. The usage file then reads as missing (0 used) but can never be saved.
#[cfg(unix)]
fn break_the_state_dir(sandbox: &Sandbox) {
    let dir = sandbox.usage_path().parent().unwrap().to_path_buf();
    std::os::unix::fs::symlink(sandbox.work_dir().join("nowhere"), dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_counter_that_cannot_be_saved_stops_every_run_before_it_sends() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    break_the_state_dir(&sandbox);

    // Without this check every run would start from 0 and the budget could
    // be passed without limit. Each run must fail, and send nothing.
    for _ in 0..3 {
        translate(&sandbox)
            .code(3)
            .stdout("")
            .stderr(predicate::str::contains("cannot be saved"))
            .stderr(predicate::str::contains("hello").not());
    }
    // Same for a Language Pair (two calls).
    sandbox
        .kun()
        .args(["--provider", "fake", "en:th", "hello"])
        .assert()
        .code(3)
        .stdout("");
}

/// Three separate `kun` processes must each fail with exit 3 and print nothing.
fn assert_every_run_fails_closed(sandbox: &Sandbox) {
    for _ in 0..3 {
        translate(sandbox)
            .code(3)
            .stdout("")
            .stderr(predicate::str::contains("hello").not());
    }
}

#[test]
fn a_usage_file_that_is_a_directory_stops_every_run_before_it_sends() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    std::fs::create_dir_all(sandbox.usage_path()).unwrap();

    assert_every_run_fails_closed(&sandbox);
    translate(&sandbox).stderr(predicate::str::contains("not a regular file"));
    assert!(sandbox.usage_path().is_dir(), "nothing was replaced");
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_as_the_usage_file_stops_every_run_before_it_sends() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    let path = sandbox.usage_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let target = sandbox.work_dir().join("nowhere");
    std::os::unix::fs::symlink(&target, &path).unwrap();

    assert_every_run_fails_closed(&sandbox);
    translate(&sandbox).stderr(predicate::str::contains("not a regular file"));
    // Never followed, never replaced.
    assert!(!target.exists());
    assert!(path.symlink_metadata().unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn a_read_only_state_directory_stops_every_run_before_it_sends() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    let dir = sandbox.usage_path().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root can write anywhere, so the setup does not hold: skip.
    let root = std::fs::write(dir.join("probe"), "x").is_ok();

    let outputs: Vec<_> = (0..3)
        .map(|_| {
            sandbox
                .kun()
                .args(["--provider", "fake", ":th", "hello"])
                .output()
                .unwrap()
        })
        .collect();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if root {
        return;
    }

    for output in outputs {
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stdout.is_empty(), "something was sent");
    }
    assert!(!sandbox.usage_path().exists());
}

#[test]
fn repeated_runs_can_never_pass_the_budget() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(12));

    let mut printed = 0;
    for _ in 0..6 {
        let output = sandbox
            .kun()
            .args(["--provider", "fake", ":th", "hello"])
            .output()
            .unwrap();
        if output.status.success() {
            printed += 1;
        } else {
            assert_eq!(output.status.code(), Some(8));
            assert!(output.stdout.is_empty());
        }
    }
    // 5 characters each: two fit in 12, the third would make 15.
    assert_eq!(printed, 2);
    assert_eq!(usage_json(&sandbox)["google"], 10);
}

#[test]
fn a_run_that_sends_nothing_does_not_need_the_usage_file() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox.write_usage("{ not json");

    // Source Language equals Target Language: no Provider call at all.
    sandbox
        .kun()
        .args(["--provider", "fake", "en:en", "hello"])
        .assert()
        .success()
        .stdout("hello\n");
    assert_eq!(
        std::fs::read_to_string(sandbox.usage_path()).unwrap(),
        "{ not json"
    );
}

#[test]
fn a_corrupt_state_file_without_a_budget_is_ignored_and_left_alone() {
    let sandbox = Sandbox::new();
    sandbox.write_usage("{ not json");

    translate(&sandbox).success().stdout("[th] hello\n");
    assert_eq!(
        std::fs::read_to_string(sandbox.usage_path()).unwrap(),
        "{ not json"
    );
}

#[test]
fn a_budget_for_another_provider_does_not_touch_the_state_file() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("[budget]\nanthropic_chars_per_month = 5\n");
    sandbox.write_usage("{ not json");

    // The fake Provider counts as Google, which has no limit here.
    translate(&sandbox).success();
    assert_eq!(
        std::fs::read_to_string(sandbox.usage_path()).unwrap(),
        "{ not json"
    );
}

// --- dry run -------------------------------------------------------------

#[test]
fn a_dry_run_reports_the_budget_and_never_counts() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(100));
    sandbox.write_usage(&format!(
        r#"{{"version":1,"month":"{}","google":40,"anthropic":0}}"#,
        current_month(&sandbox)
    ));
    let before = std::fs::read_to_string(sandbox.usage_path()).unwrap();

    sandbox
        .kun()
        .args(["--provider", "fake", "--dry-run", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "budget: 40 of 100 used this month; this run needs 5",
        ))
        .stdout(predicate::str::contains("a real run would be sent"));

    assert_eq!(
        std::fs::read_to_string(sandbox.usage_path()).unwrap(),
        before
    );
}

#[test]
fn a_dry_run_says_when_a_real_run_would_be_refused() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(4));

    sandbox
        .kun()
        .args(["--provider", "fake", "--dry-run", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "budget: 0 of 4 used this month; this run needs 5",
        ))
        .stdout(predicate::str::contains("a real run would be refused"));
    assert!(!sandbox.usage_path().exists());
}

#[test]
fn a_dry_run_that_sends_nothing_is_not_refused_even_over_the_budget() {
    let sandbox = Sandbox::new();
    // 10 used, and the limit lowered to 5 afterwards.
    sandbox.write_user_config(&limit(5));
    sandbox.write_usage(&format!(
        r#"{{"version":1,"month":"{}","google":10,"anthropic":0}}"#,
        current_month(&sandbox)
    ));

    // A real run returns the text unchanged, with no Provider call...
    sandbox
        .kun()
        .args(["--provider", "fake", "en:en", "hello"])
        .assert()
        .success()
        .stdout("hello\n");
    // ...so the dry run must not say it would be refused.
    sandbox
        .kun()
        .args(["--provider", "fake", "--dry-run", "en:en", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "budget: 10 of 5 used this month; this run needs 0",
        ))
        .stdout(predicate::str::contains("no Provider call"))
        .stdout(predicate::str::contains("refused").not());

    let output = sandbox
        .kun()
        .args([
            "--provider",
            "fake",
            "--dry-run",
            "--json",
            "en:en",
            "hello",
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["budget"]["would_refuse"], false);
    assert_eq!(json["budget"]["needs"], 0);
}

#[test]
fn unset_keeps_the_comment_lines_above_the_removed_key() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(
        "[budget]\n# my free tier\ngoogle_chars_per_month = 10\n# paid\nanthropic_chars_per_month = 5\n",
    );

    sandbox
        .kun()
        .args(["budget", "unset", "google"])
        .assert()
        .success();

    let text = std::fs::read_to_string(sandbox.user_config_path()).unwrap();
    assert!(text.contains("# my free tier"), "{text}");
    assert!(text.contains("# paid"), "{text}");
    assert!(!text.contains("google_chars_per_month"), "{text}");
    assert!(text.contains("anthropic_chars_per_month = 5"), "{text}");
}

#[test]
fn the_help_mentions_the_usage_budget() {
    Sandbox::new()
        .kun()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("kun budget set google 450000"));
}

#[test]
fn a_dry_run_without_a_budget_has_no_budget_line() {
    let sandbox = Sandbox::new();
    sandbox
        .kun()
        .args(["--provider", "fake", "--dry-run", ":th", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("budget:").not());
    assert!(!sandbox.usage_path().exists());
}

#[test]
fn the_dry_run_json_has_a_budget_object_or_null() {
    let sandbox = Sandbox::new();
    let run = |sandbox: &Sandbox| {
        let output = sandbox
            .kun()
            .args(["--provider", "fake", "--dry-run", "--json", ":th", "hello"])
            .output()
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    assert!(run(&sandbox)["budget"].is_null());

    sandbox.write_user_config(&limit(4));
    let json = run(&sandbox);
    assert_eq!(json["budget"]["limit"], 4);
    assert_eq!(json["budget"]["used"], 0);
    assert_eq!(json["budget"]["needs"], 5);
    assert_eq!(json["budget"]["would_refuse"], true);
}

// --- the Project Config is untrusted (ADR 0003) -------------------------

#[test]
fn a_budget_in_a_project_config_is_a_config_error() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("[budget]\ngoogle_chars_per_month = 999999999\n");

    translate(&sandbox)
        .code(3)
        .stdout("")
        .stderr(predicate::str::contains("Project Config"))
        .stderr(predicate::str::contains("`budget`"))
        .stderr(predicate::str::contains("999999999").not());
}

#[test]
fn a_project_config_cannot_remove_the_user_budget() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&limit(4));
    sandbox.write_project_config("target = \"th\"\n");

    sandbox
        .kun()
        .args(["--provider", "fake", "hello"])
        .assert()
        .code(8);
}
