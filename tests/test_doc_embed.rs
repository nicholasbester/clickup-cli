use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clickup(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "123")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

/// Drop a tiny file into `dir`; the CLI does not validate image content.
fn write_image(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"not-a-real-png").unwrap();
    path
}

#[tokio::test]
async fn embed_image_uploads_then_appends_markdown() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let img = write_image(dir.path(), "i.png");

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "a1",
            "url": "https://cdn.test/i.png"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Alt defaults to the file name; embed_snippet wraps the markdown in newlines.
    Mock::given(method("PUT"))
        .and(path_matcher("/v3/workspaces/123/docs/d1/pages/p1"))
        .and(body_json(serde_json::json!({
            "content": "\n![i.png](https://cdn.test/i.png)\n",
            "content_edit_mode": "append"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "p1",
            "name": "page"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "doc",
            "embed-image",
            "d1",
            "p1",
            img.to_str().unwrap(),
            "--via-task",
            "t1",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("https://cdn.test/i.png"));
}

#[tokio::test]
async fn embed_image_edit_failure_reports_uploaded_url() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let img = write_image(dir.path(), "i.png");

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "a1",
            "url": "https://cdn.test/i.png"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v3/workspaces/123/docs/d1/pages/p1"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "err": "Internal server error"
        })))
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "doc",
            "embed-image",
            "d1",
            "p1",
            img.to_str().unwrap(),
            "--via-task",
            "t1",
        ])
        .assert()
        .failure()
        .code(5)
        .stderr(predicates::str::contains("Retry without re-uploading"))
        .stderr(predicates::str::contains("https://cdn.test/i.png"));
}

#[test]
fn embed_image_without_task_explains_host_task_requirement() {
    let dir = TempDir::new().unwrap();
    let img = write_image(dir.path(), "i.png");

    // No CLICKUP_TASK_ID, branch detection disabled -> task resolution must
    // fail deterministically. Token comes from env because execute() resolves
    // it before require_task.
    Command::cargo_bin("clickup-cli")
        .unwrap()
        .current_dir(dir.path())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "123")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID")
        .args(["doc", "embed-image", "d1", "p1", img.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("host task"))
        .stderr(predicates::str::contains("--via-task"));
}
