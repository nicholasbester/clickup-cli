//! Regression coverage for GH #126: `--due-date YYYY-MM-DD` must resolve to a
//! moment inside the requested *local* calendar day, not midnight UTC. ClickUp
//! snaps date-only due dates to 04:00 in the workspace timezone, so midnight
//! UTC lands on the previous day for every user west of UTC.
//!
//! The binary is run with `TZ=America/New_York` (UTC-5 in December) so the
//! expected millisecond values below are deterministic regardless of the CI
//! host's own timezone. chrono honours `TZ` on Unix only, so the local-time
//! cases are gated to Unix; the offset-carrying ISO cases run everywhere.

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
        .env("CLICKUP_WORKSPACE", "99")
        .env("TZ", "America/New_York")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

async fn expect_task_create(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/task"))
        .and(body_json(body))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "id": "t1", "name": "x" })),
        )
        .expect(1)
        .mount(server)
        .await;
}

// 2026-12-31T00:00:00-05:00 = 1_798_693_200_000 (from the issue report).
// Local noon on that day is +12h.
const NY_2026_12_31_NOON_MS: i64 = 1_798_693_200_000 + 12 * 3_600_000;
// 2026-12-31T09:30:00-05:00
const NY_2026_12_31_0930_MS: i64 = 1_798_693_200_000 + (9 * 3_600 + 30 * 60) * 1_000;
// 2026-12-31T12:00:00Z
const UTC_2026_12_31_NOON_MS: i64 = 1_798_718_400_000;

#[cfg(unix)]
#[tokio::test]
async fn date_only_due_date_is_local_noon_not_utc_midnight() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    expect_task_create(
        &server,
        serde_json::json!({ "name": "x", "due_date": NY_2026_12_31_NOON_MS }),
    )
    .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "x",
            "--due-date",
            "2026-12-31",
        ])
        .assert()
        .success();
}

#[cfg(unix)]
#[tokio::test]
async fn naive_datetime_is_interpreted_in_local_time_and_sets_due_date_time() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    expect_task_create(
        &server,
        serde_json::json!({
            "name": "x",
            "due_date": NY_2026_12_31_0930_MS,
            "due_date_time": true
        }),
    )
    .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "x",
            "--due-date",
            "2026-12-31T09:30",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn datetime_with_explicit_offset_ignores_local_timezone() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    expect_task_create(
        &server,
        serde_json::json!({
            "name": "x",
            "due_date": UTC_2026_12_31_NOON_MS,
            "due_date_time": true
        }),
    )
    .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "x",
            "--due-date",
            "2026-12-31T12:00:00Z",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn raw_unix_ms_is_passed_through_unchanged() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    expect_task_create(
        &server,
        serde_json::json!({ "name": "x", "due_date": 1_798_693_200_000_i64 }),
    )
    .await;

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "x",
            "--due-date",
            "1798693200000",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn invalid_due_date_fails_locally_with_format_guidance() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // No mock mounted: any request would 404 and fail the test differently.

    clickup(dir.path(), &server)
        .args([
            "task",
            "create",
            "--list",
            "list-1",
            "--name",
            "x",
            "--due-date",
            "31/12/2026",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Invalid date '31/12/2026'"))
        .stderr(predicates::str::contains("YYYY-MM-DD"))
        .stderr(predicates::str::contains("YYYY-MM-DDTHH:MM"));
}

#[cfg(unix)]
#[tokio::test]
async fn task_update_accepts_due_date() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path_matcher("/v2/task/abc123"))
        .and(body_json(
            serde_json::json!({ "due_date": NY_2026_12_31_NOON_MS }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "id": "abc123", "name": "x" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "update", "abc123", "--due-date", "2026-12-31"])
        .assert()
        .success();
}

#[cfg(unix)]
#[tokio::test]
async fn list_create_due_date_is_local_noon() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_matcher("/v2/folder/f1/list"))
        .and(body_json(
            serde_json::json!({ "name": "L", "due_date": NY_2026_12_31_NOON_MS }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "id": "l1", "name": "L" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "list",
            "create",
            "--folder",
            "f1",
            "--name",
            "L",
            "--due-date",
            "2026-12-31",
        ])
        .assert()
        .success();
}
