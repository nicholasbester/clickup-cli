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
        },
        {
            "name": "clickup_checklist_create",
            "description": "Create a checklist on a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "name": {"type": "string", "description": "Checklist name"}
                },
                "required": ["task_id", "name"]
            }
        },
        {
            "name": "clickup_checklist_delete",
            "description": "Delete a checklist from a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "Checklist ID"}
                },
                "required": ["checklist_id"]
            }
        },
        {
            "name": "clickup_goal_list",
            "description": "List goals in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_goal_get",
            "description": "Get details of a specific goal",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "Goal ID"}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_goal_create",
            "description": "Create a new goal in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Goal name"},
                    "due_date": {"type": "integer", "description": "Due date as Unix timestamp (milliseconds)"},
                    "description": {"type": "string", "description": "Goal description"},
                    "owner_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "List of owner user IDs"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_goal_update",
            "description": "Update an existing goal",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "Goal ID"},
                    "name": {"type": "string", "description": "New goal name"},
                    "due_date": {"type": "integer", "description": "New due date as Unix timestamp (milliseconds)"},
                    "description": {"type": "string", "description": "New goal description"}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_view_list",
            "description": "List views for a space, folder, list, or workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID (mutually exclusive with other IDs)"},
                    "folder_id": {"type": "string", "description": "Folder ID (mutually exclusive with other IDs)"},
                    "list_id": {"type": "string", "description": "List ID (mutually exclusive with other IDs)"},
                    "team_id": {"type": "string", "description": "Workspace (team) ID for workspace-level views. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_view_tasks",
            "description": "Get tasks in a specific view",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "View ID"},
                    "page": {"type": "integer", "description": "Page number (0-indexed, default 0)"}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_doc_list",
            "description": "List docs in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_doc_get",
            "description": "Get a specific doc from a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "Doc ID"}
                },
                "required": ["doc_id"]
            }
        },
        {
            "name": "clickup_doc_pages",
            "description": "List pages in a doc",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "Doc ID"},
                    "content": {"type": "boolean", "description": "Include page content in the response"}
                },
                "required": ["doc_id"]
            }
        },
        {
            "name": "clickup_tag_list",
            "description": "List tags for a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_task_add_tag",
            "description": "Add a tag to a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "tag_name": {"type": "string", "description": "Tag name to add"}
                },
                "required": ["task_id", "tag_name"]
            }
        },
        {
            "name": "clickup_task_remove_tag",
            "description": "Remove a tag from a ClickUp task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "tag_name": {"type": "string", "description": "Tag name to remove"}
                },
                "required": ["task_id", "tag_name"]
            }
        },
        {
            "name": "clickup_webhook_list",
            "description": "List webhooks for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_member_list",
            "description": "List members of a task or list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID (mutually exclusive with list_id)"},
                    "list_id": {"type": "string", "description": "List ID (mutually exclusive with task_id)"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_template_list",
            "description": "List task templates for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "page": {"type": "integer", "description": "Page number (0-indexed, default 0)"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_space_get",
            "description": "Get details of a specific space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_space_create",
            "description": "Create a new space in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Space name"},
                    "private": {"type": "boolean", "description": "Whether the space is private"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_space_update",
            "description": "Update an existing space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "name": {"type": "string", "description": "New space name"},
                    "private": {"type": "boolean", "description": "Whether the space is private"},
                    "archived": {"type": "boolean", "description": "Archive/unarchive the space"}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_space_delete",
            "description": "Delete a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_folder_get",
            "description": "Get details of a specific folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID"}
                },
                "required": ["folder_id"]
            }
        },
        {
            "name": "clickup_folder_create",
            "description": "Create a new folder in a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "name": {"type": "string", "description": "Folder name"}
                },
                "required": ["space_id", "name"]
            }
        },
        {
            "name": "clickup_folder_update",
            "description": "Update an existing folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID"},
                    "name": {"type": "string", "description": "New folder name"}
                },
                "required": ["folder_id", "name"]
            }
        },
        {
            "name": "clickup_folder_delete",
            "description": "Delete a folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID"}
                },
                "required": ["folder_id"]
            }
        },
        {
            "name": "clickup_list_get",
            "description": "Get details of a specific list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_create",
            "description": "Create a new list in a folder or space (folderless)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID (mutually exclusive with space_id)"},
                    "space_id": {"type": "string", "description": "Space ID for a folderless list (mutually exclusive with folder_id)"},
                    "name": {"type": "string", "description": "List name"},
                    "content": {"type": "string", "description": "List description"},
                    "due_date": {"type": "integer", "description": "Due date as Unix timestamp (milliseconds)"},
                    "status": {"type": "string", "description": "List status"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_list_update",
            "description": "Update an existing list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "name": {"type": "string", "description": "New list name"},
                    "content": {"type": "string", "description": "New description"},
                    "due_date": {"type": "integer", "description": "Due date as Unix timestamp (milliseconds)"},
                    "status": {"type": "string", "description": "List status"}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_delete",
            "description": "Delete a list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_add_task",
            "description": "Add a task to an additional list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["list_id", "task_id"]
            }
        },
        {
            "name": "clickup_list_remove_task",
            "description": "Remove a task from an additional list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["list_id", "task_id"]
            }
        },
        {
            "name": "clickup_comment_update",
            "description": "Update a comment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "Comment ID"},
                    "text": {"type": "string", "description": "New comment text"},
                    "assignee": {"type": "integer", "description": "Reassign comment to user ID"},
                    "resolved": {"type": "boolean", "description": "Mark comment as resolved"}
                },
                "required": ["comment_id", "text"]
            }
        },
        {
            "name": "clickup_comment_delete",
            "description": "Delete a comment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "Comment ID"}
                },
                "required": ["comment_id"]
            }
        },
        {
            "name": "clickup_task_add_dep",
            "description": "Add a dependency to a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "depends_on": {"type": "string", "description": "Task ID that this task depends on"},
                    "dependency_of": {"type": "string", "description": "Task ID that depends on this task"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_remove_dep",
            "description": "Remove a dependency from a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "depends_on": {"type": "string", "description": "Task ID to remove as a depends_on dependency"},
                    "dependency_of": {"type": "string", "description": "Task ID to remove as a dependency_of dependency"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_link",
            "description": "Link two tasks together",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "links_to": {"type": "string", "description": "Task ID to link to"}
                },
                "required": ["task_id", "links_to"]
            }
        },
        {
            "name": "clickup_task_unlink",
            "description": "Remove a link between two tasks",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "links_to": {"type": "string", "description": "Task ID to unlink"}
                },
                "required": ["task_id", "links_to"]
            }
        },
        {
            "name": "clickup_goal_delete",
            "description": "Delete a goal",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "Goal ID"}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_goal_add_kr",
            "description": "Add a key result to a goal",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "Goal ID"},
                    "name": {"type": "string", "description": "Key result name"},
                    "type": {"type": "string", "description": "Key result type (number, currency, boolean, percentage, automatic)"},
                    "steps_start": {"type": "number", "description": "Starting value"},
                    "steps_end": {"type": "number", "description": "Target value"},
                    "unit": {"type": "string", "description": "Unit label"},
                    "owner_ids": {"type": "array", "items": {"type": "integer"}, "description": "Owner user IDs"},
                    "task_ids": {"type": "array", "items": {"type": "string"}, "description": "Task IDs to link (for automatic type)"},
                    "list_ids": {"type": "array", "items": {"type": "string"}, "description": "List IDs to link (for automatic type)"}
                },
                "required": ["goal_id", "name", "type", "steps_start", "steps_end"]
            }
        },
        {
            "name": "clickup_goal_update_kr",
            "description": "Update a key result",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kr_id": {"type": "string", "description": "Key result ID"},
                    "steps_current": {"type": "number", "description": "Current progress value"},
                    "name": {"type": "string", "description": "New key result name"},
                    "unit": {"type": "string", "description": "Unit label"}
                },
                "required": ["kr_id"]
            }
        },
        {
            "name": "clickup_goal_delete_kr",
            "description": "Delete a key result",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kr_id": {"type": "string", "description": "Key result ID"}
                },
                "required": ["kr_id"]
            }
        },
        {
            "name": "clickup_time_get",
            "description": "Get a specific time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "Time entry ID"}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_time_create",
            "description": "Create a time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "Task ID to log time against"},
                    "start": {"type": "integer", "description": "Start time as Unix timestamp (milliseconds)"},
                    "duration": {"type": "integer", "description": "Duration in milliseconds"},
                    "description": {"type": "string", "description": "Time entry description"},
                    "billable": {"type": "boolean", "description": "Whether this entry is billable"}
                },
                "required": ["start", "duration"]
            }
        },
        {
            "name": "clickup_time_update",
            "description": "Update a time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "Time entry ID"},
                    "start": {"type": "integer", "description": "New start time as Unix timestamp (milliseconds)"},
                    "duration": {"type": "integer", "description": "New duration in milliseconds"},
                    "description": {"type": "string", "description": "New description"},
                    "billable": {"type": "boolean", "description": "Whether this entry is billable"}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_time_delete",
            "description": "Delete a time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "Time entry ID"}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_view_get",
            "description": "Get details of a specific view",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "View ID"}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_view_create",
            "description": "Create a view for a space, folder, list, or workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "description": "Scope type: space, folder, list, or team"},
                    "scope_id": {"type": "string", "description": "ID of the scope object"},
                    "name": {"type": "string", "description": "View name"},
                    "type": {"type": "string", "description": "View type (list, board, calendar, table, timeline, etc.)"}
                },
                "required": ["scope", "scope_id", "name", "type"]
            }
        },
        {
            "name": "clickup_view_update",
            "description": "Update an existing view",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "View ID"},
                    "name": {"type": "string", "description": "New view name"},
                    "type": {"type": "string", "description": "New view type"}
                },
                "required": ["view_id", "name", "type"]
            }
        },
        {
            "name": "clickup_view_delete",
            "description": "Delete a view",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "View ID"}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_doc_create",
            "description": "Create a new doc in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Doc name"},
                    "parent": {"type": "object", "description": "Parent object with id and type fields"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_doc_add_page",
            "description": "Add a page to a doc",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "Doc ID"},
                    "name": {"type": "string", "description": "Page name"},
                    "content": {"type": "string", "description": "Page content (markdown)"},
                    "sub_title": {"type": "string", "description": "Page subtitle"},
                    "parent_page_id": {"type": "string", "description": "Parent page ID for nested pages"}
                },
                "required": ["doc_id", "name"]
            }
        },
        {
            "name": "clickup_doc_edit_page",
            "description": "Edit an existing page in a doc",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "Doc ID"},
                    "page_id": {"type": "string", "description": "Page ID"},
                    "name": {"type": "string", "description": "New page name"},
                    "content": {"type": "string", "description": "New page content (markdown)"}
                },
                "required": ["doc_id", "page_id"]
            }
        },
        {
            "name": "clickup_chat_channel_create",
            "description": "Create a chat channel in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Channel name"},
                    "description": {"type": "string", "description": "Channel description"},
                    "visibility": {"type": "string", "description": "Channel visibility (public or private)"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_chat_channel_get",
            "description": "Get a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_update",
            "description": "Update a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"},
                    "name": {"type": "string", "description": "New channel name"},
                    "description": {"type": "string", "description": "New channel description"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_delete",
            "description": "Delete a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_list",
            "description": "List messages in a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"},
                    "cursor": {"type": "string", "description": "Pagination cursor"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_send",
            "description": "Send a message to a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"},
                    "content": {"type": "string", "description": "Message content"}
                },
                "required": ["channel_id", "content"]
            }
        },
        {
            "name": "clickup_chat_message_delete",
            "description": "Delete a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_dm",
            "description": "Send a direct message to a user",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID to DM"},
                    "content": {"type": "string", "description": "Message content"}
                },
                "required": ["user_id", "content"]
            }
        },
        {
            "name": "clickup_webhook_create",
            "description": "Create a webhook for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "endpoint": {"type": "string", "description": "Webhook URL endpoint"},
                    "events": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of events to subscribe to (use ['*'] for all events)"
                    },
                    "space_id": {"type": "string", "description": "Filter events to a specific space"},
                    "folder_id": {"type": "string", "description": "Filter events to a specific folder"},
                    "list_id": {"type": "string", "description": "Filter events to a specific list"},
                    "task_id": {"type": "string", "description": "Filter events to a specific task"}
                },
                "required": ["endpoint", "events"]
            }
        },
        {
            "name": "clickup_webhook_update",
            "description": "Update a webhook",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "webhook_id": {"type": "string", "description": "Webhook ID"},
                    "endpoint": {"type": "string", "description": "New webhook URL endpoint"},
                    "events": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "New list of events to subscribe to"
                    },
                    "status": {"type": "string", "description": "Webhook status (active or suspended)"}
                },
                "required": ["webhook_id"]
            }
        },
        {
            "name": "clickup_webhook_delete",
            "description": "Delete a webhook",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "webhook_id": {"type": "string", "description": "Webhook ID"}
                },
                "required": ["webhook_id"]
            }
        },
        {
            "name": "clickup_checklist_add_item",
            "description": "Add an item to a checklist",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "Checklist ID"},
                    "name": {"type": "string", "description": "Item name"},
                    "assignee": {"type": "integer", "description": "Assign item to user ID"}
                },
                "required": ["checklist_id", "name"]
            }
        },
        {
            "name": "clickup_checklist_update_item",
            "description": "Update a checklist item",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "Checklist ID"},
                    "item_id": {"type": "string", "description": "Checklist item ID"},
                    "name": {"type": "string", "description": "New item name"},
                    "resolved": {"type": "boolean", "description": "Mark item as resolved"},
                    "assignee": {"type": "integer", "description": "Reassign item to user ID"}
                },
                "required": ["checklist_id", "item_id"]
            }
        },
        {
            "name": "clickup_checklist_delete_item",
            "description": "Delete a checklist item",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "Checklist ID"},
                    "item_id": {"type": "string", "description": "Checklist item ID"}
                },
                "required": ["checklist_id", "item_id"]
            }
        },
        {
            "name": "clickup_user_get",
            "description": "Get a user in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID"}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_workspace_seats",
            "description": "Get seat usage for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_workspace_plan",
            "description": "Get the plan for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_tag_create",
            "description": "Create a tag in a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "name": {"type": "string", "description": "Tag name"},
                    "tag_fg": {"type": "string", "description": "Foreground color (hex)"},
                    "tag_bg": {"type": "string", "description": "Background color (hex)"}
                },
                "required": ["space_id", "name"]
            }
        },
        {
            "name": "clickup_tag_update",
            "description": "Update a tag in a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "tag_name": {"type": "string", "description": "Current tag name"},
                    "name": {"type": "string", "description": "New tag name"},
                    "tag_fg": {"type": "string", "description": "New foreground color (hex)"},
                    "tag_bg": {"type": "string", "description": "New background color (hex)"}
                },
                "required": ["space_id", "tag_name"]
            }
        },
        {
            "name": "clickup_tag_delete",
            "description": "Delete a tag from a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "tag_name": {"type": "string", "description": "Tag name"}
                },
                "required": ["space_id", "tag_name"]
            }
        },
        {
            "name": "clickup_field_unset",
            "description": "Remove a custom field value from a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "field_id": {"type": "string", "description": "Custom field ID"}
                },
                "required": ["task_id", "field_id"]
            }
        },
        {
            "name": "clickup_attachment_list",
            "description": "List attachments on a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_shared_list",
            "description": "Get shared hierarchy (tasks, lists, folders) for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_group_list",
            "description": "List user groups (teams) in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "group_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by specific group IDs"
                    }
                },
                "required": []
            }
        },
        {
            "name": "clickup_group_create",
            "description": "Create a user group in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Group name"},
                    "member_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add as members"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_group_update",
            "description": "Update a user group",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": {"type": "string", "description": "Group ID"},
                    "name": {"type": "string", "description": "New group name"},
                    "add_members": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add"
                    },
                    "rem_members": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to remove"
                    }
                },
                "required": ["group_id"]
            }
        },
        {
            "name": "clickup_group_delete",
            "description": "Delete a user group",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": {"type": "string", "description": "Group ID"}
                },
                "required": ["group_id"]
            }
        },
        {
            "name": "clickup_role_list",
            "description": "List custom roles in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_guest_get",
            "description": "Get a guest in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Guest user ID"}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_task_time_in_status",
            "description": "Get time a task has spent in each status",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_move",
            "description": "Move a task to a different list (change home list)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "list_id": {"type": "string", "description": "Destination list ID"},
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": ["task_id", "list_id"]
            }
        },
        {
            "name": "clickup_task_set_estimate",
            "description": "Set a time estimate for a specific user on a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID to set estimate for"},
                    "time_estimate": {"type": "integer", "description": "Time estimate in milliseconds"}
                },
                "required": ["task_id", "user_id", "time_estimate"]
            }
        },
        {
            "name": "clickup_task_replace_estimates",
            "description": "Replace all time estimates for a task (PUT replaces all user estimates)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID"},
                    "time_estimate": {"type": "integer", "description": "Time estimate in milliseconds"}
                },
                "required": ["task_id", "user_id", "time_estimate"]
            }
        },
        {
            "name": "clickup_auth_check",
            "description": "Check if the current API token is valid",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "clickup_checklist_update",
            "description": "Update a checklist (rename or reorder)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "Checklist ID"},
                    "name": {"type": "string", "description": "New checklist name"},
                    "position": {"type": "integer", "description": "New position index"}
                },
                "required": ["checklist_id"]
            }
        },
        {
            "name": "clickup_comment_replies",
            "description": "Get replies to a comment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "Comment ID"}
                },
                "required": ["comment_id"]
            }
        },
        {
            "name": "clickup_comment_reply",
            "description": "Post a reply to a comment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "Comment ID"},
                    "text": {"type": "string", "description": "Reply text"},
                    "assignee": {"type": "integer", "description": "Assign the reply to a user ID"}
                },
                "required": ["comment_id", "text"]
            }
        },
        {
            "name": "clickup_chat_channel_list",
            "description": "List chat channels in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "include_closed": {"type": "boolean", "description": "Include closed channels"}
                },
                "required": []
            }
        },
        {
            "name": "clickup_chat_channel_followers",
            "description": "Get followers of a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_members",
            "description": "Get members of a chat channel",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "Channel ID"}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_update",
            "description": "Update (edit) a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"},
                    "text": {"type": "string", "description": "New message content"}
                },
                "required": ["message_id", "text"]
            }
        },
        {
            "name": "clickup_chat_reaction_list",
            "description": "List reactions on a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_reaction_add",
            "description": "Add a reaction to a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"},
                    "emoji": {"type": "string", "description": "Emoji reaction (e.g. '👍')"}
                },
                "required": ["message_id", "emoji"]
            }
        },
        {
            "name": "clickup_chat_reaction_remove",
            "description": "Remove a reaction from a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"},
                    "emoji": {"type": "string", "description": "Emoji to remove"}
                },
                "required": ["message_id", "emoji"]
            }
        },
        {
            "name": "clickup_chat_reply_list",
            "description": "List replies to a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_reply_send",
            "description": "Send a reply to a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"},
                    "text": {"type": "string", "description": "Reply content"}
                },
                "required": ["message_id", "text"]
            }
        },
        {
            "name": "clickup_chat_tagged_users",
            "description": "Get users tagged in a chat message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "Message ID"}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_time_current",
            "description": "Get the currently running time tracking entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_time_tags",
            "description": "List all time entry tags for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_time_add_tags",
            "description": "Add tags to time entries",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "entry_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Time entry IDs to tag"
                    },
                    "tag_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tag names to add"
                    }
                },
                "required": ["entry_ids", "tag_names"]
            }
        },
        {
            "name": "clickup_time_remove_tags",
            "description": "Remove tags from time entries",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "entry_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Time entry IDs to untag"
                    },
                    "tag_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tag names to remove"
                    }
                },
                "required": ["entry_ids", "tag_names"]
            }
        },
        {
            "name": "clickup_time_rename_tag",
            "description": "Rename a time entry tag",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Current tag name"},
                    "new_name": {"type": "string", "description": "New tag name"}
                },
                "required": ["name", "new_name"]
            }
        },
        {
            "name": "clickup_time_history",
            "description": "Get the history of changes for a time entry",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "Time entry ID"}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_guest_invite",
            "description": "Invite a guest to a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "email": {"type": "string", "description": "Guest email address"},
                    "can_edit_tags": {"type": "boolean", "description": "Allow guest to edit tags"},
                    "can_see_time_spent": {"type": "boolean", "description": "Allow guest to see time spent"},
                    "can_create_views": {"type": "boolean", "description": "Allow guest to create views"}
                },
                "required": ["email"]
            }
        },
        {
            "name": "clickup_guest_update",
            "description": "Update a guest's permissions in a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Guest user ID"},
                    "can_edit_tags": {"type": "boolean", "description": "Allow guest to edit tags"},
                    "can_see_time_spent": {"type": "boolean", "description": "Allow guest to see time spent"},
                    "can_create_views": {"type": "boolean", "description": "Allow guest to create views"}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_guest_remove",
            "description": "Remove a guest from a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Guest user ID"}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_guest_share_task",
            "description": "Share a task with a guest",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"},
                    "permission": {"type": "string", "description": "Permission level (read, comment, create, edit)"}
                },
                "required": ["task_id", "guest_id", "permission"]
            }
        },
        {
            "name": "clickup_guest_unshare_task",
            "description": "Revoke a guest's access to a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"}
                },
                "required": ["task_id", "guest_id"]
            }
        },
        {
            "name": "clickup_guest_share_list",
            "description": "Share a list with a guest",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"},
                    "permission": {"type": "string", "description": "Permission level (read, comment, create, edit)"}
                },
                "required": ["list_id", "guest_id", "permission"]
            }
        },
        {
            "name": "clickup_guest_unshare_list",
            "description": "Revoke a guest's access to a list",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"}
                },
                "required": ["list_id", "guest_id"]
            }
        },
        {
            "name": "clickup_guest_share_folder",
            "description": "Share a folder with a guest",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"},
                    "permission": {"type": "string", "description": "Permission level (read, comment, create, edit)"}
                },
                "required": ["folder_id", "guest_id", "permission"]
            }
        },
        {
            "name": "clickup_guest_unshare_folder",
            "description": "Revoke a guest's access to a folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "Folder ID"},
                    "guest_id": {"type": "integer", "description": "Guest user ID"}
                },
                "required": ["folder_id", "guest_id"]
            }
        },
        {
            "name": "clickup_user_invite",
            "description": "Invite a user to a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "email": {"type": "string", "description": "User email address"},
                    "admin": {"type": "boolean", "description": "Grant admin role"}
                },
                "required": ["email"]
            }
        },
        {
            "name": "clickup_user_update",
            "description": "Update a workspace member's role or username",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID"},
                    "username": {"type": "string", "description": "New username"},
                    "admin": {"type": "boolean", "description": "Grant or revoke admin role"}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_user_remove",
            "description": "Remove a user from a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "User ID"}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_template_apply_task",
            "description": "Create a task from a task template",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "List ID to create the task in"},
                    "template_id": {"type": "string", "description": "Template ID"},
                    "name": {"type": "string", "description": "Task name"}
                },
                "required": ["list_id", "template_id", "name"]
            }
        },
        {
            "name": "clickup_template_apply_list",
            "description": "Create a list from a list template in a folder or space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "template_id": {"type": "string", "description": "Template ID"},
                    "name": {"type": "string", "description": "New list name"},
                    "folder_id": {"type": "string", "description": "Folder ID (mutually exclusive with space_id)"},
                    "space_id": {"type": "string", "description": "Space ID (mutually exclusive with folder_id)"}
                },
                "required": ["template_id", "name"]
            }
        },
        {
            "name": "clickup_template_apply_folder",
            "description": "Create a folder from a folder template in a space",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID"},
                    "template_id": {"type": "string", "description": "Template ID"},
                    "name": {"type": "string", "description": "New folder name"}
                },
                "required": ["space_id", "template_id", "name"]
            }
        },
        {
            "name": "clickup_attachment_upload",
            "description": "Upload a file as an attachment to a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID"},
                    "file_path": {"type": "string", "description": "Absolute path to the file to upload"}
                },
                "required": ["task_id", "file_path"]
            }
        },
        {
            "name": "clickup_task_type_list",
            "description": "List custom task types (custom items) for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_doc_get_page",
            "description": "Get a specific page from a doc",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "Doc ID"},
                    "page_id": {"type": "string", "description": "Page ID"}
                },
                "required": ["doc_id", "page_id"]
            }
        },
        {
            "name": "clickup_audit_log_query",
            "description": "Query the audit log for a workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "type": {"type": "string", "description": "Audit log event type"},
                    "user_id": {"type": "integer", "description": "Filter by user ID"},
                    "start_date": {"type": "integer", "description": "Start date as Unix timestamp (milliseconds)"},
                    "end_date": {"type": "integer", "description": "End date as Unix timestamp (milliseconds)"}
                },
                "required": ["type"]
            }
        },
        {
            "name": "clickup_acl_update",
            "description": "Update access control (privacy) for a workspace object",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "object_type": {"type": "string", "description": "Object type (e.g. space, folder, list)"},
                    "object_id": {"type": "string", "description": "Object ID"},
                    "private": {"type": "boolean", "description": "Set to true to make private, false to make public"}
                },
                "required": ["object_type", "object_id"]
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

        "clickup_checklist_create" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let path = format!("/v2/task/{}/checklist", task_id);
            let body = json!({"name": name});
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            Ok(compact_items(&[checklist], &["id", "name"]))
        }

        "clickup_checklist_delete" => {
            let checklist_id = args
                .get("checklist_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let path = format!("/v2/checklist/{}", checklist_id);
            client.delete(&path).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Checklist {} deleted", checklist_id)}))
        }

        "clickup_goal_list" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v2/team/{}/goal", team_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let goals = resp.get("goals").and_then(|g| g.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&goals, &["id", "name", "percent_completed", "due_date"]))
        }

        "clickup_goal_get" => {
            let goal_id = args
                .get("goal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            let path = format!("/v2/goal/{}", goal_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            Ok(compact_items(&[goal], &["id", "name", "percent_completed", "due_date", "description"]))
        }

        "clickup_goal_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) {
                body["due_date"] = json!(due_date);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(owner_ids) = args.get("owner_ids") {
                body["owners"] = owner_ids.clone();
            }
            let path = format!("/v2/team/{}/goal", team_id);
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            Ok(compact_items(&[goal], &["id", "name"]))
        }

        "clickup_goal_update" => {
            let goal_id = args
                .get("goal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) {
                body["due_date"] = json!(due_date);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            let path = format!("/v2/goal/{}", goal_id);
            let resp = client.put(&path, &body).await.map_err(|e| e.to_string())?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            Ok(compact_items(&[goal], &["id", "name"]))
        }

        "clickup_view_list" => {
            let path = if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                format!("/v2/space/{}/view", space_id)
            } else if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                format!("/v2/folder/{}/view", folder_id)
            } else if let Some(list_id) = args.get("list_id").and_then(|v| v.as_str()) {
                format!("/v2/list/{}/view", list_id)
            } else {
                let team_id = resolve_workspace(args)?;
                format!("/v2/team/{}/view", team_id)
            };
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let views = resp.get("views").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&views, &["id", "name", "type"]))
        }

        "clickup_view_tasks" => {
            let view_id = args
                .get("view_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
            let path = format!("/v2/view/{}/task?page={}", view_id, page);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tasks = resp.get("tasks").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&tasks, &["id", "name", "status", "priority", "assignees", "due_date"]))
        }

        "clickup_doc_list" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v3/workspaces/{}/docs", team_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let docs = resp.get("docs").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&docs, &["id", "name", "date_created"]))
        }

        "clickup_doc_get" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let path = format!("/v3/workspaces/{}/docs/{}", team_id, doc_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "date_created"]))
        }

        "clickup_doc_pages" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let content = args.get("content").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = format!("/v3/workspaces/{}/docs/{}/pages?content_format=text/md&max_page_depth=-1&include_content={}", team_id, doc_id, content);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let pages = resp.get("pages").and_then(|p| p.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&pages, &["id", "name"]))
        }

        "clickup_tag_list" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let path = format!("/v2/space/{}/tag", space_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tags = resp.get("tags").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&tags, &["name", "tag_fg", "tag_bg"]))
        }

        "clickup_task_add_tag" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let tag_name = args
                .get("tag_name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            let path = format!("/v2/task/{}/tag/{}", task_id, tag_name);
            client.post(&path, &json!({})).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' added to task {}", tag_name, task_id)}))
        }

        "clickup_task_remove_tag" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let tag_name = args
                .get("tag_name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            let path = format!("/v2/task/{}/tag/{}", task_id, tag_name);
            client.delete(&path).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' removed from task {}", tag_name, task_id)}))
        }

        "clickup_webhook_list" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v2/team/{}/webhook", team_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let webhooks = resp.get("webhooks").and_then(|w| w.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&webhooks, &["id", "endpoint", "events", "status"]))
        }

        "clickup_member_list" => {
            let path = if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) {
                format!("/v2/task/{}/member", task_id)
            } else if let Some(list_id) = args.get("list_id").and_then(|v| v.as_str()) {
                format!("/v2/list/{}/member", list_id)
            } else {
                return Err("Provide either task_id or list_id".to_string());
            };
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let members = resp.get("members").and_then(|m| m.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&members, &["id", "username", "email"]))
        }

        "clickup_template_list" => {
            let team_id = resolve_workspace(args)?;
            let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
            let path = format!("/v2/team/{}/taskTemplate?page={}", team_id, page);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let templates = resp.get("templates").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&templates, &["id", "name"]))
        }

        "clickup_space_get" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let resp = client.get(&format!("/v2/space/{}", space_id)).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "private", "archived"]))
        }

        "clickup_space_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) {
                body["private"] = json!(private);
            }
            let resp = client.post(&format!("/v2/team/{}/space", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "private"]))
        }

        "clickup_space_update" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) { body["private"] = json!(private); }
            if let Some(archived) = args.get("archived").and_then(|v| v.as_bool()) { body["archived"] = json!(archived); }
            let resp = client.put(&format!("/v2/space/{}", space_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "private", "archived"]))
        }

        "clickup_space_delete" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            client.delete(&format!("/v2/space/{}", space_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Space {} deleted", space_id)}))
        }

        "clickup_folder_get" => {
            let folder_id = args.get("folder_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let resp = client.get(&format!("/v2/folder/{}", folder_id)).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "task_count"]))
        }

        "clickup_folder_create" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client.post(&format!("/v2/space/{}/folder", space_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_folder_update" => {
            let folder_id = args.get("folder_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client.put(&format!("/v2/folder/{}", folder_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_folder_delete" => {
            let folder_id = args.get("folder_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            client.delete(&format!("/v2/folder/{}", folder_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Folder {} deleted", folder_id)}))
        }

        "clickup_list_get" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let resp = client.get(&format!("/v2/list/{}", list_id)).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "task_count", "status", "due_date"]))
        }

        "clickup_list_create" => {
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) { body["content"] = json!(content); }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) { body["due_date"] = json!(due_date); }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) { body["status"] = json!(status); }
            let path = if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                format!("/v2/folder/{}/list", folder_id)
            } else if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                format!("/v2/space/{}/list", space_id)
            } else {
                return Err("Provide either folder_id or space_id".to_string());
            };
            let resp = client.post(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_list_update" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) { body["content"] = json!(content); }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) { body["due_date"] = json!(due_date); }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) { body["status"] = json!(status); }
            let resp = client.put(&format!("/v2/list/{}", list_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "task_count", "status"]))
        }

        "clickup_list_delete" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            client.delete(&format!("/v2/list/{}", list_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("List {} deleted", list_id)}))
        }

        "clickup_list_add_task" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            client.post(&format!("/v2/list/{}/task/{}", list_id, task_id), &json!({})).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} added to list {}", task_id, list_id)}))
        }

        "clickup_list_remove_task" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            client.delete(&format!("/v2/list/{}/task/{}", list_id, task_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} removed from list {}", task_id, list_id)}))
        }

        "clickup_comment_update" => {
            let comment_id = args.get("comment_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let text = args.get("text").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let mut body = json!({"comment_text": text});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) { body["assignee"] = json!(assignee); }
            if let Some(resolved) = args.get("resolved").and_then(|v| v.as_bool()) { body["resolved"] = json!(resolved); }
            client.put(&format!("/v2/comment/{}", comment_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Comment {} updated", comment_id)}))
        }

        "clickup_comment_delete" => {
            let comment_id = args.get("comment_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            client.delete(&format!("/v2/comment/{}", comment_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Comment {} deleted", comment_id)}))
        }

        "clickup_task_add_dep" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let mut body = json!({});
            if let Some(dep) = args.get("depends_on").and_then(|v| v.as_str()) { body["depends_on"] = json!(dep); }
            if let Some(dep) = args.get("dependency_of").and_then(|v| v.as_str()) { body["dependency_of"] = json!(dep); }
            client.post(&format!("/v2/task/{}/dependency", task_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Dependency added to task {}", task_id)}))
        }

        "clickup_task_remove_dep" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let mut body = json!({});
            if let Some(dep) = args.get("depends_on").and_then(|v| v.as_str()) { body["depends_on"] = json!(dep); }
            if let Some(dep) = args.get("dependency_of").and_then(|v| v.as_str()) { body["dependency_of"] = json!(dep); }
            client.delete_with_body(&format!("/v2/task/{}/dependency", task_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Dependency removed from task {}", task_id)}))
        }

        "clickup_task_link" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let links_to = args.get("links_to").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: links_to")?;
            let resp = client.post(&format!("/v2/task/{}/link/{}", task_id, links_to), &json!({})).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} linked to {}", task_id, links_to), "data": resp}))
        }

        "clickup_task_unlink" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let links_to = args.get("links_to").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: links_to")?;
            client.delete(&format!("/v2/task/{}/link/{}", task_id, links_to)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} unlinked from {}", task_id, links_to)}))
        }

        "clickup_goal_delete" => {
            let goal_id = args.get("goal_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            client.delete(&format!("/v2/goal/{}", goal_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Goal {} deleted", goal_id)}))
        }

        "clickup_goal_add_kr" => {
            let goal_id = args.get("goal_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let kr_type = args.get("type").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let steps_start = args.get("steps_start").and_then(|v| v.as_f64())
                .ok_or("Missing required parameter: steps_start")?;
            let steps_end = args.get("steps_end").and_then(|v| v.as_f64())
                .ok_or("Missing required parameter: steps_end")?;
            let mut body = json!({"name": name, "type": kr_type, "steps_start": steps_start, "steps_end": steps_end});
            if let Some(unit) = args.get("unit").and_then(|v| v.as_str()) { body["unit"] = json!(unit); }
            if let Some(owners) = args.get("owner_ids") { body["owners"] = owners.clone(); }
            if let Some(task_ids) = args.get("task_ids") { body["task_ids"] = task_ids.clone(); }
            if let Some(list_ids) = args.get("list_ids") { body["list_ids"] = list_ids.clone(); }
            let resp = client.post(&format!("/v2/goal/{}/key_result", goal_id), &body).await.map_err(|e| e.to_string())?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            Ok(compact_items(&[kr], &["id", "name", "type", "steps_start", "steps_end", "steps_current"]))
        }

        "clickup_goal_update_kr" => {
            let kr_id = args.get("kr_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: kr_id")?;
            let mut body = json!({});
            if let Some(v) = args.get("steps_current").and_then(|v| v.as_f64()) { body["steps_current"] = json!(v); }
            if let Some(v) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(v); }
            if let Some(v) = args.get("unit").and_then(|v| v.as_str()) { body["unit"] = json!(v); }
            let resp = client.put(&format!("/v2/key_result/{}", kr_id), &body).await.map_err(|e| e.to_string())?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            Ok(compact_items(&[kr], &["id", "name", "steps_current", "steps_end"]))
        }

        "clickup_goal_delete_kr" => {
            let kr_id = args.get("kr_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: kr_id")?;
            client.delete(&format!("/v2/key_result/{}", kr_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Key result {} deleted", kr_id)}))
        }

        "clickup_time_get" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args.get("timer_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let resp = client.get(&format!("/v2/team/{}/time_entries/{}", team_id, timer_id)).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "end", "billable"]))
        }

        "clickup_time_create" => {
            let team_id = resolve_workspace(args)?;
            let start = args.get("start").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: start")?;
            let duration = args.get("duration").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: duration")?;
            let mut body = json!({"start": start, "duration": duration});
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) { body["tid"] = json!(task_id); }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) { body["description"] = json!(desc); }
            if let Some(billable) = args.get("billable").and_then(|v| v.as_bool()) { body["billable"] = json!(billable); }
            let resp = client.post(&format!("/v2/team/{}/time_entries", team_id), &body).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "billable"]))
        }

        "clickup_time_update" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args.get("timer_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let mut body = json!({});
            if let Some(start) = args.get("start").and_then(|v| v.as_i64()) { body["start"] = json!(start); }
            if let Some(duration) = args.get("duration").and_then(|v| v.as_i64()) { body["duration"] = json!(duration); }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) { body["description"] = json!(desc); }
            if let Some(billable) = args.get("billable").and_then(|v| v.as_bool()) { body["billable"] = json!(billable); }
            let resp = client.put(&format!("/v2/team/{}/time_entries/{}", team_id, timer_id), &body).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "billable"]))
        }

        "clickup_time_delete" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args.get("timer_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            client.delete(&format!("/v2/team/{}/time_entries/{}", team_id, timer_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time entry {} deleted", timer_id)}))
        }

        "clickup_view_get" => {
            let view_id = args.get("view_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            let resp = client.get(&format!("/v2/view/{}", view_id)).await.map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_create" => {
            let scope = args.get("scope").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: scope")?;
            let scope_id = args.get("scope_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: scope_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let view_type = args.get("type").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let body = json!({"name": name, "type": view_type});
            let resp = client.post(&format!("/v2/{}/{}/view", scope, scope_id), &body).await.map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_update" => {
            let view_id = args.get("view_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let view_type = args.get("type").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let body = json!({"name": name, "type": view_type});
            let resp = client.put(&format!("/v2/view/{}", view_id), &body).await.map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_delete" => {
            let view_id = args.get("view_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            client.delete(&format!("/v2/view/{}", view_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("View {} deleted", view_id)}))
        }

        "clickup_doc_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(parent) = args.get("parent") { body["parent"] = parent.clone(); }
            let resp = client.post(&format!("/v3/workspaces/{}/docs", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_doc_add_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args.get("doc_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) { body["content"] = json!(content); }
            if let Some(subtitle) = args.get("sub_title").and_then(|v| v.as_str()) { body["sub_title"] = json!(subtitle); }
            if let Some(parent_page_id) = args.get("parent_page_id").and_then(|v| v.as_str()) { body["parent_page_id"] = json!(parent_page_id); }
            let resp = client.post(&format!("/v3/workspaces/{}/docs/{}/pages", team_id, doc_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_doc_edit_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args.get("doc_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let page_id = args.get("page_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: page_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) { body["content"] = json!(content); }
            let resp = client.put(&format!("/v3/workspaces/{}/docs/{}/pages/{}", team_id, doc_id, page_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_chat_channel_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) { body["description"] = json!(desc); }
            if let Some(vis) = args.get("visibility").and_then(|v| v.as_str()) { body["visibility"] = json!(vis); }
            let resp = client.post(&format!("/v3/workspaces/{}/chat/channels", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "visibility"]))
        }

        "clickup_chat_channel_get" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/channels/{}", team_id, channel_id)).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "visibility"]))
        }

        "clickup_chat_channel_update" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) { body["description"] = json!(desc); }
            let resp = client.patch(&format!("/v3/workspaces/{}/chat/channels/{}", team_id, channel_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_chat_channel_delete" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            client.delete(&format!("/v3/workspaces/{}/chat/channels/{}", team_id, channel_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Channel {} deleted", channel_id)}))
        }

        "clickup_chat_message_list" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let mut path = format!("/v3/workspaces/{}/chat/channels/{}/messages", team_id, channel_id);
            if let Some(cursor) = args.get("cursor").and_then(|v| v.as_str()) {
                path.push_str(&format!("?cursor={}", cursor));
            }
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let messages = resp.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&messages, &["id", "content", "date"]))
        }

        "clickup_chat_message_send" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let content = args.get("content").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: content")?;
            let body = json!({"content": content});
            let resp = client.post(&format!("/v3/workspaces/{}/chat/channels/{}/messages", team_id, channel_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Message sent", "id": resp.get("id")}))
        }

        "clickup_chat_message_delete" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            client.delete(&format!("/v3/workspaces/{}/chat/messages/{}", team_id, message_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Message {} deleted", message_id)}))
        }

        "clickup_chat_dm" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let content = args.get("content").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: content")?;
            let body = json!({"user_id": user_id, "content": content});
            let resp = client.post(&format!("/v3/workspaces/{}/chat/channels/direct_message", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "DM sent", "id": resp.get("id")}))
        }

        "clickup_webhook_create" => {
            let team_id = resolve_workspace(args)?;
            let endpoint = args.get("endpoint").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: endpoint")?;
            let events = args.get("events").ok_or("Missing required parameter: events")?;
            let mut body = json!({"endpoint": endpoint, "events": events});
            if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) { body["space_id"] = json!(space_id); }
            if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) { body["folder_id"] = json!(folder_id); }
            if let Some(list_id) = args.get("list_id").and_then(|v| v.as_str()) { body["list_id"] = json!(list_id); }
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) { body["task_id"] = json!(task_id); }
            let resp = client.post(&format!("/v2/team/{}/webhook", team_id), &body).await.map_err(|e| e.to_string())?;
            let webhook = resp.get("webhook").cloned().unwrap_or(resp);
            Ok(compact_items(&[webhook], &["id", "endpoint", "events", "status"]))
        }

        "clickup_webhook_update" => {
            let webhook_id = args.get("webhook_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: webhook_id")?;
            let mut body = json!({});
            if let Some(endpoint) = args.get("endpoint").and_then(|v| v.as_str()) { body["endpoint"] = json!(endpoint); }
            if let Some(events) = args.get("events") { body["events"] = events.clone(); }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) { body["status"] = json!(status); }
            let resp = client.put(&format!("/v2/webhook/{}", webhook_id), &body).await.map_err(|e| e.to_string())?;
            let webhook = resp.get("webhook").cloned().unwrap_or(resp);
            Ok(compact_items(&[webhook], &["id", "endpoint", "events", "status"]))
        }

        "clickup_webhook_delete" => {
            let webhook_id = args.get("webhook_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: webhook_id")?;
            client.delete(&format!("/v2/webhook/{}", webhook_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Webhook {} deleted", webhook_id)}))
        }

        "clickup_checklist_add_item" => {
            let checklist_id = args.get("checklist_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) { body["assignee"] = json!(assignee); }
            let resp = client.post(&format!("/v2/checklist/{}/checklist_item", checklist_id), &body).await.map_err(|e| e.to_string())?;
            let item = resp.get("checklist").cloned().unwrap_or(resp);
            Ok(compact_items(&[item], &["id", "name"]))
        }

        "clickup_checklist_update_item" => {
            let checklist_id = args.get("checklist_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let item_id = args.get("item_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: item_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(resolved) = args.get("resolved").and_then(|v| v.as_bool()) { body["resolved"] = json!(resolved); }
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) { body["assignee"] = json!(assignee); }
            client.put(&format!("/v2/checklist/{}/checklist_item/{}", checklist_id, item_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Checklist item {} updated", item_id)}))
        }

        "clickup_checklist_delete_item" => {
            let checklist_id = args.get("checklist_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let item_id = args.get("item_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: item_id")?;
            client.delete(&format!("/v2/checklist/{}/checklist_item/{}", checklist_id, item_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Checklist item {} deleted", item_id)}))
        }

        "clickup_user_get" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let resp = client.get(&format!("/v2/team/{}/user/{}", team_id, user_id)).await.map_err(|e| e.to_string())?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            Ok(compact_items(&[member], &["user", "role"]))
        }

        "clickup_workspace_seats" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/seats", team_id)).await.map_err(|e| e.to_string())?;
            Ok(json!(resp))
        }

        "clickup_workspace_plan" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/plan", team_id)).await.map_err(|e| e.to_string())?;
            Ok(json!(resp))
        }

        "clickup_tag_create" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut tag = json!({"name": name});
            if let Some(fg) = args.get("tag_fg").and_then(|v| v.as_str()) { tag["tag_fg"] = json!(fg); }
            if let Some(bg) = args.get("tag_bg").and_then(|v| v.as_str()) { tag["tag_bg"] = json!(bg); }
            let body = json!({"tag": tag});
            client.post(&format!("/v2/space/{}/tag", space_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' created in space {}", name, space_id)}))
        }

        "clickup_tag_update" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let tag_name = args.get("tag_name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            let mut tag = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { tag["name"] = json!(name); }
            if let Some(fg) = args.get("tag_fg").and_then(|v| v.as_str()) { tag["tag_fg"] = json!(fg); }
            if let Some(bg) = args.get("tag_bg").and_then(|v| v.as_str()) { tag["tag_bg"] = json!(bg); }
            let body = json!({"tag": tag});
            client.put(&format!("/v2/space/{}/tag/{}", space_id, tag_name), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' updated", tag_name)}))
        }

        "clickup_tag_delete" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let tag_name = args.get("tag_name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            client.delete(&format!("/v2/space/{}/tag/{}", space_id, tag_name)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' deleted from space {}", tag_name, space_id)}))
        }

        "clickup_field_unset" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let field_id = args.get("field_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: field_id")?;
            client.delete(&format!("/v2/task/{}/field/{}", task_id, field_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Field {} unset on task {}", field_id, task_id)}))
        }

        "clickup_attachment_list" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/task/{}/attachments", team_id, task_id)).await.map_err(|e| e.to_string())?;
            let attachments = resp.get("attachments").and_then(|a| a.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&attachments, &["id", "title", "url", "date"]))
        }

        "clickup_shared_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/shared", team_id)).await.map_err(|e| e.to_string())?;
            Ok(json!(resp))
        }

        "clickup_group_list" => {
            let team_id = resolve_workspace(args)?;
            let mut qs = format!("team_id={}", team_id);
            if let Some(group_ids) = args.get("group_ids").and_then(|v| v.as_array()) {
                for id in group_ids {
                    if let Some(id) = id.as_str() {
                        qs.push_str(&format!("&group_ids[]={}", id));
                    }
                }
            }
            let resp = client.get(&format!("/v2/group?{}", qs)).await.map_err(|e| e.to_string())?;
            let groups = resp.get("groups").and_then(|g| g.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&groups, &["id", "name", "members"]))
        }

        "clickup_group_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(members) = args.get("member_ids") { body["members"] = members.clone(); }
            let resp = client.post(&format!("/v2/team/{}/group", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_group_update" => {
            let group_id = args.get("group_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: group_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(add) = args.get("add_members") {
                body["members"] = json!({"add": add, "rem": args.get("rem_members").cloned().unwrap_or(json!([]))});
            } else if let Some(rem) = args.get("rem_members") {
                body["members"] = json!({"add": [], "rem": rem});
            }
            let resp = client.put(&format!("/v2/group/{}", group_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_group_delete" => {
            let group_id = args.get("group_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: group_id")?;
            client.delete(&format!("/v2/group/{}", group_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Group {} deleted", group_id)}))
        }

        "clickup_role_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/customroles", team_id)).await.map_err(|e| e.to_string())?;
            let roles = resp.get("roles").and_then(|r| r.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&roles, &["id", "name"]))
        }

        "clickup_guest_get" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let resp = client.get(&format!("/v2/team/{}/guest/{}", team_id, guest_id)).await.map_err(|e| e.to_string())?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            Ok(compact_items(&[guest], &["user", "role"]))
        }

        "clickup_task_time_in_status" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let resp = client.get(&format!("/v2/task/{}/time_in_status", task_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_task_move" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            client.put(&format!("/v3/workspaces/{}/tasks/{}/home_list/{}", team_id, task_id, list_id), &json!({})).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} moved to list {}", task_id, list_id)}))
        }

        "clickup_task_set_estimate" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let time_estimate = args.get("time_estimate").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: time_estimate")?;
            let body = json!({"time_estimates": [{"user_id": user_id, "time_estimate": time_estimate}]});
            client.patch(&format!("/v3/workspaces/{}/tasks/{}/time_estimates_by_user", team_id, task_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time estimate set for task {}", task_id)}))
        }

        "clickup_task_replace_estimates" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let time_estimate = args.get("time_estimate").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: time_estimate")?;
            let body = json!({"time_estimates": [{"user_id": user_id, "time_estimate": time_estimate}]});
            client.put(&format!("/v3/workspaces/{}/tasks/{}/time_estimates_by_user", team_id, task_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time estimates replaced for task {}", task_id)}))
        }

        "clickup_auth_check" => {
            client.get("/v2/user").await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Token valid"}))
        }

        "clickup_checklist_update" => {
            let checklist_id = args.get("checklist_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) { body["name"] = json!(name); }
            if let Some(position) = args.get("position").and_then(|v| v.as_i64()) { body["position"] = json!(position); }
            let resp = client.put(&format!("/v2/checklist/{}", checklist_id), &body).await.map_err(|e| e.to_string())?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            Ok(compact_items(&[checklist], &["id", "name"]))
        }

        "clickup_comment_replies" => {
            let comment_id = args.get("comment_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let resp = client.get(&format!("/v2/comment/{}/reply", comment_id)).await.map_err(|e| e.to_string())?;
            let comments = resp.get("comments")
                .or_else(|| resp.get("replies"))
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&comments, &["id", "user", "date", "comment_text"]))
        }

        "clickup_comment_reply" => {
            let comment_id = args.get("comment_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let text = args.get("text").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let mut body = json!({"comment_text": text});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) { body["assignee"] = json!(assignee); }
            let resp = client.post(&format!("/v2/comment/{}/reply", comment_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Reply posted", "id": resp.get("id")}))
        }

        "clickup_chat_channel_list" => {
            let team_id = resolve_workspace(args)?;
            let mut path = format!("/v3/workspaces/{}/chat/channels", team_id);
            if let Some(include_closed) = args.get("include_closed").and_then(|v| v.as_bool()) {
                path.push_str(&format!("?include_closed={}", include_closed));
            }
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let channels = resp.get("channels").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&channels, &["id", "name", "type"]))
        }

        "clickup_chat_channel_followers" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/channels/{}/followers", team_id, channel_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_channel_members" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args.get("channel_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/channels/{}/members", team_id, channel_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_message_update" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let text = args.get("text").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let body = json!({"content": text});
            client.patch(&format!("/v3/workspaces/{}/chat/messages/{}", team_id, message_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Message {} updated", message_id)}))
        }

        "clickup_chat_reaction_list" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/messages/{}/reactions", team_id, message_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_reaction_add" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let emoji = args.get("emoji").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: emoji")?;
            let body = json!({"emoji": emoji});
            client.post(&format!("/v3/workspaces/{}/chat/messages/{}/reactions", team_id, message_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Reaction '{}' added to message {}", emoji, message_id)}))
        }

        "clickup_chat_reaction_remove" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let emoji = args.get("emoji").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: emoji")?;
            client.delete(&format!("/v3/workspaces/{}/chat/messages/{}/reactions/{}", team_id, message_id, emoji)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Reaction '{}' removed from message {}", emoji, message_id)}))
        }

        "clickup_chat_reply_list" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/messages/{}/replies", team_id, message_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_reply_send" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let text = args.get("text").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let body = json!({"content": text});
            let resp = client.post(&format!("/v3/workspaces/{}/chat/messages/{}/replies", team_id, message_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Reply sent", "id": resp.get("id")}))
        }

        "clickup_chat_tagged_users" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/chat/messages/{}/tagged_users", team_id, message_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_time_current" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/time_entries/current", team_id)).await.map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(&[data], &["id", "task", "duration", "start", "billable"]))
        }

        "clickup_time_tags" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/time_entries/tags", team_id)).await.map_err(|e| e.to_string())?;
            let tags = resp.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&tags, &["name"]))
        }

        "clickup_time_add_tags" => {
            let team_id = resolve_workspace(args)?;
            let entry_ids = args.get("entry_ids").and_then(|v| v.as_array())
                .ok_or("Missing required parameter: entry_ids")?;
            let tag_names = args.get("tag_names").and_then(|v| v.as_array())
                .ok_or("Missing required parameter: tag_names")?;
            let tags: Vec<Value> = tag_names.iter()
                .filter_map(|n| n.as_str())
                .map(|n| json!({"name": n}))
                .collect();
            let body = json!({"time_entry_ids": entry_ids, "tags": tags});
            client.post(&format!("/v2/team/{}/time_entries/tags", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Tags added to time entries"}))
        }

        "clickup_time_remove_tags" => {
            let team_id = resolve_workspace(args)?;
            let entry_ids = args.get("entry_ids").and_then(|v| v.as_array())
                .ok_or("Missing required parameter: entry_ids")?;
            let tag_names = args.get("tag_names").and_then(|v| v.as_array())
                .ok_or("Missing required parameter: tag_names")?;
            let tags: Vec<Value> = tag_names.iter()
                .filter_map(|n| n.as_str())
                .map(|n| json!({"name": n}))
                .collect();
            let body = json!({"time_entry_ids": entry_ids, "tags": tags});
            client.delete_with_body(&format!("/v2/team/{}/time_entries/tags", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Tags removed from time entries"}))
        }

        "clickup_time_rename_tag" => {
            let team_id = resolve_workspace(args)?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let new_name = args.get("new_name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: new_name")?;
            let body = json!({"name": name, "new_name": new_name});
            client.put(&format!("/v2/team/{}/time_entries/tags", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' renamed to '{}'", name, new_name)}))
        }

        "clickup_time_history" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args.get("timer_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let resp = client.get(&format!("/v2/team/{}/time_entries/{}/history", team_id, timer_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_guest_invite" => {
            let team_id = resolve_workspace(args)?;
            let email = args.get("email").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: email")?;
            let mut body = json!({"email": email});
            if let Some(v) = args.get("can_edit_tags").and_then(|v| v.as_bool()) { body["can_edit_tags"] = json!(v); }
            if let Some(v) = args.get("can_see_time_spent").and_then(|v| v.as_bool()) { body["can_see_time_spent"] = json!(v); }
            if let Some(v) = args.get("can_create_views").and_then(|v| v.as_bool()) { body["can_create_views"] = json!(v); }
            let resp = client.post(&format!("/v2/team/{}/guest", team_id), &body).await.map_err(|e| e.to_string())?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            let user = guest.get("user").cloned().unwrap_or(guest);
            Ok(compact_items(&[user], &["id", "email"]))
        }

        "clickup_guest_update" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let mut body = json!({});
            if let Some(v) = args.get("can_edit_tags").and_then(|v| v.as_bool()) { body["can_edit_tags"] = json!(v); }
            if let Some(v) = args.get("can_see_time_spent").and_then(|v| v.as_bool()) { body["can_see_time_spent"] = json!(v); }
            if let Some(v) = args.get("can_create_views").and_then(|v| v.as_bool()) { body["can_create_views"] = json!(v); }
            client.put(&format!("/v2/team/{}/guest/{}", team_id, guest_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} updated", guest_id)}))
        }

        "clickup_guest_remove" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client.delete(&format!("/v2/team/{}/guest/{}", team_id, guest_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} removed", guest_id)}))
        }

        "clickup_guest_share_task" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args.get("permission").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client.post(&format!("/v2/task/{}/guest/{}", task_id, guest_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} shared with guest {}", task_id, guest_id)}))
        }

        "clickup_guest_unshare_task" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client.delete(&format!("/v2/task/{}/guest/{}", task_id, guest_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from task {}", guest_id, task_id)}))
        }

        "clickup_guest_share_list" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args.get("permission").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client.post(&format!("/v2/list/{}/guest/{}", list_id, guest_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("List {} shared with guest {}", list_id, guest_id)}))
        }

        "clickup_guest_unshare_list" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client.delete(&format!("/v2/list/{}/guest/{}", list_id, guest_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from list {}", guest_id, list_id)}))
        }

        "clickup_guest_share_folder" => {
            let folder_id = args.get("folder_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args.get("permission").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client.post(&format!("/v2/folder/{}/guest/{}", folder_id, guest_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Folder {} shared with guest {}", folder_id, guest_id)}))
        }

        "clickup_guest_unshare_folder" => {
            let folder_id = args.get("folder_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let guest_id = args.get("guest_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client.delete(&format!("/v2/folder/{}/guest/{}", folder_id, guest_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from folder {}", guest_id, folder_id)}))
        }

        "clickup_user_invite" => {
            let team_id = resolve_workspace(args)?;
            let email = args.get("email").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: email")?;
            let mut body = json!({"email": email});
            if let Some(admin) = args.get("admin").and_then(|v| v.as_bool()) { body["admin"] = json!(admin); }
            let resp = client.post(&format!("/v2/team/{}/user", team_id), &body).await.map_err(|e| e.to_string())?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            let user = member.get("user").cloned().unwrap_or(member);
            Ok(compact_items(&[user], &["id", "username", "email"]))
        }

        "clickup_user_update" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let mut body = json!({});
            if let Some(username) = args.get("username").and_then(|v| v.as_str()) { body["username"] = json!(username); }
            if let Some(admin) = args.get("admin").and_then(|v| v.as_bool()) { body["admin"] = json!(admin); }
            client.put(&format!("/v2/team/{}/user/{}", team_id, user_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("User {} updated", user_id)}))
        }

        "clickup_user_remove" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args.get("user_id").and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            client.delete(&format!("/v2/team/{}/user/{}", team_id, user_id)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("User {} removed from workspace", user_id)}))
        }

        "clickup_template_apply_task" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let template_id = args.get("template_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client.post(&format!("/v2/list/{}/taskTemplate/{}", list_id, template_id), &body).await.map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_template_apply_list" => {
            let template_id = args.get("template_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let path = if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                format!("/v2/folder/{}/list_template/{}", folder_id, template_id)
            } else if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                format!("/v2/space/{}/list_template/{}", space_id, template_id)
            } else {
                return Err("Provide either folder_id or space_id".to_string());
            };
            client.post(&path, &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("List '{}' created from template {}", name, template_id)}))
        }

        "clickup_template_apply_folder" => {
            let space_id = args.get("space_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let template_id = args.get("template_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args.get("name").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            client.post(&format!("/v2/space/{}/folder_template/{}", space_id, template_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Folder '{}' created from template {}", name, template_id)}))
        }

        "clickup_attachment_upload" => {
            let task_id = args.get("task_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let file_path = args.get("file_path").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: file_path")?;
            let path = format!("/v2/task/{}/attachment", task_id);
            let resp = client.upload_file(&path, std::path::Path::new(file_path)).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "File uploaded", "id": resp.get("id"), "url": resp.get("url")}))
        }

        "clickup_task_type_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client.get(&format!("/v2/team/{}/custom_item", team_id)).await.map_err(|e| e.to_string())?;
            let items = resp.get("custom_items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
            Ok(compact_items(&items, &["id", "name", "name_plural"]))
        }

        "clickup_doc_get_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args.get("doc_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let page_id = args.get("page_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: page_id")?;
            let resp = client.get(&format!("/v3/workspaces/{}/docs/{}/pages/{}", team_id, doc_id, page_id)).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_audit_log_query" => {
            let team_id = resolve_workspace(args)?;
            let event_type = args.get("type").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let mut body = json!({"type": event_type});
            if let Some(user_id) = args.get("user_id").and_then(|v| v.as_i64()) {
                body["user_id"] = json!(user_id);
            }
            if let Some(start_date) = args.get("start_date").and_then(|v| v.as_i64()) {
                body["date_filter"] = json!({"start_date": start_date, "end_date": args.get("end_date").and_then(|v| v.as_i64()).unwrap_or(i64::MAX)});
            } else if let Some(end_date) = args.get("end_date").and_then(|v| v.as_i64()) {
                body["date_filter"] = json!({"end_date": end_date});
            }
            let resp = client.post(&format!("/v3/workspaces/{}/auditlogs", team_id), &body).await.map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_acl_update" => {
            let team_id = resolve_workspace(args)?;
            let object_type = args.get("object_type").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: object_type")?;
            let object_id = args.get("object_id").and_then(|v| v.as_str())
                .ok_or("Missing required parameter: object_id")?;
            let mut body = json!({});
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) { body["private"] = json!(private); }
            client.patch(&format!("/v3/workspaces/{}/{}/{}/acls", team_id, object_type, object_id), &body).await.map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("ACL updated for {} {}", object_type, object_id)}))
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
