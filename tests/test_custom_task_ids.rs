//! Regression tests for issue #98: task-scoped commands outside `task`/`doc`
//! dropped the `custom_task_ids=true&team_id=<ws>` pair for custom-format
//! task IDs (`PROJ-42`), so ClickUp answered with the misleading
//! 401 "Team not authorized". Every command that embeds a resolved task ID
//! in a `/v2/task/{id}/...` path must append the pair when the ID is custom
//! and omit it otherwise.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIELD_UUID: &str = "b955c4dc-b8a8-48d8-a0c6-b4200788a683";

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

async fn mock_custom(server: &MockServer, m: &str, path: &str, body: serde_json::Value) {
    Mock::given(method(m))
        .and(path_matcher(path.to_string()))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn field_unset_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "DELETE",
        &format!("/v2/task/PROJ-42/field/{}", FIELD_UUID),
        serde_json::json!({}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["field", "unset", FIELD_UUID, "PROJ-42"])
        .assert()
        .success();
}

#[tokio::test]
async fn field_set_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "POST",
        &format!("/v2/task/PROJ-42/field/{}", FIELD_UUID),
        serde_json::json!({}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["field", "set", FIELD_UUID, "--value", "5", "PROJ-42"])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_list_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "GET",
        "/v2/task/PROJ-42/comment",
        serde_json::json!({"comments": []}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["comment", "list", "--task", "PROJ-42"])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_create_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "POST",
        "/v2/task/PROJ-42/comment",
        serde_json::json!({"id": "c1"}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["comment", "create", "--task", "PROJ-42", "--text", "hi"])
        .assert()
        .success();
}

#[tokio::test]
async fn checklist_create_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "POST",
        "/v2/task/PROJ-42/checklist",
        serde_json::json!({"checklist": {"id": "cl1", "name": "x"}}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["checklist", "create", "--task", "PROJ-42", "--name", "x"])
        .assert()
        .success();
}

#[tokio::test]
async fn member_list_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "GET",
        "/v2/task/PROJ-42/member",
        serde_json::json!({"members": []}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["member", "list", "--task", "PROJ-42"])
        .assert()
        .success();
}

#[tokio::test]
async fn attachment_list_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_custom(
        &server,
        "GET",
        "/v2/task/PROJ-42",
        serde_json::json!({"attachments": []}),
    )
    .await;

    clickup(dir.path(), &server)
        .args(["attachment", "list", "--task", "PROJ-42"])
        .assert()
        .success();
}

#[tokio::test]
async fn attachment_upload_custom_id_appends_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "hello").unwrap();
    mock_custom(
        &server,
        "POST",
        "/v2/task/PROJ-42/attachment",
        serde_json::json!({"id": "a1"}),
    )
    .await;

    clickup(dir.path(), &server)
        .args([
            "attachment",
            "upload",
            file.to_str().unwrap(),
            "--task",
            "PROJ-42",
        ])
        .assert()
        .success();
}

/// Plain task IDs must NOT gain the pair — byte-identical requests to today.
#[tokio::test]
async fn field_unset_plain_id_omits_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path_matcher(format!(
            "/v2/task/86czkjbtq/field/{}",
            FIELD_UUID
        )))
        .and(query_param_is_missing("custom_task_ids"))
        .and(query_param_is_missing("team_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["field", "unset", FIELD_UUID, "86czkjbtq"])
        .assert()
        .success();
}

/// A custom ID with no workspace configured must fail with a clear error
/// (the pair needs team_id), not silently send a request ClickUp rejects.
#[tokio::test]
async fn custom_id_without_workspace_errors_clearly() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_WORKSPACE")
        .env_remove("CLICKUP_TASK_ID")
        // Isolate from any real global config.
        .env("XDG_CONFIG_HOME", dir.path())
        .env("HOME", dir.path());

    // The exact message depends on whether a config file exists at all
    // ("Not configured" from Config::load vs "No default workspace" from
    // resolve_workspace) — what matters is a local, actionable failure
    // pointing at setup instead of a misleading API 401.
    cmd.args(["field", "unset", FIELD_UUID, "PROJ-42"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("setup"));
}
