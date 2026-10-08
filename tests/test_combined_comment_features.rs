//! Final union: previews, offset-based spacing and corrected mentions share
//! actual CLI/MCP payloads. Only loopback HTTP and isolated fake credentials.
use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const URL: &str = "https://example.com/a_b?x=1&y=2#@Same";

fn tag(id: i64, name: &str) -> Value {
    json!({"type":"tag", "text":format!("@{name}"), "user":{"id":id}})
}
fn preview(mode: &str) -> Value {
    if mode == "inline" {
        json!({"type":"link_mention","link_mention":{"url":URL}})
    } else {
        json!({"type":"bookmark","bookmark":{"service":"custom","id":URL,"url":URL},
            "attributes":{"body-type":"table","unfurled":"true"}})
    }
}
fn cli(dir: &TempDir, server: &MockServer, oauth: bool) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_CONFIG", dir.path().join("missing-config.toml"))
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_WORKSPACE", "99")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID")
        .env_remove("CLICKUP_TOKEN")
        .env_remove("CLICKUP_OAUTH_TOKEN")
        .env_remove("CLICKUP_MCP_PROFILE")
        .env_remove("CLICKUP_MCP_GROUPS")
        .env_remove("CLICKUP_MCP_TOOLS")
        .env(
            if oauth {
                "CLICKUP_OAUTH_TOKEN"
            } else {
                "CLICKUP_TOKEN"
            },
            "fixture",
        );
    cmd
}

async fn matrix(text: &str, markdown: bool, expected_ops: impl Fn(&str) -> Vec<Value>) {
    for oauth in [false, true] {
        for mcp in [false, true] {
            for mode in ["inline", "card"] {
                for route in ["task", "list", "view", "update", "reply"] {
                    // MCP create supports tasks; list/view creation is CLI-only.
                    if mcp && matches!(route, "list" | "view") {
                        continue;
                    }
                    let dir = TempDir::new().unwrap();
                    let server = MockServer::start().await;
                    let auth = if oauth { "Bearer fixture" } else { "fixture" };
                    Mock::given(method("GET"))
                        .and(path("/v2/team"))
                        .and(header("authorization", auth))
                        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"teams":[
                            {"id":"other","members":[{"user":{"id":999,"username":"A&B"}}]},
                            {"id":"99","members":[
                                {"user":{"id":41,"username":"A&B"}},
                                {"user":{"id":42,"username":"Ada"}},
                                {"user":{"id":51,"username":"Same"}},
                                {"user":{"id":52,"username":"Same"}}
                            ]}
                        ]})))
                        .expect(1)
                        .mount(&server)
                        .await;
                    let (action, verb, endpoint) = match route {
                        "task" => ("create", "POST", "/v2/task/t1/comment"),
                        "list" => ("create", "POST", "/v2/list/l1/comment"),
                        "view" => ("create", "POST", "/v2/view/v1/comment"),
                        "update" => ("update", "PUT", "/v2/comment/c1"),
                        _ => ("reply", "POST", "/v2/comment/c1/reply"),
                    };
                    Mock::given(method(verb))
                        .and(path(endpoint))
                        .and(header("authorization", auth))
                        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"c1"})))
                        .expect(1)
                        .mount(&server)
                        .await;
                    let mut expected = json!({"comment":expected_ops(mode)});
                    if route == "task" {
                        expected["notify_all"] = json!(false);
                    }
                    if mcp {
                        let mut args = json!({"text":text,"markdown":markdown,"link_preview":mode});
                        let (key, id) = match route {
                            "task" => ("task_id", "t1"),
                            "list" => ("list_id", "l1"),
                            "view" => ("view_id", "v1"),
                            _ => ("comment_id", "c1"),
                        };
                        args[key] = json!(id);
                        if route == "task" {
                            args["notify_all"] = json!(false);
                        }
                        let out = cli(&dir, &server, oauth).args(["mcp","serve"])
                            .write_stdin(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                                "params":{"name":format!("clickup_comment_{action}"),"arguments":args}}).to_string()+"\n")
                            .assert().success().get_output().stdout.clone();
                        let rpc: Value = serde_json::from_slice(&out).unwrap();
                        assert!(rpc.get("error").is_none(), "{rpc}");
                        assert_ne!(rpc["result"]["isError"], true, "{rpc}");
                    } else {
                        let mut cmd = cli(&dir, &server, oauth);
                        cmd.args(["comment", action]);
                        match route {
                            "task" => {
                                cmd.args(["--task", "t1"]);
                            }
                            "list" => {
                                cmd.args(["--list", "l1"]);
                            }
                            "view" => {
                                cmd.args(["--view", "v1"]);
                            }
                            _ => {
                                cmd.arg("c1");
                            }
                        }
                        // CLI @file syntax requires @@ for a literal leading @.
                        let cli_text = if text.starts_with('@') {
                            format!("@{text}")
                        } else {
                            text.to_owned()
                        };
                        cmd.args(["--text", &cli_text, "--link-preview", mode]);
                        if markdown {
                            cmd.arg("--markdown");
                        }
                        cmd.assert().success();
                    }
                    let requests = server.received_requests().await.unwrap();
                    assert_eq!(requests.len(), 2);
                    let actual = requests
                        .iter()
                        .find(|r| r.method.as_str() == verb)
                        .unwrap()
                        .body_json::<Value>()
                        .unwrap();
                    assert_eq!(
                        actual, expected,
                        "oauth={oauth} mcp={mcp} mode={mode} route={route} markdown={markdown}"
                    );
                    tokio::task::spawn_blocking(move || drop(server))
                        .await
                        .unwrap();
                }
            }
        }
    }
}

