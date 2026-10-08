//! Issue #135: Delete Dependency reads the relationship from the query string,
//! while Add Dependency still reads JSON. Inspect actual HTTP requests from
//! both public entry points, including reserved characters and custom IDs.

use assert_cmd::Command;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn command(dir: &TempDir, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

fn call_mcp(cmd: &mut Command, tool: &str, args: Value) -> Value {
    let input = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": args}
    });
    let output = cmd
        .args(["mcp", "serve"])
        .write_stdin(format!("{input}\n"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn assert_query(request: &Request, expected: &[(&str, &str)]) {
    let pairs: Vec<_> = request.url.query_pairs().collect();
    assert_eq!(pairs.len(), expected.len(), "URL: {}", request.url);
    let actual: BTreeMap<_, _> = pairs.into_iter().collect();
    let expected: BTreeMap<_, _> = expected
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    assert_eq!(actual, expected);
}

async fn dependency_requests(mcp: bool, remove: bool) {
    for direction in ["depends_on", "dependency_of"] {
        for (task_id, resolved, custom) in [
            ("abc123", "abc123", false),
            ("CU-abc123", "abc123", false),
            ("PROJ-42", "PROJ-42", true),
        ] {
            // The second value exercises encoding rather than ID validation:
            // delimiters must remain one value, never inject/override queries.
            for other in ["PROJ-43", "OTHER &team_id=evil+/#?%é"] {
                let dir = TempDir::new().unwrap();
                let server = MockServer::start().await;
                Mock::given(method(if remove { "DELETE" } else { "POST" }))
                    .and(path(format!("/v2/task/{resolved}/dependency")))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                    .expect(1)
                    .mount(&server)
                    .await;

                let mut cmd = command(&dir, &server);
                if mcp {
                    let mut args = json!({"task_id": task_id});
                    args[direction] = json!(other);
                    let tool = if remove {
                        "clickup_task_remove_dep"
                    } else {
                        "clickup_task_add_dep"
                    };
                    let response = call_mcp(&mut cmd, tool, args);
                    assert!(response.get("error").is_none(), "{response}");
                    assert_ne!(response["result"]["isError"], true, "{response}");
                    assert!(response["result"]["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains(if remove {
                            "Dependency removed"
                        } else {
                            "Dependency added"
                        }));
                } else {
                    cmd.args([
                        "task",
                        if remove { "remove-dep" } else { "add-dep" },
                        task_id,
                        &format!("--{}", direction.replace('_', "-")),
                        other,
                    ])
                    .assert()
                    .success()
                    .stdout(predicates::str::contains(if remove {
                        "Dependency removed"
                    } else {
                        "Dependency added"
                    }));
                }

                let requests = server.received_requests().await.unwrap();
                assert_eq!(requests.len(), 1);
                let request = &requests[0];
                let mut expected = Vec::new();
                if custom {
                    expected.extend([("custom_task_ids", "true"), ("team_id", "99")]);
                }
                if remove {
                    expected.push((direction, other));
                    assert!(request.body.is_empty(), "DELETE must have no JSON body");
                } else {
                    assert_eq!(
                        request.body_json::<Value>().unwrap(),
                        json!({direction: other})
                    );
                }
                assert_query(request, &expected);
            }
        }
    }
}

#[tokio::test]
async fn cli_remove_dependency_query_contract() {
    dependency_requests(false, true).await;
}

#[tokio::test]
async fn mcp_remove_dependency_query_contract() {
    dependency_requests(true, true).await;
}

#[tokio::test]
async fn cli_add_dependency_keeps_json_body() {
    dependency_requests(false, false).await;
}

#[tokio::test]
async fn mcp_add_dependency_keeps_json_body() {
    dependency_requests(true, false).await;
}

#[tokio::test]
async fn cli_remove_dependency_rejects_missing_or_conflicting_directions() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    command(&dir, &server)
        .args(["task", "remove-dep", "abc123"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "Specify --depends-on or --dependency-of",
        ));
    command(&dir, &server)
        .args([
            "task",
            "remove-dep",
            "abc123",
            "--depends-on",
            "other",
            "--dependency-of",
            "other",
        ])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("cannot be used with"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn mcp_remove_dependency_rejects_invalid_directions_without_http() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    for mut args in [
        json!({}),
        json!({"depends_on": "a", "dependency_of": "b"}),
        json!({"depends_on": 42}),
        json!({"dependency_of": null}),
        json!({"depends_on": "a", "dependency_of": false}),
        json!({"depends_on": [], "dependency_of": "b"}),
    ] {
        args["task_id"] = json!("abc123");
        let response = call_mcp(&mut command(&dir, &server), "clickup_task_remove_dep", args);
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Specify exactly one of depends_on or dependency_of as a string"));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn remove_dependency_custom_id_requires_workspace_without_http() {
    let dir = TempDir::new().unwrap();
    // Select this fixture explicitly: CLICKUP_CONFIG overrides project discovery.
    // A token without a workspace keeps the missing-workspace check self-contained.
    let config_path = dir.path().join(".clickup.toml");
    std::fs::write(
        &config_path,
        "[auth]\ntoken = \"pk_test\"\n",
    )
    .unwrap();
    let server = MockServer::start().await;
    command(&dir, &server)
        .env("CLICKUP_CONFIG", &config_path)
        .env_remove("CLICKUP_WORKSPACE")
        .args(["task", "remove-dep", "PROJ-42", "--depends-on", "PROJ-43"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("workspace"));
    let response = call_mcp(
        command(&dir, &server)
            .env("CLICKUP_CONFIG", &config_path)
            .env_remove("CLICKUP_WORKSPACE"),
        "clickup_task_remove_dep",
        json!({"task_id": "PROJ-42", "dependency_of": "PROJ-43"}),
    );
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("workspace"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn remove_dependency_custom_id_honors_workspace_override() {
    for mcp in [false, true] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/v2/task/PROJ-42/dependency"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        if mcp {
            let response = call_mcp(
                &mut command(&dir, &server),
                "clickup_task_remove_dep",
                json!({"task_id": "PROJ-42", "depends_on": "PROJ-43", "team_id": "123"}),
            );
            assert!(response.get("error").is_none(), "{response}");
            assert_ne!(response["result"]["isError"], true, "{response}");
        } else {
            command(&dir, &server)
                .args([
                    "--workspace",
                    "123",
                    "task",
                    "remove-dep",
                    "PROJ-42",
                    "--depends-on",
                    "PROJ-43",
                ])
                .assert()
                .success();
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].body.is_empty());
        assert_query(
            &requests[0],
            &[
                ("custom_task_ids", "true"),
                ("team_id", "123"),
                ("depends_on", "PROJ-43"),
            ],
        );
    }
}
