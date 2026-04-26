use crate::client::ClickUpClient;
use crate::config::Config;
use crate::output::{compact_items, flatten_value};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

pub mod classify;
pub mod filter;

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

/// Inspects a `tools/call` request and returns a JSON-RPC response when the
/// request can be resolved WITHOUT executing the tool itself (missing tool
/// name, or tool is filtered out). Returns `None` when the caller should
/// proceed to invoke the tool.
pub fn handle_tools_call_early(
    id: &Value,
    params: &Value,
    filter: &filter::Filter,
) -> Option<Value> {
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

    if tool_name.is_empty() {
        return Some(ok_response(id, tool_error("Missing tool name".to_string())));
    }

    if !filter.allows(tool_name) {
        return Some(error_response(
            id,
            -32601,
            &format!("Method not found: {} (filtered out at startup)", tool_name),
        ));
    }

    None
}

// ── Tool definitions ──────────────────────────────────────────────────────────

pub fn tool_list() -> Value {
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
            "description": "List all spaces in a ClickUp workspace. Spaces are the top-level containers below the workspace and hold folders, lists, and tasks. Returns a compact array of space objects (id, name, private, archived). Use clickup_folder_list or clickup_list_list with a space_id to drill down.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config (defaults.workspace_id)."},
                    "archived": {"type": "boolean", "description": "true = include archived spaces in the result; false or omitted = only active spaces. Defaults to false."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_folder_list",
            "description": "List all folders in a ClickUp space. Folders are optional groupings that contain lists; a space may also have folderless lists (use clickup_list_list with space_id for those). Returns a compact array of folder objects (id, name, task_count, archived).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the parent space. Obtain from clickup_space_list (field: id)."},
                    "archived": {"type": "boolean", "description": "true = include archived folders; false or omitted = only active folders. Defaults to false."}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_list_list",
            "description": "List ClickUp lists under either a folder or a space (folderless lists). Exactly one of folder_id or space_id must be provided. Returns a compact array of list objects (id, name, task_count, archived). Use clickup_task_list to drill into a specific list.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the parent folder. Obtain from clickup_folder_list (field: id). Mutually exclusive with space_id."},
                    "space_id": {"type": "string", "description": "ID of a space — returns only the folderless lists attached directly to the space. Obtain from clickup_space_list (field: id). Mutually exclusive with folder_id."},
                    "archived": {"type": "boolean", "description": "true = include archived lists; false or omitted = only active lists. Defaults to false."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_task_list",
            "description": "List tasks in a specific ClickUp list with optional status/assignee filters. Returns the first page of task objects in compact form (id, name, status, assignees, due_date). For cross-list or cross-space queries use clickup_task_search instead; for a single task use clickup_task_get.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to read tasks from. Obtain from clickup_list_list (field: id)."},
                    "statuses": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Status names to include (e.g. ['open','in progress']). Case-sensitive, must match a status defined on the list. Omit to return tasks in any open status."
                    },
                    "assignees": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "User IDs (as strings) to filter assignees. Obtain from clickup_member_list or clickup_user_get. Omit to return tasks regardless of assignee."
                    },
                    "include_closed": {"type": "boolean", "description": "true = include tasks whose status is in the 'closed' group; false or omitted = exclude closed tasks from the response."}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_task_get",
            "description": "Fetch the full object for a single ClickUp task — name, description, status, assignees, tags, custom fields, checklists, due date, time estimates, dependencies, and more. Returns the task object. Use clickup_task_list or clickup_task_search to find a task_id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to fetch. Obtain from clickup_task_list (field: id) or clickup_task_search."},
                    "include_subtasks": {"type": "boolean", "description": "true = include the task's subtasks in the response under the 'subtasks' field; false or omitted = return only the parent task."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_create",
            "description": "Create a new task in a ClickUp list. The task starts in the list's default status unless 'status' is supplied. Returns the created task object including its new id, which you can pass to clickup_task_update, clickup_task_get, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list the task will live in. Obtain from clickup_list_list (field: id)."},
                    "name": {"type": "string", "description": "Task title shown in the list view. Required and non-empty."},
                    "description": {"type": "string", "description": "Task body. Markdown supported (headings, links, checkboxes, @mentions). Omit to create the task with no description."},
                    "status": {"type": "string", "description": "Status name to start in (case-sensitive; must match a status configured on the list). Omit to use the list's default initial status."},
                    "priority": {"type": "integer", "description": "Task priority: 1=Urgent, 2=High, 3=Normal, 4=Low. Omit for no priority."},
                    "assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to assign to the task. Obtain from clickup_member_list or clickup_user_get. Omit for an unassigned task."
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tag names to apply. Tags must already exist in the parent space (use clickup_tag_list to see available tags or clickup_tag_create to add new ones)."
                    },
                    "due_date": {"type": "integer", "description": "Due date as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01). Omit for no due date."}
                },
                "required": ["list_id", "name"]
            }
        },
        {
            "name": "clickup_task_update",
            "description": "Update fields on an existing ClickUp task — name, description, status, priority, and incrementally add/remove assignees. Only provided fields are changed; omitted fields keep their current value. For tags use clickup_task_add_tag/remove_tag; for moving between lists use clickup_task_move. Returns the updated task object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to update. Obtain from clickup_task_list (field: id) or clickup_task_search."},
                    "name": {"type": "string", "description": "New task title. Omit to keep current name."},
                    "status": {"type": "string", "description": "New status name (case-sensitive, must match a status defined on the parent list). Omit to keep current status."},
                    "priority": {"type": "integer", "description": "New priority: 1=Urgent, 2=High, 3=Normal, 4=Low. Omit to keep current priority."},
                    "description": {"type": "string", "description": "New task body — replaces the current description entirely. Markdown supported. Omit to keep current description."},
                    "add_assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add as assignees (additive; does not replace existing assignees). Obtain from clickup_member_list."
                    },
                    "rem_assignees": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to remove from assignees (no-op if the user is not currently assigned)."
                    }
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_delete",
            "description": "Permanently delete a ClickUp task along with all its subtasks, comments, checklists, attachments, and time entries. Destructive, irreversible, and cascading — confirm with the user before calling. To mark a task done without deleting, use clickup_task_update with a 'closed' status instead. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to delete. Obtain from clickup_task_list (field: id) or clickup_task_search. All subtasks, comments, checklists, attachments, and time entries on this task are deleted with it."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_search",
            "description": "Search tasks across an entire ClickUp workspace with optional space/list/status/assignee filters — useful for cross-list queries. Returns a paginated array of task objects. For tasks in a single list, prefer clickup_task_list (fewer parameters, same shape).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID to search within. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "space_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Restrict results to these space IDs. Obtain from clickup_space_list (field: id). Omit to search all spaces."
                    },
                    "list_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Restrict results to these list IDs. Obtain from clickup_list_list (field: id). Omit to search all lists."
                    },
                    "statuses": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Status names to include (e.g. ['open','in review']). Case-sensitive. Omit for any open status."
                    },
                    "assignees": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "User IDs (as strings) to restrict to tasks assigned to them. Obtain from clickup_member_list. Omit to return tasks regardless of assignee."
                    }
                },
                "required": []
            }
        },
        {
            "name": "clickup_comment_list",
            "description": "List comments on a ClickUp task in chronological order (oldest first). Only top-level comments are returned; use clickup_comment_replies to fetch a threaded reply chain. Returns a compact array of comment objects (id, comment_text, user, resolved, date).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to read comments from. Obtain from clickup_task_list (field: id) or clickup_task_search."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_comment_create",
            "description": "Post a new top-level comment on a ClickUp task. Supports markdown and @mentions in the text body. Returns the created comment object including its new id, which you can pass to clickup_comment_reply, clickup_comment_update, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to comment on. Obtain from clickup_task_list (field: id) or clickup_task_search."},
                    "text": {"type": "string", "description": "Comment body. Markdown and @mentions (e.g. '@username') are supported."},
                    "assignee": {"type": "integer", "description": "Optional user ID to assign the comment to — they will receive a notification. Obtain from clickup_member_list."},
                    "notify_all": {"type": "boolean", "description": "true = send a notification to every assignee of the task; false or omitted = only notify people mentioned or the explicit assignee."}
                },
                "required": ["task_id", "text"]
            }
        },
        {
            "name": "clickup_field_list",
            "description": "List the custom field definitions available on a ClickUp list — field id, name, type (text, number, drop_down, labels, date, url, email, phone, money, progress, formula, etc.), and for drop_down/labels fields the permitted option values extracted from type_config.options. Use this before clickup_field_set to learn the correct field_id, option ids, and value shape. Returns an array of custom field definitions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list whose custom fields to enumerate. Obtain from clickup_list_list (field: id). Fields are defined per-list (or inherited from folder/space)."}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_field_set",
            "description": "Set or overwrite a single custom field value on a ClickUp task. The value's JSON shape must match the field type (string for text/url/email and for a drop_down option id, number for number/currency/progress, array of option ids for labels, Unix ms for date, etc.). Use clickup_field_list first to see the field type and option ids. Use clickup_field_unset to clear a value. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task whose field value should change. Obtain from clickup_task_list (field: id)."},
                    "field_id": {"type": "string", "description": "ID of the custom field to set. Obtain from clickup_field_list (field: id) or clickup_task_get (custom_fields[].id)."},
                    "value": {"description": "New value; the accepted type depends on the custom field type. Examples: 'hello' (text), 42 (number), 'option-uuid' (drop_down), ['option-uuid'] (labels), 1735689600000 (date as Unix ms). See clickup_field_list for the field's type and option ids."}
                },
                "required": ["task_id", "field_id", "value"]
            }
        },
        {
            "name": "clickup_time_start",
            "description": "Start a live time-tracking timer for the authenticated user. If a timer is already running it will be stopped first. Pair with clickup_time_stop to end the timer and record the entry. Use clickup_time_current to inspect the running timer. Returns the newly started time entry object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "ID of the task to attribute this timer to. Obtain from clickup_task_list (field: id). Omit to track time without a task."},
                    "description": {"type": "string", "description": "Free-text description shown on the time entry (e.g. 'pair debugging session'). Optional."},
                    "billable": {"type": "boolean", "description": "true = mark this time entry as billable (shows as $ in reports); false or omitted = non-billable."}
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
            "description": "List historical time tracking entries for a workspace, optionally filtered by date range and/or task. Covers both manually-created entries and stopped timers. Returns a compact array of time entry objects (id, user, task, start, duration, billable, description).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "start_date": {"type": "integer", "description": "Inclusive lower bound as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01). Omit for no lower bound."},
                    "end_date": {"type": "integer", "description": "Inclusive upper bound as a Unix timestamp in milliseconds. Omit for no upper bound. Note: ClickUp caps the range to ~30 days by default."},
                    "task_id": {"type": "string", "description": "Return only entries attributed to this task. Obtain from clickup_task_list (field: id). Omit to list entries across all tasks."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_checklist_create",
            "description": "Create a new checklist (to-do group) on a ClickUp task. The checklist starts empty — add items via clickup_checklist_add_item. A task can have multiple checklists. Returns the created checklist object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to attach the checklist to. Obtain from clickup_task_list (field: id)."},
                    "name": {"type": "string", "description": "Display name for the checklist (e.g. 'Launch prep', 'QA steps'). Shown as a heading above the items."}
                },
                "required": ["task_id", "name"]
            }
        },
        {
            "name": "clickup_checklist_delete",
            "description": "Permanently delete an entire checklist from a ClickUp task, including all its items. Destructive and irreversible. To remove a single item instead, use clickup_checklist_delete_item. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "ID of the checklist to delete. Obtain from clickup_task_get (field: checklists[].id). All items on this checklist are deleted with it."}
                },
                "required": ["checklist_id"]
            }
        },
        {
            "name": "clickup_goal_list",
            "description": "List all ClickUp goals in a workspace. Each goal represents an OKR-style objective and can have multiple key results (sub-targets). Returns a compact array of goal objects (id, name, percent_completed, due_date). Use clickup_goal_get for the full goal including its key_results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_goal_get",
            "description": "Fetch a single ClickUp goal including its key results, owners, due date, and current percent-complete. Returns the goal object with its key_results array populated.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "ID of the goal to fetch. Obtain from clickup_goal_list (field: id). The authenticated user must have view access to the goal's workspace."}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_goal_create",
            "description": "Create a new OKR-style goal in a workspace. The goal starts with zero key results — add them via clickup_goal_add_kr. The goal's percent-complete is auto-calculated from the average progress of its key results. Returns the created goal object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Goal title (e.g. 'Q1 revenue target'). Required and non-empty."},
                    "due_date": {"type": "integer", "description": "Target completion date as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01)."},
                    "description": {"type": "string", "description": "Goal description / rationale. Markdown supported. Omit for no description."},
                    "owner_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to assign as goal owners (they receive notifications about progress). Obtain from clickup_member_list."
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_goal_update",
            "description": "Modify a ClickUp goal's top-level fields (name, description, due date). To change progress, update the goal's key results instead via clickup_goal_update_kr — the goal's percent complete is derived automatically. Returns the updated goal object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "ID of the goal to update. Obtain from clickup_goal_list (field: id)."},
                    "name": {"type": "string", "description": "New goal title. Omit to keep current name."},
                    "due_date": {"type": "integer", "description": "New due date as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01)."},
                    "description": {"type": "string", "description": "New goal description. Markdown supported."}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_view_list",
            "description": "List saved views (board, list, calendar, gantt, timeline, etc.) attached to a space, folder, list, or the whole workspace. Exactly one of space_id/folder_id/list_id must be provided — or omit all three to list workspace-level views. Returns a compact array of view objects (id, name, type).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "Space ID whose views to list. Obtain from clickup_space_list. Mutually exclusive with folder_id/list_id."},
                    "folder_id": {"type": "string", "description": "Folder ID whose views to list. Obtain from clickup_folder_list. Mutually exclusive with space_id/list_id."},
                    "list_id": {"type": "string", "description": "List ID whose views to list. Obtain from clickup_list_list. Mutually exclusive with space_id/folder_id."},
                    "team_id": {"type": "string", "description": "Workspace (team) ID — used when all three scope IDs are omitted, to return workspace-level (Everything) views. Obtain from clickup_workspace_list. Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_view_tasks",
            "description": "Fetch the tasks currently visible in a ClickUp view, honouring the view's configured filters, sort order, and grouping. Returns a paginated array of task objects. Use clickup_view_list to discover view IDs and clickup_view_get for the view's definition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "ID of the view to read tasks from. Obtain from clickup_view_list (field: id)."},
                    "page": {"type": "integer", "description": "Zero-indexed page number (default 0). Each page returns up to 30 tasks; increment to paginate."}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_doc_list",
            "description": "List all ClickUp docs in a workspace. Docs are long-form markdown documents separate from tasks and can contain nested pages. Returns a compact array of doc objects (id, name, date_created, date_updated). Use clickup_doc_get for a single doc or clickup_doc_pages to list a doc's pages. Uses v3 cursor pagination.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_doc_get",
            "description": "Fetch metadata for a single ClickUp doc — name, parent, dates, type. Does not return the page bodies; use clickup_doc_pages (with content=true) or clickup_doc_get_page for the markdown content. Returns the doc object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the doc to fetch. Obtain from clickup_doc_list (field: id)."}
                },
                "required": ["doc_id"]
            }
        },
        {
            "name": "clickup_doc_pages",
            "description": "List the pages inside a ClickUp doc, including any nested subpages. Returns an array of page objects (id, name, sub_title, parent_page_id, and optionally content). Pages are ordered by their tree position.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the parent doc. Obtain from clickup_doc_list (field: id)."},
                    "content": {"type": "boolean", "description": "true = include each page's full markdown body in the 'content' field; false or omitted = return page metadata only (faster, smaller payload)."}
                },
                "required": ["doc_id"]
            }
        },
        {
            "name": "clickup_tag_list",
            "description": "List all tags defined in a ClickUp space. Tags are space-scoped labels (with foreground/background hex colours) that can be applied to tasks within that space. Returns an array of tag objects (name, tag_fg, tag_bg, creator).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space whose tags to list. Obtain from clickup_space_list (field: id). Tags are defined per-space, not workspace-wide."}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_task_add_tag",
            "description": "Attach an existing tag to a ClickUp task. The tag must already be defined in the task's parent space — use clickup_tag_list to check and clickup_tag_create to add a new tag first. Tags are identified by name, not ID. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to tag. Obtain from clickup_task_list (field: id)."},
                    "tag_name": {"type": "string", "description": "Name of the tag to apply (case-sensitive, must already exist in the task's space). Obtain from clickup_tag_list."}
                },
                "required": ["task_id", "tag_name"]
            }
        },
        {
            "name": "clickup_task_remove_tag",
            "description": "Detach a tag from a ClickUp task. The tag definition itself is preserved in the space — only this task's association with it is removed. No-op if the tag was not on the task. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to untag. Obtain from clickup_task_list (field: id) or clickup_task_get."},
                    "tag_name": {"type": "string", "description": "Name of the tag to remove (case-sensitive). See clickup_task_get (field: tags[].name) for tags currently on the task."}
                },
                "required": ["task_id", "tag_name"]
            }
        },
        {
            "name": "clickup_webhook_list",
            "description": "List all webhooks registered on a ClickUp workspace. Each webhook specifies a target endpoint, subscribed events, and optional scope. Returns an array of webhook objects (id, endpoint, events, status, secret, health). Use clickup_webhook_create/update/delete to manage them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_member_list",
            "description": "List members (users with direct access) of a specific task or list. Exactly one of task_id or list_id must be provided. Returns an array of user objects (id, username, email, color). Use clickup_user_invite to add people to the workspace first; use clickup_guest_share_task/list to add guests.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task ID whose members to list. Obtain from clickup_task_list (field: id). Mutually exclusive with list_id."},
                    "list_id": {"type": "string", "description": "List ID whose members to list. Obtain from clickup_list_list (field: id). Mutually exclusive with task_id."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_template_list",
            "description": "List the task templates available in a workspace. Task templates are saved task shapes (name, description, checklists, subtasks, custom fields, etc.) that can be applied via clickup_template_apply_task to create new tasks quickly. Returns an array of template objects (id, name).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "page": {"type": "integer", "description": "Zero-indexed page number (default 0). Each page returns up to 100 templates; increment to paginate."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_space_get",
            "description": "Fetch the full object for a single ClickUp space — name, privacy, statuses, features (time tracking, tags, due dates enabled, etc.), and members. Returns the space object. Use clickup_folder_list or clickup_list_list to enumerate the space's contents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space to fetch. Obtain from clickup_space_list (field: id)."}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_space_create",
            "description": "Create a new top-level space in a ClickUp workspace. The new space uses the workspace's default feature set and statuses — customise later via the web UI or by creating folders/lists under it. Returns the created space object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Display name for the space (shown in the sidebar)."},
                    "private": {"type": "boolean", "description": "true = private space (only explicit members see it); false or omitted = visible to the whole workspace."}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_space_update",
            "description": "Modify a ClickUp space — rename it, toggle privacy, or archive/unarchive it. Archiving is the reversible alternative to deletion: archived spaces are hidden from default views but retain all their folders, lists, and tasks. Returns the updated space object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space to update. Obtain from clickup_space_list (field: id)."},
                    "name": {"type": "string", "description": "New space name. Omit to keep current name."},
                    "private": {"type": "boolean", "description": "true = space is private (visible only to explicit members); false = space is visible to the whole workspace."},
                    "archived": {"type": "boolean", "description": "true = archive (hide but preserve); false = restore from archive."}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_space_delete",
            "description": "Permanently delete a ClickUp space along with every folder, list, and task inside it. Destructive, irreversible, and widely cascading — confirm with the user before calling. To hide a space without destroying its contents, use clickup_space_update with archived=true instead (archival is reversible). Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space to delete. Obtain from clickup_space_list (field: id). All descendant folders, lists, and tasks are deleted with it."}
                },
                "required": ["space_id"]
            }
        },
        {
            "name": "clickup_folder_get",
            "description": "Fetch the full object for a single ClickUp folder — name, task_count (a string, per API), archived status, and its child lists. Returns the folder object. Use clickup_list_list with folder_id to get just the lists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the folder to fetch. Obtain from clickup_folder_list (field: id)."}
                },
                "required": ["folder_id"]
            }
        },
        {
            "name": "clickup_folder_create",
            "description": "Create a new folder inside a ClickUp space. Folders group related lists and start empty — add lists via clickup_list_create with folder_id. Returns the created folder object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the parent space. Obtain from clickup_space_list (field: id)."},
                    "name": {"type": "string", "description": "Display name for the folder. Must be non-empty and unique within the space."}
                },
                "required": ["space_id", "name"]
            }
        },
        {
            "name": "clickup_folder_update",
            "description": "Rename a ClickUp folder. Only the folder's display name can be changed via this endpoint — to move the folder to a different space, delete and recreate. Returns the updated folder object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the folder to rename. Obtain from clickup_folder_list (field: id) or clickup_folder_get."},
                    "name": {"type": "string", "description": "New display name for the folder. Must be non-empty and unique within its parent space."}
                },
                "required": ["folder_id", "name"]
            }
        },
        {
            "name": "clickup_folder_delete",
            "description": "Permanently delete a ClickUp folder along with every list and task inside it. Destructive, irreversible, and cascading — confirm with the user before calling. If you only want to hide the folder, use clickup_space_update with archived=true on the parent space instead (archival is reversible). Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the folder to delete. Obtain from clickup_folder_list (field: id). All descendant lists and tasks are deleted with it."}
                },
                "required": ["folder_id"]
            }
        },
        {
            "name": "clickup_list_get",
            "description": "Fetch the full object for a single ClickUp list — name, content/description, statuses, task_count, assignees, due date, and parent folder/space. Returns the list object. Use clickup_task_list with list_id to enumerate tasks inside it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to fetch. Obtain from clickup_list_list (field: id)."}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_create",
            "description": "Create a new task list inside either a folder or a space (folderless). Exactly one of folder_id or space_id must be provided. Returns the created list object including its new id — use it as list_id for clickup_task_create and related calls.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the parent folder. Obtain from clickup_folder_list (field: id). Mutually exclusive with space_id."},
                    "space_id": {"type": "string", "description": "ID of the parent space — creates a folderless list attached directly to the space. Obtain from clickup_space_list (field: id). Mutually exclusive with folder_id."},
                    "name": {"type": "string", "description": "Display name for the list. Required and non-empty."},
                    "content": {"type": "string", "description": "List description shown at the top of the list. Markdown supported. Omit for no description."},
                    "due_date": {"type": "integer", "description": "List-level due date as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01). Individual tasks retain their own due dates."},
                    "status": {"type": "string", "description": "Default status for tasks added to this list. Must match a status name from the parent space's status set."}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_list_update",
            "description": "Modify a ClickUp list's name, description, due date, or status. To move tasks between lists use clickup_task_move, and to add or remove this list from tasks with multi-list membership use clickup_list_add_task / clickup_list_remove_task. Returns the updated list object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to update. Obtain from clickup_list_list (field: id)."},
                    "name": {"type": "string", "description": "New list name. Omit to keep current name."},
                    "content": {"type": "string", "description": "New description for the list. Markdown supported."},
                    "due_date": {"type": "integer", "description": "List-level due date as a Unix timestamp in milliseconds. Individual tasks retain their own due dates."},
                    "status": {"type": "string", "description": "Default status for tasks added to this list (must match an existing status name in the list's status set)."}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_delete",
            "description": "Permanently delete a ClickUp list along with every task inside it, plus their comments, checklists, and attachments. Destructive, irreversible, and cascading — confirm with the user before calling. To move tasks elsewhere first, use clickup_task_move on each, or use clickup_list_update to rename/archive instead. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to delete. Obtain from clickup_list_list (field: id). All tasks inside are deleted with it."}
                },
                "required": ["list_id"]
            }
        },
        {
            "name": "clickup_list_add_task",
            "description": "Add a task to a secondary list (multi-list membership) without moving it. The task remains in its original home list and becomes additionally visible in this one — useful for shared roadmaps. To change the home list instead, use clickup_task_move. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the secondary list the task should also appear in. Obtain from clickup_list_list (field: id)."},
                    "task_id": {"type": "string", "description": "ID of the task to add. Obtain from clickup_task_list (field: id)."}
                },
                "required": ["list_id", "task_id"]
            }
        },
        {
            "name": "clickup_list_remove_task",
            "description": "Remove a task from a secondary list it was added to via clickup_list_add_task. The task itself is NOT deleted — it remains in its home list (and any other secondary lists). Use clickup_task_delete to remove the task entirely. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the secondary list to detach the task from. Obtain from clickup_list_list (field: id). Must not be the task's home list."},
                    "task_id": {"type": "string", "description": "ID of the task to detach. Obtain from clickup_task_list (field: id)."}
                },
                "required": ["list_id", "task_id"]
            }
        },
        {
            "name": "clickup_comment_update",
            "description": "Edit the text, assignee, or resolution state of a ClickUp comment on a task, list, or view. The entire comment body is replaced by the new text (no partial edits). Marking resolved=true strikes through the comment and closes the thread. Returns the updated comment object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "ID of the comment to edit. Obtain from clickup_comment_list (field: id)."},
                    "text": {"type": "string", "description": "Replacement body for the comment. ClickUp accepts markdown plus @mentions (e.g. '@username'). The previous body is overwritten entirely."},
                    "assignee": {"type": "integer", "description": "Reassign the comment to this user ID, who will receive a notification. Obtain from clickup_member_list."},
                    "resolved": {"type": "boolean", "description": "true = mark the comment thread resolved/closed; false = reopen it."}
                },
                "required": ["comment_id", "text"]
            }
        },
        {
            "name": "clickup_comment_delete",
            "description": "Permanently delete a ClickUp comment from a task, list, or view. Destructive and irreversible — the comment and any threaded replies are removed. Use clickup_comment_update with resolved=true instead if you only want to mark the comment as handled. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "ID of the comment to delete. Obtain from clickup_comment_list (field: id). The authenticated user must have edit permission on the parent task/list/view."}
                },
                "required": ["comment_id"]
            }
        },
        {
            "name": "clickup_task_add_dep",
            "description": "Create a dependency relationship between two tasks — either 'task_id depends on depends_on' (blocks until that is done) or 'dependency_of depends on task_id' (task_id blocks that). Provide exactly one of depends_on or dependency_of. Use clickup_task_link for a simple non-blocking reference. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the primary task. Obtain from clickup_task_list (field: id)."},
                    "depends_on": {"type": "string", "description": "ID of a task that task_id should wait for (task_id is blocked until depends_on is complete). Obtain from clickup_task_list. Mutually exclusive with dependency_of."},
                    "dependency_of": {"type": "string", "description": "ID of a task that depends on task_id (that task is blocked until task_id is complete). Obtain from clickup_task_list. Mutually exclusive with depends_on."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_remove_dep",
            "description": "Remove an existing dependency relationship between two tasks. Provide exactly one of depends_on or dependency_of, matching the direction you set with clickup_task_add_dep. The tasks themselves are not affected. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the primary task in the dependency. Obtain from clickup_task_list (field: id) or clickup_task_get (field: dependencies[])."},
                    "depends_on": {"type": "string", "description": "ID of the upstream task to detach from task_id (removes the 'task_id waits for this' edge). Mutually exclusive with dependency_of."},
                    "dependency_of": {"type": "string", "description": "ID of the downstream task to detach (removes the 'this waits for task_id' edge). Mutually exclusive with depends_on."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_task_link",
            "description": "Create a bidirectional reference link between two tasks — a non-blocking 'see also' relationship, unlike dependencies. Both tasks show the other in their 'Linked tasks' panel. Use clickup_task_unlink to remove. For blocking relationships, use clickup_task_add_dep instead. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the first task. Obtain from clickup_task_list (field: id)."},
                    "links_to": {"type": "string", "description": "ID of the second task to link to. Obtain from clickup_task_list (field: id). The link is visible from both tasks."}
                },
                "required": ["task_id", "links_to"]
            }
        },
        {
            "name": "clickup_task_unlink",
            "description": "Remove a bidirectional reference link previously created with clickup_task_link. The tasks themselves are not affected, only the link between them. No-op if no link exists. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the first task. Obtain from clickup_task_list (field: id) or clickup_task_get (field: linked_tasks[])."},
                    "links_to": {"type": "string", "description": "ID of the linked task to unlink from task_id."}
                },
                "required": ["task_id", "links_to"]
            }
        },
        {
            "name": "clickup_goal_delete",
            "description": "Permanently delete a ClickUp goal along with all its key results. Destructive, irreversible, and cascading — confirm with the user before calling. Historical progress data on the goal is lost. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "ID of the goal to delete. Obtain from clickup_goal_list (field: id) or clickup_goal_get. All child key results are deleted with it."}
                },
                "required": ["goal_id"]
            }
        },
        {
            "name": "clickup_goal_add_kr",
            "description": "Add a new key result (KR / sub-target) to a ClickUp goal. KRs drive the goal's overall percent-complete — each KR's progress is averaged. For 'automatic' KRs, link tasks or lists and progress is derived from their status; for number/currency/percentage KRs, report progress via clickup_goal_update_kr. Returns the created key result object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal_id": {"type": "string", "description": "ID of the parent goal. Obtain from clickup_goal_list (field: id)."},
                    "name": {"type": "string", "description": "Display name of the key result (e.g. 'MRR reaches $50k')."},
                    "type": {"type": "string", "description": "Key result type: 'number' (numeric target), 'currency' (monetary target), 'boolean' (done/not-done), 'percentage' (0–100), or 'automatic' (derived from linked tasks/lists)."},
                    "steps_start": {"type": "number", "description": "Starting value of the metric (e.g. 0 for a from-zero KR, current baseline otherwise). Ignored for 'boolean'."},
                    "steps_end": {"type": "number", "description": "Target value the KR aims to reach. For 'percentage' KRs use 100; for 'boolean' use 1."},
                    "unit": {"type": "string", "description": "Unit label shown next to numeric values (e.g. 'USD', 'users', 'signups'). Ignored for 'boolean' and 'automatic'."},
                    "owner_ids": {"type": "array", "items": {"type": "integer"}, "description": "User IDs responsible for this KR. Obtain from clickup_member_list."},
                    "task_ids": {"type": "array", "items": {"type": "string"}, "description": "Task IDs whose completion drives progress (only for type='automatic'). Obtain from clickup_task_list."},
                    "list_ids": {"type": "array", "items": {"type": "string"}, "description": "List IDs whose task-completion percentage drives progress (only for type='automatic'). Obtain from clickup_list_list."}
                },
                "required": ["goal_id", "name", "type", "steps_start", "steps_end"]
            }
        },
        {
            "name": "clickup_goal_update_kr",
            "description": "Update a key result (sub-target) on a ClickUp goal — typically to record current progress, rename, or adjust the unit label. The goal's completion percentage is auto-recalculated from all its key results. Returns the updated key result object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kr_id": {"type": "string", "description": "Key result ID. Obtain from clickup_goal_get (field: key_results[].id) or the response of clickup_goal_add_kr."},
                    "steps_current": {"type": "number", "description": "Current progress toward steps_end. For boolean KRs use 0 (not done) or 1 (done). For percentage KRs use 0–100."},
                    "name": {"type": "string", "description": "New display name for the key result. Omit to keep current name."},
                    "unit": {"type": "string", "description": "Unit label shown next to numeric values (e.g. 'MRR', 'users'). Ignored for boolean and automatic types."}
                },
                "required": ["kr_id"]
            }
        },
        {
            "name": "clickup_goal_delete_kr",
            "description": "Permanently delete a single key result from a ClickUp goal. Destructive and irreversible — the historical progress for this key result is lost, and the goal's overall completion percentage is recalculated from the remaining key results. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kr_id": {"type": "string", "description": "ID of the key result to delete. Obtain from clickup_goal_get (field: key_results[].id)."}
                },
                "required": ["kr_id"]
            }
        },
        {
            "name": "clickup_time_get",
            "description": "Fetch the full object for a single time tracking entry — user, task, start timestamp, duration, description, billable flag, and tags. Returns the time entry object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "ID of the time entry. Obtain from clickup_time_list (field: id) or clickup_time_current."}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_time_create",
            "description": "Manually record a historical time tracking entry with a fixed start and duration. Use this for backfilling time (e.g. work done offline). For live timing use clickup_time_start/stop instead. Returns the created time entry object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "ID of the task to attribute the time to. Obtain from clickup_task_list. Omit for a task-less time entry."},
                    "start": {"type": "integer", "description": "Entry start time as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01 00:00 UTC)."},
                    "duration": {"type": "integer", "description": "Duration in milliseconds (e.g. 3600000 for one hour)."},
                    "description": {"type": "string", "description": "Free-text description of the work logged. Optional."},
                    "billable": {"type": "boolean", "description": "true = mark as billable (shows with $ in reports); false or omitted = non-billable."}
                },
                "required": ["start", "duration"]
            }
        },
        {
            "name": "clickup_time_update",
            "description": "Modify a recorded time tracking entry. Only the supplied fields are changed; omitted fields keep their current value. Use clickup_time_add_tags / remove_tags for tag changes. Returns the updated time entry object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "ID of the time entry to update. Obtain from clickup_time_list (field: id)."},
                    "start": {"type": "integer", "description": "New start time as a Unix timestamp in milliseconds. Omit to keep current start."},
                    "duration": {"type": "integer", "description": "New duration in milliseconds (e.g. 3600000 for one hour). Omit to keep current duration."},
                    "description": {"type": "string", "description": "New description for the entry. Omit to keep current description."},
                    "billable": {"type": "boolean", "description": "true = billable, false = non-billable. Omit to keep current value."}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_time_delete",
            "description": "Permanently delete a recorded time tracking entry. Destructive and irreversible — the logged time is removed from reports. To stop a currently running timer, use clickup_time_stop instead (which preserves the record). Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "ID of the time entry to delete. Obtain from clickup_time_list (field: id). Only the entry's owner (or a workspace admin) can delete it."}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_view_get",
            "description": "Fetch the full definition of a single ClickUp view — name, type (list/board/calendar/gantt/etc.), parent scope, filters, grouping, sort order, and column layout. Does not return the tasks inside the view; use clickup_view_tasks for that. Returns the view object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "ID of the view to fetch. Obtain from clickup_view_list (field: id)."}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_view_create",
            "description": "Create a new saved view (board, list, calendar, timeline, etc.) attached to a space, folder, list, or the workspace. Creates an empty view with default filters — customise filters/grouping/sort later via the web UI. Returns the created view object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "description": "Where to attach the view: 'space', 'folder', 'list', or 'team' (workspace-level 'Everything' view)."},
                    "scope_id": {"type": "string", "description": "ID of the scope object. For scope='space' use a space_id, for 'folder' a folder_id, for 'list' a list_id, for 'team' a workspace/team id (from clickup_workspace_list)."},
                    "name": {"type": "string", "description": "Display name for the view."},
                    "type": {"type": "string", "description": "View type: 'list', 'board', 'calendar', 'table', 'timeline', 'gantt', 'map', 'workload', 'activity', 'chat', 'mind_map', 'doc', or 'form'."}
                },
                "required": ["scope", "scope_id", "name", "type"]
            }
        },
        {
            "name": "clickup_view_update",
            "description": "Rename a view or change its display type (e.g. from list to board). To change filters, grouping, or sort order, use the ClickUp web UI — those are not exposed via the API. Returns the updated view object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "ID of the view to update. Obtain from clickup_view_list (field: id)."},
                    "name": {"type": "string", "description": "New display name for the view."},
                    "type": {"type": "string", "description": "New view type: 'list', 'board', 'calendar', 'table', 'timeline', 'gantt', 'map', 'workload', 'activity', 'chat', 'mind_map', 'doc', or 'form'."}
                },
                "required": ["view_id", "name", "type"]
            }
        },
        {
            "name": "clickup_view_delete",
            "description": "Permanently delete a ClickUp view (board, list, calendar, gantt, etc.). Destructive and irreversible for custom views — default views cannot be deleted and will return a 400 error. The underlying tasks are not affected, only the view definition. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_id": {"type": "string", "description": "ID of the view to delete. Obtain from clickup_view_list (field: id). Must be a user-created view, not a ClickUp-default one."}
                },
                "required": ["view_id"]
            }
        },
        {
            "name": "clickup_doc_create",
            "description": "Create a new ClickUp doc in a workspace. The doc starts with no pages — add pages via clickup_doc_add_page. Optionally attach the doc under a parent space/folder/list/task instead of the workspace root. Returns the created doc object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Display name for the doc (shown in the doc tree)."},
                    "parent": {"type": "object", "description": "Optional parent object to attach the doc under. Shape: { 'id': '<id>', 'type': <int> } where type is 4=space, 5=folder, 6=list, 7=task. Omit to create at the workspace root."}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_doc_add_page",
            "description": "Create a new page inside an existing ClickUp doc. Pages support a markdown body plus optional subtitle. Supply parent_page_id to nest the page under another page (creates a page tree). Returns the created page object including its new id, which you can pass to clickup_doc_edit_page or clickup_doc_get_page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the parent doc. Obtain from clickup_doc_list (field: id)."},
                    "name": {"type": "string", "description": "Title shown in the doc's left-hand page navigator."},
                    "content": {"type": "string", "description": "Initial body of the page in ClickUp-flavoured markdown. Omit to create an empty page you can populate later via clickup_doc_edit_page."},
                    "sub_title": {"type": "string", "description": "Optional subtitle rendered under the page title."},
                    "parent_page_id": {"type": "string", "description": "ID of a sibling page to nest this page under. Omit to create a top-level page. Obtain from clickup_doc_pages (field: id)."}
                },
                "required": ["doc_id", "name"]
            }
        },
        {
            "name": "clickup_doc_edit_page",
            "description": "Rename or rewrite an existing page inside a ClickUp doc. The supplied content replaces the current page body entirely (not an append). For a fresh page use clickup_doc_add_page instead. Returns the updated page object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the parent doc. Obtain from clickup_doc_list (field: id)."},
                    "page_id": {"type": "string", "description": "ID of the page to edit. Obtain from clickup_doc_pages (field: id)."},
                    "name": {"type": "string", "description": "New page title. Omit to keep current title."},
                    "content": {"type": "string", "description": "New page body in ClickUp-flavoured markdown. Replaces the existing body entirely. Omit to leave content unchanged."}
                },
                "required": ["doc_id", "page_id"]
            }
        },
        {
            "name": "clickup_chat_channel_create",
            "description": "Create a new ClickUp Chat channel in a workspace. For one-on-one messages use clickup_chat_dm instead. Add members later via the channel-members endpoint. Returns the created channel object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Channel name (e.g. 'product-launch'). Must be unique within the workspace."},
                    "description": {"type": "string", "description": "Optional channel topic/description shown in the header."},
                    "visibility": {"type": "string", "description": "Channel visibility: 'public' (any workspace member can join) or 'private' (invite only). Defaults to 'public'."}
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_chat_channel_get",
            "description": "Fetch metadata for a single ClickUp chat channel — name, description, visibility, member count, latest activity. Does not return the messages themselves; use clickup_chat_message_list for that. Returns the channel object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel to fetch. Obtain from clickup_chat_channel_list (field: id)."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_update",
            "description": "Rename a ClickUp chat channel or change its description. To change membership or visibility use the dedicated channel-members and channel-followers tools. Returns the updated channel object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel to update. Obtain from clickup_chat_channel_list (field: id)."},
                    "name": {"type": "string", "description": "New display name for the channel. Must be unique within the workspace."},
                    "description": {"type": "string", "description": "New channel description/topic shown in the channel header. Markdown supported."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_delete",
            "description": "Permanently delete a ClickUp Chat channel along with every message and reply it contains. Destructive, irreversible, and cascading — confirm with the user before calling. The channel vanishes from the workspace for all members. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel to delete. Obtain from clickup_chat_channel_list (field: id). All messages and replies inside are deleted with it."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_list",
            "description": "List messages in a ClickUp Chat channel, newest first. Only top-level messages are returned; use clickup_chat_reply_list for threaded replies. Uses v3 cursor pagination — pass the 'cursor' from the previous response to page further back. Returns an array of message objects plus a next_cursor.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel. Obtain from clickup_chat_channel_list (field: id)."},
                    "cursor": {"type": "string", "description": "Opaque pagination cursor from the previous response's next_cursor field. Omit for the first page (newest messages)."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_send",
            "description": "Post a new top-level message to a ClickUp Chat channel. For replies inside a thread use clickup_chat_reply_send; for DMs use clickup_chat_dm. Returns the created message object including its new id, which you can pass to clickup_chat_reaction_add, clickup_chat_reply_send, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the target channel. Obtain from clickup_chat_channel_list (field: id)."},
                    "content": {"type": "string", "description": "Message body. Supports markdown, @mentions (e.g. '@username'), and emoji."}
                },
                "required": ["channel_id", "content"]
            }
        },
        {
            "name": "clickup_chat_message_delete",
            "description": "Permanently delete a message from a ClickUp chat channel or DM thread. Destructive and irreversible — the message and its threaded replies are removed for all viewers. Only the message author or a workspace admin can delete; other users will get a 403. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message to delete. Obtain from clickup_chat_message_list (field: id) or clickup_chat_reply_list."}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_dm",
            "description": "Send a direct message from the authenticated user to another workspace member. If no DM channel exists between the two users, one is created automatically. Returns the created message object. Use clickup_chat_message_send for channel messages.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "Numeric user ID of the recipient. Obtain from clickup_member_list or clickup_user_get (field: id)."},
                    "content": {"type": "string", "description": "Message body. Supports markdown, emoji, and @mentions."}
                },
                "required": ["user_id", "content"]
            }
        },
        {
            "name": "clickup_webhook_create",
            "description": "Register an HTTPS endpoint that ClickUp will POST events to as things happen in the workspace (tasks created, comments added, status changes, etc.). Optionally scope the webhook to a single space, folder, list, or task. The response includes a 'secret' you should use to verify the X-Signature header on incoming payloads. Returns the created webhook object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "endpoint": {"type": "string", "description": "Public HTTPS URL that will receive event POSTs. Must respond 2xx within 5 seconds or ClickUp will retry/suspend."},
                    "events": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Event names to subscribe to (e.g. ['taskCreated','taskUpdated','taskStatusUpdated','commentPosted']). Pass ['*'] to subscribe to every event ClickUp emits."
                    },
                    "space_id": {"type": "string", "description": "Scope events to this space only. Mutually exclusive with folder_id/list_id/task_id."},
                    "folder_id": {"type": "string", "description": "Scope events to this folder only. Mutually exclusive with space_id/list_id/task_id."},
                    "list_id": {"type": "string", "description": "Scope events to this list only. Mutually exclusive with space_id/folder_id/task_id."},
                    "task_id": {"type": "string", "description": "Scope events to this task only. Mutually exclusive with space_id/folder_id/list_id."}
                },
                "required": ["endpoint", "events"]
            }
        },
        {
            "name": "clickup_webhook_update",
            "description": "Change the delivery endpoint, subscribed events, or active status of a ClickUp webhook. To temporarily pause deliveries without losing the webhook config, set status='suspended' (then resume later with status='active'). Returns the updated webhook object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "webhook_id": {"type": "string", "description": "ID of the webhook to update. Obtain from clickup_webhook_list (field: id)."},
                    "endpoint": {"type": "string", "description": "New HTTPS URL that ClickUp will POST events to. Must be publicly reachable and respond with 2xx within 5 seconds."},
                    "events": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "New list of event names to subscribe to (e.g. ['taskCreated','taskUpdated']). Pass ['*'] to subscribe to every event. Omit to leave subscriptions unchanged."
                    },
                    "status": {"type": "string", "description": "'active' to deliver events; 'suspended' to pause deliveries without deleting the webhook."}
                },
                "required": ["webhook_id"]
            }
        },
        {
            "name": "clickup_webhook_delete",
            "description": "Permanently delete a ClickUp webhook, stopping all future event deliveries to its endpoint. Destructive and irreversible — the webhook record is removed immediately. If you only want to pause deliveries, use clickup_webhook_update with status='suspended' instead. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "webhook_id": {"type": "string", "description": "ID of the webhook to delete. Obtain from clickup_webhook_list (field: id). The authenticated user must own the webhook or be a workspace admin."}
                },
                "required": ["webhook_id"]
            }
        },
        {
            "name": "clickup_checklist_add_item",
            "description": "Append a new item to an existing ClickUp checklist on a task. The item starts unresolved. To edit or resolve items use clickup_checklist_update_item; to remove them use clickup_checklist_delete_item. Returns the updated checklist object (with all items).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "ID of the parent checklist. Obtain from clickup_task_get (field: checklists[].id)."},
                    "name": {"type": "string", "description": "Display text of the new checklist item (e.g. 'Send release notes')."},
                    "assignee": {"type": "integer", "description": "Optional user ID to assign this item to (they will see it on their assigned work). Obtain from clickup_member_list."}
                },
                "required": ["checklist_id", "name"]
            }
        },
        {
            "name": "clickup_checklist_update_item",
            "description": "Modify a single checklist item on a ClickUp task — rename it, toggle its resolved state, or change its assignee. Use clickup_checklist_add_item to create new items and clickup_checklist_delete_item to remove them. Returns the updated checklist object (all items).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "ID of the parent checklist. Obtain from clickup_task_get (field: checklists[].id)."},
                    "item_id": {"type": "string", "description": "ID of the item to update. Obtain from clickup_task_get (field: checklists[].items[].id)."},
                    "name": {"type": "string", "description": "New text for the item. Omit to keep current text."},
                    "resolved": {"type": "boolean", "description": "true = mark as done (strike-through); false = mark as open."},
                    "assignee": {"type": "integer", "description": "Reassign the item to this user ID. Obtain user IDs from clickup_member_list or clickup_user_get. Pass no value to leave assignee unchanged."}
                },
                "required": ["checklist_id", "item_id"]
            }
        },
        {
            "name": "clickup_checklist_delete_item",
            "description": "Permanently delete a single item from a ClickUp checklist. Destructive and irreversible. To resolve the item (mark done) without deleting, use clickup_checklist_update_item with resolved=true. Returns the updated checklist object (remaining items).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "ID of the parent checklist. Obtain from clickup_task_get (field: checklists[].id)."},
                    "item_id": {"type": "string", "description": "ID of the item to delete. Obtain from clickup_task_get (field: checklists[].items[].id)."}
                },
                "required": ["checklist_id", "item_id"]
            }
        },
        {
            "name": "clickup_user_get",
            "description": "Fetch the profile of a specific member of a ClickUp workspace — username, email, color, profile picture, role. Returns the user object. Use clickup_member_list for task/list members; use clickup_whoami for the authenticated user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "Numeric user ID. Obtain from clickup_member_list or clickup_workspace_list (field: members[].user.id)."}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_workspace_seats",
            "description": "Get the seat-usage breakdown for a ClickUp workspace — how many paid member seats, guest seats, and internal seats are used vs. available. Useful before inviting new users to confirm capacity. Returns an object with member/guest/internal seat counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_workspace_plan",
            "description": "Get the current subscription plan of a ClickUp workspace (Free, Unlimited, Business, Business Plus, Enterprise), along with plan_name and plan_id. Some features (guests, audit logs, ACLs) require Enterprise. Returns the plan object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_tag_create",
            "description": "Define a new tag in a ClickUp space. Tags are space-scoped and must be created before they can be applied to tasks via clickup_task_add_tag. Note: create uses tag_fg/tag_bg, but clickup_tag_update uses fg_color/bg_color (API inconsistency). Returns an empty object on success; use clickup_tag_list to see the created tag.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space to create the tag in. Obtain from clickup_space_list (field: id)."},
                    "name": {"type": "string", "description": "Tag name (e.g. 'blocked', 'priority'). Must be unique within the space."},
                    "tag_fg": {"type": "string", "description": "Text (foreground) hex colour including leading '#' (e.g. '#FFFFFF'). Omit for default."},
                    "tag_bg": {"type": "string", "description": "Pill (background) hex colour including leading '#' (e.g. '#FF0000'). Omit for default."}
                },
                "required": ["space_id", "name"]
            }
        },
        {
            "name": "clickup_tag_update",
            "description": "Rename a tag or change its colours within a ClickUp space. All tasks using the tag are automatically updated with the new name/colours. Note: update uses fg_color/bg_color whereas tag_create uses tag_fg/tag_bg (API inconsistency). Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space containing the tag. Obtain from clickup_space_list (field: id)."},
                    "tag_name": {"type": "string", "description": "Current name of the tag to update. Obtain from clickup_tag_list (field: name)."},
                    "name": {"type": "string", "description": "New tag name. Omit to keep current name."},
                    "tag_fg": {"type": "string", "description": "New text (foreground) hex colour with leading '#'. Note: forwarded as fg_color to the API."},
                    "tag_bg": {"type": "string", "description": "New pill (background) hex colour with leading '#'. Note: forwarded as bg_color to the API."}
                },
                "required": ["space_id", "tag_name"]
            }
        },
        {
            "name": "clickup_tag_delete",
            "description": "Delete a tag from a ClickUp space. The tag is removed from every task that uses it (the tasks themselves are not affected). Destructive and irreversible. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the space containing the tag. Obtain from clickup_space_list (field: id)."},
                    "tag_name": {"type": "string", "description": "Name of the tag to delete. Obtain from clickup_tag_list (field: name)."}
                },
                "required": ["space_id", "tag_name"]
            }
        },
        {
            "name": "clickup_field_unset",
            "description": "Clear a custom field value on a ClickUp task — sets it back to empty/unset. The field definition on the list remains intact. Use clickup_field_set to assign a new value instead. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task. Obtain from clickup_task_list (field: id)."},
                    "field_id": {"type": "string", "description": "ID of the custom field to clear. Obtain from clickup_field_list (field: id) or clickup_task_get (field: custom_fields[].id)."}
                },
                "required": ["task_id", "field_id"]
            }
        },
        {
            "name": "clickup_attachment_list",
            "description": "List files attached to a ClickUp task — each attachment's id, title, size, mime type, url, and uploader. Extracts the attachments array from the Get Task response (ClickUp has no dedicated list endpoint). Use clickup_attachment_upload to add a new file. Returns an array of attachment objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "task_id": {"type": "string", "description": "ID of the task whose attachments to list. Obtain from clickup_task_list (field: id)."}
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "clickup_shared_list",
            "description": "List every task, list, and folder that has been explicitly shared with the authenticated user from outside their default hierarchy (e.g. items shared by other workspace members). Useful for discovering items you have access to but don't own. Returns an object with shared tasks, lists, and folders arrays.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_group_list",
            "description": "List user groups (also called 'teams' in the ClickUp UI) in a workspace. A group is a named collection of users that can be @-mentioned or assigned as a unit. Returns an array of group objects (id, name, members).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "group_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional filter to return only these group IDs. Omit to return all groups in the workspace."
                    }
                },
                "required": []
            }
        },
        {
            "name": "clickup_group_create",
            "description": "Create a new user group ('team' in ClickUp's UI) in a workspace. Groups let you @-mention or assign multiple users as a unit. At least one initial member is required. Returns the created group object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Display name for the group (e.g. 'Frontend Engineers')."},
                    "member_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add as initial members. Obtain from clickup_member_list or clickup_user_get (field: id)."
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "clickup_group_update",
            "description": "Rename a user group and/or add/remove its members. All changes are applied in one call. Use clickup_group_list first to see current membership. Returns the updated group object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": {"type": "string", "description": "ID of the group to update. Obtain from clickup_group_list (field: id)."},
                    "name": {"type": "string", "description": "New display name. Omit to keep current name."},
                    "add_members": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to add to the group (additive — does not replace current members)."
                    },
                    "rem_members": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "User IDs to remove from the group (no-op if not currently a member)."
                    }
                },
                "required": ["group_id"]
            }
        },
        {
            "name": "clickup_group_delete",
            "description": "Permanently delete a ClickUp user group. Destructive and irreversible — assignments and mentions that referenced the group remain as historical records, but the group can no longer be used going forward. The individual users are not affected. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": {"type": "string", "description": "ID of the group to delete. Obtain from clickup_group_list (field: id)."}
                },
                "required": ["group_id"]
            }
        },
        {
            "name": "clickup_role_list",
            "description": "List the custom roles defined in a ClickUp workspace (Member, Guest, Admin, Owner, plus any custom roles on Enterprise plans). Roles define baseline permissions assigned to users. Returns an array of role objects (id, name, members).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_guest_get",
            "description": "Fetch the profile of a specific guest user in a ClickUp workspace — email, permissions (can_edit_tags, can_see_time_spent, can_create_views), and shared items. Guests are external collaborators with limited access. Requires Enterprise plan. Returns the guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Numeric guest user ID. Obtain from clickup_guest_invite (returns the new guest) or the guest's entry in a shared-item's members list."}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_task_time_in_status",
            "description": "Report how long a task has spent in each status since creation (e.g. 3 days in 'open', 1 day in 'in review'). Useful for cycle-time analysis. Returns an object mapping status names to total-time and since-timestamp values (all times in milliseconds).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task. Obtain from clickup_task_list (field: id) or clickup_task_search."}
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
            "description": "Set a per-user time estimate on a ClickUp task. Additive — other users' estimates are untouched. To replace all user estimates at once use clickup_task_replace_estimates instead. Estimates are used in workload views and reports. Returns the updated task estimate object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task. Obtain from clickup_task_list (field: id)."},
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "Numeric ID of the user whose estimate to set. Obtain from clickup_member_list."},
                    "time_estimate": {"type": "integer", "description": "Estimated effort in milliseconds (e.g. 3600000 = 1 hour, 28800000 = 8 hours)."}
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
            "description": "Verify that the configured ClickUp API token is valid by hitting the /user endpoint. Returns an ok:true result if the token is accepted, or an error if it's missing, malformed, expired, or revoked. Use clickup_whoami instead to also get the authenticated user's profile.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "clickup_checklist_update",
            "description": "Rename a checklist or change its position among the task's checklists. Does not affect the checklist's items — use clickup_checklist_update_item / add_item / delete_item for those. Returns the updated checklist object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checklist_id": {"type": "string", "description": "ID of the checklist to update. Obtain from clickup_task_get (field: checklists[].id)."},
                    "name": {"type": "string", "description": "New display name for the checklist. Omit to keep current name."},
                    "position": {"type": "integer", "description": "Zero-indexed position among the task's checklists (0 = first). Omit to keep current position."}
                },
                "required": ["checklist_id"]
            }
        },
        {
            "name": "clickup_comment_replies",
            "description": "List the threaded replies attached to a top-level ClickUp comment, oldest first. Returns an array of reply objects (id, comment_text, user, date). Use clickup_comment_reply to post a new reply to the thread.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "ID of the parent comment. Obtain from clickup_comment_list (field: id)."}
                },
                "required": ["comment_id"]
            }
        },
        {
            "name": "clickup_comment_reply",
            "description": "Post a threaded reply under an existing ClickUp comment. Replies appear indented beneath the parent comment. Returns the created reply object including its new id. For a top-level comment, use clickup_comment_create instead.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "comment_id": {"type": "string", "description": "ID of the parent comment to reply to. Obtain from clickup_comment_list (field: id)."},
                    "text": {"type": "string", "description": "Reply body. Markdown and @mentions supported."},
                    "assignee": {"type": "integer", "description": "Optional user ID to assign the reply to — they receive a notification. Obtain from clickup_member_list."}
                },
                "required": ["comment_id", "text"]
            }
        },
        {
            "name": "clickup_chat_channel_list",
            "description": "List all ClickUp Chat channels in a workspace that the authenticated user can see. Uses v3 cursor pagination. Returns an array of channel objects (id, name, visibility, topic, last_message_at). Use clickup_chat_channel_create to create new channels.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "include_closed": {"type": "boolean", "description": "true = include archived/closed channels in the result; false or omitted = only active channels."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_chat_channel_followers",
            "description": "List the users who follow (receive notifications from) a ClickUp Chat channel. Followers are a subset of members — a member may or may not be a follower. Returns an array of user objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel. Obtain from clickup_chat_channel_list (field: id)."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_channel_members",
            "description": "List the users who are members of a ClickUp Chat channel (can read and post). For notification-receivers only use clickup_chat_channel_followers. Returns an array of user objects (id, username, email).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "channel_id": {"type": "string", "description": "ID of the channel. Obtain from clickup_chat_channel_list (field: id)."}
                },
                "required": ["channel_id"]
            }
        },
        {
            "name": "clickup_chat_message_update",
            "description": "Edit the body of an existing ClickUp Chat message. Only the author (or a workspace admin) can edit a message; others will get a 403. The supplied text replaces the existing body entirely. Returns the updated message object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message to edit. Obtain from clickup_chat_message_list (field: id) or clickup_chat_reply_list."},
                    "text": {"type": "string", "description": "Replacement body for the message. Markdown, emoji, and @mentions supported. Overwrites the existing body entirely."}
                },
                "required": ["message_id", "text"]
            }
        },
        {
            "name": "clickup_chat_reaction_list",
            "description": "List the emoji reactions on a ClickUp Chat message grouped by emoji — each entry includes the emoji, count, and the users who reacted with it. Returns an array of reaction summary objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message whose reactions to list. Obtain from clickup_chat_message_list (field: id)."}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_reaction_add",
            "description": "Add an emoji reaction from the authenticated user to a ClickUp Chat message. If the user has already reacted with the same emoji, the call is a no-op. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message to react to. Obtain from clickup_chat_message_list (field: id) or clickup_chat_reply_list."},
                    "emoji": {"type": "string", "description": "Unicode emoji character to add (e.g. '👍', '🎉', '❤️'). Custom Slack-style shortcodes (':+1:') are not supported — use the raw emoji character."}
                },
                "required": ["message_id", "emoji"]
            }
        },
        {
            "name": "clickup_chat_reaction_remove",
            "description": "Remove the authenticated user's emoji reaction from a ClickUp Chat message. Only removes your own reaction — other users' reactions of the same emoji are preserved. No-op if you haven't reacted with the given emoji. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message. Obtain from clickup_chat_message_list (field: id)."},
                    "emoji": {"type": "string", "description": "The Unicode emoji character to remove (e.g. '👍'). Must match the exact emoji you reacted with."}
                },
                "required": ["message_id", "emoji"]
            }
        },
        {
            "name": "clickup_chat_reply_list",
            "description": "List the threaded replies attached to a top-level ClickUp Chat message, oldest first. Returns an array of reply objects. Use clickup_chat_reply_send to post a new reply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the parent message. Obtain from clickup_chat_message_list (field: id)."}
                },
                "required": ["message_id"]
            }
        },
        {
            "name": "clickup_chat_reply_send",
            "description": "Post a threaded reply beneath an existing ClickUp Chat message. The reply appears in the message's thread panel. Returns the created reply object. Use clickup_chat_message_send for new top-level messages.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the parent message to reply to. Obtain from clickup_chat_message_list (field: id)."},
                    "text": {"type": "string", "description": "Reply body. Markdown, emoji, and @mentions supported."}
                },
                "required": ["message_id", "text"]
            }
        },
        {
            "name": "clickup_chat_tagged_users",
            "description": "List the users explicitly @-mentioned (tagged) in a ClickUp Chat message body. Useful for reading who a message was addressed to. Returns an array of user objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "message_id": {"type": "string", "description": "ID of the message. Obtain from clickup_chat_message_list (field: id)."}
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
            "description": "Apply one or more tags to one or more time tracking entries in a single call. Tags are created automatically if they don't yet exist in the workspace's time-entry tag set. Use clickup_time_tags to list existing tags. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "entry_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "IDs of the time entries to tag. Obtain from clickup_time_list (field: id)."
                    },
                    "tag_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tag names to apply. Created if they don't exist in the workspace's tag set."
                    }
                },
                "required": ["entry_ids", "tag_names"]
            }
        },
        {
            "name": "clickup_time_remove_tags",
            "description": "Detach one or more tags from one or more time tracking entries in a single call. The tag definitions themselves remain in the workspace. No-op for entries not currently carrying the tag. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "entry_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "IDs of the time entries to untag. Obtain from clickup_time_list (field: id)."
                    },
                    "tag_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tag names to remove. Obtain from clickup_time_tags (field: name)."
                    }
                },
                "required": ["entry_ids", "tag_names"]
            }
        },
        {
            "name": "clickup_time_rename_tag",
            "description": "Rename a time-entry tag across the entire workspace. All historical time entries carrying the old name are updated. Cannot change colour via this endpoint. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "name": {"type": "string", "description": "Current name of the tag to rename. Obtain from clickup_time_tags (field: name)."},
                    "new_name": {"type": "string", "description": "Replacement name for the tag. Must not collide with an existing time-entry tag."}
                },
                "required": ["name", "new_name"]
            }
        },
        {
            "name": "clickup_time_history",
            "description": "Fetch the audit history of edits made to a time tracking entry — every start/duration/description/billable change, the user who made it, and when. Useful for auditing. Returns an array of history event objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "timer_id": {"type": "string", "description": "ID of the time entry. Obtain from clickup_time_list (field: id)."}
                },
                "required": ["timer_id"]
            }
        },
        {
            "name": "clickup_guest_invite",
            "description": "Invite a new external guest user to a ClickUp workspace by email. Guests have limited access and don't consume paid member seats (they use guest seats). The invitation email is sent automatically; the guest must accept before they can log in. Share specific items with them via clickup_guest_share_task / _share_list / _share_folder. Requires Enterprise plan. Returns the created guest object including its new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "email": {"type": "string", "description": "Email address to send the invitation to. Must be a valid email that isn't already a member or guest of the workspace."},
                    "can_edit_tags": {"type": "boolean", "description": "true = allow the guest to create/rename/delete tags on items shared with them; false or omitted = tag management denied."},
                    "can_see_time_spent": {"type": "boolean", "description": "true = allow the guest to see time-tracking data on shared tasks; false or omitted = hidden."},
                    "can_create_views": {"type": "boolean", "description": "true = allow the guest to create their own saved views on shared items; false or omitted = cannot create views."}
                },
                "required": ["email"]
            }
        },
        {
            "name": "clickup_guest_update",
            "description": "Update a ClickUp guest's workspace-wide capability flags (edit tags, see time spent, create views). Does not change which items are shared with them — use clickup_guest_share_* / _unshare_* for that. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Numeric guest user ID. Obtain from clickup_guest_get or from clickup_guest_invite (response.id)."},
                    "can_edit_tags": {"type": "boolean", "description": "true = allow the guest to manage tags on shared items; false = deny. Omit to keep current value."},
                    "can_see_time_spent": {"type": "boolean", "description": "true = guest can see time-tracking on shared tasks; false = hidden. Omit to keep current value."},
                    "can_create_views": {"type": "boolean", "description": "true = guest can create saved views; false = cannot. Omit to keep current value."}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_guest_remove",
            "description": "Permanently revoke a guest user's access to a ClickUp workspace. All share-records for the guest are deleted and they can no longer log in. Destructive and irreversible — to re-invite, use clickup_guest_invite (a new guest_id will be assigned). Requires Enterprise plan. Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "guest_id": {"type": "integer", "description": "Numeric guest user ID to remove. Obtain from clickup_guest_get or clickup_guest_invite. All their shared-item access is revoked."}
                },
                "required": ["guest_id"]
            }
        },
        {
            "name": "clickup_guest_share_task",
            "description": "Grant a ClickUp guest user access to a single task at a specified permission level. Scopes strictly to that task — subtasks and the parent list are not shared. Use clickup_guest_unshare_task to revoke. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to share. Obtain from clickup_task_list (field: id) or clickup_task_search."},
                    "guest_id": {"type": "integer", "description": "Numeric ID of the guest user. Obtain from clickup_guest_get or clickup_guest_invite (response.id)."},
                    "permission": {"type": "string", "description": "Access level: 'read' (view only), 'comment' (view + comment), 'create' (comment + create subtasks), 'edit' (full edit rights on this task)."}
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
            "description": "Grant a ClickUp guest user access to a specific list at a chosen permission level. Guests are external collaborators (not paid workspace seats); this is how you scope what a guest can see/do. To revoke access later use clickup_guest_unshare_list. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to share. Obtain from clickup_list_list (field: id)."},
                    "guest_id": {"type": "integer", "description": "Numeric ID of the guest user. Obtain from clickup_guest_get or the response of clickup_guest_invite."},
                    "permission": {"type": "string", "description": "Access level: 'read' (view only), 'comment' (view + comment), 'create' (comment + create tasks), 'edit' (full edit rights on existing items)."}
                },
                "required": ["list_id", "guest_id", "permission"]
            }
        },
        {
            "name": "clickup_guest_unshare_list",
            "description": "Revoke a guest user's access to a specific list. The guest keeps any separate task-level or folder-level grants they may also have. Destructive in that the guest immediately loses access, but the guest account itself remains — re-share with clickup_guest_share_list. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list whose access to revoke. Obtain from clickup_list_list (field: id)."},
                    "guest_id": {"type": "integer", "description": "Numeric guest user ID. Obtain from clickup_guest_get or clickup_guest_invite."}
                },
                "required": ["list_id", "guest_id"]
            }
        },
        {
            "name": "clickup_guest_share_folder",
            "description": "Grant a ClickUp guest user access to an entire folder — including all its lists and tasks — at a specified permission level. Use clickup_guest_share_list or _share_task for narrower scope. Use clickup_guest_unshare_folder to revoke. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the folder to share. Obtain from clickup_folder_list (field: id). All descendant lists and tasks are accessible via this grant."},
                    "guest_id": {"type": "integer", "description": "Numeric ID of the guest user. Obtain from clickup_guest_get or clickup_guest_invite (response.id)."},
                    "permission": {"type": "string", "description": "Access level applied to every descendant: 'read' (view only), 'comment' (view + comment), 'create' (comment + create tasks), 'edit' (full edit rights)."}
                },
                "required": ["folder_id", "guest_id", "permission"]
            }
        },
        {
            "name": "clickup_guest_unshare_folder",
            "description": "Revoke a guest user's access to a folder (and, cascading, to every list and task under it granted via the folder). Separate list-level or task-level grants are preserved. Re-share later with clickup_guest_share_folder. Requires Enterprise plan. Returns the updated guest object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_id": {"type": "string", "description": "ID of the folder whose access to revoke. Obtain from clickup_folder_list (field: id)."},
                    "guest_id": {"type": "integer", "description": "Numeric guest user ID. Obtain from clickup_guest_get or clickup_guest_invite."}
                },
                "required": ["folder_id", "guest_id"]
            }
        },
        {
            "name": "clickup_user_invite",
            "description": "Invite a new paid member to a ClickUp workspace by email. Consumes a member seat (see clickup_workspace_seats for availability). For external collaborators who shouldn't have full access, use clickup_guest_invite instead. Returns the created user object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "email": {"type": "string", "description": "Email address to send the invitation to. Must be a valid email not already a member or guest of the workspace."},
                    "admin": {"type": "boolean", "description": "true = grant the Admin role (can manage settings, billing, users); false or omitted = standard Member role."}
                },
                "required": ["email"]
            }
        },
        {
            "name": "clickup_user_update",
            "description": "Update a ClickUp workspace member's username and/or admin role. Only the authenticated user (if self) or a workspace admin can call this. To change per-item permissions use role-based or share endpoints instead. Returns the updated user object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "Numeric user ID to update. Obtain from clickup_member_list or clickup_user_get (field: id)."},
                    "username": {"type": "string", "description": "New display name. Omit to keep current username."},
                    "admin": {"type": "boolean", "description": "true = grant Admin role, false = revoke Admin (revert to Member). Omit to keep current role."}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_user_remove",
            "description": "Remove a member from a ClickUp workspace, freeing their paid seat. Destructive — their assignments and comments are preserved as historical records but they lose access immediately. To re-add, use clickup_user_invite (a new invitation will be sent). Returns an empty object on success.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "user_id": {"type": "integer", "description": "Numeric user ID to remove. Obtain from clickup_member_list (field: id). Cannot remove the workspace Owner."}
                },
                "required": ["user_id"]
            }
        },
        {
            "name": "clickup_template_apply_task",
            "description": "Create a new task in a list by instantiating a saved task template. The new task inherits the template's description, checklists, subtasks, custom fields, etc., but uses the supplied name. Use clickup_template_list to discover templates. Returns the created task object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list_id": {"type": "string", "description": "ID of the list to create the task in. Obtain from clickup_list_list (field: id)."},
                    "template_id": {"type": "string", "description": "ID of the task template to instantiate. Obtain from clickup_template_list (field: id)."},
                    "name": {"type": "string", "description": "Name for the newly-created task. Overrides the template's default name."}
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
            "description": "Create a new folder in a space by instantiating a saved folder template. The new folder inherits the template's list structure, statuses, default fields, and other presets, using the supplied name. Use clickup_template_list to discover templates. Returns the created folder object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "space_id": {"type": "string", "description": "ID of the parent space. Obtain from clickup_space_list (field: id)."},
                    "template_id": {"type": "string", "description": "ID of the folder template to instantiate. Obtain from clickup_template_list (field: id)."},
                    "name": {"type": "string", "description": "Name for the newly-created folder. Must be unique within the parent space."}
                },
                "required": ["space_id", "template_id", "name"]
            }
        },
        {
            "name": "clickup_attachment_upload",
            "description": "Upload a local file as an attachment on a ClickUp task. The file is read from disk, posted as multipart/form-data, and stored on ClickUp's CDN. Use clickup_attachment_list to see attachments afterward. Returns the created attachment object (id, title, size, url).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "ID of the task to attach the file to. Obtain from clickup_task_list (field: id)."},
                    "file_path": {"type": "string", "description": "Absolute path to a readable file on the server running this MCP. The filename (basename) is used as the attachment title; size limits apply per workspace plan."}
                },
                "required": ["task_id", "file_path"]
            }
        },
        {
            "name": "clickup_task_type_list",
            "description": "List the custom task types (ClickUp 'Custom Items' — e.g. Bug, Epic, Feature) defined at the workspace level. Each has an id, name, and icon and can be chosen when creating tasks. Returns an array of custom item type objects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."}
                },
                "required": []
            }
        },
        {
            "name": "clickup_doc_get_page",
            "description": "Fetch a single page from a ClickUp doc including its full markdown content, title, subtitle, and parent-page link. Use clickup_doc_pages to list all pages in a doc first. Returns the page object with content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the parent doc. Obtain from clickup_doc_list (field: id)."},
                    "page_id": {"type": "string", "description": "ID of the page to fetch. Obtain from clickup_doc_pages (field: id)."}
                },
                "required": ["doc_id", "page_id"]
            }
        },
        {
            "name": "clickup_audit_log_query",
            "description": "Query the ClickUp audit log (who did what, when) for a workspace — filter by event type, acting user, and date range. Requires Enterprise plan. Uses v3 cursor pagination. Returns an array of audit event objects (actor, event, target, timestamp).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "type": {"type": "string", "description": "Audit event type filter (e.g. 'task_created', 'user_added', 'permission_changed'). Required. See ClickUp docs for the full list."},
                    "user_id": {"type": "integer", "description": "Restrict to events performed by this user ID. Obtain from clickup_member_list. Omit for all users."},
                    "start_date": {"type": "integer", "description": "Inclusive lower bound as a Unix timestamp in milliseconds (e.g. 1735689600000 for 2025-01-01). Omit for no lower bound."},
                    "end_date": {"type": "integer", "description": "Inclusive upper bound as a Unix timestamp in milliseconds. Omit for no upper bound."}
                },
                "required": ["type"]
            }
        },
        {
            "name": "clickup_acl_update",
            "description": "Change the privacy (ACL) of a ClickUp hierarchy object — make a space/folder/list private (explicit members only) or public (whole workspace). Uses the v3 ACL endpoint. Requires Enterprise plan. Returns the updated object.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Obtain from clickup_workspace_list (field: id). Omit to use the default workspace from config."},
                    "object_type": {"type": "string", "description": "Type of object to change: 'space', 'folder', or 'list'."},
                    "object_id": {"type": "string", "description": "ID of the space/folder/list. Obtain from the matching list endpoint (clickup_space_list, clickup_folder_list, or clickup_list_list)."},
                    "private": {"type": "boolean", "description": "true = make the object private (only explicit members see it); false = make it public (visible to the whole workspace)."}
                },
                "required": ["object_type", "object_id"]
            }
        }
    ])
}

