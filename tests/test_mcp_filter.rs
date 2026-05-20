//! Integration tests for MCP tool classification and filtering.

use clickup_cli::mcp::classify::{classify, ALL_GROUPS};
use clickup_cli::mcp::filtered_tool_list;
use clickup_cli::mcp::tool_list;

#[test]
fn every_tool_classifies() {
    let tools = tool_list();
    let array = tools
        .as_array()
        .expect("tool_list must return a JSON array");
    assert!(!array.is_empty(), "tool_list is empty");

    let mut unclassified: Vec<String> = Vec::new();
    let mut unknown_group: Vec<(String, String)> = Vec::new();

    for tool in array {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .expect("each tool must have a string `name`");

        match classify(name) {
            None => unclassified.push(name.to_string()),
            Some(meta) => {
                if !ALL_GROUPS.contains(&meta.group) {
                    unknown_group.push((name.to_string(), meta.group.to_string()));
                }
            }
        }
    }

    assert!(
        unclassified.is_empty(),
        "unclassified tools (add to OVERRIDES or extend verb sets): {:?}",
        unclassified
    );
    assert!(
        unknown_group.is_empty(),
        "tools mapped to unknown groups: {:?}",
        unknown_group
    );
}

#[test]
fn expected_tool_count() {
    // Sanity check: we don't want a future refactor to silently drop tools.
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    assert_eq!(array.len(), 143, "tool count changed; update this test");
}

use clickup_cli::mcp::filter::{Filter, FilterError, Profile, RawFilter};

fn tool_names_in(filter: &Filter) -> Vec<String> {
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    array
        .iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
        .filter(|n| filter.allows(n))
        .collect()
}

#[test]
fn default_exposes_all_tools() {
    let filter = Filter::resolve(RawFilter::default()).unwrap();
    assert_eq!(filter.profile, Profile::All);
    assert_eq!(tool_names_in(&filter).len(), 143);
}

#[test]
fn read_profile_excludes_writes_and_destructives() {
    let raw = RawFilter {
        profile: Some("read".into()),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.iter().all(|n| {
        use clickup_cli::mcp::classify::{classify, Class};
        classify(n).map(|m| m.class == Class::Read).unwrap_or(false)
    }));
    assert!(names.contains(&"clickup_task_list".to_string()));
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(!names.contains(&"clickup_task_create".to_string()));
}

#[test]
fn safe_profile_excludes_destructives_only() {
    let raw = RawFilter {
        profile: Some("safe".into()),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.contains(&"clickup_task_create".to_string()));
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(!names.contains(&"clickup_list_remove_task".to_string()));
}

