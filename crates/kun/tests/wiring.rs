//! S7 end-to-end tests: Provider wiring, `kun auth`, `kun init`, `--json`.
//!
//! With the `fake-provider` feature the binary uses an in-memory credential
//! store (see `auth::open_store`), so these tests never touch the real
//! keychain, and they never use the network.

#![cfg(feature = "fake-provider")]

mod common;

use common::Sandbox;
use predicates::prelude::*;

/// Obviously fake credential.
const FAKE_API_KEY: &str = "AIzaFAKE-EXAMPLE-KEY-0123456789";

// --- Provider wiring ----------------------------------------------------

#[test]
fn google_without_a_credential_exits_4_with_a_helpful_message() {
    Sandbox::new()
        .kun()
        .args([":th", "hello"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "No Google API key found. Run \"kun auth login google\" or set KUN_GOOGLE_API_KEY.",
        ));
}

#[cfg(unix)]
#[test]
fn an_insecure_user_config_warns_and_its_credentials_are_ignored() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!(
        "[credentials]\ngoogle_api_key = \"{FAKE_API_KEY}\"\n"
    ));
    let path = sandbox.user_config_path();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    sandbox
        .kun()
        .args([":th", "hello"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "is readable by others; its [credentials] were ignored. Run: chmod 600",
        ))
        .stderr(predicate::str::contains(FAKE_API_KEY).not());
}

#[test]
fn anthropic_without_a_credential_exits_4_with_a_helpful_message() {
    Sandbox::new()
        .kun()
        .args(["--provider", "anthropic", ":th", "hello"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "No Anthropic API key found. Run \"kun auth login anthropic\" or set ANTHROPIC_API_KEY.",
        ));
}

// --- kun auth -----------------------------------------------------------

#[test]
fn auth_status_names_the_source_and_never_prints_a_key() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!(
        "[credentials]\nanthropic_api_key = \"{FAKE_API_KEY}-FILE\"\n"
    ));
    sandbox
        .kun()
        .env("KUN_GOOGLE_API_KEY", FAKE_API_KEY)
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("google: env"))
        .stdout(predicate::str::contains("anthropic: config file"))
        .stdout(predicate::str::contains("EXAMPLE").not())
        .stderr(predicate::str::contains("EXAMPLE").not());
}

#[test]
fn auth_status_works_with_a_broken_project_config() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("this is [[[ not toml");
    sandbox
        .kun()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("google: none"));
}

#[cfg(unix)]
#[test]
fn auth_status_reports_ignored_credentials_of_an_insecure_user_config() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    sandbox.write_user_config(&format!(
        "[credentials]\ngoogle_api_key = \"{FAKE_API_KEY}\"\n"
    ));
    std::fs::set_permissions(
        sandbox.user_config_path(),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    sandbox
        .kun()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "google: none (config file credentials ignored",
        ))
        .stdout(predicate::str::contains("EXAMPLE").not())
        .stderr(predicate::str::contains("readable by others"));
}

#[test]
fn auth_status_says_none_when_there_is_no_key() {
    Sandbox::new()
        .kun()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("google: none"))
        .stdout(predicate::str::contains("anthropic: none"));
}

