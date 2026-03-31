use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum ChecklistCommands {
    /// Create a checklist on a task
    Create {
        /// Task ID
        #[arg(long)]
        task: String,
        /// Checklist name
        #[arg(long)]
        name: String,
    },
    /// Update a checklist
    Update {
        /// Checklist ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Position
        #[arg(long)]
        position: Option<i64>,
    },
    /// Delete a checklist
    Delete {
        /// Checklist ID
        id: String,
    },
    /// Add an item to a checklist
    AddItem {
        /// Checklist ID
        id: String,
        /// Item name
        #[arg(long)]
        name: String,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
    },
    /// Update a checklist item
    UpdateItem {
        /// Checklist ID
        id: String,
        /// Item ID
        item_id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Mark as resolved
        #[arg(long)]
        resolved: bool,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
        /// Parent item ID (nest under another item)
        #[arg(long)]
        parent: Option<String>,
    },
    /// Delete a checklist item
    DeleteItem {
        /// Checklist ID
        id: String,
        /// Item ID
        item_id: String,
    },
}

pub async fn execute(command: ChecklistCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        ChecklistCommands::Create { task, name } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .post(&format!("/v2/task/{}/checklist", task), &body)
                .await?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            output.print_single(&checklist, &["id", "name", "task_id", "orderindex"], "id");
            Ok(())
        }
        ChecklistCommands::Update { id, name, position } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(p) = position {
                body.insert("position".into(), serde_json::json!(p));
            }
            let resp = client
                .put(&format!("/v2/checklist/{}", id), &serde_json::Value::Object(body))
                .await?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            output.print_single(&checklist, &["id", "name", "orderindex"], "id");
            Ok(())
        }
        ChecklistCommands::Delete { id } => {
            client.delete(&format!("/v2/checklist/{}", id)).await?;
            output.print_message(&format!("Checklist {} deleted", id));
            Ok(())
        }
        ChecklistCommands::AddItem { id, name, assignee } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
            let resp = client
                .post(&format!("/v2/checklist/{}/checklist_item", id), &body)
                .await?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            output.print_single(&checklist, &["id", "name"], "id");
            Ok(())
        }
        ChecklistCommands::UpdateItem {
            id,
            item_id,
            name,
            resolved,
            assignee,
            parent,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if resolved {
                body.insert("resolved".into(), serde_json::Value::Bool(true));
            }
            if let Some(a) = assignee {
                body.insert("assignee".into(), serde_json::json!(a));
            }
            if let Some(p) = parent {
                body.insert("parent".into(), serde_json::Value::String(p));
            }
            let resp = client
                .put(
                    &format!("/v2/checklist/{}/checklist_item/{}", id, item_id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let checklist = resp.get("checklist").cloned().unwrap_or(resp);
            output.print_single(&checklist, &["id", "name"], "id");
            Ok(())
        }
        ChecklistCommands::DeleteItem { id, item_id } => {
            client
                .delete(&format!("/v2/checklist/{}/checklist_item/{}", id, item_id))
                .await?;
            output.print_message(&format!("Checklist item {} deleted", item_id));
            Ok(())
        }
    }
}