#[test]
fn read_only_flag_equivalent_to_profile_read() {
    let raw = RawFilter {
        read_only: true,
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    assert_eq!(filter.profile, Profile::Read);
}

#[test]
fn groups_filter_restricts_to_listed_groups() {
    let raw = RawFilter {
        groups: Some(vec!["task".into(), "comment".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.contains(&"clickup_task_get".to_string()));
    assert!(names.contains(&"clickup_comment_list".to_string()));
    assert!(!names.contains(&"clickup_chat_channel_list".to_string()));
    // group "task-type" is NOT in "task" — confirm the `task_type_list` tool is NOT included
    assert!(!names.contains(&"clickup_task_type_list".to_string()));
}

#[test]
fn exclude_groups_drops_listed_groups() {
    let raw = RawFilter {
        exclude_groups: Some(vec!["chat".into(), "audit-log".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(!names.iter().any(|n| n.starts_with("clickup_chat_")));
    assert!(!names.iter().any(|n| n.starts_with("clickup_audit_log_")));
    assert!(names.contains(&"clickup_task_list".to_string()));
}

#[test]
fn tools_filter_intersects_with_profile() {
    let raw = RawFilter {
        profile: Some("read".into()),
        tools: Some(vec!["clickup_task_get".into(), "clickup_task_list".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert_eq!(names.len(), 2);
}

#[test]
fn tool_excluded_by_profile_errors() {
    let raw = RawFilter {
        profile: Some("read".into()),
        tools: Some(vec!["clickup_task_delete".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::ToolExcludedByProfile { .. }));
}

#[test]
fn exclude_tools_drops_them() {
    let raw = RawFilter {
        exclude_tools: Some(vec!["clickup_task_delete".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(names.contains(&"clickup_task_create".to_string()));
}

#[test]
fn empty_final_set_errors() {
    let raw = RawFilter {
        groups: Some(vec!["task".into()]),
        exclude_groups: Some(vec!["task".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::EmptyFilter));
}

#[test]
fn read_only_plus_non_read_profile_errors() {
    let raw = RawFilter {
        profile: Some("safe".into()),
        read_only: true,
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::ConflictingProfile { .. }));
}

#[test]
fn unknown_profile_errors() {
    let raw = RawFilter {
        profile: Some("gibberish".into()),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::UnknownProfile { .. }));
}

#[test]
fn unknown_group_errors() {
    let raw = RawFilter {
        groups: Some(vec!["nope".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::UnknownGroup { .. }));
}

#[test]
fn unknown_tool_errors_with_hint() {
    let raw = RawFilter {
        tools: Some(vec!["clickup_task_lst".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    match err {
        FilterError::UnknownTool { name, suggestion } => {
            assert_eq!(name, "clickup_task_lst");
            assert_eq!(suggestion.as_deref(), Some("clickup_task_list"));
        }
        other => panic!("expected UnknownTool, got {:?}", other),
    }
}

#[test]
fn read_only_plus_profile_read_is_not_a_conflict() {
    // Setting both to the same effective value should harmonize, not error.
    let raw = RawFilter {
        profile: Some("read".into()),
        read_only: true,
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    assert_eq!(filter.profile, Profile::Read);
}

#[test]
fn filtered_tool_list_returns_only_allowed_tools() {
    let raw = RawFilter {
        profile: Some("read".into()),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let value = filtered_tool_list(&filter);
    let array = value.as_array().unwrap();
    for tool in array {
        let name = tool.get("name").unwrap().as_str().unwrap();
        assert!(filter.allows(name), "tool {} leaked past filter", name);
    }
    assert_eq!(array.len(), filter.allowed_count());
    assert!(
        !array
            .iter()
            .any(|t| t.get("name").and_then(|v| v.as_str()) == Some("clickup_task_delete")),
        "destructive tool leaked into read-profile filtered list"
    );
}

#[test]
fn task_search_exposes_filtered_team_task_filters() {
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    let task_search = array
        .iter()
        .find(|tool| tool.get("name").and_then(|v| v.as_str()) == Some("clickup_task_search"))
        .expect("missing clickup_task_search");
    let properties = task_search["inputSchema"]["properties"]
        .as_object()
        .expect("clickup_task_search properties must be an object");

    for prop in [
        "include_closed",
        "project_ids",
        "tags",
        "due_date_gt",
        "due_date_lt",
        "date_created_gt",
        "date_created_lt",
        "date_updated_gt",
        "date_updated_lt",
        "date_done_gt",
        "date_done_lt",
        "custom_fields",
        "parent",
        "custom_items",
        "order_by",
        "reverse",
        "subtasks",
        "include_markdown_description",
    ] {
        assert!(
            properties.contains_key(prop),
            "clickup_task_search should expose {}",
            prop
        );
    }
}

// ── handle_tools_call_early tests ────────────────────────────────────────────

use clickup_cli::mcp::handle_tools_call_early;
use serde_json::json;

#[test]
fn tools_call_rejects_filtered_tool_with_minus_32601() {
    let raw = RawFilter {
        profile: Some("read".into()),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();

    let id = json!(42);
    let params = json!({ "name": "clickup_task_delete", "arguments": {} });
    let response = handle_tools_call_early(&id, &params, &filter)
        .expect("filtered tool should yield an early response");

    assert_eq!(response["jsonrpc"], json!("2.0"));
    assert_eq!(response["id"], json!(42));
    assert_eq!(response["error"]["code"], json!(-32601));
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("clickup_task_delete"));
    assert!(message.contains("filtered out at startup"));
    assert!(
        response.get("result").is_none(),
        "must be a JSON-RPC error, not a success"
    );
}

#[test]
fn tools_call_passes_through_allowed_tool() {
    let raw = RawFilter::default();
    let filter = Filter::resolve(raw).unwrap();

    let id = json!(1);
    let params = json!({ "name": "clickup_task_list", "arguments": {} });
    let early = handle_tools_call_early(&id, &params, &filter);
    assert!(early.is_none(), "allowed tool should proceed to call_tool");
}

#[test]
fn tools_call_with_empty_name_returns_tool_error() {
    let raw = RawFilter::default();
    let filter = Filter::resolve(raw).unwrap();

    let id = json!(7);
    let params = json!({ "name": "" });
    let response = handle_tools_call_early(&id, &params, &filter)
        .expect("empty name should yield an early response");
    // Empty name is returned as a successful JSON-RPC response whose payload is
    // a tool_error — matching existing behavior.
    assert_eq!(response["jsonrpc"], json!("2.0"));
    assert_eq!(response["id"], json!(7));
    assert!(response.get("result").is_some());
    assert!(response.get("error").is_none());
}
