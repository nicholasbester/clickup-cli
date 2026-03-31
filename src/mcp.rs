use crate::client::ClickUpClient;
use crate::config::Config;
use crate::output::compact_items;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

// ── JSON-RPC helpers ──────────────────────────────────────────────────────────

fn ok_response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn tool_result(text: String) -> Value {
    json!({"content":[{"type":"text","text":text}]})
}

fn tool_error(msg: String) -> Value {
    json!({"content":[{"type":"text","text":msg}],"isError":true})
}

// ── Tool definitions ──────────────────────────────────────────────────────────

fn tool_list() -> Value {
    json!([
        {
            "name": "clickup_whoami",
            "description": "Get the currently authenticated ClickUp user",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "clickup_workspace_list",
            "description": "List all ClickUp workspaces (teams) accessible to the current user",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "clickup_space_list",
            "description": "List spaces in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "archived": {"type": "boolean", "description": "Include archived spaces"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_folder_list",
            "description": "List folders in a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "archived": {"type": "boolean", "description": "Include archived folders"}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_list_list",
            "description": "List ClickUp lists in a folder or space (folderless lists)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID (mutually exclusive with space_id)"},
                    "space_id": {"type": "string", "description": "Space ID for folderless lists (mutually exclusive with folder_id)"},
                    "archived": {"type": "boolean", "description": "Include archived lists"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_task_list",
            "description": "List tasks in a ClickUp list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "statuses": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by status names"
                    },
                    "assignees": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by assignee user IDs"
                    },
                    "include_closed": {"type": "boolean", "description": "Include closed tasks"}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_task_get",
            "description": "Get details of a specific ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "include_subtasks": {"type": "boolean", "description": "Include subtasks in the response"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_create",
            "description": "Create a new task in a ClickUp list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID to create the task in"},
                    "name": {"type": "string", "description": "Task name"},
                    "description": {"type": "string", "description": "Task description (markdown supported)"},
                    "status": {"type": "string", "description": "Task status"},
                    "priority": {"type": "integer", "description": "Priority (1=urgent, 2=high, 3=normal, 4=low)"},
                    "assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "List of assignee user IDs"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of tag names"
                    },
                    "due_date": {"type": "integer", "description": "Due date as Unix timestamp (milliseconds)"}
                },
                "required": ["list_id", "name"]
            }
        },
        {
            "name": "clickup_task_update",
            "description": "Update an existing ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "name": {"type": "string", "description": "New task name"},
                    "status": {"type": "string", "description": "New status"},
                    "priority": {"type": "integer", "description": "New priority (1=urgent, 2=high, 3=normal, 4=low)"},
                    "description": {"type": "string", "description": "New description"},
                    "add_assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add as assignees"
                    },
                    "rem_assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to remove from assignees"
                    }
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_delete",
            "description": "Delete a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_search",
            "description": "Search tasks across a workspace with optional filters",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "space_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by space IDs"
                    },
                    "list_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by list IDs"
                    },
                    "statuses": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by status names"
                    },
                    "assignees": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by assignee user IDs"
                    }
                },
                "required": []
            }
        },
        {
            "name": "clickup_comment_list",
            "description": "List comments on a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_comment_create",
            "description": "Create a comment on a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "text": {"type": "string", "description": "Comment text"},
                    "assignee": {"type": "integer", "description": "Assign the comment to a user ID"},
                    "notify_all": {"type": "boolean", "description": "Notify all assignees"}
                },
                "required": ["task_id", "text"]
            }
        },
        {
            "name": "clickup_field_list",
            "description": "List custom fields for a ClickUp list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_field_set",
            "description": "Set a custom field value on a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "field_id": {"type": "string", "description": "Custom field ID"},
                    "value": {"description": "Value to set (type depends on the custom field type)"}
                },
                "required": ["task_id", "field_id", "value"]
            }
        },
        {
            "name": "clickup_time_start",
            "description": "Start a time tracking entry for a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "Task ID to track time against"},
                    "description": {"type": "string", "description": "Description of the time entry"},
                    "billable": {"type": "boolean", "description": "Whether this entry is billable"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_time_stop",
            "description": "Stop the currently running time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_time_list",
            "description": "List time tracking entries for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "start_date": {"type": "integer", "description": "Start date as Unix timestamp (milliseconds)"},
                    "end_date": {"type": "integer", "description": "End date as Unix timestamp (milliseconds)"},
                    "task_id": {"type": "string", "description": "Filter by task ID"}
                },
                "required": []
            }
        }
    ])
}

// ── Tool execution ────────────────────────────────────────────────────────────

async fn call_tool(
    name: &str,
    args: &Value,
    client: &ClickUpClient,
    workspace_id: &Option<String>,
) -> Value {
    let result = dispatch_tool(name, args, client, workspace_id).await;
    match result {
        Ok(v) => tool_result(v.to_string()),
        Err(e) => tool_error(format!("Error: {}", e)),
    }
}

