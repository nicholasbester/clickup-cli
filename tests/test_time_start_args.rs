//! Regression tests for issue #91: `time create`/`time update` panicked on
//! `--start` because the global pagination `--start` (i64) collided with the
//! subcommands' local `--start` (String). The pagination pair `--start` /
//! `--start-id` is now scoped to the comment commands that actually use it.

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
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

/// The clap command definition must be internally consistent. This is the
/// startup-time check that would have caught the global/local `--start`
/// type collision before release.
#[test]
fn clap_command_definition_is_consistent() {
    use clap::CommandFactory;
    clickup_cli::Cli::command().debug_assert();
}

#[tokio::test]
async fn time_create_accepts_start() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/team/99/time_entries"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"id": "et1", "start": "1753800000000", "duration": 3600000}
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "time",
            "create",
            "--start",
            "1753800000000",
            "--duration",
            "3600000",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("et1"));
}

#[tokio::test]
async fn time_update_accepts_start() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path_matcher("/v2/team/99/time_entries/et1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"id": "et1", "start": "1753800000000"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["time", "update", "et1", "--start", "1753800000000"])
        .assert()
        .success()
        .stdout(predicates::str::contains("et1"));
}

/// The pagination boundary pair must keep working on the comment commands
/// after being de-globalized.
#[tokio::test]
async fn comment_list_still_passes_start_boundary_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(query_param("start", "1700000000000"))
        .and(query_param("start_id", "c9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [{"id": "c10", "comment_text": "hi", "date": "1700000000001"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "list",
            "--task",
            "t1",
            "--start",
            "1700000000000",
            "--start-id",
            "c9",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("c10"));
}

#[tokio::test]
async fn comment_replies_still_pass_start_boundary_pair() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(query_param("start", "1700000000000"))
        .and(query_param("start_id", "c9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [{"id": "c10", "comment_text": "re", "date": "1700000000001"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "replies",
            "c1",
            "--start",
            "1700000000000",
            "--start-id",
            "c9",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("c10"));
}

/// `--start` is no longer a global flag, so commands without their own
/// `--start` must reject it instead of silently accepting a no-op.
#[tokio::test]
async fn time_list_rejects_start_flag() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    clickup(dir.path(), &server)
        .args(["time", "list", "--start", "1700000000000"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unexpected argument"));
}
