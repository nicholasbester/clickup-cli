//! Request-contract regressions only. Synthetic empty responses do not verify
//! Enterprise response shape, timestamps, or pagination semantics (#59).
use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn command(dir: &TempDir, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_MCP_PROFILE")
        .env_remove("CLICKUP_MCP_GROUPS")
        .env_remove("CLICKUP_MCP_TOOLS");
    cmd
}

async fn expect_body(server: &MockServer, body: Value) {
    Mock::given(method("POST"))
        .and(path("/v3/workspaces/99/auditlogs"))
        .and(body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn cli_audit_filters_and_direction_use_documented_request_contract() {
    for (input, wire) in [
        ("before", "before"),
        ("after", "after"),
        ("PREVIOUS", "before"),
        ("NEXT", "after"),
    ] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        expect_body(
            &server,
            json!({
                "applicability": "auth-and-security",
                "filter": {"eventType": "USER_LOGIN", "eventStatus": "failed",
                    "userId": ["12", "13"], "userEmail": ["test@example.com"],
                    "startTime": 1718754539000_i64, "endTime": 1727221739000_i64},
                "pagination": {"pageRows": 10, "pageTimestamp": 1727221739000_i64,
                    "pageDirection": wire}
            }),
        )
        .await;
        command(&dir, &server)
            .args([
                "audit-log",
                "query",
                "--applicability",
                "auth-and-security",
                "--event-type",
                "USER_LOGIN",
                "--event-status",
                "failed",
                "--user-id",
                "12",
                "--user-id",
                "13",
                "--user-email",
                "test@example.com",
                "--start-time",
                "1718754539000",
                "--end-time",
                "1727221739000",
                "--page-rows",
                "10",
                "--page-timestamp",
                "1727221739000",
                "--page-direction",
                input,
                "--output",
                "json",
            ])
            .assert()
            .success()
            .stdout("[]\n");
    }
}

#[tokio::test]
async fn mcp_audit_filters_and_direction_use_documented_request_contract() {
    for (input, wire) in [
        ("before", "before"),
        ("after", "after"),
        ("PREVIOUS", "before"),
        ("NEXT", "after"),
    ] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        expect_body(
            &server,
            json!({
                "applicability": "auth-and-security",
                "filter": {"eventType": "USER_LOGIN", "eventStatus": "failed",
                    "userId": ["12", "13"], "userEmail": ["test@example.com"],
                    "startTime": 1718754539000_i64, "endTime": 1727221739000_i64},
                "pagination": {"pageRows": 10, "pageTimestamp": 1727221739000_i64,
                    "pageDirection": wire}
            }),
        )
        .await;
        let rpc = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "clickup_audit_log_query", "arguments": {
                "applicability": "auth-and-security", "event_type": "USER_LOGIN",
                "event_status": "failed", "user_id": ["12", "13"],
                "user_email": ["test@example.com"], "start_time": 1718754539000_i64,
                "end_time": 1727221739000_i64, "page_rows": 10,
                "page_timestamp": 1727221739000_i64, "page_direction": input}}});
        let output = command(&dir, &server)
            .args(["mcp", "serve"])
            .write_stdin(format!("{rpc}\n"))
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let response: Value = serde_json::from_slice(&output).unwrap();
        assert_ne!(response["result"]["isError"], true, "{response}");
        let result: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(result["items"], json!([]));
        // Envelope preserves the caller's spelling; only the wire is normalized.
        assert_eq!(result["pagination"]["page_direction"], input);
    }
}

#[tokio::test]
async fn omitted_audit_filters_and_pagination_stay_omitted() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    expect_body(&server, json!({"applicability": "auth-and-security"})).await;
    command(&dir, &server)
        .args([
            "audit-log",
            "query",
            "--applicability",
            "auth-and-security",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout("[]\n");
}

#[test]
fn audit_help_and_mcp_schema_advertise_current_categories() {
    let help = Command::cargo_bin("clickup-cli")
        .unwrap()
        .args(["audit-log", "query", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(help).unwrap();
    let tools = clickup_cli::mcp::tool_list();
    let tool = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "clickup_audit_log_query")
        .unwrap();
    let props = &tool["inputSchema"]["properties"];
    let categories = [
        "agent-settings-activity",
        "auth-and-security",
        "custom-fields",
        "hierarchy-activity",
        "user-activity",
        "other-activity",
    ];
    assert_eq!(props["applicability"]["enum"], json!(categories));
    for category in categories {
        assert!(help.contains(category));
    }
    assert!(!help.contains("WORKSPACE, TEAMS, USERS"));
    assert!(help.contains("USER_LOGIN"));
    assert!(help.contains("failed"));
    assert_eq!(
        props["page_direction"]["enum"],
        json!(["before", "after", "NEXT", "PREVIOUS"])
    );
    assert_eq!(props["start_time"]["type"], "integer");
}
