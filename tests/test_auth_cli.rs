use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cli(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .env("CLICKUP_CONFIG", dir.path().join("config.toml"))
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path())
        .env("APPDATA", dir.path())
        .env_remove("CLICKUP_TOKEN")
        .env_remove("CLICKUP_OAUTH_TOKEN")
        .env_remove("CLICKUP_OAUTH_CLIENT_ID")
        .env_remove("CLICKUP_OAUTH_CLIENT_SECRET")
        .env_remove("CLICKUP_API_URL")
        .env_remove("CLICKUP_WORKSPACE");
    cmd
}

#[test]
fn login_without_app_credentials_gives_byo_instructions_and_never_saves() {
    let dir = TempDir::new().unwrap();
    cli(&dir)
        .args(["auth", "login", "--no-browser"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Bring your own ClickUp OAuth app"))
        .stderr(predicate::str::contains("CLICKUP_OAUTH_CLIENT_SECRET"));
    assert!(!dir.path().join("config.toml").exists());
    cli(&dir)
        .args(["--token-kind", "oauth", "auth", "check"])
        .assert()
        .failure();
}

#[test]
fn no_browser_timeout_and_port_conflict_leave_credentials_unchanged() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[auth]\ntoken='previous-fixture'\n").unwrap();
    let before = std::fs::read(&config).unwrap();
    cli(&dir)
        .args([
            "auth",
            "login",
            "--no-browser",
            "--client-id",
            "fixture-id",
            "--client-secret",
            "fixture-secret",
            "--redirect-port",
            "0",
            "--login-timeout",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("https://app.clickup.com/api?"))
        .stderr(predicate::str::contains("timed out"))
        .stderr(predicate::str::contains("fixture-secret").not());
    assert_eq!(std::fs::read(&config).unwrap(), before);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    cli(&dir)
        .args([
            "auth",
            "login",
            "--no-browser",
            "--client-id",
            "fixture",
            "--client-secret",
            "fixture-secret",
            "--redirect-port",
            &port,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Could not bind OAuth loopback port",
        ));
    assert_eq!(std::fs::read(&config).unwrap(), before);
}

#[cfg(not(feature = "keyring"))]
#[test]
fn lean_build_keyring_flag_fails_before_browser_or_persistence() {
    let dir = TempDir::new().unwrap();
    cli(&dir)
        .args([
            "auth",
            "login",
            "--keyring",
            "--no-browser",
            "--client-id",
            "fixture",
            "--client-secret",
            "fixture",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Enable the keyring Cargo feature"))
        .stderr(predicate::str::contains("Authorize this app in your browser").not());
    assert!(!dir.path().join("config.toml").exists());
}

#[tokio::test]
async fn effective_status_cli_mcp_env_flag_and_file_headers_agree() {
    for (source, kind, expected) in [
        ("file", "oauth", "Bearer fixture"),
        ("CLICKUP_OAUTH_TOKEN", "oauth", "Bearer fixture"),
        ("CLICKUP_TOKEN", "personal", "fixture"),
        ("flag", "oauth", "Bearer fixture"),
    ] {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/user"))
            .and(header("authorization", expected))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"user":{"id":1,"username":"Fixture User"}})),
            )
            .expect(2)
            .mount(&server)
            .await;
        std::fs::write(
            dir.path().join("config.toml"),
            "[auth]\ntoken='fixture'\nkind='oauth'\n[defaults]\nworkspace_id='123'\n",
        )
        .unwrap();
        for mcp in [false, true] {
            let mut cmd = cli(&dir);
            cmd.env("CLICKUP_API_URL", server.uri());
            match source {
                "CLICKUP_TOKEN" => {
                    cmd.env("CLICKUP_TOKEN", "fixture")
                        .env("CLICKUP_OAUTH_TOKEN", "loser");
                }
                "CLICKUP_OAUTH_TOKEN" => {
                    cmd.env("CLICKUP_OAUTH_TOKEN", "fixture");
                }
                "flag" => {
                    cmd.args(["--token", "fixture", "--token-kind", "oauth"])
                        .env("CLICKUP_TOKEN", "loser");
                }
                _ => {}
            }
            if mcp {
                cmd.args(["mcp", "serve"]).write_stdin("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"clickup_whoami\",\"arguments\":{}}}\n")
                    .assert().success().stdout(predicate::str::contains("Fixture User"));
            } else {
                cmd.args(["auth", "status", "--output", "json"])
                    .assert()
                    .success()
                    .stdout(predicate::str::contains(format!("\"kind\": \"{kind}\"")))
                    .stdout(predicate::str::contains(format!(
                        "\"source\": \"{source}\""
                    )))
                    .stdout(predicate::str::contains("Fixture User"))
                    .stdout(predicate::str::contains("fixture").not());
            }
        }
    }
}

