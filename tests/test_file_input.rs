use assert_cmd::Command;
use std::path::Path;
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
async fn test_task_create_description_from_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Multiline content that PowerShell would otherwise split at the boundary.
    let desc_path = dir.path().join("desc.md");
    std::fs::write(&desc_path, "First line\nSecond line\n").unwrap();

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/task"))
        .and(body_json(serde_json::json!({
            "name": "demo",
            "markdown_content": "First line\nSecond line"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "t1",
            "name": "demo"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "demo",
            "--description",
            &format!("@{}", desc_path.display()),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_comment_create_text_from_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    let text_path = dir.path().join("body.txt");
    std::fs::write(&text_path, "line one\nline two\n").unwrap();

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/comment"))
        .and(body_json(serde_json::json!({
            "comment_text": "line one\nline two"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c1"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--list",
            "list-1",
            "--text",
            &format!("@{}", text_path.display()),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_literal_at_is_escaped_with_double_at() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    // `@@everyone` must reach the API as the literal `@everyone`, NOT a file read.
    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/comment"))
        .and(body_json(serde_json::json!({
            "comment_text": "@everyone ship it"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--list",
            "list-1",
            "--text",
            "@@everyone ship it",
        ])
        .assert()
        .success();
}
