//! Regression tests for issue #101: the MCP `clickup_member_list` tool passed
//! `task_id` raw into `/v2/task/{id}/member`, skipping the `resolve_task`
//! handling every sibling task-scoped MCP handler uses — so custom-format IDs
//! (`PROJ-42`) drew ClickUp's misleading 401 "Team not authorized" and `CU-`
//! prefixes were not stripped.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher, query_param, query_param_is_missing};
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
async fn member_list_custom_task_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/PROJ-42/member"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "members": [{"id": 1, "username": "nick", "email": "n@x.com"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_member_list",
            serde_json::json!({"task_id": "PROJ-42"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("nick"));
}

#[tokio::test]
async fn member_list_plain_task_id_omits_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/86czkjbtq/member"))
        .and(query_param_is_missing("custom_task_ids"))
        .and(query_param_is_missing("team_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "members": [{"id": 1, "username": "nick", "email": "n@x.com"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_member_list",
            serde_json::json!({"task_id": "86czkjbtq"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("nick"));
}

/// `CU-` prefixes are stripped by resolve_task on every sibling handler;
/// member_list must behave the same.
#[tokio::test]
async fn member_list_cu_prefix_is_stripped() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // path_matcher ignores query strings, so explicitly pin that CU-abc123
    // is not misclassified as a custom ID (which would append the pair).
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123/member"))
        .and(query_param_is_missing("custom_task_ids"))
        .and(query_param_is_missing("team_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "members": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_member_list",
            serde_json::json!({"task_id": "CU-abc123"}),
        ))
        .assert()
        .success();
}
