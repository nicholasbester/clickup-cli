use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum ListCommands {
    /// List lists in a folder or space
    List {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless lists)
        #[arg(long)]
        space: Option<String>,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get list details
    Get {
        /// List ID
        id: String,
    },
    /// Create a list
    Create {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless)
        #[arg(long)]
        space: Option<String>,
        /// List name
        #[arg(long)]
        name: String,
        /// List content/description
        #[arg(long)]
        content: Option<String>,
        /// Priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
    },
    /// Update a list
    Update {
        /// List ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New content
        #[arg(long)]
        content: Option<String>,
    },
    /// Delete a list
    Delete {
        /// List ID
        id: String,
    },
    /// Add a task to this list
    AddTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
    /// Remove a task from this list
    RemoveTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
}

pub async fn execute(command: ListCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let default_fields = &["id", "name", "task_count", "status", "due_date"];

    match command {
        ListCommands::List { folder, space, archived } => {
            let path = match (&folder, &space) {
                (Some(f), _) => format!("/v2/folder/{}/list?archived={}", f, archived),
                (_, Some(s)) => format!("/v2/space/{}/list?archived={}", s, archived),
                _ => {
                    return Err(CliError::ClientError {
                        message: "Provide --folder or --space".into(),
                        status: 0,
                    });
                }
            };
            let resp = client.get(&path).await?;
            let lists = resp
                .get("lists")
                .and_then(|l| l.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&lists, default_fields, "id");
            Ok(())
        }
        ListCommands::Get { id } => {
            let resp = client.get(&format!("/v2/list/{}", id)).await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Create {
            folder,
            space,
            name,
            content,
            priority,
            due_date,
        } => {
            let path = match (&folder, &space) {
                (Some(f), _) => format!("/v2/folder/{}/list", f),
                (_, Some(s)) => format!("/v2/space/{}/list", s),
                _ => {
                    return Err(CliError::ClientError {
                        message: "Provide --folder or --space".into(),
                        status: 0,
                    });
                }
            };
            let mut body = serde_json::json!({ "name": name });
            if let Some(c) = content {
                body["content"] = serde_json::Value::String(c);
            }
            if let Some(p) = priority {
                body["priority"] = serde_json::json!(p);
            }
            if let Some(d) = due_date {
                body["due_date"] = serde_json::Value::String(date_to_ms(&d)?);
            }
            let resp = client.post(&path, &body).await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Update { id, name, content } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(c) = content {
                body.insert("content".into(), serde_json::Value::String(c));
            }
            let resp = client
                .put(&format!("/v2/list/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Delete { id } => {
            client.delete(&format!("/v2/list/{}", id)).await?;
            output.print_message(&format!("List {} deleted", id));
            Ok(())
        }
        ListCommands::AddTask { list_id, task_id } => {
            client
                .post(
                    &format!("/v2/list/{}/task/{}", list_id, task_id),
                    &serde_json::json!({}),
                )
                .await?;
            output.print_message(&format!("Task {} added to list {}", task_id, list_id));
            Ok(())
        }
        ListCommands::RemoveTask { list_id, task_id } => {
            client
                .delete(&format!("/v2/list/{}/task/{}", list_id, task_id))
                .await?;
            output.print_message(&format!("Task {} removed from list {}", task_id, list_id));
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
    let dt = naive
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    Ok((dt.timestamp_millis()).to_string())
}
