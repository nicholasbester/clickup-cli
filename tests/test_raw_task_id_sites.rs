//! Regression tests for issue #104: `task move`, `task set-estimate`/
//! `replace-estimates` (v3 endpoints), and `list add-task`/`remove-task`
//! passed task IDs through without `CU-` stripping, and silently sent
//! custom-format IDs (`PROJ-42`) to endpoints that do not document
//! `custom_task_ids` support — a guaranteed, confusing upstream failure.
//! Now: `CU-` prefixes are stripped everywhere, and custom-format IDs fail
//! locally with an actionable message naming the regular-ID workaround.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clickup(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

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

// ---------- CU- stripping ----------

#[tokio::test]
async fn cli_list_add_task_strips_cu_prefix() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/9/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["list", "add-task", "9", "CU-abc123"])
        .assert()
        .success();
}

#[tokio::test]
async fn cli_list_remove_task_strips_cu_prefix() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path_matcher("/v2/list/9/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["list", "remove-task", "9", "CU-abc123"])
        .assert()
        .success();
}

#[tokio::test]
async fn mcp_task_move_strips_cu_prefix() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v3/workspaces/99/tasks/abc123/home_list/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_move",
            serde_json::json!({"task_id": "CU-abc123", "list_id": "9"}),
        ))
        .assert()
        .success();
}

#[tokio::test]
async fn mcp_list_add_task_strips_cu_prefix() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/9/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_list_add_task",
            serde_json::json!({"list_id": "9", "task_id": "CU-abc123"}),
        ))
        .assert()
        .success();
}

// ---------- custom-format IDs fail locally with guidance ----------

#[tokio::test]
async fn cli_task_move_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // No mocks: the CLI must not send any request.

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "move", "PROJ-42", "--list", "9"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("requires the regular task id"))
        .stderr(predicates::str::contains("task get"));
}

#[tokio::test]
async fn cli_list_add_task_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["list", "add-task", "9", "PROJ-42"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("requires the regular task id"));
}

#[tokio::test]
async fn mcp_task_move_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // The MCP protocol reports tool errors in-band; the process exits 0.
    // The error text must appear in the response and no HTTP call be made.

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_move",
            serde_json::json!({"task_id": "PROJ-42", "list_id": "9"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("requires the regular task id"));
}

#[tokio::test]
async fn mcp_task_replace_estimates_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_replace_estimates",
            serde_json::json!({"task_id": "PROJ-42", "estimates": [{"assignee": 1, "time": 60000}]}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("requires the regular task id"));
}

#[tokio::test]
async fn mcp_list_remove_task_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_list_remove_task",
            serde_json::json!({"list_id": "9", "task_id": "PROJ-42"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("requires the regular task id"));
}

#[tokio::test]
async fn cli_task_move_strips_cu_prefix() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v3/workspaces/99/tasks/abc123/home_list/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "move", "CU-abc123", "--list", "9"])
        .assert()
        .success();
}

#[tokio::test]
async fn cli_set_estimate_assignee_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "set-estimate",
            "--id",
            "PROJ-42",
            "--assignee",
            "1",
            "--time",
            "60000",
        ])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("requires the regular task id"));
}

#[tokio::test]
async fn cli_replace_estimates_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "replace-estimates",
            "--id",
            "PROJ-42",
            "--estimate",
            "1:60000",
        ])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("requires the regular task id"));
}

/// The flag-less set-estimate path (v2, custom-ID capable) must keep
/// working for custom IDs — the deliberately preserved branch.
#[tokio::test]
async fn cli_set_estimate_flagless_custom_id_still_works() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v2/task/PROJ-42"))
        .and(wiremock::matchers::query_param("custom_task_ids", "true"))
        .and(wiremock::matchers::query_param("team_id", "99"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "abc123", "name": "t"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "set-estimate", "--id", "PROJ-42", "--time", "60000"])
        .assert()
        .success();
}

#[tokio::test]
async fn mcp_set_estimate_user_id_rejects_custom_id_locally() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_set_estimate",
            serde_json::json!({"task_id": "PROJ-42", "user_id": 1, "time_estimate": 60000}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("requires the regular task id"));
}
