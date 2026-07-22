use assert_cmd::Command;
use predicates::prelude::*;
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
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

#[tokio::test]
async fn test_task_get_csv_strips_optional_marker_from_field_name() {
    // `--fields "custom_id?"` uses the optional-omission marker recognized by
    // compact_items (MCP/json-compact path). The CLI's csv/table branches must
    // strip the trailing `?` too: the header and the looked-up key should be
    // the clean "custom_id", not the literal "custom_id?".
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "custom_id": "PROJ-42"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "get",
            "abc123",
            "--fields",
            "custom_id?",
            "--output",
            "csv",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("custom_id?").not())
        .stdout(predicates::str::contains("custom_id"))
        .stdout(predicates::str::contains("PROJ-42"));
}
