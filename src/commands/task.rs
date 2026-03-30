use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List tasks in a list
    List {
        /// List ID
        #[arg(long)]
        list: String,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Include closed tasks
        #[arg(long)]
        include_closed: bool,
        /// Order by field
        #[arg(long)]
        order_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse: bool,
    },
    /// Search tasks across workspace
    Search {
        /// Filter by space
        #[arg(long)]
        space: Option<String>,
        /// Filter by folder
        #[arg(long)]
        folder: Option<String>,
        /// Filter by list
        #[arg(long)]
        list: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
    },
    /// Get task details
    Get {
        /// Task ID
        id: String,
        /// Include subtasks
        #[arg(long)]
        subtasks: bool,
        /// Treat ID as custom task ID
        #[arg(long)]
        custom_task_id: bool,
    },
    /// Create a task
    Create {
        /// List ID
        #[arg(long)]
        list: String,
        /// Task name
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Status
        #[arg(long)]
        status: Option<String>,
        /// Priority (1=urgent, 2=high, 3=normal, 4=low)
        #[arg(long)]
        priority: Option<u8>,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Tag name
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
        /// Parent task ID (creates subtask)
        #[arg(long)]
        parent: Option<String>,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New status
        #[arg(long)]
        status: Option<String>,
        /// New priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Add assignee
        #[arg(long)]
        add_assignee: Option<Vec<String>>,
        /// Remove assignee
        #[arg(long)]
        rem_assignee: Option<Vec<String>>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
    /// Get time in status for task(s)
    TimeInStatus {
        /// Task ID(s) — multiple IDs triggers bulk mode
        ids: Vec<String>,
    },
}

const TASK_FIELDS: &[&str] = &["id", "name", "status", "priority", "assignees", "due_date"];

