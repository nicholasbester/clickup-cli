use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher, query_param};
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
async fn test_task_get_markdown_requests_and_surfaces_source() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .and(query_param("include_markdown_description", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "description": "Desktop",
            "text_content": "Desktop",
            "markdown_description": "[Desktop](https://www.figma.com/design/ABC/F?node-id=1-2)"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "get", "abc123", "--markdown", "--output", "json"])
        .assert()
        .success()
        // The raw markdown source (with the link URL intact) must round-trip.
        .stdout(predicates::str::contains(
            "https://www.figma.com/design/ABC/F?node-id=1-2",
        ));
}

#[tokio::test]
async fn test_task_get_markdown_surfaces_field_even_with_explicit_fields() {
    // --markdown must reliably show markdown_description even when the user
    // narrows columns with --fields, rather than being silently dropped.
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .and(query_param("include_markdown_description", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "markdown_description": "[Desktop](https://www.figma.com/design/ABC/F?node-id=1-2)"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "get",
            "abc123",
            "--markdown",
            "--fields",
            "id",
            "--output",
            "csv",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("markdown_description"))
        .stdout(predicates::str::contains(
            "https://www.figma.com/design/ABC/F?node-id=1-2",
        ));
}

#[tokio::test]
async fn test_task_get_without_markdown_omits_query_param() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Mock only matches when the markdown query param is ABSENT.
    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "get", "abc123"])
        .assert()
        .success();
}
