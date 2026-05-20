use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn test_task_set_estimate_v2() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Mock for v2 update task (task-level estimate)
    Mock::given(method("PUT"))
        .and(path_matcher("/v2/task/abc123"))
        .and(body_json(serde_json::json!({
            "time_estimate": 900000
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "Test"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "set-estimate", "--id", "abc123", "--time", "900000"])
        .assert()
        .success();
}

#[tokio::test]
async fn test_task_set_estimate_v3() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Mock for v3 per-assignee estimate
    Mock::given(method("PATCH"))
        .and(path_matcher(
            "/v3/workspaces/99/tasks/abc123/time_estimates_by_user",
        ))
        .and(body_json(serde_json::json!({
            "time_estimates": [{"user_id": "123", "time_estimate": 900000}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "set-estimate",
            "--id",
            "abc123",
            "--time",
            "900000",
            "--assignee",
            "123",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_error_reporting_improved() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Mock for 400 error with message
    Mock::given(method("PUT"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "err": "Task is closed",
            "ECODE": "TASK_001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "set-estimate", "--id", "abc123", "--time", "900000"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Task is closed"));
}
