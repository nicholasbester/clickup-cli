use clickup_cli::auth_token::{AuthToken, TokenKind};
use clickup_cli::commands::auth::resolve_with;
use clickup_cli::config::{AuthConfig, Config, TokenStorage};
use clickup_cli::credentials::{clear_token, save_token, CredentialStore};
use clickup_cli::error::CliError;
use std::cell::RefCell;

#[derive(Default)]
struct MockStore {
    value: RefCell<Option<String>>,
    calls: RefCell<usize>,
    fail: bool,
}
impl CredentialStore for MockStore {
    fn get(&self) -> Result<Option<String>, CliError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CliError::ConfigError("mock locked".into()));
        }
        Ok(self.value.borrow().clone())
    }
    fn set(&self, value: &str) -> Result<(), CliError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CliError::ConfigError("mock locked".into()));
        }
        *self.value.borrow_mut() = Some(value.into());
        Ok(())
    }
    fn delete(&self) -> Result<(), CliError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CliError::ConfigError("mock locked".into()));
        }
        *self.value.borrow_mut() = None;
        Ok(())
    }
}
fn token(kind: TokenKind) -> AuthToken {
    AuthToken {
        kind,
        token: "fixture-secret".into(),
    }
}
fn config() -> Config {
    Config {
        auth: AuthConfig {
            token: "config-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn precedence_is_flag_personal_env_oauth_env_file_and_keychain_is_lazy() {
    let store = MockStore::default();
    for (flag, kind, personal, oauth, expected_kind, source, value) in [
        (
            Some("flag"),
            None,
            Some("env"),
            Some("oauth"),
            TokenKind::Personal,
            "flag",
            "flag",
        ),
        (
            Some("flag"),
            Some(TokenKind::Oauth),
            Some("env"),
            Some("oauth"),
            TokenKind::Oauth,
            "flag",
            "flag",
        ),
        (
            None,
            None,
            Some("env"),
            Some("oauth"),
            TokenKind::Personal,
            "CLICKUP_TOKEN",
            "env",
        ),
        (
            None,
            None,
            Some(""),
            Some("oauth"),
            TokenKind::Oauth,
            "CLICKUP_OAUTH_TOKEN",
            "oauth",
        ),
        (
            None,
            None,
            None,
            Some(""),
            TokenKind::Personal,
            "file",
            "config-fixture",
        ),
    ] {
        let resolved = resolve_with(
            flag,
            kind,
            personal.map(str::to_owned),
            oauth.map(str::to_owned),
            || Ok(config()),
            &store,
        )
        .unwrap();
        assert_eq!(resolved.token.kind, expected_kind);
        assert_eq!(resolved.token.token, value);
        assert_eq!(resolved.source, source);
    }
    let resolved = resolve_with(
        Some("flag"),
        None,
        None,
        None,
        || panic!("flag must not load config"),
        &store,
    )
    .unwrap();
    assert_eq!(resolved.token.token, "flag");
    assert_eq!(*store.calls.borrow(), 0);
    assert!(resolve_with(Some(""), None, None, None, || Ok(config()), &store).is_err());
}

#[test]
fn keychain_roundtrip_switch_to_file_and_logout_preserve_settings() {
    let store = MockStore::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut config = config();
    config.defaults.workspace_id = Some("123".into());
    config.git.enabled = Some(false);
    save_token(
        &mut config,
        &path,
        token(TokenKind::Oauth),
        TokenStorage::Keychain,
        &store,
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("fixture-secret"));
    let loaded = Config::load_from(&path).unwrap();
    let resolved = resolve_with(None, None, None, None, || Ok(loaded), &store).unwrap();
    assert_eq!(resolved.source, "keychain");
    assert_eq!(resolved.token.kind, TokenKind::Oauth);
    assert_eq!(resolved.token.token, "fixture-secret");
    save_token(
        &mut config,
        &path,
        token(TokenKind::Personal),
        TokenStorage::File,
        &store,
    )
    .unwrap();
    assert!(store.value.borrow().is_none());
    assert_eq!(config.auth.kind, TokenKind::Personal);
    save_token(
        &mut config,
        &path,
        token(TokenKind::Oauth),
        TokenStorage::Keychain,
        &store,
    )
    .unwrap();
    clear_token(&mut config, &path, &store).unwrap();
    assert!(store.value.borrow().is_none());
    let loaded = Config::load_from(&path).unwrap();
    assert!(loaded.auth.token.is_empty());
    assert_eq!(loaded.auth.kind, TokenKind::Personal);
    assert_eq!(loaded.auth.storage, TokenStorage::File);
    assert_eq!(loaded.defaults.workspace_id.as_deref(), Some("123"));
    assert_eq!(loaded.git.enabled, Some(false));
}

#[test]
fn storage_failure_never_falls_back_or_erases_marker() {
    let store = MockStore {
        fail: true,
        ..Default::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut config = config();
    config.save_to(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(save_token(
        &mut config,
        &path,
        token(TokenKind::Oauth),
        TokenStorage::Keychain,
        &store
    )
    .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    config.auth.storage = TokenStorage::Keychain;
    assert!(clear_token(&mut config, &path, &store).is_err());
    assert_eq!(config.auth.storage, TokenStorage::Keychain);
}

#[test]
fn config_save_failure_restores_previous_keychain_record() {
    let store = MockStore::default();
    let dir = tempfile::tempdir().unwrap();
    let mut config = config();
    store.set("previous-record").unwrap();
    assert!(save_token(
        &mut config,
        dir.path(),
        token(TokenKind::Oauth),
        TokenStorage::Keychain,
        &store
    )
    .is_err());
    assert_eq!(store.get().unwrap().as_deref(), Some("previous-record"));
    assert_eq!(config.auth.token, "config-fixture");
    store.delete().unwrap();
    assert!(save_token(
        &mut config,
        dir.path(),
        token(TokenKind::Oauth),
        TokenStorage::Keychain,
        &store
    )
    .is_err());
    assert!(store.get().unwrap().is_none());
}

#[test]
fn missing_malformed_or_conflicting_keychain_credentials_fail_closed() {
    let store = MockStore::default();
    let auth = AuthConfig {
        kind: TokenKind::Oauth,
        storage: TokenStorage::Keychain,
        ..Default::default()
    };
    assert!(auth.load_token(&store).is_err());
    store.set("invalid-secret-json").unwrap();
    let err = auth.load_token(&store).unwrap_err();
    assert!(!err.to_string().contains("invalid-secret-json"));
    store
        .set(&serde_json::to_string(&token(TokenKind::Personal)).unwrap())
        .unwrap();
    assert!(auth.load_token(&store).is_err());
    store
        .set(&serde_json::to_string(&token(TokenKind::Oauth)).unwrap())
        .unwrap();
    assert!(auth.load_token(&store).is_ok());
    let conflicted = AuthConfig {
        token: "file-value".into(),
        ..auth
    };
    assert!(conflicted.load_token(&store).is_err());
}

#[test]
fn tokens_are_redacted_and_invalid_header_values_are_rejected() {
    for value in ["", "has space", "evil\r\nHeader:bad", "Bearer already", "é"] {
        assert!(AuthToken::personal(value).header().is_err());
    }
    assert!(!format!("{:?}", token(TokenKind::Oauth)).contains("fixture-secret"));
    assert!(!format!("{:?}", config()).contains("config-fixture"));
    assert!(token(TokenKind::Oauth).header().unwrap().is_sensitive());
}
