use assert_cmd::Command;
use predicates::prelude::*;

fn clickup() -> Command {
    Command::cargo_bin("clickup").unwrap()
}

#[test]
fn test_help() {
    clickup()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI for the ClickUp API"));
}

#[test]
fn test_version() {
    clickup()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("clickup 0.1.0"));
}

#[test]
fn test_setup_help() {
    clickup()
        .args(["setup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("token"));
}

#[test]
fn test_auth_help() {
    clickup()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("check"));
}

#[test]
fn test_workspace_help() {
    clickup()
        .args(["workspace", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("seats"))
        .stdout(predicate::str::contains("plan"));
}

#[test]
fn test_space_help() {
    clickup()
        .args(["space", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn test_folder_help() {
    clickup()
        .args(["folder", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn test_list_help() {
    clickup()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("add-task"))
        .stdout(predicate::str::contains("remove-task"));
}

#[test]
fn test_task_help() {
    clickup()
        .args(["task", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("time-in-status"));
}

#[test]
fn test_no_subcommand_shows_help() {
    clickup()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}
