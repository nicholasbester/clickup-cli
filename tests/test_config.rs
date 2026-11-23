use clickup_cli::config::Config;
use tempfile::TempDir;

fn with_test_config_dir<F: FnOnce(&std::path::Path)>(f: F) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    f(&config_path);
}

#[test]
fn test_config_save_and_load() {
    with_test_config_dir(|path| {
        let config = Config {
            auth: clickup_cli::config::AuthConfig {
                token: "pk_test_123".into(),
            },
            defaults: clickup_cli::config::DefaultsConfig {
                workspace_id: Some("12345".into()),
                output: None,
            },
            git: Default::default(),
        };
        config.save_to(path).unwrap();
        let loaded = Config::load_from(path).unwrap();
        assert_eq!(loaded.auth.token, "pk_test_123");
        assert_eq!(loaded.defaults.workspace_id, Some("12345".into()));
    });
}

#[test]
fn test_config_load_missing_file() {
    with_test_config_dir(|path| {
        let result = Config::load_from(path);
        assert!(result.is_err());
    });
}

#[test]
fn test_config_minimal_toml() {
    with_test_config_dir(|path| {
        std::fs::write(path, "[auth]\ntoken = \"pk_abc\"\n").unwrap();
        let config = Config::load_from(path).unwrap();
        assert_eq!(config.auth.token, "pk_abc");
        assert_eq!(config.defaults.workspace_id, None);
    });
}

#[test]
fn test_find_project_config_walks_up_to_root() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join(".clickup.toml"), "[auth]\ntoken = \"pk_root\"\n").unwrap();
    let nested = root.join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).unwrap();

    let found = Config::find_project_config(&nested).expect("should find ancestor config");
    assert_eq!(found, root.join(".clickup.toml"));
}

#[test]
fn test_find_project_config_prefers_nearest() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join(".clickup.toml"), "[auth]\ntoken = \"pk_root\"\n").unwrap();
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join(".clickup.toml"), "[auth]\ntoken = \"pk_sub\"\n").unwrap();
    let nested = sub.join("deeper");
    std::fs::create_dir_all(&nested).unwrap();

    let found = Config::find_project_config(&nested).expect("should find nearest config");
    assert_eq!(found, sub.join(".clickup.toml"));
}

#[test]
fn test_find_project_config_none_when_absent() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();
    assert!(Config::find_project_config(&nested).is_none());
}

#[test]
fn test_config_save_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep").join("nested").join("config.toml");
    let config = Config {
        auth: clickup_cli::config::AuthConfig {
            token: "pk_test".into(),
        },
        defaults: clickup_cli::config::DefaultsConfig::default(),
        git: Default::default(),
    };
    config.save_to(&path).unwrap();
    assert!(path.exists());
}

// ── Hardening (GH #155) ───────────────────────────────────────────────────────

fn sample_config(token: &str) -> Config {
    let mut config = Config::default();
    config.auth.token = token.into();
    config
}

#[cfg(unix)]
#[test]
fn test_config_save_is_private_even_over_a_world_readable_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[auth]\ntoken = \"old\"\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    sample_config("pk_new").save_to(&path).unwrap();

    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "config must be owner-only, got {mode:o}");
    assert_eq!(Config::load_from(&path).unwrap().auth.token, "pk_new");
    // No temp files left behind next to the config.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn test_config_save_refuses_to_replace_a_symlink() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("elsewhere.toml");
    std::fs::write(&target, "untouched").unwrap();
    let link = dir.path().join("config.toml");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let err = sample_config("pk_x").save_to(&link).unwrap_err();
    assert!(err.to_string().contains("symlink"), "{err}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched");
}

#[test]
fn test_config_parse_error_does_not_echo_the_token() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    // Unterminated string on the token line: toml's own error would quote it.
    std::fs::write(&path, "[auth]\ntoken = \"pk_SECRET_123\n").unwrap();
    let msg = Config::load_from(&path).unwrap_err().to_string();
    assert!(!msg.contains("pk_SECRET_123"), "token leaked: {msg}");
    assert!(msg.contains("line 2"), "location missing: {msg}");
}

#[test]
fn test_config_debug_redacts_token() {
    let rendered = format!("{:?}", sample_config("pk_SECRET_456"));
    assert!(!rendered.contains("pk_SECRET_456"), "{rendered}");
    assert!(rendered.contains("REDACTED"));
}

#[test]
fn test_config_without_auth_table_loads() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[defaults]\nworkspace_id = \"42\"\n").unwrap();
    let config = Config::load_from(&path).unwrap();
    assert!(config.auth.token.is_empty());
    assert_eq!(config.defaults.workspace_id.as_deref(), Some("42"));
}

fn clickup_in(dir: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env_remove("CLICKUP_TOKEN")
        .env_remove("CLICKUP_WORKSPACE");
    cmd
}

#[test]
fn test_agent_config_init_serializes_token_safely() {
    // A quote in the token used to be spliced straight into the TOML, breaking
    // the file (or letting a crafted value inject extra keys).
    let dir = TempDir::new().unwrap();
    clickup_in(dir.path())
        .args([
            "agent-config",
            "init",
            "--token",
            "pk_a\"b\\c",
            "--workspace",
            "99",
        ])
        .assert()
        .success();
    let config = Config::load_from(&dir.path().join(".clickup.toml")).unwrap();
    assert_eq!(config.auth.token, "pk_a\"b\\c");
    assert_eq!(config.defaults.workspace_id.as_deref(), Some("99"));
}

#[test]
fn test_agent_config_init_rejects_token_with_whitespace() {
    let dir = TempDir::new().unwrap();
    clickup_in(dir.path())
        .args(["agent-config", "init", "--token", "pk_a\nworkspace_id = 1"])
        .assert()
        .failure();
    assert!(!dir.path().join(".clickup.toml").exists());
}

#[test]
fn test_agent_config_init_falls_back_to_global_token_flag() {
    let dir = TempDir::new().unwrap();
    clickup_in(dir.path())
        .args(["--token", "pk_global", "agent-config", "init"])
        .assert()
        .success();
    let config = Config::load_from(&dir.path().join(".clickup.toml")).unwrap();
    assert_eq!(config.auth.token, "pk_global");
}

#[test]
fn test_agent_config_init_without_token_writes_loadable_file() {
    let dir = TempDir::new().unwrap();
    clickup_in(dir.path())
        .args(["agent-config", "init"])
        .assert()
        .success();
    let config = Config::load_from(&dir.path().join(".clickup.toml")).unwrap();
    assert!(config.auth.token.is_empty());
}