#[test]
fn auth_login_reads_a_piped_key_and_never_echoes_it() {
    Sandbox::new()
        .kun()
        .args(["auth", "login", "google"])
        .write_stdin(format!("  {FAKE_API_KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("EXAMPLE").not())
        .stderr(predicate::str::contains("EXAMPLE").not());
}

#[test]
fn auth_login_rejects_an_empty_key_with_exit_2() {
    Sandbox::new()
        .kun()
        .args(["auth", "login", "google"])
        .write_stdin("   \n")
        .assert()
        .code(2);
}

#[test]
fn auth_login_rejects_an_unknown_provider_with_exit_2() {
    Sandbox::new()
        .kun()
        .args(["auth", "login", "deepl"])
        .write_stdin("x\n")
        .assert()
        .code(2);
}

#[test]
fn auth_logout_succeeds_even_when_nothing_is_stored() {
    Sandbox::new()
        .kun()
        .args(["auth", "logout", "google"])
        .assert()
        .success();
}

#[test]
fn auth_help_lists_the_subcommands() {
    Sandbox::new()
        .kun()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("logout"))
        .stdout(predicate::str::contains("status"));
}

// --- kun init -----------------------------------------------------------

#[test]
fn init_creates_a_project_config_that_the_loader_accepts() {
    let sandbox = Sandbox::new();
    sandbox.kun().arg("init").assert().success();

    let path = sandbox.work_dir().join(".kun.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Credentials"), "must warn about credentials");
    kun_core::config::load_project_config(&path).expect("the template must parse");

    // Every example in the template is valid once it is uncommented.
    let uncommented: String = text
        .lines()
        .map(|line| match line.strip_prefix('#') {
            Some(rest) if !rest.starts_with(' ') && !rest.is_empty() => rest,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let full = sandbox.work_dir().join("full.toml");
    std::fs::write(&full, uncommented).unwrap();
    kun_core::config::load_project_config(&full).expect("the examples must parse");
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("target = \"fr\"\n");
    sandbox.kun().arg("init").assert().code(2);
    let text = std::fs::read_to_string(sandbox.work_dir().join(".kun.toml")).unwrap();
    assert_eq!(text, "target = \"fr\"\n");
}

#[test]
fn init_force_overwrites() {
    let sandbox = Sandbox::new();
    sandbox.write_project_config("target = \"fr\"\n");
    sandbox.kun().args(["init", "--force"]).assert().success();
    let text = std::fs::read_to_string(sandbox.work_dir().join(".kun.toml")).unwrap();
    assert_ne!(text, "target = \"fr\"\n");
}

// --- --json -------------------------------------------------------------

fn json_of(output: &[u8]) -> serde_json::Value {
    serde_json::from_slice(output).expect("stdout must be one JSON object")
}

/// An empty regular file to use as stdin. Unlike `Stdio::null()` it is known
/// to be empty on every platform (the Windows `NUL` device cannot be told
/// apart safely, see `input.rs`), so no "ignoring stdin" warning is expected.
fn empty_stdin(sandbox: &Sandbox) -> std::process::Stdio {
    let path = sandbox.work_dir().join("empty-stdin.txt");
    std::fs::write(&path, "").unwrap();
    std::process::Stdio::from(std::fs::File::open(&path).unwrap())
}

#[test]
fn json_output_has_the_documented_shape() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .std_command()
        .args(["--json", "--provider", "fake", ":th", "hello"])
        .stdin(empty_stdin(&sandbox))
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json_of(&output.stdout);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["text"], "[th] hello");
    assert_eq!(value["source"], "en");
    assert_eq!(value["target"], "th");
    assert_eq!(value["provider"], "fake");
    assert_eq!(value["warnings"], serde_json::json!([]));
    assert_eq!(value.as_object().unwrap().len(), 6);
}

#[test]
fn json_warnings_move_from_stderr_into_the_object() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .std_command()
        .args(["--json", "--provider", "fake", "en:en", "hello"])
        .stdin(empty_stdin(&sandbox))
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json_of(&output.stdout);
    assert_eq!(value["text"], "hello");
    assert_eq!(value["warnings"].as_array().unwrap().len(), 1);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("warning"));
}

#[test]
fn json_errors_still_go_to_stderr_with_the_same_exit_code() {
    let output = Sandbox::new()
        .kun()
        .args(["--json", "--provider", "fake", "hello"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

// --- stdin that carries nothing -----------------------------------------

// Unix only: the Windows `NUL` device cannot be detected without `unsafe`,
// so there it still gets the warning.
#[cfg(unix)]
#[test]
fn stdin_from_dev_null_does_not_trigger_the_ignored_stdin_warning() {
    use std::process::Stdio;
    let output = Sandbox::new()
        .std_command()
        .args(["--provider", "fake", ":th", "hello"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("ignoring stdin"));
}

#[test]
fn an_empty_regular_file_on_stdin_does_not_trigger_the_warning() {
    use std::process::Stdio;
    let sandbox = Sandbox::new();
    let empty = sandbox.work_dir().join("empty.txt");
    std::fs::write(&empty, "").unwrap();
    let output = sandbox
        .std_command()
        .args(["--provider", "fake", ":th", "hello"])
        .stdin(Stdio::from(std::fs::File::open(&empty).unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("ignoring stdin"));
}

#[test]
fn a_non_empty_file_on_stdin_still_triggers_the_warning() {
    use std::process::Stdio;
    let sandbox = Sandbox::new();
    let file = sandbox.work_dir().join("data.txt");
    std::fs::write(&file, "data").unwrap();
    let output = sandbox
        .std_command()
        .args(["--provider", "fake", ":th", "hello"])
        .stdin(Stdio::from(std::fs::File::open(&file).unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ignoring stdin"));
}