async fn dispatch_tool(
    name: &str,
    args: &Value,
    client: &ClickUpClient,
    workspace_id: &Option<String>,
) -> Result<Value, String> {
    let empty = json!({});
    let args = if args.is_null() { &empty } else { args };

    // Resolve workspace ID from args or config
    let resolve_workspace = |args: &Value| -> Result<String, String> {
        if let Some(id) = args.get("team_id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }
        workspace_id
            .clone()
            .ok_or_else(|| "No workspace_id found in config. Please run `clickup setup` or provide team_id in the tool arguments.".to_string())
    };

    match name {
        "clickup_whoami" => {
            let resp = client.get("/v2/user").await.map_err(|e| e.to_string())?;
            let user = resp.get("user").cloned().unwrap_or(resp);
            Ok(compact_items(&[user], &["id", "username", "email"]))
        }

        "clickup_workspace_list" => {
            let resp = client.get("/v2/team").await.map_err(|e| e.to_string())?;
            let teams = resp.get("teams").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            let items: Vec<Value> = teams.iter().map(|ws| {
                json!({
                    "id": ws.get("id"),
                    "name": ws.get("name"),
                    "members": ws.get("members").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
                })
            }).collect();
            Ok(compact_items(&items, &["id", "name", "members"]))
        }

        "clickup_space_list" => {
            let team_id = resolve_workspace(args)?;
            let archived = args.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = format!("/v2/team/{}/space?archived={}", team_id, archived);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let spaces = resp.get("spaces").and_then(|s| s.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&spaces, &["id", "name", "private", "archived"]))
        }

        "clickup_folder_list" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let archived = args.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = format!("/v2/space/{}/folder?archived={}", space_id, archived);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let folders = resp.get("folders").and_then(|f| f.as_array()).cloned().unwrap_or_default();
            let items: Vec<Value> = folders.iter().map(|f| {
                let list_count = f.get("lists").and_then(|l| l.as_array()).map(|a| a.len()).unwrap_or(0);
                json!({
                    "id": f.get("id"),
                    "name": f.get("name"),
                    "task_count": f.get("task_count"),
                    "list_count": list_count,
                })
            }).collect();
            Ok(compact_items(&items, &["id", "name", "task_count", "list_count"]))
        }

        "clickup_list_list" => {
            let archived = args.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                format!("/v2/folder/{}/list?archived={}", folder_id, archived)
            } else if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                format!("/v2/space/{}/list?archived={}", space_id, archived)
            } else {
                return Err("Provide either folder_id or space_id".to_string());
            };
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let lists = resp.get("lists").and_then(|l| l.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&lists, &["id", "name", "task_count", "status", "due_date"]))
        }

        "clickup_task_list" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let mut qs = String::new();
            if let Some(include_closed) = args.get("include_closed").and_then(|v| v.as_bool()) {
                qs.push_str(&format!("&include_closed={}", include_closed));
            }
            if let Some(statuses) = args.get("statuses").and_then(|v| v.as_array()) {
                for s in statuses {
                    if let Some(s) = s.as_str() {
                        qs.push_str(&format!("&statuses[]={}", s));
                    }
                }
            }
            if let Some(assignees) = args.get("assignees").and_then(|v| v.as_array()) {
                for a in assignees {
                    if let Some(a) = a.as_str() {
                        qs.push_str(&format!("&assignees[]={}", a));
                    }
                }
            }
            let path = format!("/v2/list/{}/task?{}", list_id, qs.trim_start_matches('&'));
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tasks = resp.get("tasks").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&tasks, &["id", "name", "status", "priority", "assignees", "due_date"]))
        }

        "clickup_task_get" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let include_subtasks = args
                .get("include_subtasks")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let path = format!(
                "/v2/task/{}?include_subtasks={}",
                task_id, include_subtasks
            );
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "status", "priority", "assignees", "due_date", "description"]))
        }

        "clickup_task_create" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
                body["status"] = json!(status);
            }
            if let Some(priority) = args.get("priority").and_then(|v| v.as_i64()) {
                body["priority"] = json!(priority);
            }
            if let Some(assignees) = args.get("assignees") {
                body["assignees"] = assignees.clone();
            }
            if let Some(tags) = args.get("tags") {
                body["tags"] = tags.clone();
            }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) {
                body["due_date"] = json!(due_date);
            }
            let path = format!("/v2/list/{}/task", list_id);
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "status", "priority", "assignees", "due_date"]))
        }

        "clickup_task_update" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
                body["status"] = json!(status);
            }
            if let Some(priority) = args.get("priority").and_then(|v| v.as_i64()) {
                body["priority"] = json!(priority);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(add) = args.get("add_assignees") {
                body["assignees"] = json!({"add": add, "rem": args.get("rem_assignees").cloned().unwrap_or(json!([]))});
            } else if let Some(rem) = args.get("rem_assignees") {
                body["assignees"] = json!({"add": [], "rem": rem});
            }
            let path = format!("/v2/task/{}", task_id);
            let resp = client.put(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "status", "priority", "assignees", "due_date"]))
        }

        "clickup_task_delete" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let path = format!("/v2/task/{}", task_id);
            client.delete(&path).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} deleted", task_id)}))
        }

        "clickup_task_search" => {
            let team_id = resolve_workspace(args)?;
            let mut qs = String::new();
            if let Some(space_ids) = args.get("space_ids").and_then(|v| v.as_array()) {
                for id in space_ids {
                    if let Some(id) = id.as_str() {
                        qs.push_str(&format!("&space_ids[]={}", id));
                    }
                }
            }
            if let Some(list_ids) = args.get("list_ids").and_then(|v| v.as_array()) {
                for id in list_ids {
                    if let Some(id) = id.as_str() {
                        qs.push_str(&format!("&list_ids[]={}", id));
                    }
                }
            }
            if let Some(statuses) = args.get("statuses").and_then(|v| v.as_array()) {
                for s in statuses {
                    if let Some(s) = s.as_str() {
                        qs.push_str(&format!("&statuses[]={}", s));
                    }
                }
            }
            if let Some(assignees) = args.get("assignees").and_then(|v| v.as_array()) {
                for a in assignees {
                    if let Some(a) = a.as_str() {
                        qs.push_str(&format!("&assignees[]={}", a));
                    }
                }
            }
            let path = format!(
                "/v2/team/{}/task?{}",
                team_id,
                qs.trim_start_matches('&')
            );
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tasks = resp.get("tasks").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&tasks, &["id", "name", "status", "priority", "assignees", "due_date"]))
        }

        "clickup_comment_list" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let path = format!("/v2/task/{}/comment", task_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let comments = resp.get("comments").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&comments, &["id", "user", "date", "comment_text"]))
        }

        "clickup_comment_create" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let mut body = json!({"comment_text": text});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) {
                body["assignee"] = json!(assignee);
            }
            if let Some(notify_all) = args.get("notify_all").and_then(|v| v.as_bool()) {
                body["notify_all"] = json!(notify_all);
            }
            let path = format!("/v2/task/{}/comment", task_id);
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Comment created", "id": resp.get("id")}))
        }

        "clickup_field_list" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let path = format!("/v2/list/{}/field", list_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let fields = resp.get("fields").and_then(|f| f.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&fields, &["id", "name", "type", "required"]))
        }

        "clickup_field_set" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let field_id = args
                .get("field_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: field_id")?;
            let value = args.get("value").ok_or("Missing required parameter: value")?;
            let body = json!({"value": value});
            let path = format!("/v2/task/{}/field/{}", task_id, field_id);
            client.post(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Field {} set on task {}", field_id, task_id)}))
        }

        "clickup_time_start" => {
            let team_id = resolve_workspace(args)?;
            let mut body = json!({});
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) {
                body["tid"] = json!(task_id);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(billable) = args.get("billable").and_then(|v| v.as_bool()) {
                body["billable"] = json!(billable);
            }
            let path = format!("/v2/team/{}/time_entries/start", team_id);
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "billable"]))
        }

        "clickup_time_stop" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v2/team/{}/time_entries/stop", team_id);
            let resp = client.post(&path, &json!({})).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "end", "billable"]))
        }

        "clickup_time_list" => {
            let team_id = resolve_workspace(args)?;
            let mut qs = String::new();
            if let Some(start_date) = args.get("start_date").and_then(|v| v.as_i64()) {
                qs.push_str(&format!("&start_date={}", start_date));
            }
            if let Some(end_date) = args.get("end_date").and_then(|v| v.as_i64()) {
                qs.push_str(&format!("&end_date={}", end_date));
            }
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) {
                qs.push_str(&format!("&task_id={}", task_id));
            }
            let path = format!(
                "/v2/team/{}/time_entries?{}",
                team_id,
                qs.trim_start_matches('&')
            );
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let entries = resp.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&entries, &["id", "task", "duration", "start", "billable"]))
        }

        unknown => Err(format!("Unknown tool: {}", unknown)),
    }
}

