//! #130: folder hierarchy across CLI, MCP, models, and output modes.
//! HTTP is mocked; these tests do not establish live workspace capabilities.
use assert_cmd::Command;
use clickup_cli::{mcp::tool_list, models::folder::Folder};
use serde_json::{json, Value};
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cli(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd
}

fn mcp(dir: &Path, server: &MockServer, tool: &str, args: Value) -> Value {
    let output = cli(dir, server)
        .args(["mcp", "serve"])
        .write_stdin(
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": tool, "arguments": args}
            })
            .to_string()
                + "\n",
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rpc: Value = serde_json::from_slice(&output).unwrap();
    assert_ne!(rpc["result"]["isError"], true, "{rpc}");
    assert!(rpc.get("error").is_none(), "{rpc}");
    serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn folders() -> Value {
    json!([
        {"id": "457", "name": "Top", "task_count": "3", "lists": []},
        {"id": "458", "name": "Child", "parent_folder": "457", "task_count": "2",
         "lists": [{"id": "10", "name": "Work"}], "archived": false,
         "space": {"id": "789"}, "future_field": {"keep": true}},
        {"id": "459", "name": "Other", "parent_folder": "999", "task_count": "0"},
        {"id": "460", "name": "Null parent", "parent_folder": null, "lists": null}
    ])
}

async fn mock_list(server: &MockServer, archived: bool) {
    Mock::given(method("GET"))
        .and(path("/v2/space/789/folder"))
        .and(query_param("archived", archived.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"folders": folders()})))
        .mount(server)
        .await;
}

