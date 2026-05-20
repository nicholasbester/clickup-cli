//! Integration tests for `attachment list`.
//!
//! ClickUp has no dedicated list-attachments endpoint — the attachments array
//! comes inline on GET /v2/task/{id}. These tests lock in that path so we
//! don't regress to the broken v3 workspace-scoped path (see issue #9).

use assert_cmd::Command;
use wiremock::matchers::{method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clickup(server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID")
        // Force non-repo cwd so branch-detect stays out of it.
        .current_dir(std::env::temp_dir());
    cmd
}

#[tokio::test]
async fn attachment_list_hits_v2_task_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "Test",
            "attachments": [
                {"id": "att1", "title": "foo.txt", "url": "https://example/foo.txt", "date": "1"},
                {"id": "att2", "title": "bar.png", "url": "https://example/bar.png", "date": "2"}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(&server)
        .args(["--output", "json", "attachment", "list", "--task", "abc123"])
        .assert()
        .success();
}

#[tokio::test]
async fn attachment_list_empty_when_no_attachments_field() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc123"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(&server)
        .args(["-q", "attachment", "list", "--task", "abc123"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}