pub async fn execute(command: TaskCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        TaskCommands::List {
            list,
            status,
            assignee,
            tag,
            include_closed,
            order_by,
            reverse,
        } => {
            let mut params = Vec::new();
            if include_closed {
                params.push("include_closed=true".to_string());
            }
            if let Some(statuses) = &status {
                for s in statuses {
                    params.push(format!("statuses[]={}", s));
                }
            }
            if let Some(assignees) = &assignee {
                for a in assignees {
                    params.push(format!("assignees[]={}", a));
                }
            }
            if let Some(tags) = &tag {
                for t in tags {
                    params.push(format!("tags[]={}", t));
                }
            }
            if let Some(ob) = &order_by {
                params.push(format!("order_by={}", ob));
            }
            if reverse {
                params.push("reverse=true".to_string());
            }
            if let Some(page) = cli.page {
                params.push(format!("page={}", page));
            }

            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };

            if cli.all {
                // Auto-paginate
                let mut all_tasks = Vec::new();
                let mut page = 0u32;
                loop {
                    let mut page_params = params.clone();
                    page_params.push(format!("page={}", page));
                    let page_query = format!("?{}", page_params.join("&"));
                    let resp = client
                        .get(&format!("/v2/list/{}/task{}", list, page_query))
                        .await?;
                    let tasks = resp
                        .get("tasks")
                        .and_then(|t| t.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let is_last = resp
                        .get("last_page")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    all_tasks.extend(tasks);
                    if is_last {
                        break;
                    }
                    if let Some(limit) = cli.limit {
                        if all_tasks.len() >= limit {
                            all_tasks.truncate(limit);
                            break;
                        }
                    }
                    page += 1;
                }
                output.print_items(&all_tasks, TASK_FIELDS, "id");
            } else {
                let resp = client
                    .get(&format!("/v2/list/{}/task{}", list, query))
                    .await?;
                let mut tasks = resp
                    .get("tasks")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();
                if let Some(limit) = cli.limit {
                    tasks.truncate(limit);
                }
                output.print_items(&tasks, TASK_FIELDS, "id");
            }
            Ok(())
        }
        TaskCommands::Search {
            space,
            folder,
            list,
            status,
            assignee,
            tag,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let mut params = Vec::new();
            if let Some(s) = &space {
                params.push(format!("space_ids[]={}", s));
            }
            if let Some(f) = &folder {
                params.push(format!("project_ids[]={}", f));
            }
            if let Some(l) = &list {
                params.push(format!("list_ids[]={}", l));
            }
            if let Some(statuses) = &status {
                for s in statuses {
                    params.push(format!("statuses[]={}", s));
                }
            }
            if let Some(assignees) = &assignee {
                for a in assignees {
                    params.push(format!("assignees[]={}", a));
                }
            }
            if let Some(tags) = &tag {
                for t in tags {
                    params.push(format!("tags[]={}", t));
                }
            }
            if let Some(page) = cli.page {
                params.push(format!("page={}", page));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client
                .get(&format!("/v2/team/{}/task{}", ws_id, query))
                .await?;
            let mut tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = cli.limit {
                tasks.truncate(limit);
            }
            output.print_items(&tasks, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Get {
            id,
            subtasks,
            custom_task_id,
        } => {
            let mut params = Vec::new();
            if subtasks {
                params.push("include_subtasks=true".to_string());
            }
            if custom_task_id {
                params.push("custom_task_ids=true".to_string());
                let ws_id = resolve_workspace(cli)?;
                params.push(format!("team_id={}", ws_id));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client
                .get(&format!("/v2/task/{}{}", id, query))
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Create {
            list,
            name,
            description,
            status,
            priority,
            assignee,
            tag,
            due_date,
            parent,
        } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }
            if let Some(s) = status {
                body["status"] = serde_json::Value::String(s);
            }
            if let Some(p) = priority {
                body["priority"] = serde_json::json!(p);
            }
            if let Some(assignees) = assignee {
                let ids: Vec<serde_json::Value> = assignees
                    .iter()
                    .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                    .collect();
                body["assignees"] = serde_json::Value::Array(ids);
            }
            if let Some(tags) = tag {
                body["tags"] = serde_json::json!(tags);
            }
            if let Some(d) = due_date {
                body["due_date"] = serde_json::Value::String(date_to_ms(&d)?);
            }
            if let Some(p) = parent {
                body["parent"] = serde_json::Value::String(p);
            }
            let resp = client
                .post(&format!("/v2/list/{}/task", list), &body)
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Update {
            id,
            name,
            status,
            priority,
            add_assignee,
            rem_assignee,
            description,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(s) = status {
                body.insert("status".into(), serde_json::Value::String(s));
            }
            if let Some(p) = priority {
                body.insert("priority".into(), serde_json::json!(p));
            }
            if let Some(d) = description {
                body.insert("description".into(), serde_json::Value::String(d));
            }
            // Assignee add/remove uses nested object
            if add_assignee.is_some() || rem_assignee.is_some() {
                let mut assignees = serde_json::Map::new();
                if let Some(add) = add_assignee {
                    let ids: Vec<serde_json::Value> = add
                        .iter()
                        .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                        .collect();
                    assignees.insert("add".into(), serde_json::Value::Array(ids));
                }
                if let Some(rem) = rem_assignee {
                    let ids: Vec<serde_json::Value> = rem
                        .iter()
                        .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                        .collect();
                    assignees.insert("rem".into(), serde_json::Value::Array(ids));
                }
                body.insert("assignees".into(), serde_json::Value::Object(assignees));
            }
            let resp = client
                .put(&format!("/v2/task/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Delete { id } => {
            client.delete(&format!("/v2/task/{}", id)).await?;
            output.print_message(&format!("Task {} deleted", id));
            Ok(())
        }
        TaskCommands::TimeInStatus { ids } => {
            if ids.len() == 1 {
                let resp = client
                    .get(&format!("/v2/task/{}/time_in_status", ids[0]))
                    .await?;
                if cli.output == "json" {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                } else {
                    // Print status durations
                    if let Some(statuses) = resp.get("current_status").and_then(|v| v.as_object()) {
                        println!("Current: {} ({}ms)",
                            statuses.get("status").and_then(|v| v.as_str()).unwrap_or("-"),
                            statuses.get("total_time").and_then(|v| v.as_object())
                                .and_then(|o| o.get("by_minute"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                        );
                    }
                    // Print all statuses
                    if let Some(statuses_arr) = resp.get("status_history").and_then(|v| v.as_array()) {
                        for s in statuses_arr {
                            let name = s.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                            let time = s.get("total_time").and_then(|v| v.as_object())
                                .and_then(|o| o.get("by_minute"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            println!("  {} — {}ms", name, time);
                        }
                    }
                }
            } else {
                // Bulk mode
                let task_ids = ids.join(",");
                let resp = client
                    .get(&format!("/v2/task/bulk_time_in_status/task_ids?task_ids={}", task_ids))
                    .await?;
                if cli.output == "json" {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                } else {
                    // Print per-task summary
                    if let Some(obj) = resp.as_object() {
                        for (task_id, data) in obj {
                            let current = data
                                .get("current_status")
                                .and_then(|v| v.get("status"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("-");
                            println!("{}: {}", task_id, current);
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

fn date_to_ms(date_str: &str) -> Result<String, CliError> {
    let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| CliError::ClientError {
            message: format!("Invalid date '{}'. Use YYYY-MM-DD format.", date_str),
            status: 0,
        })?;
    let dt = naive.and_hms_opt(0, 0, 0).unwrap().and_utc();
    Ok((dt.timestamp_millis()).to_string())
}
