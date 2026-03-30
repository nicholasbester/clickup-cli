use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum FolderCommands {
    /// List folders in a space
    List {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get folder details
    Get {
        /// Folder ID
        id: String,
    },
    /// Create a folder
    Create {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Folder name
        #[arg(long)]
        name: String,
    },
    /// Update a folder
    Update {
        /// Folder ID
        id: String,
        /// New name
        #[arg(long)]
        name: String,
    },
    /// Delete a folder
    Delete {
        /// Folder ID
        id: String,
    },
}

pub async fn execute(command: FolderCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        FolderCommands::List { space, archived } => {
            let resp = client
                .get(&format!("/v2/space/{}/folder?archived={}", space, archived))
                .await?;
            let folders = resp
                .get("folders")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            // Flatten: extract list_count from lists array length
            let items: Vec<serde_json::Value> = folders
                .iter()
                .map(|f| {
                    let list_count = f
                        .get("lists")
                        .and_then(|l| l.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    serde_json::json!({
                        "id": f.get("id"),
                        "name": f.get("name"),
                        "task_count": f.get("task_count"),
                        "list_count": list_count,
                    })
                })
                .collect();
            output.print_items(&items, &["id", "name", "task_count", "list_count"], "id");
            Ok(())
        }
        FolderCommands::Get { id } => {
            let resp = client.get(&format!("/v2/folder/{}", id)).await?;
            let list_count = resp
                .get("lists")
                .and_then(|l| l.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let mut item = resp.clone();
            item.as_object_mut()
                .map(|o| o.insert("list_count".into(), serde_json::json!(list_count)));
            output.print_single(&item, &["id", "name", "task_count", "list_count"], "id");
            Ok(())
        }
        FolderCommands::Create { space, name } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .post(&format!("/v2/space/{}/folder", space), &body)
                .await?;
            output.print_single(&resp, &["id", "name"], "id");
            Ok(())
        }
        FolderCommands::Update { id, name } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .put(&format!("/v2/folder/{}", id), &body)
                .await?;
            output.print_single(&resp, &["id", "name"], "id");
            Ok(())
        }
        FolderCommands::Delete { id } => {
            client.delete(&format!("/v2/folder/{}", id)).await?;
            output.print_message(&format!("Folder {} deleted", id));
            Ok(())
        }
    }
}
