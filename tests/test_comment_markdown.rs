//! `comment create --markdown` / `comment reply --markdown` submit
//! ClickUp's documented rich-comment ops array instead of comment_text.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path as path_matcher};
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

#[tokio::test]
async fn comment_create_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "hello "},
                {"text": "bold", "attributes": {"bold": true}},
                {"text": "\n"}
            ],
            "notify_all": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--task",
            "t1",
            "--markdown",
            "--text",
            "hello **bold**",
        ])
        .assert()
        .success();
}

/// The markdown body must NOT contain comment_text (wiremock's
/// body_partial_json can't assert absence, so parse the received request).
#[tokio::test]
async fn comment_create_markdown_omits_comment_text() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--task",
            "t1",
            "--markdown",
            "--text",
            "*x*",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(body.get("comment_text").is_none(), "body: {body}");
    assert!(body.get("comment").is_some(), "body: {body}");
}

#[tokio::test]
async fn comment_create_list_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/9/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "hello "},
                {"text": "bold", "attributes": {"bold": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--list",
            "9",
            "--markdown",
            "--text",
            "hello **bold**",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_create_view_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/view/v1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "ls", "attributes": {"code": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--view",
            "v1",
            "--markdown",
            "--text",
            "`ls`",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_reply_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "ls", "attributes": {"code": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "reply", "c1", "--markdown", "--text", "`ls`"])
        .assert()
        .success();
}

/// Without --markdown the body is byte-identical to today: comment_text
/// carries the raw string, no `comment` key.
#[tokio::test]
async fn comment_create_without_flag_unchanged() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment_text": "hello **bold**"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--task",
            "t1",
            "--text",
            "hello **bold**",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(body.get("comment").is_none(), "body: {body}");
}

#[tokio::test]
async fn mcp_comment_create_markdown_sends_ops() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "x", "attributes": {"bold": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd.write_stdin(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "clickup_comment_create", "arguments": {
                "task_id": "t1", "text": "**x**", "markdown": true
            }}
        })
        .to_string()
            + "\n",
    )
    .assert()
    .success()
    .stdout(predicates::str::contains("Comment created"));
}

/// Ops-empty fallback: input that reduces to nothing still posts a
/// non-empty comment via comment_text.
#[tokio::test]
async fn comment_create_markdown_empty_ops_falls_back_to_text() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(
            serde_json::json!({"comment_text": "   "}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--task",
            "t1",
            "--markdown",
            "--text",
            "   ",
        ])
        .assert()
        .success();
}

// ---------- v2: MCP reply parity (#120) ----------

#[tokio::test]
async fn mcp_comment_reply_markdown_sends_ops() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "x", "attributes": {"italic": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd.write_stdin(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "clickup_comment_reply", "arguments": {
                "comment_id": "c1", "text": "*x*", "markdown": true
            }}
        })
        .to_string()
            + "\n",
    )
    .assert()
    .success()
    .stdout(predicates::str::contains("Reply posted"));
}
