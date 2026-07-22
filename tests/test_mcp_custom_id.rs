use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mcp_serve(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

fn rpc_call(tool: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    })
    .to_string()
        + "\n"
}

#[tokio::test]
async fn task_get_includes_custom_id_when_set() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "custom_id": "PROJ-42",
            "status": {"status": "open"},
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_get",
            serde_json::json!({"task_id": "abc123"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("custom_id"))
        .stdout(predicates::str::contains("PROJ-42"));
}

#[tokio::test]
async fn task_get_omits_custom_id_when_null() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "custom_id": null,
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_get",
            serde_json::json!({"task_id": "abc123"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("custom_id").not());
}

#[tokio::test]
async fn task_search_includes_custom_id_when_set() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                {"id": "abc123", "name": "with custom", "custom_id": "PROJ-42"},
                {"id": "def456", "name": "without custom", "custom_id": null},
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call("clickup_task_search", serde_json::json!({})))
        .assert()
        .success()
        .stdout(predicates::str::contains("PROJ-42"))
        // custom_id must appear exactly once: emitted for abc123, omitted
        // entirely for def456 (whose custom_id is null).
        .stdout(predicates::str::contains("custom_id").count(1));
}

#[tokio::test]
async fn task_create_includes_custom_id_when_set() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/9/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "new1",
            "name": "created",
            "custom_id": "PROJ-99",
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_create",
            serde_json::json!({"list_id": "9", "name": "created"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("PROJ-99"));
}
