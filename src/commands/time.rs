use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TimeCommands {
    /// List time entries
    List {
        /// Filter by start date (ISO 8601 or Unix ms)
        #[arg(long)]
        start_date: Option<String>,
        /// Filter by end date (ISO 8601 or Unix ms)
        #[arg(long)]
        end_date: Option<String>,
        /// Filter by assignee user ID
        #[arg(long)]
        assignee: Option<String>,
        /// Filter by task ID
        #[arg(long)]
        task: Option<String>,
    },
    /// Get a time entry by ID
    Get {
        /// Time entry ID
        id: String,
    },
    /// Get the currently running timer
    Current,
    /// Create a time entry
    Create {
        /// Start time (Unix ms)
        #[arg(long)]
        start: String,
        /// Duration in milliseconds
        #[arg(long)]
        duration: String,
        /// Task ID
        #[arg(long)]
        task: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Mark as billable
        #[arg(long)]
        billable: bool,
    },
    /// Update a time entry
    Update {
        /// Time entry ID
        id: String,
        /// New start time (Unix ms)
        #[arg(long)]
        start: Option<String>,
        /// New end time (Unix ms)
        #[arg(long)]
        end: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Mark as billable
        #[arg(long)]
        billable: Option<bool>,
    },
    /// Delete a time entry
    Delete {
        /// Time entry ID
        id: String,
    },
    /// Start a timer
    Start {
        /// Task ID to associate with the timer
        #[arg(long)]
        task: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Mark as billable
        #[arg(long)]
        billable: bool,
    },
    /// Stop the currently running timer
    Stop,
    /// List time entry tags for workspace
    Tags,
    /// Add tags to a time entry
    AddTags {
        /// Time entry ID
        #[arg(long)]
        entry_id: String,
        /// Tag name(s) to add
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Remove tags from a time entry
    RemoveTags {
        /// Time entry ID
        #[arg(long)]
        entry_id: String,
        /// Tag name(s) to remove
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Rename a time entry tag
    RenameTag {
        /// Current tag name
        #[arg(long)]
        name: String,
        /// New tag name
        #[arg(long)]
        new_name: String,
    },
    /// Get history for a time entry
    History {
        /// Time entry ID
        id: String,
    },
}

const TIME_FIELDS: &[&str] = &["id", "task", "duration", "start", "billable"];

/// Flatten a time entry's "task" field from object to name string if needed.
fn flatten_task_field(mut entry: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = entry.as_object_mut() {
        if let Some(task_val) = obj.get("task").cloned() {
            if let Some(name) = task_val.get("name").and_then(|n| n.as_str()) {
                obj.insert("task".to_string(), serde_json::Value::String(name.to_string()));
            }
        }
    }
    entry
}

pub async fn execute(command: TimeCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let ws_id = resolve_workspace(cli)?;

    match command {
        TimeCommands::List {
            start_date,
            end_date,
            assignee,
            task,
        } => {
            let mut params = Vec::new();
            if let Some(s) = start_date {
                params.push(format!("start_date={}", s));
            }
            if let Some(e) = end_date {
                params.push(format!("end_date={}", e));
            }
            if let Some(a) = assignee {
                params.push(format!("assignee={}", a));
            }
            if let Some(t) = task {
                params.push(format!("task_id={}", t));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client
                .get(&format!("/v2/team/{}/time_entries{}", ws_id, query))
                .await?;
            let entries: Vec<serde_json::Value> = resp
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(flatten_task_field)
                .collect();
            output.print_items(&entries, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Get { id } => {
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/{}", ws_id, id))
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Current => {
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/current", ws_id))
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Create {
            start,
            duration,
            task,
            description,
            billable,
        } => {
            let mut body = serde_json::json!({
                "start": start,
                "duration": duration,
                "billable": billable,
            });
            if let Some(t) = task {
                body["tid"] = serde_json::Value::String(t);
            }
            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }
            let resp = client
                .post(&format!("/v2/team/{}/time_entries", ws_id), &body)
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Update {
            id,
            start,
            end,
            description,
            billable,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(s) = start {
                body.insert("start".into(), serde_json::Value::String(s));
            }
            if let Some(e) = end {
                body.insert("end".into(), serde_json::Value::String(e));
            }
            if let Some(d) = description {
                body.insert("description".into(), serde_json::Value::String(d));
            }
            if let Some(b) = billable {
                body.insert("billable".into(), serde_json::Value::Bool(b));
            }
            let resp = client
                .put(
                    &format!("/v2/team/{}/time_entries/{}", ws_id, id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Delete { id } => {
            client
                .delete(&format!("/v2/team/{}/time_entries/{}", ws_id, id))
                .await?;
            output.print_message(&format!("Time entry {} deleted", id));
            Ok(())
        }
        TimeCommands::Start {
            task,
            description,
            billable,
        } => {
            let mut body = serde_json::json!({ "billable": billable });
            if let Some(t) = task {
                body["tid"] = serde_json::Value::String(t);
            }
            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }
            let resp = client
                .post(&format!("/v2/team/{}/time_entries/start", ws_id), &body)
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Stop => {
            let body = serde_json::json!({});
            let resp = client
                .post(&format!("/v2/team/{}/time_entries/stop", ws_id), &body)
                .await?;
            let entry = resp
                .get("data")
                .cloned()
                .map(flatten_task_field)
                .unwrap_or(resp);
            output.print_single(&entry, TIME_FIELDS, "id");
            Ok(())
        }
        TimeCommands::Tags => {
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/tags", ws_id))
                .await?;
            let tags = resp
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&tags, &["name"], "name");
            Ok(())
        }
        TimeCommands::AddTags { entry_id, tags } => {
            let tag_objects: Vec<serde_json::Value> = tags
                .iter()
                .map(|n| serde_json::json!({ "name": n }))
                .collect();
            let body = serde_json::json!({
                "time_entry_ids": [entry_id],
                "tags": tag_objects,
            });
            client
                .post(&format!("/v2/team/{}/time_entries/tags", ws_id), &body)
                .await?;
            output.print_message("Tags added");
            Ok(())
        }
        TimeCommands::RemoveTags { entry_id, tags } => {
            let tag_objects: Vec<serde_json::Value> = tags
                .iter()
                .map(|n| serde_json::json!({ "name": n }))
                .collect();
            let body = serde_json::json!({
                "time_entry_ids": [entry_id],
                "tags": tag_objects,
            });
            client
                .delete_with_body(&format!("/v2/team/{}/time_entries/tags", ws_id), &body)
                .await?;
            output.print_message("Tags removed");
            Ok(())
        }
        TimeCommands::RenameTag { name, new_name } => {
            let body = serde_json::json!({
                "name": name,
                "new_name": new_name,
            });
            client
                .put(&format!("/v2/team/{}/time_entries/tags", ws_id), &body)
                .await?;
            output.print_message(&format!("Tag '{}' renamed to '{}'", name, new_name));
            Ok(())
        }
        TimeCommands::History { id } => {
            let resp = client
                .get(&format!("/v2/team/{}/time_entries/{}/history", ws_id, id))
                .await?;
            let history = resp
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&history, &["id", "date", "field", "before", "after"], "id");
            Ok(())
        }
    }
}
