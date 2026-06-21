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
async fn test_task_create_with_parent_sends_subtask_body() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/task"))
        .and(body_json(serde_json::json!({
            "name": "child",
            "parent": "parent-1"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "child-1",
            "name": "child"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task", "create", "--list", "list-1", "--name", "child", "--parent", "parent-1",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_task_update_with_parent_reparents() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v2/task/abc123"))
        .and(body_json(serde_json::json!({
            "parent": "parent-1"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "Test"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "update", "abc123", "--parent", "parent-1"])
        .assert()
        .success();
}
