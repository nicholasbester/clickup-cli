use clickup_cli::mcp::compact_custom_fields;
use serde_json::json;

#[test]
fn compact_custom_fields_promotes_type_config_options() {
    let fields = vec![json!({
        "id": "e61c8995",
        "name": "Stage",
        "type": "drop_down",
        "required": false,
        "type_config": {
            "options": [
                {
                    "id": "option-1",
                    "name": "Ready",
                    "color": "#2ecd6f",
                    "orderindex": 0
                },
                {
                    "id": "option-2",
                    "name": "Blocked",
                    "color": "#e50000",
                    "orderindex": 1
                }
            ]
        }
    })];

    let result = compact_custom_fields(&fields);

    assert_eq!(result[0]["id"], json!("e61c8995"));
    assert_eq!(result[0]["type"], json!("drop_down"));
    assert_eq!(result[0]["required"], json!("false"));
    assert_eq!(result[0]["options"][0]["id"], json!("option-1"));
    assert_eq!(result[0]["options"][0]["name"], json!("Ready"));
    assert_eq!(result[0]["options"][1]["id"], json!("option-2"));
}

#[test]
fn compact_custom_fields_omits_options_when_type_config_has_no_options() {
    let fields = vec![json!({
        "id": "text-field",
        "name": "Notes",
        "type": "text",
        "required": true,
        "type_config": {}
    })];

    let result = compact_custom_fields(&fields);

    assert_eq!(result[0]["id"], json!("text-field"));
    assert_eq!(result[0]["type"], json!("text"));
    assert_eq!(result[0]["required"], json!("true"));
    assert!(result[0].get("options").is_none());
}

#[test]
fn compact_custom_fields_keeps_empty_options_array() {
    let fields = vec![json!({
        "id": "labels-field",
        "name": "Labels",
        "type": "labels",
        "required": false,
        "type_config": {
            "options": []
        }
    })];

    let result = compact_custom_fields(&fields);

    assert_eq!(result[0]["options"], json!([]));
}