/// Returns `tool_list()` with any tool the filter disallows removed.
pub fn filtered_tool_list(filter: &filter::Filter) -> serde_json::Value {
    let all = tool_list();
    let filtered: Vec<serde_json::Value> = all
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|tool| {
                    tool.get("name")
                        .and_then(|v| v.as_str())
                        .map(|n| filter.allows(n))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    serde_json::Value::Array(filtered)
}

/// Compact custom field definitions while preserving option IDs for fields
/// whose possible values live in `type_config.options`.
pub fn compact_custom_fields(fields: &[Value]) -> Value {
    let compacted: Vec<Value> = fields
        .iter()
        .map(|field| {
            let mut obj = serde_json::Map::new();
            for key in ["id", "name", "type", "required"] {
                obj.insert(
                    key.to_string(),
                    Value::String(flatten_value(field.get(key))),
                );
            }

            if let Some(options) = field
                .get("type_config")
                .and_then(|config| config.get("options"))
                .and_then(|options| options.as_array())
            {
                obj.insert("options".to_string(), Value::Array(options.clone()));
            }

            Value::Object(obj)
        })
        .collect();

    Value::Array(compacted)
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
            let teams = resp
                .get("teams")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
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
            let archived = args
                .get("archived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let path = format!("/v2/team/{}/space?archived={}", team_id, archived);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let spaces = resp
                .get("spaces")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &spaces,
                &["id", "name", "private", "archived"],
            ))
        }

        "clickup_folder_list" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let archived = args
                .get("archived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let path = format!("/v2/space/{}/folder?archived={}", space_id, archived);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let folders = resp
                .get("folders")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            let items: Vec<Value> = folders
                .iter()
                .map(|f| {
                    let list_count = f
                        .get("lists")
                        .and_then(|l| l.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    json!({
                        "id": f.get("id"),
                        "name": f.get("name"),
                        "task_count": f.get("task_count"),
                        "list_count": list_count,
                    })
                })
                .collect();
            Ok(compact_items(
                &items,
                &["id", "name", "task_count", "list_count"],
            ))
        }

        "clickup_list_list" => {
            let archived = args
                .get("archived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let path = if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                format!("/v2/folder/{}/list?archived={}", folder_id, archived)
            } else if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                format!("/v2/space/{}/list?archived={}", space_id, archived)
            } else {
                return Err("Provide either folder_id or space_id".to_string());
            };
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let lists = resp
                .get("lists")
                .and_then(|l| l.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &lists,
                &["id", "name", "task_count", "status", "due_date"],
            ))
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
            let tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &tasks,
                &["id", "name", "status", "priority", "assignees", "due_date"],
            ))
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
            let path = format!("/v2/task/{}?include_subtasks={}", task_id, include_subtasks);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            Ok(compact_items(
                &[resp],
                &[
                    "id",
                    "name",
                    "status",
                    "priority",
                    "assignees",
                    "due_date",
                    "description",
                ],
            ))
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
            Ok(compact_items(
                &[resp],
                &["id", "name", "status", "priority", "assignees", "due_date"],
            ))
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
            Ok(compact_items(
                &[resp],
                &["id", "name", "status", "priority", "assignees", "due_date"],
            ))
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
            let path = format!("/v2/team/{}/task?{}", team_id, qs.trim_start_matches('&'));
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &tasks,
                &["id", "name", "status", "priority", "assignees", "due_date"],
            ))
        }

        "clickup_comment_list" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let path = format!("/v2/task/{}/comment", task_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let comments = resp
                .get("comments")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &comments,
                &["id", "user", "date", "comment_text"],
            ))
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
            let fields = resp
                .get("fields")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_custom_fields(&fields))
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
            let value = args
                .get("value")
                .ok_or("Missing required parameter: value")?;
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
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "billable"],
            ))
        }

        "clickup_time_stop" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v2/team/{}/time_entries/stop", team_id);
            let resp = client
                .post(&path, &json!({}))
                .await
                .map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "end", "billable"],
            ))
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
            let entries = resp
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &entries,
                &["id", "task", "duration", "start", "billable"],
            ))
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
            let goals = resp
                .get("goals")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &goals,
                &["id", "name", "percent_completed", "due_date"],
            ))
        }

        "clickup_goal_get" => {
            let goal_id = args
                .get("goal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            let path = format!("/v2/goal/{}", goal_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[goal],
                &["id", "name", "percent_completed", "due_date", "description"],
            ))
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
            let views = resp
                .get("views")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
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
            let tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &tasks,
                &["id", "name", "status", "priority", "assignees", "due_date"],
            ))
        }

        "clickup_doc_list" => {
            let team_id = resolve_workspace(args)?;
            let path = format!("/v3/workspaces/{}/docs", team_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let docs = resp
                .get("docs")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
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
            let content = args
                .get("content")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let path = format!("/v3/workspaces/{}/docs/{}/pages?content_format=text/md&max_page_depth=-1&include_content={}", team_id, doc_id, content);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let pages = resp
                .get("pages")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&pages, &["id", "name"]))
        }

        "clickup_tag_list" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let path = format!("/v2/space/{}/tag", space_id);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let tags = resp
                .get("tags")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
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
            client
                .post(&path, &json!({}))
                .await
                .map_err(|e| e.to_string())?;
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
            let webhooks = resp
                .get("webhooks")
                .and_then(|w| w.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &webhooks,
                &["id", "endpoint", "events", "status"],
            ))
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
            let members = resp
                .get("members")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&members, &["id", "username", "email"]))
        }

        "clickup_template_list" => {
            let team_id = resolve_workspace(args)?;
            let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
            let path = format!("/v2/team/{}/taskTemplate?page={}", team_id, page);
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let templates = resp
                .get("templates")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&templates, &["id", "name"]))
        }

        "clickup_space_get" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let resp = client
                .get(&format!("/v2/space/{}", space_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(
                &[resp],
                &["id", "name", "private", "archived"],
            ))
        }

        "clickup_space_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) {
                body["private"] = json!(private);
            }
            let resp = client
                .post(&format!("/v2/team/{}/space", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "private"]))
        }

        "clickup_space_update" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) {
                body["private"] = json!(private);
            }
            if let Some(archived) = args.get("archived").and_then(|v| v.as_bool()) {
                body["archived"] = json!(archived);
            }
            let resp = client
                .put(&format!("/v2/space/{}", space_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(
                &[resp],
                &["id", "name", "private", "archived"],
            ))
        }

        "clickup_space_delete" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            client
                .delete(&format!("/v2/space/{}", space_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Space {} deleted", space_id)}))
        }

        "clickup_folder_get" => {
            let folder_id = args
                .get("folder_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let resp = client
                .get(&format!("/v2/folder/{}", folder_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "task_count"]))
        }

        "clickup_folder_create" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client
                .post(&format!("/v2/space/{}/folder", space_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_folder_update" => {
            let folder_id = args
                .get("folder_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client
                .put(&format!("/v2/folder/{}", folder_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_folder_delete" => {
            let folder_id = args
                .get("folder_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            client
                .delete(&format!("/v2/folder/{}", folder_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Folder {} deleted", folder_id)}))
        }

        "clickup_list_get" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let resp = client
                .get(&format!("/v2/list/{}", list_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(
                &[resp],
                &["id", "name", "task_count", "status", "due_date"],
            ))
        }

        "clickup_list_create" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                body["content"] = json!(content);
            }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) {
                body["due_date"] = json!(due_date);
            }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
                body["status"] = json!(status);
            }
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
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                body["content"] = json!(content);
            }
            if let Some(due_date) = args.get("due_date").and_then(|v| v.as_i64()) {
                body["due_date"] = json!(due_date);
            }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
                body["status"] = json!(status);
            }
            let resp = client
                .put(&format!("/v2/list/{}", list_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(
                &[resp],
                &["id", "name", "task_count", "status"],
            ))
        }

        "clickup_list_delete" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            client
                .delete(&format!("/v2/list/{}", list_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("List {} deleted", list_id)}))
        }

        "clickup_list_add_task" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            client
                .post(
                    &format!("/v2/list/{}/task/{}", list_id, task_id),
                    &json!({}),
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} added to list {}", task_id, list_id)}))
        }

        "clickup_list_remove_task" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            client
                .delete(&format!("/v2/list/{}/task/{}", list_id, task_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} removed from list {}", task_id, list_id)}))
        }

        "clickup_comment_update" => {
            let comment_id = args
                .get("comment_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let mut body = json!({"comment_text": text});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) {
                body["assignee"] = json!(assignee);
            }
            if let Some(resolved) = args.get("resolved").and_then(|v| v.as_bool()) {
                body["resolved"] = json!(resolved);
            }
            client
                .put(&format!("/v2/comment/{}", comment_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Comment {} updated", comment_id)}))
        }

        "clickup_comment_delete" => {
            let comment_id = args
                .get("comment_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            client
                .delete(&format!("/v2/comment/{}", comment_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Comment {} deleted", comment_id)}))
        }

        "clickup_task_add_dep" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let mut body = json!({});
            if let Some(dep) = args.get("depends_on").and_then(|v| v.as_str()) {
                body["depends_on"] = json!(dep);
            }
            if let Some(dep) = args.get("dependency_of").and_then(|v| v.as_str()) {
                body["dependency_of"] = json!(dep);
            }
            client
                .post(&format!("/v2/task/{}/dependency", task_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Dependency added to task {}", task_id)}))
        }

        "clickup_task_remove_dep" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let mut body = json!({});
            if let Some(dep) = args.get("depends_on").and_then(|v| v.as_str()) {
                body["depends_on"] = json!(dep);
            }
            if let Some(dep) = args.get("dependency_of").and_then(|v| v.as_str()) {
                body["dependency_of"] = json!(dep);
            }
            client
                .delete_with_body(&format!("/v2/task/{}/dependency", task_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Dependency removed from task {}", task_id)}))
        }

        "clickup_task_link" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let links_to = args
                .get("links_to")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: links_to")?;
            let resp = client
                .post(
                    &format!("/v2/task/{}/link/{}", task_id, links_to),
                    &json!({}),
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} linked to {}", task_id, links_to), "data": resp}))
        }

        "clickup_task_unlink" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let links_to = args
                .get("links_to")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: links_to")?;
            client
                .delete(&format!("/v2/task/{}/link/{}", task_id, links_to))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} unlinked from {}", task_id, links_to)}))
        }

        "clickup_goal_delete" => {
            let goal_id = args
                .get("goal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            client
                .delete(&format!("/v2/goal/{}", goal_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Goal {} deleted", goal_id)}))
        }

        "clickup_goal_add_kr" => {
            let goal_id = args
                .get("goal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: goal_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let kr_type = args
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let steps_start = args
                .get("steps_start")
                .and_then(|v| v.as_f64())
                .ok_or("Missing required parameter: steps_start")?;
            let steps_end = args
                .get("steps_end")
                .and_then(|v| v.as_f64())
                .ok_or("Missing required parameter: steps_end")?;
            let mut body = json!({"name": name, "type": kr_type, "steps_start": steps_start, "steps_end": steps_end});
            if let Some(unit) = args.get("unit").and_then(|v| v.as_str()) {
                body["unit"] = json!(unit);
            }
            if let Some(owners) = args.get("owner_ids") {
                body["owners"] = owners.clone();
            }
            if let Some(task_ids) = args.get("task_ids") {
                body["task_ids"] = task_ids.clone();
            }
            if let Some(list_ids) = args.get("list_ids") {
                body["list_ids"] = list_ids.clone();
            }
            let resp = client
                .post(&format!("/v2/goal/{}/key_result", goal_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[kr],
                &[
                    "id",
                    "name",
                    "type",
                    "steps_start",
                    "steps_end",
                    "steps_current",
                ],
            ))
        }

        "clickup_goal_update_kr" => {
            let kr_id = args
                .get("kr_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: kr_id")?;
            let mut body = json!({});
            if let Some(v) = args.get("steps_current").and_then(|v| v.as_f64()) {
                body["steps_current"] = json!(v);
            }
            if let Some(v) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(v);
            }
            if let Some(v) = args.get("unit").and_then(|v| v.as_str()) {
                body["unit"] = json!(v);
            }
            let resp = client
                .put(&format!("/v2/key_result/{}", kr_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[kr],
                &["id", "name", "steps_current", "steps_end"],
            ))
        }

        "clickup_goal_delete_kr" => {
            let kr_id = args
                .get("kr_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: kr_id")?;
            client
                .delete(&format!("/v2/key_result/{}", kr_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Key result {} deleted", kr_id)}))
        }

        "clickup_time_get" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args
                .get("timer_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/{}", team_id, timer_id))
                .await
                .map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "end", "billable"],
            ))
        }

        "clickup_time_create" => {
            let team_id = resolve_workspace(args)?;
            let start = args
                .get("start")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: start")?;
            let duration = args
                .get("duration")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: duration")?;
            let mut body = json!({"start": start, "duration": duration});
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) {
                body["tid"] = json!(task_id);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(billable) = args.get("billable").and_then(|v| v.as_bool()) {
                body["billable"] = json!(billable);
            }
            let resp = client
                .post(&format!("/v2/team/{}/time_entries", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "billable"],
            ))
        }

        "clickup_time_update" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args
                .get("timer_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let mut body = json!({});
            if let Some(start) = args.get("start").and_then(|v| v.as_i64()) {
                body["start"] = json!(start);
            }
            if let Some(duration) = args.get("duration").and_then(|v| v.as_i64()) {
                body["duration"] = json!(duration);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(billable) = args.get("billable").and_then(|v| v.as_bool()) {
                body["billable"] = json!(billable);
            }
            let resp = client
                .put(
                    &format!("/v2/team/{}/time_entries/{}", team_id, timer_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "billable"],
            ))
        }

        "clickup_time_delete" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args
                .get("timer_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            client
                .delete(&format!("/v2/team/{}/time_entries/{}", team_id, timer_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time entry {} deleted", timer_id)}))
        }

        "clickup_view_get" => {
            let view_id = args
                .get("view_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            let resp = client
                .get(&format!("/v2/view/{}", view_id))
                .await
                .map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_create" => {
            let scope = args
                .get("scope")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: scope")?;
            let scope_id = args
                .get("scope_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: scope_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let view_type = args
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let body = json!({"name": name, "type": view_type});
            let resp = client
                .post(&format!("/v2/{}/{}/view", scope, scope_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_update" => {
            let view_id = args
                .get("view_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let view_type = args
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: type")?;
            let body = json!({"name": name, "type": view_type});
            let resp = client
                .put(&format!("/v2/view/{}", view_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            Ok(compact_items(&[view], &["id", "name", "type"]))
        }

        "clickup_view_delete" => {
            let view_id = args
                .get("view_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: view_id")?;
            client
                .delete(&format!("/v2/view/{}", view_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("View {} deleted", view_id)}))
        }

        "clickup_doc_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(parent) = args.get("parent") {
                body["parent"] = parent.clone();
            }
            let resp = client
                .post(&format!("/v3/workspaces/{}/docs", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_doc_add_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                body["content"] = json!(content);
            }
            if let Some(subtitle) = args.get("sub_title").and_then(|v| v.as_str()) {
                body["sub_title"] = json!(subtitle);
            }
            if let Some(parent_page_id) = args.get("parent_page_id").and_then(|v| v.as_str()) {
                body["parent_page_id"] = json!(parent_page_id);
            }
            let resp = client
                .post(
                    &format!("/v3/workspaces/{}/docs/{}/pages", team_id, doc_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_doc_edit_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let page_id = args
                .get("page_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: page_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                body["content"] = json!(content);
            }
            let resp = client
                .put(
                    &format!(
                        "/v3/workspaces/{}/docs/{}/pages/{}",
                        team_id, doc_id, page_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_chat_channel_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            if let Some(vis) = args.get("visibility").and_then(|v| v.as_str()) {
                body["visibility"] = json!(vis);
            }
            let resp = client
                .post(&format!("/v3/workspaces/{}/chat/channels", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "visibility"]))
        }

        "clickup_chat_channel_get" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/channels/{}",
                    team_id, channel_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name", "visibility"]))
        }

        "clickup_chat_channel_update" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                body["description"] = json!(desc);
            }
            let resp = client
                .patch(
                    &format!("/v3/workspaces/{}/chat/channels/{}", team_id, channel_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_chat_channel_delete" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            client
                .delete(&format!(
                    "/v3/workspaces/{}/chat/channels/{}",
                    team_id, channel_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Channel {} deleted", channel_id)}))
        }

        "clickup_chat_message_list" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let mut path = format!(
                "/v3/workspaces/{}/chat/channels/{}/messages",
                team_id, channel_id
            );
            if let Some(cursor) = args.get("cursor").and_then(|v| v.as_str()) {
                path.push_str(&format!("?cursor={}", cursor));
            }
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let messages = resp
                .get("messages")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&messages, &["id", "content", "date"]))
        }

        "clickup_chat_message_send" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: content")?;
            let body = json!({"content": content});
            let resp = client
                .post(
                    &format!(
                        "/v3/workspaces/{}/chat/channels/{}/messages",
                        team_id, channel_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "Message sent", "id": resp.get("id")}))
        }

        "clickup_chat_message_delete" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            client
                .delete(&format!(
                    "/v3/workspaces/{}/chat/messages/{}",
                    team_id, message_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Message {} deleted", message_id)}))
        }

        "clickup_chat_dm" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: content")?;
            let body = json!({"user_id": user_id, "content": content});
            let resp = client
                .post(
                    &format!("/v3/workspaces/{}/chat/channels/direct_message", team_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "DM sent", "id": resp.get("id")}))
        }

        "clickup_webhook_create" => {
            let team_id = resolve_workspace(args)?;
            let endpoint = args
                .get("endpoint")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: endpoint")?;
            let events = args
                .get("events")
                .ok_or("Missing required parameter: events")?;
            let mut body = json!({"endpoint": endpoint, "events": events});
            if let Some(space_id) = args.get("space_id").and_then(|v| v.as_str()) {
                body["space_id"] = json!(space_id);
            }
            if let Some(folder_id) = args.get("folder_id").and_then(|v| v.as_str()) {
                body["folder_id"] = json!(folder_id);
            }
            if let Some(list_id) = args.get("list_id").and_then(|v| v.as_str()) {
                body["list_id"] = json!(list_id);
            }
            if let Some(task_id) = args.get("task_id").and_then(|v| v.as_str()) {
                body["task_id"] = json!(task_id);
            }
            let resp = client
                .post(&format!("/v2/team/{}/webhook", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let webhook = resp.get("webhook").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[webhook],
                &["id", "endpoint", "events", "status"],
            ))
        }

        "clickup_webhook_update" => {
            let webhook_id = args
                .get("webhook_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: webhook_id")?;
            let mut body = json!({});
            if let Some(endpoint) = args.get("endpoint").and_then(|v| v.as_str()) {
                body["endpoint"] = json!(endpoint);
            }
            if let Some(events) = args.get("events") {
                body["events"] = events.clone();
            }
            if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
                body["status"] = json!(status);
            }
            let resp = client
                .put(&format!("/v2/webhook/{}", webhook_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let webhook = resp.get("webhook").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[webhook],
                &["id", "endpoint", "events", "status"],
            ))
        }

        "clickup_webhook_delete" => {
            let webhook_id = args
                .get("webhook_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: webhook_id")?;
            client
                .delete(&format!("/v2/webhook/{}", webhook_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Webhook {} deleted", webhook_id)}))
        }

        "clickup_checklist_add_item" => {
            let checklist_id = args
                .get("checklist_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) {
                body["assignee"] = json!(assignee);
            }
            let resp = client
                .post(
                    &format!("/v2/checklist/{}/checklist_item", checklist_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            let item = resp.get("checklist").cloned().unwrap_or(resp);
            Ok(compact_items(&[item], &["id", "name"]))
        }

        "clickup_checklist_update_item" => {
            let checklist_id = args
                .get("checklist_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let item_id = args
                .get("item_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: item_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(resolved) = args.get("resolved").and_then(|v| v.as_bool()) {
                body["resolved"] = json!(resolved);
            }
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) {
                body["assignee"] = json!(assignee);
            }
            client
                .put(
                    &format!("/v2/checklist/{}/checklist_item/{}", checklist_id, item_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Checklist item {} updated", item_id)}))
        }

        "clickup_checklist_delete_item" => {
            let checklist_id = args
                .get("checklist_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let item_id = args
                .get("item_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: item_id")?;
            client
                .delete(&format!(
                    "/v2/checklist/{}/checklist_item/{}",
                    checklist_id, item_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Checklist item {} deleted", item_id)}))
        }

        "clickup_user_get" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let resp = client
                .get(&format!("/v2/team/{}/user/{}", team_id, user_id))
                .await
                .map_err(|e| e.to_string())?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            Ok(compact_items(&[member], &["user", "role"]))
        }

        "clickup_workspace_seats" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/seats", team_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!(resp))
        }

        "clickup_workspace_plan" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/plan", team_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!(resp))
        }

        "clickup_tag_create" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut tag = json!({"name": name});
            if let Some(fg) = args.get("tag_fg").and_then(|v| v.as_str()) {
                tag["tag_fg"] = json!(fg);
            }
            if let Some(bg) = args.get("tag_bg").and_then(|v| v.as_str()) {
                tag["tag_bg"] = json!(bg);
            }
            let body = json!({"tag": tag});
            client
                .post(&format!("/v2/space/{}/tag", space_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' created in space {}", name, space_id)}))
        }

        "clickup_tag_update" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let tag_name = args
                .get("tag_name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            let mut tag = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                tag["name"] = json!(name);
            }
            if let Some(fg) = args.get("tag_fg").and_then(|v| v.as_str()) {
                tag["tag_fg"] = json!(fg);
            }
            if let Some(bg) = args.get("tag_bg").and_then(|v| v.as_str()) {
                tag["tag_bg"] = json!(bg);
            }
            let body = json!({"tag": tag});
            client
                .put(&format!("/v2/space/{}/tag/{}", space_id, tag_name), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' updated", tag_name)}))
        }

        "clickup_tag_delete" => {
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let tag_name = args
                .get("tag_name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: tag_name")?;
            client
                .delete(&format!("/v2/space/{}/tag/{}", space_id, tag_name))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' deleted from space {}", tag_name, space_id)}))
        }

        "clickup_field_unset" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let field_id = args
                .get("field_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: field_id")?;
            client
                .delete(&format!("/v2/task/{}/field/{}", task_id, field_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Field {} unset on task {}", field_id, task_id)}))
        }

        "clickup_attachment_list" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            // ClickUp has no dedicated list-attachments endpoint. The `attachments`
            // array is returned inline by GET /v2/task/{id}, per the API docs.
            let resp = client
                .get(&format!("/v2/task/{}", task_id))
                .await
                .map_err(|e| e.to_string())?;
            let attachments = resp
                .get("attachments")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&attachments, &["id", "title", "url", "date"]))
        }

        "clickup_shared_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/shared", team_id))
                .await
                .map_err(|e| e.to_string())?;
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
            let resp = client
                .get(&format!("/v2/group?{}", qs))
                .await
                .map_err(|e| e.to_string())?;
            let groups = resp
                .get("groups")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&groups, &["id", "name", "members"]))
        }

        "clickup_group_create" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let mut body = json!({"name": name});
            if let Some(members) = args.get("member_ids") {
                body["members"] = members.clone();
            }
            let resp = client
                .post(&format!("/v2/team/{}/group", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_group_update" => {
            let group_id = args
                .get("group_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: group_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(add) = args.get("add_members") {
                body["members"] = json!({"add": add, "rem": args.get("rem_members").cloned().unwrap_or(json!([]))});
            } else if let Some(rem) = args.get("rem_members") {
                body["members"] = json!({"add": [], "rem": rem});
            }
            let resp = client
                .put(&format!("/v2/group/{}", group_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_group_delete" => {
            let group_id = args
                .get("group_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: group_id")?;
            client
                .delete(&format!("/v2/group/{}", group_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Group {} deleted", group_id)}))
        }

        "clickup_role_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/customroles", team_id))
                .await
                .map_err(|e| e.to_string())?;
            let roles = resp
                .get("roles")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&roles, &["id", "name"]))
        }

        "clickup_guest_get" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let resp = client
                .get(&format!("/v2/team/{}/guest/{}", team_id, guest_id))
                .await
                .map_err(|e| e.to_string())?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            Ok(compact_items(&[guest], &["user", "role"]))
        }

        "clickup_task_time_in_status" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let resp = client
                .get(&format!("/v2/task/{}/time_in_status", task_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_task_move" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            client
                .put(
                    &format!(
                        "/v3/workspaces/{}/tasks/{}/home_list/{}",
                        team_id, task_id, list_id
                    ),
                    &json!({}),
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} moved to list {}", task_id, list_id)}))
        }

        "clickup_task_set_estimate" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let time_estimate = args
                .get("time_estimate")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: time_estimate")?;
            let body =
                json!({"time_estimates": [{"user_id": user_id, "time_estimate": time_estimate}]});
            client
                .patch(
                    &format!(
                        "/v3/workspaces/{}/tasks/{}/time_estimates_by_user",
                        team_id, task_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time estimate set for task {}", task_id)}))
        }

        "clickup_task_replace_estimates" => {
            let team_id = resolve_workspace(args)?;
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let time_estimate = args
                .get("time_estimate")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: time_estimate")?;
            let body =
                json!({"time_estimates": [{"user_id": user_id, "time_estimate": time_estimate}]});
            client
                .put(
                    &format!(
                        "/v3/workspaces/{}/tasks/{}/time_estimates_by_user",
                        team_id, task_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Time estimates replaced for task {}", task_id)}))
        }

        "clickup_auth_check" => {
            client.get("/v2/user").await.map_err(|e| e.to_string())?;
            Ok(json!({"message": "Token valid"}))
        }

        "clickup_checklist_update" => {
            let checklist_id = args
                .get("checklist_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: checklist_id")?;
            let mut body = json!({});
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = json!(name);
            }
            if let Some(position) = args.get("position").and_then(|v| v.as_i64()) {
                body["position"] = json!(position);
            }
            let resp = client
                .put(&format!("/v2/checklist/{}", checklist_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            Ok(compact_items(&[checklist], &["id", "name"]))
        }

        "clickup_comment_replies" => {
            let comment_id = args
                .get("comment_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let resp = client
                .get(&format!("/v2/comment/{}/reply", comment_id))
                .await
                .map_err(|e| e.to_string())?;
            let comments = resp
                .get("comments")
                .or_else(|| resp.get("replies"))
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(
                &comments,
                &["id", "user", "date", "comment_text"],
            ))
        }

        "clickup_comment_reply" => {
            let comment_id = args
                .get("comment_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: comment_id")?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let mut body = json!({"comment_text": text});
            if let Some(assignee) = args.get("assignee").and_then(|v| v.as_i64()) {
                body["assignee"] = json!(assignee);
            }
            let resp = client
                .post(&format!("/v2/comment/{}/reply", comment_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "Reply posted", "id": resp.get("id")}))
        }

        "clickup_chat_channel_list" => {
            let team_id = resolve_workspace(args)?;
            let mut path = format!("/v3/workspaces/{}/chat/channels", team_id);
            if let Some(include_closed) = args.get("include_closed").and_then(|v| v.as_bool()) {
                path.push_str(&format!("?include_closed={}", include_closed));
            }
            let resp = client.get(&path).await.map_err(|e| e.to_string())?;
            let channels = resp
                .get("channels")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&channels, &["id", "name", "type"]))
        }

        "clickup_chat_channel_followers" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/channels/{}/followers",
                    team_id, channel_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_channel_members" => {
            let team_id = resolve_workspace(args)?;
            let channel_id = args
                .get("channel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: channel_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/channels/{}/members",
                    team_id, channel_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_message_update" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let body = json!({"content": text});
            client
                .patch(
                    &format!("/v3/workspaces/{}/chat/messages/{}", team_id, message_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Message {} updated", message_id)}))
        }

        "clickup_chat_reaction_list" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/messages/{}/reactions",
                    team_id, message_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_reaction_add" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let emoji = args
                .get("emoji")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: emoji")?;
            let body = json!({"emoji": emoji});
            client
                .post(
                    &format!(
                        "/v3/workspaces/{}/chat/messages/{}/reactions",
                        team_id, message_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Reaction '{}' added to message {}", emoji, message_id)}))
        }

        "clickup_chat_reaction_remove" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let emoji = args
                .get("emoji")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: emoji")?;
            client
                .delete(&format!(
                    "/v3/workspaces/{}/chat/messages/{}/reactions/{}",
                    team_id, message_id, emoji
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(
                json!({"message": format!("Reaction '{}' removed from message {}", emoji, message_id)}),
            )
        }

        "clickup_chat_reply_list" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/messages/{}/replies",
                    team_id, message_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_chat_reply_send" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: text")?;
            let body = json!({"content": text});
            let resp = client
                .post(
                    &format!(
                        "/v3/workspaces/{}/chat/messages/{}/replies",
                        team_id, message_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "Reply sent", "id": resp.get("id")}))
        }

        "clickup_chat_tagged_users" => {
            let team_id = resolve_workspace(args)?;
            let message_id = args
                .get("message_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: message_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/chat/messages/{}/tagged_users",
                    team_id, message_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_time_current" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/current", team_id))
                .await
                .map_err(|e| e.to_string())?;
            let data = resp.get("data").cloned().unwrap_or(resp);
            Ok(compact_items(
                &[data],
                &["id", "task", "duration", "start", "billable"],
            ))
        }

        "clickup_time_tags" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/tags", team_id))
                .await
                .map_err(|e| e.to_string())?;
            let tags = resp
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&tags, &["name"]))
        }

        "clickup_time_add_tags" => {
            let team_id = resolve_workspace(args)?;
            let entry_ids = args
                .get("entry_ids")
                .and_then(|v| v.as_array())
                .ok_or("Missing required parameter: entry_ids")?;
            let tag_names = args
                .get("tag_names")
                .and_then(|v| v.as_array())
                .ok_or("Missing required parameter: tag_names")?;
            let tags: Vec<Value> = tag_names
                .iter()
                .filter_map(|n| n.as_str())
                .map(|n| json!({"name": n}))
                .collect();
            let body = json!({"time_entry_ids": entry_ids, "tags": tags});
            client
                .post(&format!("/v2/team/{}/time_entries/tags", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "Tags added to time entries"}))
        }

        "clickup_time_remove_tags" => {
            let team_id = resolve_workspace(args)?;
            let entry_ids = args
                .get("entry_ids")
                .and_then(|v| v.as_array())
                .ok_or("Missing required parameter: entry_ids")?;
            let tag_names = args
                .get("tag_names")
                .and_then(|v| v.as_array())
                .ok_or("Missing required parameter: tag_names")?;
            let tags: Vec<Value> = tag_names
                .iter()
                .filter_map(|n| n.as_str())
                .map(|n| json!({"name": n}))
                .collect();
            let body = json!({"time_entry_ids": entry_ids, "tags": tags});
            client
                .delete_with_body(&format!("/v2/team/{}/time_entries/tags", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "Tags removed from time entries"}))
        }

        "clickup_time_rename_tag" => {
            let team_id = resolve_workspace(args)?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let new_name = args
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: new_name")?;
            let body = json!({"name": name, "new_name": new_name});
            client
                .put(&format!("/v2/team/{}/time_entries/tags", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Tag '{}' renamed to '{}'", name, new_name)}))
        }

        "clickup_time_history" => {
            let team_id = resolve_workspace(args)?;
            let timer_id = args
                .get("timer_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: timer_id")?;
            let resp = client
                .get(&format!(
                    "/v2/team/{}/time_entries/{}/history",
                    team_id, timer_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_guest_invite" => {
            let team_id = resolve_workspace(args)?;
            let email = args
                .get("email")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: email")?;
            let mut body = json!({"email": email});
            if let Some(v) = args.get("can_edit_tags").and_then(|v| v.as_bool()) {
                body["can_edit_tags"] = json!(v);
            }
            if let Some(v) = args.get("can_see_time_spent").and_then(|v| v.as_bool()) {
                body["can_see_time_spent"] = json!(v);
            }
            if let Some(v) = args.get("can_create_views").and_then(|v| v.as_bool()) {
                body["can_create_views"] = json!(v);
            }
            let resp = client
                .post(&format!("/v2/team/{}/guest", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            let user = guest.get("user").cloned().unwrap_or(guest);
            Ok(compact_items(&[user], &["id", "email"]))
        }

        "clickup_guest_update" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let mut body = json!({});
            if let Some(v) = args.get("can_edit_tags").and_then(|v| v.as_bool()) {
                body["can_edit_tags"] = json!(v);
            }
            if let Some(v) = args.get("can_see_time_spent").and_then(|v| v.as_bool()) {
                body["can_see_time_spent"] = json!(v);
            }
            if let Some(v) = args.get("can_create_views").and_then(|v| v.as_bool()) {
                body["can_create_views"] = json!(v);
            }
            client
                .put(&format!("/v2/team/{}/guest/{}", team_id, guest_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} updated", guest_id)}))
        }

        "clickup_guest_remove" => {
            let team_id = resolve_workspace(args)?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client
                .delete(&format!("/v2/team/{}/guest/{}", team_id, guest_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} removed", guest_id)}))
        }

        "clickup_guest_share_task" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args
                .get("permission")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client
                .post(&format!("/v2/task/{}/guest/{}", task_id, guest_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Task {} shared with guest {}", task_id, guest_id)}))
        }

        "clickup_guest_unshare_task" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client
                .delete(&format!("/v2/task/{}/guest/{}", task_id, guest_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from task {}", guest_id, task_id)}))
        }

        "clickup_guest_share_list" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args
                .get("permission")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client
                .post(&format!("/v2/list/{}/guest/{}", list_id, guest_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("List {} shared with guest {}", list_id, guest_id)}))
        }

        "clickup_guest_unshare_list" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client
                .delete(&format!("/v2/list/{}/guest/{}", list_id, guest_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from list {}", guest_id, list_id)}))
        }

        "clickup_guest_share_folder" => {
            let folder_id = args
                .get("folder_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            let permission = args
                .get("permission")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: permission")?;
            let body = json!({"permission_level": permission});
            client
                .post(
                    &format!("/v2/folder/{}/guest/{}", folder_id, guest_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Folder {} shared with guest {}", folder_id, guest_id)}))
        }

        "clickup_guest_unshare_folder" => {
            let folder_id = args
                .get("folder_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: folder_id")?;
            let guest_id = args
                .get("guest_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: guest_id")?;
            client
                .delete(&format!("/v2/folder/{}/guest/{}", folder_id, guest_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("Guest {} unshared from folder {}", guest_id, folder_id)}))
        }

        "clickup_user_invite" => {
            let team_id = resolve_workspace(args)?;
            let email = args
                .get("email")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: email")?;
            let mut body = json!({"email": email});
            if let Some(admin) = args.get("admin").and_then(|v| v.as_bool()) {
                body["admin"] = json!(admin);
            }
            let resp = client
                .post(&format!("/v2/team/{}/user", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            let user = member.get("user").cloned().unwrap_or(member);
            Ok(compact_items(&[user], &["id", "username", "email"]))
        }

        "clickup_user_update" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            let mut body = json!({});
            if let Some(username) = args.get("username").and_then(|v| v.as_str()) {
                body["username"] = json!(username);
            }
            if let Some(admin) = args.get("admin").and_then(|v| v.as_bool()) {
                body["admin"] = json!(admin);
            }
            client
                .put(&format!("/v2/team/{}/user/{}", team_id, user_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("User {} updated", user_id)}))
        }

        "clickup_user_remove" => {
            let team_id = resolve_workspace(args)?;
            let user_id = args
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("Missing required parameter: user_id")?;
            client
                .delete(&format!("/v2/team/{}/user/{}", team_id, user_id))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("User {} removed from workspace", user_id)}))
        }

        "clickup_template_apply_task" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: list_id")?;
            let template_id = args
                .get("template_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            let resp = client
                .post(
                    &format!("/v2/list/{}/taskTemplate/{}", list_id, template_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(compact_items(&[resp], &["id", "name"]))
        }

        "clickup_template_apply_list" => {
            let template_id = args
                .get("template_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
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
            let space_id = args
                .get("space_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: space_id")?;
            let template_id = args
                .get("template_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: template_id")?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: name")?;
            let body = json!({"name": name});
            client
                .post(
                    &format!("/v2/space/{}/folder_template/{}", space_id, template_id),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(
                json!({"message": format!("Folder '{}' created from template {}", name, template_id)}),
            )
        }

        "clickup_attachment_upload" => {
            let task_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: task_id")?;
            let file_path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: file_path")?;
            let path = format!("/v2/task/{}/attachment", task_id);
            let resp = client
                .upload_file(&path, std::path::Path::new(file_path))
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": "File uploaded", "id": resp.get("id"), "url": resp.get("url")}))
        }

        "clickup_task_type_list" => {
            let team_id = resolve_workspace(args)?;
            let resp = client
                .get(&format!("/v2/team/{}/custom_item", team_id))
                .await
                .map_err(|e| e.to_string())?;
            let items = resp
                .get("custom_items")
                .and_then(|i| i.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(compact_items(&items, &["id", "name", "name_plural"]))
        }

        "clickup_doc_get_page" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let page_id = args
                .get("page_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: page_id")?;
            let resp = client
                .get(&format!(
                    "/v3/workspaces/{}/docs/{}/pages/{}",
                    team_id, doc_id, page_id
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_audit_log_query" => {
            let team_id = resolve_workspace(args)?;
            let event_type = args
                .get("type")
                .and_then(|v| v.as_str())
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
            let resp = client
                .post(&format!("/v3/workspaces/{}/auditlogs", team_id), &body)
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp)
        }

        "clickup_acl_update" => {
            let team_id = resolve_workspace(args)?;
            let object_type = args
                .get("object_type")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: object_type")?;
            let object_id = args
                .get("object_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: object_id")?;
            let mut body = json!({});
            if let Some(private) = args.get("private").and_then(|v| v.as_bool()) {
                body["private"] = json!(private);
            }
            client
                .patch(
                    &format!(
                        "/v3/workspaces/{}/{}/{}/acls",
                        team_id, object_type, object_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"message": format!("ACL updated for {} {}", object_type, object_id)}))
        }

        unknown => Err(format!("Unknown tool: {}", unknown)),
    }
}

// ── Main server loop ──────────────────────────────────────────────────────────

pub async fn serve(filter: filter::Filter) -> Result<(), Box<dyn std::error::Error>> {
    // Resolve token: CLICKUP_TOKEN env > config file
    let token = std::env::var("CLICKUP_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .or_else(|| {
            Config::load()
                .ok()
                .map(|c| c.auth.token)
                .filter(|t| !t.is_empty())
        })
        .ok_or("No API token. Set CLICKUP_TOKEN env var or run `clickup setup`.")?;

    // Resolve workspace: CLICKUP_WORKSPACE env > config file
    let workspace_id = std::env::var("CLICKUP_WORKSPACE")
        .ok()
        .filter(|w| !w.is_empty())
        .or_else(|| Config::load().ok().and_then(|c| c.defaults.workspace_id));

    let client = ClickUpClient::new(&token, 30)
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let groups_str = filter
        .groups
        .as_ref()
        .map(|g| format!(", groups=[{}]", g.join(",")))
        .unwrap_or_default();
    let excluded_groups_str = filter
        .exclude_groups
        .as_ref()
        .map(|g| format!(", exclude-groups=[{}]", g.join(",")))
        .unwrap_or_default();
    eprintln!(
        "MCP: profile={}{}{}, exposing {}/{} tools",
        filter.profile.as_str(),
        groups_str,
        excluded_groups_str,
        filter.allowed_count(),
        tool_list().as_array().map(Vec::len).unwrap_or(0),
    );

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

            "tools/list" => ok_response(&id, json!({"tools": filtered_tool_list(&filter)})),

            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                if let Some(response) = handle_tools_call_early(&id, &params, &filter) {
                    response
                } else {
                    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
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
