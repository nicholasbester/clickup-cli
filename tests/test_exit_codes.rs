//! Regression tests for issue #109: clap usage errors exited with clap's
//! default code 2, which the documented exit-code contract reserves for
//! auth/permission errors (401/403). Bad input — including unparseable
//! invocations — is documented as exit code 1.

use assert_cmd::Command;

fn clickup() -> Command {
    Command::cargo_bin("clickup-cli").unwrap()
}

#[test]
fn unknown_subcommand_exits_1() {
    clickup()
        .arg("bogus-cmd")
        .assert()
        .code(1)
        .stderr(predicates::str::contains("unrecognized subcommand"));
}

#[test]
fn unknown_flag_exits_1() {
    clickup()
        .args(["task", "get", "--nonsense-flag", "x"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("unexpected argument"));
}

#[test]
fn missing_required_arg_exits_1() {
    // `space get` requires an ID positional.
    clickup()
        .args(["space", "get"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("required"));
}

#[test]
fn help_exits_0_on_stdout() {
    clickup()
        .arg("--help")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("Usage:"));
}

#[test]
fn version_exits_0_on_stdout() {
    clickup()
        .arg("--version")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("clickup-cli"));
}

/// The short alias binary shares main.rs and must behave identically.
#[test]
fn clkup_unknown_subcommand_exits_1() {
    Command::cargo_bin("clkup")
        .unwrap()
        .arg("bogus-cmd")
        .assert()
        .code(1)
        .stderr(predicates::str::contains("unrecognized subcommand"));
}

/// Bare invocation triggers clap's help-on-missing-subcommand kind: help
/// text renders, but on stderr with a bad-input exit — unlike --help.
#[test]
fn bare_invocation_exits_1_with_help_on_stderr() {
    clickup()
        .assert()
        .code(1)
        .stderr(predicates::str::contains("Usage:"));
}