#[tokio::test]
async fn cli_and_mcp_create_send_exact_parent_folder_id_or_omit_it() {
    let dir = TempDir::new().unwrap();
    for parent in [None, Some("457")] {
        let server = MockServer::start().await;
        let mut body = json!({"name": "New"});
        if let Some(id) = parent {
            body["parent_folder_id"] = json!(id);
        }
        Mock::given(method("POST"))
            .and(path("/v2/space/789/folder"))
            .and(body_json(&body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": "458", "name": "New"})),
            )
            .expect(2)
            .mount(&server)
            .await;
        let mut cmd = cli(dir.path(), &server);
        cmd.args(["folder", "create", "--space", "789", "--name", "New"]);
        if let Some(id) = parent {
            cmd.args(["--parent", id]);
        }
        cmd.assert().success();
        body["space_id"] = json!("789");
        let result = mcp(dir.path(), &server, "clickup_folder_create", body);
        assert_eq!(result[0]["id"], "458");
    }
}

#[tokio::test]
async fn list_compact_and_mcp_preserve_counts_and_expose_parent() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_list(&server, false).await;
    let output = cli(dir.path(), &server)
        .args([
            "folder",
            "list",
            "--space",
            "789",
            "--output",
            "json-compact",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        result,
        mcp(
            dir.path(),
            &server,
            "clickup_folder_list",
            json!({"space_id": "789"})
        )
    );
    assert_eq!(result[0]["parent_folder"], "-");
    assert_eq!(
        result[1],
        json!({"id": "458", "name": "Child", "parent_folder": "457", "task_count": "2", "list_count": "1"})
    );
    assert_eq!(result[2]["list_count"], "0");
    assert_eq!(result[3]["parent_folder"], "-");
}

#[tokio::test]
async fn cli_and_mcp_parent_filter_handles_matches_empty_and_archived() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_list(&server, true).await;
    for (parent, count) in [("457", 1), ("absent", 0)] {
        let output = cli(dir.path(), &server)
            .args([
                "folder",
                "list",
                "--space",
                "789",
                "--archived",
                "--parent",
                parent,
                "--output",
                "json-compact",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let result: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(result.as_array().unwrap().len(), count);
        assert_eq!(
            result,
            mcp(
                dir.path(),
                &server,
                "clickup_folder_list",
                json!({"space_id": "789", "archived": true, "parent_folder_id": parent})
            )
        );
        if count == 1 {
            assert_eq!(result[0]["id"], "458");
        }
    }
    // Parent filtering is local; no invented API query parameter is sent.
    for request in server.received_requests().await.unwrap() {
        assert_eq!(request.url.query(), Some("archived=true"));
    }
}

#[tokio::test]
async fn list_raw_json_preserves_api_fields_and_quiet_fields_csv_still_work() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_list(&server, false).await;
    let base = ["folder", "list", "--space", "789", "--parent", "457"];
    let output = cli(dir.path(), &server)
        .args(base)
        .args(["--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: Value = serde_json::from_slice(&output).unwrap();
    let mut expected = folders()[1].clone();
    expected["list_count"] = json!(1);
    assert_eq!(result, json!([expected]));
    cli(dir.path(), &server)
        .args(base)
        .arg("--quiet")
        .assert()
        .success()
        .stdout("458\n");
    cli(dir.path(), &server)
        .args(base)
        .args([
            "--output",
            "csv",
            "--fields",
            "id,parent_folder",
            "--no-header",
        ])
        .assert()
        .success()
        .stdout("458,457\n");
    let output = cli(dir.path(), &server)
        .args(base)
        .args(["--output", "json-compact", "--fields", "parent_folder"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!([{"parent_folder": "457"}])
    );
}

#[tokio::test]
async fn get_parent_compact_mcp_and_raw_json() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    for folder in folders().as_array().unwrap() {
        let id = folder["id"].as_str().unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/v2/folder/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(folder))
            .mount(&server)
            .await;
        let output = cli(dir.path(), &server)
            .args(["folder", "get", id, "--output", "json-compact"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let result: Value = serde_json::from_slice(&output).unwrap();
        let parent = folder["parent_folder"].as_str().unwrap_or("-");
        assert_eq!(result[0]["parent_folder"], parent);
        assert_eq!(
            mcp(
                dir.path(),
                &server,
                "clickup_folder_get",
                json!({"folder_id": id})
            )[0]["parent_folder"],
            parent
        );
        let output = cli(dir.path(), &server)
            .args(["folder", "get", id, "--output", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let result: Value = serde_json::from_slice(&output).unwrap();
        for (key, value) in folder.as_object().unwrap() {
            assert_eq!(&result[0][key], value);
        }
    }
}

#[tokio::test]
async fn list_and_get_table_show_parent_column() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    mock_list(&server, false).await;
    Mock::given(method("GET"))
        .and(path("/v2/folder/458"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&folders()[1]))
        .mount(&server)
        .await;
    for args in [
        vec!["folder", "list", "--space", "789", "--parent", "457"],
        vec!["folder", "get", "458"],
    ] {
        cli(dir.path(), &server)
            .args(args)
            .assert()
            .success()
            .stdout(predicates::str::contains("parent_folder"))
            .stdout(predicates::str::contains("457"));
    }
}

#[test]
fn model_accepts_subfolder_missing_and_null_parent() {
    for folder in folders().as_array().unwrap() {
        let parsed: Folder = serde_json::from_value(folder.clone()).unwrap();
        assert_eq!(
            parsed.parent_folder.as_deref(),
            folder["parent_folder"].as_str()
        );
    }
}

#[test]
fn mcp_schema_advertises_optional_parent_only_for_create_and_list() {
    let tools = tool_list();
    for (name, required) in [
        ("clickup_folder_create", json!(["space_id", "name"])),
        ("clickup_folder_list", json!(["space_id"])),
    ] {
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert_eq!(
            tool["inputSchema"]["properties"]["parent_folder_id"]["type"],
            "string"
        );
        assert_eq!(tool["inputSchema"]["required"], required);
    }
    let update = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "clickup_folder_update")
        .unwrap();
    assert!(update["inputSchema"]["properties"]
        .get("parent_folder_id")
        .is_none());
}

#[tokio::test]
async fn rename_still_sends_only_name_and_delete_uses_existing_endpoint() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/folder/458"))
        .and(body_json(json!({"name": "Renamed"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id": "458", "name": "Renamed", "parent_folder": "457"})),
        )
        .expect(2)
        .mount(&server)
        .await;
    cli(dir.path(), &server)
        .args(["folder", "update", "458", "--name", "Renamed"])
        .assert()
        .success();
    assert_eq!(
        mcp(
            dir.path(),
            &server,
            "clickup_folder_update",
            json!({"folder_id": "458", "name": "Renamed"})
        )[0]["name"],
        "Renamed"
    );
    Mock::given(method("DELETE"))
        .and(path("/v2/folder/458"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(2)
        .mount(&server)
        .await;
    cli(dir.path(), &server)
        .args(["folder", "delete", "458"])
        .assert()
        .success();
    assert!(mcp(
        dir.path(),
        &server,
        "clickup_folder_delete",
        json!({"folder_id": "458"})
    )["message"]
        .as_str()
        .unwrap()
        .contains("deleted"));
}
