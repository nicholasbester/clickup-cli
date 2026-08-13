//! Regression tests for issue #96: `field unset <task_id> <field_uuid>`
//! (arguments in the natural API order, but swapped relative to the CLI's
//! `<FIELD_ID> [TASK_ID]` signature) built a URL with the IDs reversed and
//! surfaced ClickUp's misleading 401 "Team not authorized". Field IDs are
//! always UUIDs and task IDs never are, so the swap is detected and the
//! arguments are reinterpreted, with a breadcrumb on stderr.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIELD_UUID: &str = "b955c4dc-b8a8-48d8-a0c6-b4200788a683";
const TASK_ID: &str = "86czkjbtq";

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
async fn field_unset_swapped_args_are_corrected_with_breadcrumb() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // The mock only matches the CORRECT url (task in the task slot).
    Mock::given(method("DELETE"))
        .and(path_matcher(format!(
            "/v2/task/{}/field/{}",
            TASK_ID, FIELD_UUID
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    // Issue #96 invocation: task first, field UUID second.
    clickup(dir.path(), &server)
        .args(["field", "unset", TASK_ID, FIELD_UUID])
        .assert()
        .success()
        .stderr(predicates::str::contains("swapped"));
}

#[tokio::test]
async fn field_set_swapped_args_are_corrected_with_breadcrumb() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher(format!(
            "/v2/task/{}/field/{}",
            TASK_ID, FIELD_UUID
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["field", "set", TASK_ID, "--value", "5", FIELD_UUID])
        .assert()
        .success()
        .stderr(predicates::str::contains("swapped"));
}

#[tokio::test]
async fn field_unset_documented_order_unchanged_no_breadcrumb() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path_matcher(format!(
            "/v2/task/{}/field/{}",
            TASK_ID, FIELD_UUID
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["field", "unset", FIELD_UUID, TASK_ID])
        .assert()
        .success()
        .stderr(predicates::str::contains("swapped").not());
}

/// Two UUIDs is ambiguous — no reinterpretation, args used as given.
#[tokio::test]
async fn field_unset_both_uuids_not_swapped() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let other_uuid = "0f6e2a4e-1111-4222-8333-444455556666";

    Mock::given(method("DELETE"))
        .and(path_matcher(format!(
            "/v2/task/{}/field/{}",
            other_uuid, FIELD_UUID
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["field", "unset", FIELD_UUID, other_uuid])
        .assert()
        .success()
        .stderr(predicates::str::contains("swapped").not());
}

use predicates::prelude::*;
