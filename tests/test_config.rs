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
fn test_config_save_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep").join("nested").join("config.toml");
    let config = Config {
        auth: clickup_cli::config::AuthConfig {
            token: "pk_test".into(),
        },
        defaults: clickup_cli::config::DefaultsConfig::default(),
    };
    config.save_to(&path).unwrap();
    assert!(path.exists());
}