#[test]
fn logout_clears_fixture_only_preserves_defaults_and_does_not_claim_revocation() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "[auth]\ntoken='fixture'\nkind='oauth'\n[defaults]\nworkspace_id='123'\n[git]\nenabled=false").unwrap();
    cli(&dir)
        .args(["auth", "logout"])
        .env("CLICKUP_TOKEN", "env-fixture")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Environment/flag tokens are unchanged",
        ))
        .stderr(predicate::str::contains("Revoke OAuth app access"));
    let config = clickup_cli::config::Config::load_from(&file).unwrap();
    assert!(config.auth.token.is_empty());
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(!saved.contains("kind ="));
    assert!(!saved.contains("storage ="));
    assert_eq!(
        config.auth.kind,
        clickup_cli::auth_token::TokenKind::Personal
    );
    assert_eq!(config.defaults.workspace_id.as_deref(), Some("123"));
    assert_eq!(config.git.enabled, Some(false));
}

#[tokio::test]
async fn personal_setup_still_uses_bare_header_and_saves_personal_kind() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .and(header("authorization", "pk_fixture"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"user":{"username":"Fixture"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/team"))
        .and(header("authorization", "pk_fixture"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"teams":[{"id":"123","name":"Fixture"}]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    cli(&dir)
        .args(["setup", "--token", "pk_fixture"])
        .env("CLICKUP_API_URL", server.uri())
        .assert()
        .success();
    let config = clickup_cli::config::Config::load_from(&dir.path().join("config.toml")).unwrap();
    assert_eq!(config.auth.token, "pk_fixture");
    assert_eq!(
        config.auth.kind,
        clickup_cli::auth_token::TokenKind::Personal
    );
    assert_eq!(config.defaults.workspace_id.as_deref(), Some("123"));
}

#[test]
fn agent_config_init_records_explicit_oauth_kind_and_setup_rejects_wrong_kind() {
    let dir = TempDir::new().unwrap();
    cli(&dir)
        .args([
            "agent-config",
            "init",
            "--token",
            "fixture",
            "--token-kind",
            "oauth",
            "--workspace",
            "123",
        ])
        .assert()
        .success();
    let project =
        clickup_cli::config::Config::load_from(&dir.path().join(".clickup.toml")).unwrap();
    assert_eq!(project.auth.kind, clickup_cli::auth_token::TokenKind::Oauth);
    assert_eq!(project.auth.token, "fixture");
    cli(&dir)
        .args(["setup", "--token", "fixture", "--token-kind", "oauth"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("setup configures personal tokens"));
}

#[tokio::test]
async fn ancestor_keychain_marker_selects_project_config_without_native_lookup_when_flag_overrides()
{
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("nested/deeper");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        dir.path().join(".clickup.toml"),
        "[auth]\nkind='oauth'\nstorage='keychain'\n[defaults]\nworkspace_id='project'",
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/team/project/seats"))
        .and(header("authorization", "flag-fixture"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;
    cli(&dir)
        .current_dir(&nested)
        .env_remove("CLICKUP_CONFIG")
        .env("CLICKUP_API_URL", server.uri())
        .args(["--token", "flag-fixture", "workspace", "seats"])
        .assert()
        .success();
}
