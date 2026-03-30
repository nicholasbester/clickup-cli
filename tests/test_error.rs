use clickup_cli::error::CliError;

#[test]
fn test_auth_error_exit_code() {
    let err = CliError::AuthError {
        message: "Token expired".into(),
    };
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn test_not_found_exit_code() {
    let err = CliError::NotFound {
        message: "Task not found".into(),
        resource_id: "abc123".into(),
    };
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn test_rate_limited_exit_code() {
    let err = CliError::RateLimited {
        message: "Rate limited".into(),
        retry_after: Some(30),
    };
    assert_eq!(err.exit_code(), 4);
}

#[test]
fn test_server_error_exit_code() {
    let err = CliError::ServerError {
        message: "Internal error".into(),
    };
    assert_eq!(err.exit_code(), 5);
}

#[test]
fn test_auth_error_hint() {
    let err = CliError::AuthError {
        message: "Unauthorized".into(),
    };
    assert!(err.hint().unwrap().contains("clickup setup"));
}

#[test]
fn test_not_found_hint_includes_id() {
    let err = CliError::NotFound {
        message: "Not found".into(),
        resource_id: "abc123".into(),
    };
    assert!(err.hint().unwrap().contains("abc123"));
}

#[test]
fn test_config_error_hint() {
    let err = CliError::ConfigError("Not configured".into());
    assert!(err.hint().unwrap().contains("clickup setup"));
}

#[test]
fn test_error_display() {
    let err = CliError::AuthError {
        message: "Token expired".into(),
    };
    assert_eq!(format!("{}", err), "Token expired");
}
