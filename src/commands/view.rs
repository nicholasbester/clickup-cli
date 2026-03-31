use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum ViewCommands {
    /// List views (use one scope flag: --workspace-level, --space, --folder, --list)
    List {
        /// List workspace-level views
        #[arg(long = "workspace-level", conflicts_with_all = &["space", "folder", "list"])]
        workspace_level: bool,
        /// Space ID
        #[arg(long, conflicts_with_all = &["workspace", "folder", "list"])]
        space: Option<String>,
        /// Folder ID
        #[arg(long, conflicts_with_all = &["workspace", "space", "list"])]
        folder: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = &["workspace", "space", "folder"])]
        list: Option<String>,
    },
    /// Get a view by ID
    Get {
        /// View ID
        id: String,
    },
    /// Create a view (use one scope flag: --workspace-level, --space, --folder, --list)
    Create {
        /// View name
        #[arg(long)]
        name: String,
        /// View type (list, board, calendar, gantt, activity, map, workload, table, doc, chat, embed)
        #[arg(long, name = "type")]
        view_type: String,
        /// Create workspace-level view
        #[arg(long = "workspace-level", conflicts_with_all = &["space", "folder", "list"])]
        workspace_level: bool,
        /// Space ID
        #[arg(long, conflicts_with_all = &["workspace", "folder", "list"])]
        space: Option<String>,
        /// Folder ID
        #[arg(long, conflicts_with_all = &["workspace", "space", "list"])]
        folder: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = &["workspace", "space", "folder"])]
        list: Option<String>,
    },
    /// Update a view
    Update {
        /// View ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a view
    Delete {
        /// View ID
        id: String,
    },
    /// List tasks in a view
    Tasks {
        /// View ID
        id: String,
        /// Page number (0-indexed)
        #[arg(long, default_value = "0")]
        page: u32,
    },
}

const VIEW_FIELDS: &[&str] = &["id", "name", "type"];

pub async fn execute(command: ViewCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        ViewCommands::List {
            workspace_level: workspace,
            space,
            folder,
            list,
        } => {
            let url = if workspace {
                let ws_id = resolve_workspace(cli)?;
                format!("/v2/team/{}/view", ws_id)
            } else if let Some(id) = space {
                format!("/v2/space/{}/view", id)
            } else if let Some(id) = folder {
                format!("/v2/folder/{}/view", id)
            } else if let Some(id) = list {
                format!("/v2/list/{}/view", id)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify a scope: --workspace, --space ID, --folder ID, or --list ID"
                        .into(),
                    status: 0,
                });
            };
            let resp = client.get(&url).await?;
            let views = resp
                .get("views")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&views, VIEW_FIELDS, "id");
            Ok(())
        }
        ViewCommands::Get { id } => {
            let resp = client.get(&format!("/v2/view/{}", id)).await?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            output.print_single(&view, VIEW_FIELDS, "id");
            Ok(())
        }
        ViewCommands::Create {
            name,
            view_type,
            workspace_level: workspace,
            space,
            folder,
            list,
        } => {
            let url = if workspace {
                let ws_id = resolve_workspace(cli)?;
                format!("/v2/team/{}/view", ws_id)
            } else if let Some(id) = space {
                format!("/v2/space/{}/view", id)
            } else if let Some(id) = folder {
                format!("/v2/folder/{}/view", id)
            } else if let Some(id) = list {
                format!("/v2/list/{}/view", id)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify a scope: --workspace, --space ID, --folder ID, or --list ID"
                        .into(),
                    status: 0,
                });
            };
            let body = serde_json::json!({
                "name": name,
                "type": view_type,
            });
            let resp = client.post(&url, &body).await?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            output.print_single(&view, VIEW_FIELDS, "id");
            Ok(())
        }
        ViewCommands::Update { id, name } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            let resp = client
                .put(
                    &format!("/v2/view/{}", id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let view = resp.get("view").cloned().unwrap_or(resp);
            output.print_single(&view, VIEW_FIELDS, "id");
            Ok(())
        }
        ViewCommands::Delete { id } => {
            client.delete(&format!("/v2/view/{}", id)).await?;
            output.print_message(&format!("View {} deleted", id));
            Ok(())
        }
        ViewCommands::Tasks { id, page } => {
            let resp = client
                .get(&format!("/v2/view/{}/task?page={}", id, page))
                .await?;
            let tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&tasks, &["id", "name", "status", "assignees"], "id");
            Ok(())
        }
    }
}
