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

#[tokio::test]
async fn test_doc_edit_page_content_from_file() {
    // The @file convention is wired uniformly via a clap value_parser, so a
    // newly-covered flag (doc edit-page --content) reads from a file too.
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    let content_path = dir.path().join("page.md");
    std::fs::write(&content_path, "# Heading\n\nBody paragraph.\n").unwrap();

    Mock::given(method("PUT"))
        .and(path_matcher("/v3/workspaces/99/docs/doc-1/pages/page-1"))
        .and(body_json(serde_json::json!({
            "content": "# Heading\n\nBody paragraph.",
            "content_edit_mode": "replace"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "page-1"})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "doc",
            "edit-page",
            "doc-1",
            "page-1",
            "--content",
            &format!("@{}", content_path.display()),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_missing_at_file_errors_with_escape_hint() {
    // A bare @path that doesn't resolve to a file must fail with a message that
    // points the user at the @@ escape (so an @mention typed verbatim is
    // self-correcting). Resolution happens at parse time, before any network
    // call, so no server is needed.
    let dir = tempfile::TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID")
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "demo",
            "--description",
            "@everyone",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("@@everyone"));
}
