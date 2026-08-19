//! Tests for `--subtasks` on `task list`/`task search`, `--include-closed`
//! on `task search`, and real `--all` pagination on both commands via the
//! shared page walker (previously `task search` ignored `--all` entirely —
//! contradicting the documented contract — and `task list` hand-rolled an
//! uncapped loop).

use assert_cmd::Command;
use predicates::prelude::*;
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

fn task_page(ids: &[&str], last_page: bool) -> serde_json::Value {
    serde_json::json!({
        "tasks": ids.iter().map(|id| serde_json::json!({"id": id, "name": id})).collect::<Vec<_>>(),
        "last_page": last_page,
    })
}

#[tokio::test]
async fn task_list_subtasks_flag_sends_param() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/list/9/task"))
        .and(query_param("subtasks", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1"], true)))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "list", "--list", "9", "--subtasks"])
        .assert()
        .success()
        .stdout(predicates::str::contains("t1"));
}

#[tokio::test]
async fn task_search_subtasks_and_include_closed_send_params() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .and(query_param("subtasks", "true"))
        .and(query_param("include_closed", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1"], true)))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "search", "--subtasks", "--include-closed"])
        .assert()
        .success()
        .stdout(predicates::str::contains("t1"));
}

/// `task search --all` must walk pages (documented contract, previously a
/// single request with client-side truncation).
#[tokio::test]
async fn task_search_all_walks_pages() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1"], false)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t2"], true)))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "search", "--all"])
        .assert()
        .success()
        .stdout(predicates::str::contains("t1"))
        .stdout(predicates::str::contains("t2"));
}

/// `task search --all --limit N` stops walking once N items are collected.
#[tokio::test]
async fn task_search_all_with_limit_stops_early() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1", "t2"], false)))
        .expect(1)
        .mount(&server)
        .await;
    // Page 1 must never be requested: limit satisfied by page 0.
    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t3"], true)))
        .expect(0)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "search", "--all", "--limit", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("t1"))
        .stdout(predicates::str::contains("t2"))
        .stdout(predicates::str::contains("t3").not());
}

/// `task list --all` still walks pages after the refactor onto the shared
/// walker (which also gains the documented 100-page hard cap).
#[tokio::test]
async fn task_list_all_walks_pages() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/list/9/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1"], false)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_matcher("/v2/list/9/task"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t2"], true)))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["task", "list", "--list", "9", "--all"])
        .assert()
        .success()
        .stdout(predicates::str::contains("t1"))
        .stdout(predicates::str::contains("t2"));
}

/// MCP clickup_task_list gains a `subtasks` arg.
#[tokio::test]
async fn mcp_task_list_subtasks_sends_param() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/list/9/task"))
        .and(query_param("subtasks", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_page(&["t1"], true)))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd.write_stdin(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "clickup_task_list", "arguments": {"list_id": "9", "subtasks": true}}
        })
        .to_string()
            + "\n",
    )
    .assert()
    .success()
    .stdout(predicates::str::contains("t1"));
}
