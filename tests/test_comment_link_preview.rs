use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const URL: &str = "https://github.com/org/repo/pull/1";

fn clickup(dir: &TempDir, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

async fn expect_body(server: &MockServer, verb: &str, endpoint: &str, body: Value) {
    Mock::given(method(verb))
        .and(path(endpoint))
        .and(body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "c2"})))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn cli_link_preview_is_wired_on_create_update_and_reply() {
    let mention = json!({"type": "link_mention", "link_mention": {"url": URL}});
    for (args, verb, endpoint) in [
        (
            vec!["create", "--list", "l1"],
            "POST",
            "/v2/list/l1/comment",
        ),
        (vec!["update", "c1"], "PUT", "/v2/comment/c1"),
        (vec!["reply", "c1"], "POST", "/v2/comment/c1/reply"),
    ] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        expect_body(&server, verb, endpoint, json!({"comment": [mention]})).await;
        clickup(&dir, &server)
            .arg("comment")
            .args(args)
            .args(["--text", URL, "--link-preview", "inline"])
            .assert()
            .success();
    }
}

#[tokio::test]
async fn mcp_link_preview_is_wired_on_create_update_and_reply() {
    let card = json!({
        "type": "bookmark",
        "bookmark": {"service": "custom", "id": URL, "url": URL},
        "attributes": {"body-type": "table", "unfurled": "true"}
    });
    for (tool, verb, endpoint) in [
        ("clickup_comment_create", "POST", "/v2/task/t1/comment"),
        ("clickup_comment_update", "PUT", "/v2/comment/c1"),
        ("clickup_comment_reply", "POST", "/v2/comment/c1/reply"),
    ] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        expect_body(
            &server,
            verb,
            endpoint,
            json!({"comment": [card], "assignee": 7}),
        )
        .await;
        clickup(&dir, &server)
            .args(["mcp", "serve"])
            .write_stdin(
                json!({
                    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": {"name": tool, "arguments": {
                        "task_id": "t1", "comment_id": "c1", "text": URL,
                        "link_preview": "card", "assignee": 7
                    }}
                })
                .to_string()
                    + "\n",
            )
            .assert()
            .success()
            .stdout(predicates::str::contains("\"isError\":true").not());
    }
}

#[tokio::test]
async fn invalid_link_preview_fails_without_an_http_request() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    clickup(&dir, &server)
        .args([
            "comment",
            "reply",
            "c1",
            "--text",
            "hi",
            "--link-preview",
            "bogus",
        ])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("inline, card"));
    clickup(&dir, &server)
        .args(["mcp", "serve"])
        .write_stdin(
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "clickup_comment_reply", "arguments": {
                    "comment_id": "c1", "text": "hi", "link_preview": "bogus"
                }}
            })
            .to_string()
                + "\n",
        )
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "link_preview must be inline or card",
        ));
    assert!(server.received_requests().await.unwrap().is_empty());
}
