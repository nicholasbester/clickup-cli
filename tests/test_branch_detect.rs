//! Integration tests for git-branch-based task ID auto-detection.
//!
//! Each test sets up a tempdir with a real git repo + checked-out branch, points
//! the CLI at a wiremock server via `CLICKUP_API_URL`, and asserts the request
//! path the CLI actually sent.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn git_init_with_branch(dir: &Path, branch: &str) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed")
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    run(&["commit", "--allow-empty", "-q", "-m", "init"]);
    if branch != "main" {
        run(&["checkout", "-q", "-b", branch]);
    }
}

fn clickup(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

#[tokio::test]
async fn cu_branch_resolves_standard_id() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "Test",
            "status": {"status": "open"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["--output", "json", "task", "get"])
        .assert()
        .success();
}

#[tokio::test]
async fn explicit_arg_wins_over_branch() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/xyz789"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "xyz789"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["--output", "json", "task", "get", "xyz789"])
        .assert()
        .success();
}

#[tokio::test]
async fn explicit_cu_prefix_is_stripped() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "main");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc123"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["--output", "json", "task", "get", "CU-abc123"])
        .assert()
        .success();
}

#[tokio::test]
async fn custom_id_auto_injects_query_params() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "PROJ-42-add-login");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/PROJ-42"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "99"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "PROJ-42"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["--output", "json", "task", "get"])
        .assert()
        .success();
}

#[tokio::test]
async fn no_match_branch_errors_with_hint() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "main");

    let server = MockServer::start().await;

    clickup(dir.path(), &server)
        .args(["task", "get"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("No task ID"));
}

#[tokio::test]
async fn delete_never_auto_detects() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;

    clickup(dir.path(), &server)
        .args(["task", "delete"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not auto-detect"));
}

#[tokio::test]
async fn git_detect_disabled_by_env_var() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;

    let mut cmd = clickup(dir.path(), &server);
    cmd.env("CLICKUP_GIT_DETECT", "0")
        .args(["task", "get"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("No task ID"));
}

#[tokio::test]
async fn clickup_task_id_env_var_wins_over_branch() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/envid1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "envid1"})))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = clickup(dir.path(), &server);
    cmd.env("CLICKUP_TASK_ID", "envid1")
        .args(["--output", "json", "task", "get"])
        .assert()
        .success();
}

#[tokio::test]
async fn breadcrumb_printed_on_table_output() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc123"})))
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "get"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "resolved task CU-abc123 from branch feat/CU-abc123-foo",
        ));
}

#[tokio::test]
async fn breadcrumb_suppressed_by_json_output() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc123"})))
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["--output", "json", "task", "get"])
        .assert()
        .success()
        .stderr(predicates::str::contains("resolved task").not());
}

#[tokio::test]
async fn breadcrumb_suppressed_by_quiet() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc123"})))
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["-q", "task", "get"])
        .assert()
        .success()
        .stderr(predicates::str::contains("resolved task").not());
}

#[tokio::test]
async fn add_tag_one_arg_detects_task() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "feat/CU-abc123-foo");

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/abc123/tag/urgent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "add-tag", "urgent"])
        .assert()
        .success();
}

#[tokio::test]
async fn add_tag_two_args_uses_explicit() {
    let dir = TempDir::new().unwrap();
    git_init_with_branch(dir.path(), "main");

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/xyz/tag/urgent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "add-tag", "xyz", "urgent"])
        .assert()
        .success();
}

#[tokio::test]
async fn not_in_git_repo_errors() {
    let dir = TempDir::new().unwrap();
    // No git init.

    let server = MockServer::start().await;

    clickup(dir.path(), &server)
        .args(["task", "get"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not inside a git repository"));
}