#[tokio::test]
async fn combined_plain_previews_mentions_and_unicode_all_entrypoints() {
    matrix(&format!("@A&B <@42> {URL}\n\n@Same @界"), false, |mode| {
        vec![
            tag(41, "A&B"),
            json!({"text":" "}),
            tag(42, "Ada"),
            json!({"text":" "}),
            preview(mode),
            json!({"text":"\n\n@Same @界"}),
        ]
    })
    .await;
}

#[tokio::test]
async fn combined_offset_spacing_fragmented_mentions_and_previews_all_entrypoints() {
    matrix(
        "@A&amp;B <@42> https://example.com/a_b?x=1&amp;y=2#@Same\n\n@Same @界",
        true,
        |mode| {
            vec![
                tag(41, "A&B"),
                json!({"text":" "}),
                tag(42, "Ada"),
                json!({"text":" "}),
                preview(mode),
                json!({"text":"\n"}),
                json!({"text":"\n"}),
                json!({"text":"@Same @界"}),
                json!({"text":"\n"}),
            ]
        },
    )
    .await;
}

#[tokio::test]
async fn combined_code_style_and_list_boundaries_all_entrypoints() {
    matrix("`@A&B https://code.test`\n\n**@A\\&B** https://example.com/a_b?x=1&amp;y=2#@Same\n\n- <@42>\n- @Same\n\n@界", true, |mode| vec![
        json!({"text":"@A&B https://code.test","attributes":{"code":true}}),
        json!({"text":"\n"}),json!({"text":"\n"}),tag(41,"A&B"),json!({"text":" "}),preview(mode),
        json!({"text":"\n"}),json!({"text":"\n"}),tag(42,"Ada"),
        json!({"text":"\n","attributes":{"list":{"list":"bullet"}}}),json!({"text":"@Same"}),
        json!({"text":"\n","attributes":{"list":{"list":"bullet"}}}),
        json!({"text":"\n"}),json!({"text":"@界"}),json!({"text":"\n"}),
    ]).await;
}

#[tokio::test]
async fn invalid_preview_rejected_before_mention_roster_or_post() {
    for oauth in [false, true] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        cli(&dir, &server, oauth)
            .args([
                "comment",
                "reply",
                "c1",
                "--text",
                "@@Ada",
                "--link-preview",
                "bad",
            ])
            .assert()
            .failure();
        let out = cli(&dir, &server, oauth)
            .args(["mcp", "serve"])
            .write_stdin(
                json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"clickup_comment_reply",
            "arguments":{"comment_id":"c1","text":"@Ada","link_preview":"bad"}}})
                .to_string()
                    + "\n",
            )
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let rpc: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(rpc["result"]["isError"], true);
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
