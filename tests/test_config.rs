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
