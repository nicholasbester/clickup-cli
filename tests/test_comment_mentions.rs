//! `@Display Name` / `<@user_id>` in comment create/reply/update resolve
//! against `GET /v2/team` into ClickUp `type: "tag"` ops, in plain and
//! markdown mode. ClickUp ignores `@Name` inside `comment_text`.

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

fn mcp(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd
}

fn team_with_ada() -> serde_json::Value {
    serde_json::json!({
        "teams": [{
            "id": "99",
            "members": [{
                "user": {
                    "id": 111111,
                    "username": "Ada Lovelace",
                    "email": "ada@example.com"
                }
            }]
        }]
    })
}

async fn mount_team(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path_matcher("/v2/team"))
        .respond_with(ResponseTemplate::new(200).set_body_json(team_with_ada()))
        .expect(1)
        .mount(server)
        .await;
}

fn ada_tag() -> serde_json::Value {
    serde_json::json!({"type": "tag", "text": "@Ada Lovelace", "user": {"id": 111111}})
}

#[tokio::test]
async fn comment_create_markdown_resolves_display_name() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "hey "}, ada_tag(), {"text": "\n"}]
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
            "hey @Ada Lovelace",
        ])
        .assert()
        .success();
}

/// Plain mode: a resolving mention promotes `comment_text` to a `comment`
/// ops array, because ClickUp ignores `@Name` in comment_text.
#[tokio::test]
async fn comment_create_plain_text_promotes_to_ops_for_mention() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "ping "}, ada_tag()]
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
            "ping @Ada Lovelace",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let post = requests
        .iter()
        .find(|r| r.method.as_str() == "POST")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
    assert!(body.get("comment_text").is_none(), "body: {body}");
}

/// No `@` in the text: the roster is not fetched and the request is
/// byte-identical to before (plain comment_text).
#[tokio::test]
async fn comment_create_without_mention_skips_roster() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(
            serde_json::json!({"comment_text": "no mention"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "create", "--task", "t1", "--text", "no mention"])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert!(!requests.iter().any(|r| r.url.path() == "/v2/team"));
}

/// An unmatched `@token` stays plain text and the body stays comment_text.
#[tokio::test]
async fn comment_create_unmatched_mention_stays_plain() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(
            serde_json::json!({"comment_text": "hi @Nobody Here"}),
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
            "--text",
            "hi @Nobody Here",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_reply_resolves_numeric_id() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "thanks "}, ada_tag()]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "reply", "c1", "--text", "thanks <@111111>"])
        .assert()
        .success();
}

/// `comment update` gets the same resolution. The body is replaced whole,
/// so the ops array carries the tag op alongside the other fields.
#[tokio::test]
async fn comment_update_resolves_display_name() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v2/comment/c1"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "done "}, ada_tag()],
            "resolved": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "update",
            "c1",
            "--resolved",
            "--text",
            "done @Ada Lovelace",
        ])
        .assert()
        .success();
}

/// A markdown `[@Name](user:id)` link mention and a display-name mention
/// in the same body both survive; the existing tag op is not re-resolved.
#[tokio::test]
async fn comment_create_markdown_link_and_name_mentions_coexist() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"type": "tag", "user": {"id": 222222}},
                {"text": " and "},
                ada_tag(),
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
            "--task",
            "t1",
            "--markdown",
            "--text",
            "[@Nick](user:222222) and @Ada Lovelace",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn mcp_comment_create_resolves_display_name() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "hey "}, ada_tag()]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    mcp(dir.path(), &server)
        .write_stdin(
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "clickup_comment_create", "arguments": {
                    "task_id": "t1", "text": "hey @Ada Lovelace"
                }}
            })
            .to_string()
                + "\n",
        )
        .assert()
        .success()
        .stdout(predicates::str::contains("Comment created"));
}

#[tokio::test]
async fn mcp_comment_update_resolves_numeric_id() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v2/comment/c1"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "done "}, ada_tag()]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})))
        .expect(1)
        .mount(&server)
        .await;

    mcp(dir.path(), &server)
        .write_stdin(
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "clickup_comment_update", "arguments": {
                    "comment_id": "c1", "text": "done <@111111>"
                }}
            })
            .to_string()
                + "\n",
        )
        .assert()
        .success()
        .stdout(predicates::str::contains("Comment c1 updated"));
}

#[tokio::test]
async fn mcp_comment_reply_resolves_display_name() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_team(&server).await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(body_partial_json(serde_json::json!({
            "comment": [{"text": "thanks "}, ada_tag()]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})))
        .expect(1)
        .mount(&server)
        .await;

    mcp(dir.path(), &server)
        .write_stdin(
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "clickup_comment_reply", "arguments": {
                    "comment_id": "c1", "text": "thanks @Ada Lovelace"
                }}
            })
            .to_string()
                + "\n",
        )
        .assert()
        .success();
}
