use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_name_and_version() {
    Command::cargo_bin("kun")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout("kun 0.1.0\n");
}

#[test]
fn no_args_prints_help_to_stderr_and_exits_2() {
    Command::cargo_bin("kun")
        .unwrap()
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage"));
}
