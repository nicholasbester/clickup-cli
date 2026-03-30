use clickup_cli::output::{flatten_value, OutputConfig};
use serde_json::json;

#[test]
fn test_flatten_null() {
    assert_eq!(flatten_value(None), "-");
    assert_eq!(flatten_value(Some(&json!(null))), "-");
}

#[test]
fn test_flatten_string() {
    assert_eq!(flatten_value(Some(&json!("hello"))), "hello");
}

#[test]
fn test_flatten_number() {
    assert_eq!(flatten_value(Some(&json!(42))), "42");
}

#[test]
fn test_flatten_bool() {
    assert_eq!(flatten_value(Some(&json!(true))), "true");
}

#[test]
fn test_flatten_status_object() {
    let val = json!({"status": "in progress", "color": "#abc"});
    assert_eq!(flatten_value(Some(&val)), "in progress");
}

#[test]
fn test_flatten_priority_object() {
    let val = json!({"priority": "high", "color": "#red"});
    assert_eq!(flatten_value(Some(&val)), "high");
}

#[test]
fn test_flatten_assignees_array() {
    let val = json!([{"username": "Nick"}, {"username": "Bob"}]);
    assert_eq!(flatten_value(Some(&val)), "Nick, Bob");
}

#[test]
fn test_flatten_empty_array() {
    let val = json!([]);
    assert_eq!(flatten_value(Some(&val)), "-");
}

#[test]
fn test_flatten_object_with_name() {
    let val = json!({"name": "My Space", "id": "123"});
    assert_eq!(flatten_value(Some(&val)), "My Space");
}

#[test]
fn test_output_config_parses_fields() {
    let config = OutputConfig::from_cli("table", &Some("id, name, status".into()), false, false);
    assert_eq!(
        config.fields,
        Some(vec![
            "id".to_string(),
            "name".to_string(),
            "status".to_string()
        ])
    );
}

#[test]
fn test_output_config_no_fields() {
    let config = OutputConfig::from_cli("json", &None, false, false);
    assert_eq!(config.fields, None);
}

#[test]
fn test_flatten_due_date_ms_timestamp() {
    // 2026-03-17 00:00:00 UTC as Unix ms = 1773705600000
    let val = json!("1773705600000");
    let result = flatten_value(Some(&val));
    assert_eq!(result, "2026-03-17", "Expected 2026-03-17, got: {}", result);
}

#[test]
fn test_flatten_normal_string_not_converted() {
    let val = json!("hello world");
    assert_eq!(flatten_value(Some(&val)), "hello world");
}