// ── Main server loop ──────────────────────────────────────────────────────────

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    // Load config at startup
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;
    let token = config.auth.token.clone();
    if token.is_empty() {
        return Err("No API token configured. Run `clickup setup` first.".into());
    }
    let workspace_id = config.defaults.workspace_id.clone();

    let client = ClickUpClient::new(&token, 30)
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                // Parse error — send error response with null id
                let resp = error_response(&Value::Null, -32700, &format!("Parse error: {}", e));
                println!("{}", resp);
                continue;
            }
        };

        // Notifications have no id — don't respond
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");

        if id.is_null() && method.starts_with("notifications/") {
            // Notification — no response needed
            continue;
        }

        let resp = match method {
            "initialize" => {
                let version = msg
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                ok_response(
                    &id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": {"tools": {}},
                        "serverInfo": {
                            "name": "clickup-cli",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                )
            }

            "tools/list" => ok_response(&id, json!({"tools": tool_list()})),

            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                let tool_name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                if tool_name.is_empty() {
                    let result = tool_error("Missing tool name".to_string());
                    ok_response(&id, result)
                } else {
                    let result = call_tool(tool_name, &arguments, &client, &workspace_id).await;
                    ok_response(&id, result)
                }
            }

            other => {
                // Unknown method
                eprintln!("Unknown method: {}", other);
                error_response(&id, -32601, &format!("Method not found: {}", other))
            }
        };

        println!("{}", resp);
    }

    Ok(())
}
